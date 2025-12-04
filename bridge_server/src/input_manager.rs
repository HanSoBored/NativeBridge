// Hanya akan di compile jika flag --features digunakan saat build project
#![cfg(feature = "direct_input")]

use std::fs::OpenOptions;
use std::io::Write;
use std::mem;
use std::thread;
use std::time::Duration;

// Configuration Device
// Ubah path sesuai touch device di getevent -pl cari eventx:ABS_MT_POSITION_X/Y
const TOUCH_DEVICE: &str = "/dev/input/event2";

// Struktur Data Internal Linux
#[repr(C)]
struct InputEvent {
    time_sec: usize,
    time_usec: usize,
    type_: u16,
    code: u16,
    value: i32,
}

// Fungsi Publik (API)
pub fn tap(x: i32, y: i32) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(TOUCH_DEVICE)?;

    // Logic Tap
    send_touch_event(&mut file, x, y, 1)?; // Down
    send_touch_event(&mut file, x, y, 0)?; // Up
    thread::sleep(Duration::from_millis(50));

    Ok(())
}

pub fn swipe(x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u64) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(TOUCH_DEVICE)?;

    let step_delay = 10;
    let steps = (duration_ms / step_delay).max(1);
    let dx = (x2 - x1) as f32 / steps as f32;
    let dy = (y2 - y1) as f32 / steps as f32;

    // Start
    send_touch_event(&mut file, x1, y1, 1)?;

    // Move
    let mut current_x = x1 as f32;
    let mut current_y = y1 as f32;
    for _ in 0..steps {
        current_x += dx;
        current_y += dy;
        send_move_event(&mut file, current_x as i32, current_y as i32)?;
        thread::sleep(Duration::from_millis(step_delay));
    }

    // End
    send_touch_event(&mut file, x2, y2, 0)?;
    Ok(())
}

// Fungsi Private (Helper)
fn write_event(file: &mut std::fs::File, type_: u16, code: u16, value: i32) -> std::io::Result<()> {
    let ev = InputEvent {
        time_sec: 0,
        time_usec: 0,
        type_,
        code,
        value,
    };
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(&ev as *const _ as *const u8, mem::size_of::<InputEvent>())
    };
    file.write_all(bytes)
}

fn send_touch_event(file: &mut std::fs::File, x: i32, y: i32, state: i32) -> std::io::Result<()> {
    write_event(file, 3, 53, x)?; // ABS_MT_POSITION_X
    write_event(file, 3, 54, y)?; // ABS_MT_POSITION_Y
    write_event(file, 1, 330, state)?; // BTN_TOUCH (1=Down, 0=Up)
    write_event(file, 0, 0, 0)?; // SYN_REPORT
    Ok(())
}

fn send_move_event(file: &mut std::fs::File, x: i32, y: i32) -> std::io::Result<()> {
    write_event(file, 3, 53, x)?;
    write_event(file, 3, 54, y)?;
    write_event(file, 0, 0, 0)?;
    Ok(())
}
