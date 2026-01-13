// This module will only be compiled if the `--features "direct_input"` flag is used.
#![cfg(feature = "direct_input")]

use std::fs::OpenOptions;
use std::io::Write;
use std::mem;
use std::thread;
use std::time::Duration;

// Input device configuration.
// This path is vital and must match the event device for the touchscreen on the target system.
// To find it, run `getevent -pl` in the Android shell and look for the device
// that has `ABS_MT_POSITION_X` and `ABS_MT_POSITION_Y` events.
const TOUCH_DEVICE: &str = "/dev/input/event1";

// Represents the `input_event` structure from the Linux kernel.
// This is a Rust representation of the C struct used by the kernel
// to report input events, allowing us to write these events
// directly to the device file.
#[repr(C)]
struct InputEvent {
    time_sec: usize,  // Seconds
    time_usec: usize, // Microseconds
    type_: u16,       // Event type (e.g., EV_ABS for absolute axis)
    code: u16,        // Event code (e.g., ABS_MT_POSITION_X)
    value: i32,       // Event value
}

pub fn tap(x: i32, y: i32) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(TOUCH_DEVICE)?;

    send_touch_event(&mut file, x, y, 1)?; // "touch down" event
    send_touch_event(&mut file, x, y, 0)?; // "touch up" event
    thread::sleep(Duration::from_millis(50)); // Short delay for stability

    Ok(())
}

pub fn swipe(x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u64) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(TOUCH_DEVICE)?;

    let step_delay = 10;
    let steps = (duration_ms / step_delay).max(1);
    let dx = (x2 - x1) as f32 / steps as f32;
    let dy = (y2 - y1) as f32 / steps as f32;

    // Start the swipe gesture with a "touch down" event
    send_touch_event(&mut file, x1, y1, 1)?;

    // Perform linear interpolation for a smooth movement
    let mut current_x = x1 as f32;
    let mut current_y = y1 as f32;
    for _ in 0..steps {
        current_x += dx;
        current_y += dy;
        send_move_event(&mut file, current_x as i32, current_y as i32)?;
        thread::sleep(Duration::from_millis(step_delay));
    }

    // End the swipe gesture with a "touch up" event
    send_touch_event(&mut file, x2, y2, 0)?;
    Ok(())
}

// Internal helper function to write a raw `InputEvent` to the device file.
fn write_event(file: &mut std::fs::File, type_: u16, code: u16, value: i32) -> std::io::Result<()> {
    let ev = InputEvent {
        time_sec: 0,
        time_usec: 0,
        type_,
        code,
        value,
    };
    // Convert the struct to a raw byte slice to be written to the file.
    // This is an `unsafe` operation because Rust cannot guarantee memory layout,
    // but it is safe here because `#[repr(C)]` ensures a C-like layout.
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(&ev as *const _ as *const u8, mem::size_of::<InputEvent>())
    };
    file.write_all(bytes)
}

// Sends a complete touch event (X/Y position and touch state).
fn send_touch_event(file: &mut std::fs::File, x: i32, y: i32, state: i32) -> std::io::Result<()> {
    write_event(file, 3, 53, x)?; // EV_ABS, ABS_MT_POSITION_X
    write_event(file, 3, 54, y)?; // EV_ABS, ABS_MT_POSITION_Y
    write_event(file, 1, 330, state)?; // EV_KEY, BTN_TOUCH (1=Down, 0=Up)
    write_event(file, 0, 0, 0)?; // EV_SYN, SYN_REPORT (synchronize event)
    Ok(())
}

// Sends a move event (only X/Y position).
fn send_move_event(file: &mut std::fs::File, x: i32, y: i32) -> std::io::Result<()> {
    write_event(file, 3, 53, x)?; // EV_ABS, ABS_MT_POSITION_X
    write_event(file, 3, 54, y)?; // EV_ABS, ABS_MT_POSITION_Y
    write_event(file, 0, 0, 0)?; // EV_SYN, SYN_REPORT
    Ok(())
}
