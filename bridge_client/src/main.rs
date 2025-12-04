use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use bridge_core::{BridgeCommand, BridgeResponse};

// Lokasi socket dilihat dari sisi Chroot
// Asumsi: folder /data/local/tmp/chroot di-mount sebagai / (root) atau akses relatif
// Jika di dalam Chroot ada folder /tmp yang mapping ke host, maka:
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
    // Shell biasa (/system/bin/sh)
    Exec {
        // Nama Program (misal: input, pm, am)
        program: String,
        // Argumen program
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
        // Durasi swipe dalam milidetik (default: 300ms)
        #[arg(default_value_t = 300)]
        duration: u64,
    },
    // Cek Koneksi Server
    Ping,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    // Mapping dari CLI Clap ke BridgeCommand Core
    let bridge_cmd = match cli.command {
        Commands::Exec { program, args } => BridgeCommand::Exec { program, args },
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

    // Kirim
    let mut stream = UnixStream::connect(SOCKET_PATH).inspect_err(|e| {
        eprintln!("Gagal connect ke {}. Pastikan Server nyala!", SOCKET_PATH);
    })?;

    // Serialize Command ke Bytes (Bincode)
    let bin_payload = bincode::serialize(&bridge_cmd).expect("Gagal serialize");
    stream.write_all(&bin_payload)?;

    // Baca Response
    // Bincode butuh byte array, bukan String
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer)?;
    if buffer.is_empty() {
        eprintln!("Server tidak memberikan respon.");
        return Ok(());
    }
    let response: BridgeResponse =
        bincode::deserialize(&buffer).expect("Gagal deserialize response");

    match response {
        BridgeResponse::Success(msg) => {
            if !msg.is_empty() && msg != "Tapped" && msg != "Swiped" && msg != "Pong!" {
                print!("{}", msg);
            } else if msg == "Pong!" {
                println!("Pong! Server is alive.");
            }
        }
        BridgeResponse::Error(err) => {
            eprintln!("Remote Error: {}", err);
        }
    }

    Ok(())
}
