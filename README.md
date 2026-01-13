# NativeBridge

> **Break the Chroot isolation. Control Android Host directly from your Linux Environment.**

[![Rust](https://img.shields.io/badge/Built_with-Rust-orange?style=flat-square)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Android_ARM64-green?style=flat-square)](https://developer.android.com/)

**NativeBridge** establishes a direct, high-performance communication channel between a Chrooted Linux Environment (like Ubuntu/Debian via Termux) and the Android Host System.

## Why NativeBridge?

By default, a Chroot Environment is strictly isolated. It lives in a separate filesystem "jail" (e.g., `/data/local/tmp/chrootubuntu`) and **cannot access the Android System**, hardware inputs, or global settings.

**NativeBridge solves this by creating a secure tunnel (Unix Domain Socket) that allows you to:**

- **Bypass Isolation:** Execute commands on the Android System directly from your Chroot Terminal.
- **Direct Kernel Injection:** Perform high-speed input injection (Tap/Swipe) by writing directly to `/dev/input/event*`, bypassing the heavy Android Framework (Java) for near zero-latency.
- **Protocol Optimization:** Uses **Bincode** serialization instead of JSON for minimal overhead.
- **Modular:** Built with Rust workspaces and feature flags to keep binaries small and efficient.

## Architecture

```mermaid
graph LR
   A[Android Host / Root] -- Listens on Unix Socket --> B((Bridge Server))
   C[Chroot Linux / Client] -- Sends Binary Payload --> B
   B -- Direct Kernel Write / Shell Exec --> A
   A -- Returns Output --> C
```

- **Server (`bridge_server`)**: Runs on Android Host (Root). Listens for incoming requests via a shared socket inside the Chroot path.
- **Client (`andro`)**: Runs inside the Chroot Environment. Sends commands to the Server.

## Prerequisites

- **Rooted Android Device** (KernelSu / Magisk / APatch).
- **Chroot Environment** (Ubuntu, Debian, Kali, Fedora, etc).
- **Rust Toolchain** (Only if building from source).

## Important Note on Hardcoded Paths:

Please be aware that the `TOUCH_DEVICE` path in `bridge_server/src/input_manager.rs` and the `SOCKET_PATH` (which defines the rootfs location for the Unix Domain Socket) in `bridge_server/src/main.rs` are currently hardcoded.

These paths are specific to the environment where the NativeBridge server is running and the Android device's input configuration. **You MUST adjust these paths** in the respective source files (`bridge_server/src/input_manager.rs` and `bridge_server/src/main.rs`) to match your particular root filesystem setup and the actual touch device path on your Android host system before building and running the server. Failure to do so will result in connection issues or non-functional direct input commands.

## Building from Source

Since we want to run these binaries on Android, we must build them statically using `musl`.

### 1. Setup Environment
```bash
# Add musl target for ARM64
rustup target add aarch64-unknown-linux-musl
```

### 2. Compile
You have two options depending on your needs:

**Option A: Standard Build (Exec Only)**
Lightweight, safer, only allows shell execution (`sh -c`).
```bash
cargo build --release --target aarch64-unknown-linux-musl --bin bridge_server
cargo build --release --target aarch64-unknown-linux-musl --bin bridge_client
```

**Option B: Full Build (Direct Input Support)**
Enable `direct_input` feature to allow Tap & Swipe via Kernel Injection.
*> Note: You may need to configure the `/dev/input/eventX` path in `input_manager.rs` before building.*
```bash
cargo build --release --target aarch64-unknown-linux-musl --features "direct_input" --bin bridge_server
cargo build --release --target aarch64-unknown-linux-musl --features "direct_input" --bin bridge_client
```

---

## Installation

### 1. Setup Server (Android Host)
The server must run outside the Chroot, but the binary is currently inside your build folder.

```bash
# [Inside Chroot Terminal]
# Copy binary to a temporary location accessible by Android
cp target/aarch64-unknown-linux-musl/release/bridge_server /tmp/bridge_server
```

Now, open a **separate terminal** (Termux or ADB Shell) that is **NOT** inside Chroot:

```bash
# [Inside Android Termux/ADB]
su
# Move to /data/local/tmp (Exec is allowed here)
cp /data/local/tmp/chrootubuntu/tmp/bridge_server /data/local/tmp/
chmod +x /data/local/tmp/bridge_server

# Run in background
/data/local/tmp/bridge_server &
```
*Tip: Use KernelSU `service.d` to auto-start this on boot.*

### 2. Setup Client (Chroot)
The client lives inside your Chroot environment.

```bash
# [Inside Chroot Terminal]
cp target/aarch64-unknown-linux-musl/release/bridge_client /usr/local/bin/andro
chmod +x /usr/local/bin/andro
```

---

## Usage

Simply call `andro` followed by the subcommand.

### 1. General Execution
Run any command as if you were in the Android Root Shell.
```bash
# Check User Identity (flag -e / exec)
andro -e id
# Output: uid=0(root) gid=0(root) ...

# Check Battery Status
andro -e dumpsys battery

# Reboot Device
andro -e reboot

# Monitoring Logcat Android Real-Time (flag -s / stream)
andro -s logcat
```

### 2. Direct Input (Kernel Injection)
*Requires build with `--features "direct_input"`*

```bash
# Tap (Instant click at X=500, Y=500)
andro tap 500 500

# Swipe (Scroll down)
# Format: swipe <x1> <y1> <x2> <y2> <duration_ms>
andro swipe 500 1500 500 500 300
```

### 3. Screenshot
Capture the Android screen and save it directly to the Chroot filesystem.
```bash
# Capture and redirect output to a file
andro -e screencap -p > /home/user/capture.png
```

### 4. Utilities
```bash
# Check if server is alive
andro ping

# Show help menu
andro --help
```

## Troubleshooting

**"Connection refused" or "No such file or directory"**
- Ensure `bridge_server` is running on Android Host.
- Ensure the socket path in `main.rs` matches your Chroot mount point (Default: `/data/local/tmp/chrootubuntu/tmp/bridge.sock`).

**Tap/Swipe not working**
- Did you build with `--features "direct_input"`?
- Did you set the correct Touchscreen Device path (e.g., `/dev/input/event2`) in `input_manager.rs` before building? Use `getevent -pl` in Termux to find your specific device event.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
