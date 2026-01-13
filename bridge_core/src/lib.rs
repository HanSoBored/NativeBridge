use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum BridgeCommand {
    // Generic command to execute any Android Host binary program
    // program: binary name (e.g., "input", "am", "pm", "ls")
    // args: list of arguments
    Exec {
        program: String,
        args: Vec<String>,
    },

    // Command to run a long-running process and stream its output
    Stream {
        program: String,
        args: Vec<String>,
    },

    Ping,
    // Direct input commands for low-latency interaction with kernel events.
    // This feature requires the "direct_input" flag during compilation.
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
    Success(String),     // Contains stdout for non-streaming commands
    Error(String),       // Contains stderr or general error messages
    StreamChunk(String), // One chunk of the stream output
    StreamEnd,           // Signals that streaming has finished
}
