use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;

use bridge_core::{BridgeCommand, BridgeResponse};

#[cfg(feature = "direct_input")]
mod input_manager;

// Lokasi socket dillihat dari sisi Android Host
// Pastikan path ini mengarah ke folder yang bisa dibaca oleh Chroot
const SOCKET_PATH: &str = "/data/local/ubuntu/tmp/bridge.sock";

fn main() -> std::io::Result<()> {
    // Bersihkan socket lama jika ada
    if Path::new(SOCKET_PATH).exists() {
        fs::remove_file(SOCKET_PATH)?;
    }

    // Bind Socket
    let listener = UnixListener::bind(SOCKET_PATH)?;
    Command::new("chmod").arg("777").arg(SOCKET_PATH).output()?;
    println!("Server Bridge aktif di: {}", SOCKET_PATH);

    // Log info features
    #[cfg(feature = "direct_input")]
    println!(" [Feature Enabled] Direct Kernel Input Module Loaded");

    // Loop menerima koneksi
    for stream in listener.incoming() {
        match stream {
            Ok(mut socket) => {
                // Handle setiap koneksi
                handle_client(&mut socket);
            }
            Err(err) => {
                eprintln!("Gagal menerima koneksi: {}", err);
            }
        }
    }
    Ok(())
}

fn handle_client(socket: &mut std::os::unix::net::UnixStream) {
    let mut buffer = [0; 8192];
    if let Ok(size) = socket.read(&mut buffer) {
        if size == 0 {
            return;
        }

        let response = match bincode::deserialize::<BridgeCommand>(&buffer[0..size]) {
            Ok(cmd) => execute_request(cmd),
            Err(e) => BridgeResponse::Error(format!("Invalid Payload: {}", e)),
        };
        if let Ok(bytes) = bincode::serialize(&response) {
            let _ = socket.write_all(&bytes);
        }
    }
}

fn execute_request(cmd: BridgeCommand) -> BridgeResponse {
    #[allow(unreachable_patterns)]
    match cmd {
        // Logika Universal: untuk semua program
        BridgeCommand::Exec { program, args } => {
            println!("Exec: {} {:?}", program, args); // Logging di server

            let output = Command::new(&program).args(args).output();
            match output {
                Ok(o) => {
                    if o.status.success() {
                        BridgeResponse::Success(String::from_utf8_lossy(&o.stdout).to_string())
                    } else {
                        // Jika command gagal (exit code !=0), Kirim stderr
                        BridgeResponse::Error(String::from_utf8_lossy(&o.stderr).to_string())
                    }
                }
                Err(e) => BridgeResponse::Error(e.to_string()),
            }
        }
        BridgeCommand::Ping => BridgeResponse::Success("Pong!".to_string()),

        // Command Extension jika feature direct_input aktif
        #[cfg(feature = "direct_input")]
        BridgeCommand::DirectTap { x, y } => {
            // Panggil modul terpisah
            match input_manager::tap(x, y) {
                Ok(_) => BridgeResponse::Success("".to_string()),
                Err(e) => BridgeResponse::Error(format!("Tap Failed: {}", e)),
            }
        }

        #[cfg(feature = "direct_input")]
        BridgeCommand::DirectSwipe {
            x1,
            y1,
            x2,
            y2,
            duration_ms,
        } => {
            // Panggil modul terpisah
            match input_manager::swipe(x1, y1, x2, y2, duration_ms) {
                Ok(_) => BridgeResponse::Success("".to_string()),
                Err(e) => BridgeResponse::Error(format!("Swipe Failed: {}", e)),
            }
        }
        _ => {
            BridgeResponse::Error("Command not supported or feature disabled on server".to_string())
        }
    }
}
