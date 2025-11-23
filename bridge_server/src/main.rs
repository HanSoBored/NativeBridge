use std::os::unix::net::UnixListener;
use std::io::{Read, Write};
use std::process::Command;
use std::fs::OpenOptions;
use std::time::Duration;
use std::path::Path;
use std::thread;
use std::mem;
use std::fs;

use bridge_core::{BridgeCommand, BridgeResponse};

// Lokasi socket dillihat dari sisi Android Host
// Pastikan path ini mengarah ke folder yang bisa dibaca oleh Chroot
const SOCKET_PATH: &str = "/data/local/tmp/chrootubuntu/tmp/bridge.sock";
const TOUCH_DEVICE: &str = "/dev/input/event1";

fn main() -> std::io::Result<()> {
    // Bersihkan socket lama jika ada
    if Path::new(SOCKET_PATH).exists() {
        fs::remove_file(SOCKET_PATH)?;
    }

    // Bind Socket
    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Server Bridge aktif di: {}", SOCKET_PATH);

    // Ubah permission socket agar bisa dibaca/ditulis oleh user chroot
    // Untuk saat ini Rust std lib ribet untuk ubah permission, pakai Command chroot yang simple

Command::new("chmod").arg("777").arg(SOCKET_PATH).output()?;

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

    // Baca pesan dari Client
    match socket.read(&mut buffer) {
        Ok(size) => {
            if size == 0 { return; } // Kosong

            let raw_data = &buffer[0..size];
            // Menggunakan Bincode deserialize
            let request: Result<BridgeCommand, _> = bincode::deserialize(&raw_data[0..size]);

            let response = match request {
                Ok(cmd) => execute_request(cmd),
                Err(e) => BridgeResponse::Error(format!("Invalid JSON: {}", e)),
            };

            let resp_bytes = bincode::serialize(&response).unwrap();
            let _ = socket.write_all(&resp_bytes);
        }
        Err(_) => {}
    }
}

fn write_event(file: &mut std::fs::File, type_: u16, code: u16, value: i32) -> std::io::Result<()> {
    let ev = InputEvent {
        time_sec: 0, time_usec: 0, type_, code, value
    };
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(&ev as *const _ as *const u8, mem::size_of::<InputEvent>())
    };
    file.write_all(bytes)
}

fn inject_tap(x: i32, y: i32) -> std::io::Result<()> {
    // Buka file driver touchscreen
    let mut file = OpenOptions::new().write(true).open(TOUCH_DEVICE)?;

    // Protocol Touchscreen (ABS_MT) sederhana
    // Urutan: ABS_MT_POSITION_X -> ABS_MT_POSITION_Y -> SYN_REPORT
    // Note: Implementasi kasar, Device tertentu butuh protokol MT_SLOT (Multi-Touch)
    // Untuk single tap normal

    // 1. Down (EV_ABS)
    write_event(&mut file, 3, 53, x)?; // 53 = ABS_MT_POSITION_X
    write_event(&mut file, 3, 54, y)?; // 54 = ABS_MT_POSITION_Y
    write_event(&mut file, 1, 330, 1)?; // 330 = BTN_TOUCH, Value 1 (Down)
    write_event(&mut file, 0, 0, 0)?; // SYN_REPORT (Commit)

    // 2. Tahan Sebentah (optional untuk keterbacaan system)
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 3. Up (Release)
    write_event(&mut file, 3, 53, x)?;
    write_event(&mut file, 3, 54, y)?;
    write_event(&mut file, 1, 330, 1)?;
    write_event(&mut file, 0, 0, 0)?;

    Ok(())
}

fn inject_swipe(x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u64) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(TOUCH_DEVICE)?;

    // Hitung jumlah langkah (steps)
    // Misal: update posisi setiap 10ms
    let step_delay = 10;
    let steps = (duration_ms / step_delay).max(1); // Minimal satu step_delay

    // Hitung perubahan koordinat per step (delta)
    let dx = (x2 - y1) as f32 / steps as f32;
    let dy = (y2 - x1) as f32 / steps as f32;

    // 1. Touch Down (Titik Awal)
    write_event(&mut file, 3, 53, x1)?; // ABS_MT_POSITION_X
    write_event(&mut file, 3, 54, y1)?; // ABS_MT_POSITION_Y
    write_event(&mut file, 1, 330, 1)?; // BTN_TOUCH DOWN
    write_event(&mut file, 0, 0, 0)?; // SYN_REPORT

    // 2. Loop Gerakan (Interpolasi)
    let mut current_x = x1 as f32;
    let mut current_y = y1 as f32;

    for _ in 0..steps {
        current_x += dx;
        current_y += dy;

        // Update Posisi
        write_event(&mut file, 3, 53, current_x as i32)?;
        write_event(&mut file, 3, 54, current_y as i32)?;
        write_event(&mut file, 0, 0, 0)?; 

        thread::sleep(Duration::from_millis(step_delay));
    }

    // 3. Touch Up (Release)
    write_event(&mut file, 3, 53, x2)?;
    write_event(&mut file, 3, 54, y2)?;
    write_event(&mut file, 1, 330, 0)?;
    write_event(&mut file, 0, 0, 0,)?;

    Ok(())
}

fn execute_request(cmd: BridgeCommand) -> BridgeResponse {
    match cmd {
        // Logika Universal: untuk semua program
        BridgeCommand::Exec { program, args } => {
            println!("Exec: {} {:?}", program, args); // Logging di server

            let output = Command::new(&program)
                .args(args)
                .output();

            match output {
                Ok(o) => {
                    if o.status.success() {
                        BridgeResponse::Success(String::from_utf8_lossy(&o.stdout).to_string())
                    } else {
                        // Jika command gagal (exit code !=0), Kirim stderr
                        BridgeResponse::Error(String::from_utf8_lossy(&o.stderr).to_string())
                    }
                },
                Err(e) => BridgeResponse::Error(format!("Gagal menjalankan {}: {}", program, e)),
            }
        },
        BridgeCommand::Ping => BridgeResponse::Success("Pong!".to_string()),
        // Direct Tap (Bypass Android Framework)
        BridgeCommand::DirectTap { x, y } => {
            if let Err(e) = inject_tap(x, y) {
                return BridgeResponse::Error(format!("Tap Failed: {}", e));
            }
            BridgeResponse::Success("Tapped".to_string())
        },
        BridgeCommand::DirectSwipe { x1, y1, x2, y2, duration_ms } => {
            if let Err(e) = inject_swipe(x1, y1, x2, y2, duration_ms) {
                return BridgeResponse::Error(format!("Swipe Failed: {}", e));
            }
            BridgeResponse::Success("Swiped".to_string())
        },
    }
}

// Struct Event Linux (Low Level)
#[repr(C)]
struct InputEvent {
    time_sec: usize, // Arsitektur android, 64bit = usize
    time_usec: usize,
    type_: u16,
    code: u16,
    value: i32,
}
