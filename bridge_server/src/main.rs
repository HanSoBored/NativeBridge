use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use bridge_core::{BridgeCommand, BridgeResponse};

#[cfg(feature = "direct_input")]
mod input_manager;

// Unix socket location, as seen from the Android Host side.
// This path must be accessible from within the chroot environment.
const SOCKET_PATH: &str = "/data/local/rootfs/ubuntu-resolute-26.04/tmp/bridge.sock";

fn main() -> std::io::Result<()> {
    // Ensure there are no leftover socket files from a previous session that could cause an error.
    if Path::new(SOCKET_PATH).exists() {
        fs::remove_file(SOCKET_PATH)?;
    }

    let listener = UnixListener::bind(SOCKET_PATH)?;
    // Change the socket file permissions to be accessible by all users,
    // including processes running inside the chroot with a different user.
    Command::new("chmod").arg("777").arg(SOCKET_PATH).output()?;
    println!("Bridge Server active at: {}", SOCKET_PATH);

    #[cfg(feature = "direct_input")]
    println!(" [Feature Enabled] Direct Kernel Input Module Loaded");

    for stream in listener.incoming() {
        match stream {
            Ok(mut socket) => {
                // Each client connection is handled in a separate thread.
                // This prevents one client from blocking others that want to connect,
                // which is important for handling multiple requests simultaneously.
                thread::spawn(move || {
                    handle_client(&mut socket);
                });
            }
            Err(err) => {
                eprintln!("Failed to accept connection: {}", err);
            }
        }
    }
    Ok(())
}

// Helper to send a response to the client with a length-prefix protocol.
// [8-byte data length][data]
// This ensures the client can read the message correctly, even if the data is fragmented.
fn write_response(socket: &mut UnixStream, response: &BridgeResponse) -> std::io::Result<()> {
    let bytes = bincode::serialize(response).unwrap();
    let len = bytes.len() as u64;
    socket.write_all(&len.to_be_bytes())?;
    socket.write_all(&bytes)?;
    Ok(())
}

fn handle_client(socket: &mut UnixStream) {
    let mut buffer = [0; 8192];
    if let Ok(size) = socket.read(&mut buffer) {
        if size == 0 {
            return;
        }

        match bincode::deserialize::<BridgeCommand>(&buffer[0..size]) {
            Ok(cmd) => {
                if let BridgeCommand::Stream { program, args } = cmd {
                    handle_stream_request(socket, program, args);
                } else {
                    let response = execute_request(cmd);
                    let _ = write_response(socket, &response);
                }
            }
            Err(e) => {
                let response = BridgeResponse::Error(format!("Invalid Payload: {}", e));
                let _ = write_response(socket, &response);
            }
        };
    }
}

fn handle_stream_request(socket: &mut UnixStream, program: String, args: Vec<String>) {
    println!("Stream: {} {:?}", program, args);

    let child = Command::new(&program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = write_response(socket, &BridgeResponse::Error(e.to_string()));
            let _ = write_response(socket, &BridgeResponse::StreamEnd);
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // The `socket` needs to be accessed from multiple threads (for stdout and stderr).
    // `Arc` (Atomic Reference Counting) allows for shared ownership.
    // `Mutex` (Mutual Exclusion) ensures that only one thread can write to the socket at a time.
    let socket = Arc::new(Mutex::new(socket.try_clone().unwrap()));

    let stdout_socket = Arc::clone(&socket);
    let stdout_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line_content in reader.lines().map_while(Result::ok) {
            let response = BridgeResponse::StreamChunk(line_content);
            let mut socket_guard = stdout_socket.lock().unwrap();
            if write_response(&mut socket_guard, &response).is_err() {
                break; // Stop if the client closes the connection
            }
        }
    });

    let stderr_socket = Arc::clone(&socket);
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line_content in reader.lines().map_while(Result::ok) {
            // Output from stderr is also sent as a StreamChunk, with a prefix for identification.
            let response = BridgeResponse::StreamChunk(format!("[STDERR] {}", line_content));
            let mut socket_guard = stderr_socket.lock().unwrap();
            if write_response(&mut socket_guard, &response).is_err() {
                break; // Stop if the client closes the connection
            }
        }
    });

    stdout_thread.join().unwrap();
    stderr_thread.join().unwrap();

    let _ = child.wait();
    // Send the final signal to notify the client that all output has been sent.
    let mut socket_guard = socket.lock().unwrap();
    let _ = write_response(&mut socket_guard, &BridgeResponse::StreamEnd);
}

fn execute_request(cmd: BridgeCommand) -> BridgeResponse {
    #[allow(unreachable_patterns)]
    match cmd {
        BridgeCommand::Exec { program, args } => {
            println!("Exec: {} {:?}", program, args);

            if program == "logcat" {
                // We only allow `logcat -d` and `logcat -c` with exec.
                // Any other `logcat` command should be streamed to avoid blocking.
                let is_valid_exec_logcat = match args.len() {
                    1 => args[0] == "-d" || args[0] == "-c",
                    _ => false, // Disallow `logcat` with no args or multiple args via exec
                };

                if !is_valid_exec_logcat {
                    return BridgeResponse::Error(
                        "For live logcat, use the -s flag. Only 'logcat -d' and 'logcat -c' are allowed with -e.".to_string()
                    );
                }
            }

            let output = Command::new(&program).args(args).output();
            match output {
                Ok(o) => {
                    if o.status.success() {
                        BridgeResponse::Success(String::from_utf8_lossy(&o.stdout).to_string())
                    } else {
                        BridgeResponse::Error(String::from_utf8_lossy(&o.stderr).to_string())
                    }
                }
                Err(e) => BridgeResponse::Error(e.to_string()),
            }
        }
        BridgeCommand::Ping => BridgeResponse::Success("Pong!".to_string()),
        #[cfg(feature = "direct_input")]
        BridgeCommand::DirectTap { x, y } => match input_manager::tap(x, y) {
            Ok(_) => BridgeResponse::Success("".to_string()),
            Err(e) => BridgeResponse::Error(format!("Tap Failed: {}", e)),
        },
        #[cfg(feature = "direct_input")]
        BridgeCommand::DirectSwipe {
            x1,
            y1,
            x2,
            y2,
            duration_ms,
        } => match input_manager::swipe(x1, y1, x2, y2, duration_ms) {
            Ok(_) => BridgeResponse::Success("".to_string()),
            Err(e) => BridgeResponse::Error(format!("Swipe Failed: {}", e)),
        },
        _ => {
            BridgeResponse::Error("Command not supported or feature disabled on server".to_string())
        }
    }
}
