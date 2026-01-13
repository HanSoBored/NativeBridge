use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process;

use bridge_core::{BridgeCommand, BridgeResponse};

// Socket location as seen from the Chroot side
const SOCKET_PATH: &str = "/tmp/bridge.sock";

#[derive(Parser)]
#[command(name = "andro")]
#[command(about = "NativeBridge Client for Android Chroot", long_about = None, arg_required_else_help = true)]
struct Cli {
    #[arg(short, long, value_name = "CMD", num_args=1.., conflicts_with_all=&["stream", "command"])]
    exec: Option<Vec<String>>,

    #[arg(short, long, value_name = "CMD", num_args=1.., conflicts_with_all=&["exec", "command"])]
    stream: Option<Vec<String>>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Tap {
        x: i32,
        y: i32,
    },
    Swipe {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        #[arg(default_value_t = 300)]
        duration: u64,
    },
    Ping,
}

fn main() -> std::io::Result<()> {
    // Handle Ctrl+C gracefully to ensure the program exits cleanly
    // without panicking, especially during a streaming process.
    ctrlc::set_handler(move || {
        println!("\nExiting...");
        process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let cli = Cli::parse();
    
    let (bridge_cmd, is_streaming) = if let Some(mut cmd) = cli.exec {
        let program = cmd.remove(0);
        (BridgeCommand::Exec { program, args: cmd }, false)
    } else if let Some(mut cmd) = cli.stream {
        let program = cmd.remove(0);
        (BridgeCommand::Stream { program, args: cmd }, true)
    } else if let Some(command) = cli.command {
        let cmd = match command {
            Commands::Tap { x, y } => BridgeCommand::DirectTap { x, y },
            Commands::Swipe {
                x1,
                y1,
                x2,
                y2,
                duration,
            } => BridgeCommand::DirectSwipe {
                x1,
                y1,
                x2,
                y2,
                duration_ms: duration,
            },
            Commands::Ping => BridgeCommand::Ping,
        };
        (cmd, false)
    } else {
        // This branch is unreachable because of `arg_required_else_help = true`
        unreachable!();
    };
    
    let mut stream = UnixStream::connect(SOCKET_PATH).inspect_err(|_e| {
        eprintln!(
            "Failed to connect to {}. Is the server running?",
            SOCKET_PATH
        );
    })?;

    let bin_payload = bincode::serialize(&bridge_cmd).expect("Failed to serialize command");
    stream.write_all(&bin_payload)?;

    if is_streaming {
        handle_stream_response(&mut stream)
    } else {
        handle_single_response(&mut stream)
    }
}

fn handle_stream_response(stream: &mut UnixStream) -> std::io::Result<()> {
    loop {
        // Read the first 8 bytes to determine the length of the incoming payload.
        // This is part of the length-prefix protocol to ensure the message is received completely.
        let mut len_bytes = [0u8; 8];
        if stream.read_exact(&mut len_bytes).is_err() {
            // If read fails, the server has likely closed the connection.
            break;
        }
        let len = u64::from_be_bytes(len_bytes);

        // Allocate a buffer of the specified length and read the payload.
        let mut buffer = vec![0u8; len as usize];
        stream.read_exact(&mut buffer)?;

        let response: BridgeResponse =
            bincode::deserialize(&buffer).expect("Failed to deserialize stream response");

        match response {
            BridgeResponse::StreamChunk(msg) => {
                println!("{}", msg);
            }
            BridgeResponse::StreamEnd => {
                break; // Signal from the server that streaming has ended.
            }
            BridgeResponse::Error(err) => {
                eprintln!("Remote Error: {}", err);
                break;
            }
            _ => {
                eprintln!("Received unexpected response type during stream.");
            }
        }
    }
    Ok(())
}

fn handle_single_response(stream: &mut UnixStream) -> std::io::Result<()> {
    let mut len_bytes = [0u8; 8];
    if stream.read_exact(&mut len_bytes).is_err() {
        eprintln!("Server did not provide a response.");
        return Ok(());
    }
    let len = u64::from_be_bytes(len_bytes);

    if len == 0 {
        return Ok(());
    }

    let mut buffer = vec![0u8; len as usize];
    stream.read_exact(&mut buffer)?;

    let response: BridgeResponse =
        bincode::deserialize(&buffer).expect("Failed to deserialize response");

    match response {
        BridgeResponse::Success(msg) => {
            if msg == "Pong!" {
                println!("Pong! Server is alive.");
            } else if !msg.is_empty() {
                print!("{}", msg);
            }
        }
        BridgeResponse::Error(err) => {
            eprintln!("Remote Error: {}", err);
        }
        _ => {
            eprintln!("Received unexpected response type for single command.");
        }
    }

    Ok(())
}
