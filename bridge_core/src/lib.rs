use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum BridgeCommand {
    // Perintah Generic untuk menjalankan program binary Android Host apapun
    // program: nama binary (contoh: "input", "am", "pm", "ls")
    // args: daftar argumen
    Exec {
        program: String,
        args: Vec<String>,
    },

    // tetap bisa simpan perintah khusus untuk utility lain
    Ping,
    // Add: Direct Input untuk peforma input zero latency
    DirectTap {
        x: i32,
        y: i32,
    },
    DirectSwipe {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        duration_ms: u64,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BridgeResponse {
    Success(String), // Berisi stdout
    Error(String),   // Berisi stderr
}
