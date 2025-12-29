use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process;

use bridge_core::{BridgeCommand, BridgeResponse};

// Lokasi socket dilihat dari sisi Chroot
const SOCKET_PATH: &str = "/tmp/bridge.sock";

// Definisi CLI Struktur
#[derive(Parser)]
#[command(name = "andro")]
#[command(about = "NativeBridge Client for Android Chroot", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Exec {
        program: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Stream {
        program: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
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
    // Menangani Ctrl+C
    ctrlc::set_handler(move || {
        println!("\nExiting...");
        process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let cli = Cli::parse();
    let is_streaming = matches!(cli.command, Commands::Stream { .. });

    let bridge_cmd = match cli.command {
        Commands::Exec { program, args } => BridgeCommand::Exec { program, args },
        Commands::Stream { program, args } => BridgeCommand::Stream { program, args },
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
        // Baca 8 byte pertama untuk mendapatkan ukuran payload
        let mut len_bytes = [0u8; 8];
        if stream.read_exact(&mut len_bytes).is_err() {
            break;
        }
        let len = u64::from_be_bytes(len_bytes);

        // Baca payload sesuai ukuran yang didapat
        let mut buffer = vec![0u8; len as usize];
        stream.read_exact(&mut buffer)?;

        let response: BridgeResponse =
            bincode::deserialize(&buffer).expect("Failed to deserialize stream response");

        match response {
            BridgeResponse::StreamChunk(msg) => {
                println!("{}", msg);
            }
            BridgeResponse::StreamEnd => {
                break; // Streaming selesai
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
