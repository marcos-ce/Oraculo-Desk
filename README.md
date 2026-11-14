# Oráculo Desk

Language: **English** | [Português (Brasil)](README.pt-BR.md) 

Oráculo Desk is a self-hosted, peer-to-peer remote desktop and host management suite built in Rust and Kotlin. It lets you control a Linux (Wayland or X11) or Windows computer from an Android phone or another computer over direct QUIC connections (`iroh`), bypassing NATs and firewalls without port forwarding, dynamic DNS, or third-party accounts.

## Overview

Most remote desktop tools either struggle with Linux Wayland sessions, require creating an account on a central broker, or compress the entire desktop experience into a tiny mouse-only viewer. Oráculo Desk separates everyday host tasks into dedicated, low-overhead modules that run over a single encrypted P2P connection:

- Low-latency H.264 remote desktop with hardware-accelerated encoding (NVIDIA NVENC, VA-API, OpenH264, or x264), direct touch mode, relative trackpad mode, two-finger scroll, pinch-to-zoom up to 4x, multi-monitor switching from the phone, landscape fullscreen mode for external keyboards and mice, and an on-screen modifier bar (`Ctrl`, `Alt`, `Super`, `Esc`, `Tab`, arrows, `F1`-`F12`).
- Native Wayland and login-screen support through `xdg-desktop-portal` (`ScreenCast` and `RemoteDesktop`) with persistent restore tokens so the desktop does not ask for permission on every reboot. When no graphical session is active (such as a lock screen or physical TTY), it can fall back to direct DRM/KMS framebuffer capture (`/dev/dri/card*`) and `/dev/uinput` kernel input devices.
- Interactive PTY terminal providing full shell access (`bash`, `zsh`, `fish`, or PowerShell) rendered natively on Android through a Rust `vt100` terminal state machine with ANSI/256/truecolor support and scrollback history.
- One-tap quick commands for running pre-authorized commands or scripts defined on the host (`commands.toml`) or added from the mobile app, with execution timeouts and bounded output capture.
- Sandboxed file manager confined to the host user's home directory (`~`), with in-app image and text previews, downloads to Android `Downloads`, and uploads from the phone with automatic conflict numbering and symlink escape protection.
- Live system telemetry and power control for monitoring CPU load, RAM, swap, uptime, network throughput per interface, and top processes in real time, plus session lock, suspend, reboot, power off, and local Wake-on-LAN (WoL).

## Supported platforms

| Component | Platforms |
| :--- | :--- |
| Host daemon (`oraculo-host`) | Linux (Wayland via PipeWire/`xdg-desktop-portal`, DRM/KMS, or X11) and Windows |
| Mobile client (`Oráculo`) | Android 8.0+ (API 26+, `arm64-v8a`), hardware `MediaCodec` H.264 decoding |
| Desktop CLI client (`oraculo-cli`) | Linux and Windows (for terminal-to-terminal shell, ping, telemetry, and WoL) |

## Quick start

### 1. Install runtime dependencies on the Linux host

For hardware-accelerated H.264 streaming and PipeWire capture on Linux, install the GStreamer plugins for your distribution:

**Debian / Ubuntu:**
```bash
sudo apt update
sudo apt install gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-pipewire
```

**Fedora:**
```bash
sudo dnf install gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good \
  gstreamer1-plugins-bad-free gstreamer1-plugin-openh264 pipewire-gstreamer
```

**Arch Linux:**
```bash
sudo pacman -S gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad \
  gst-plugin-pipewire openh264
```

### 2. Start the host service

After building or downloading `oraculo-host` and `oraculo-cli`, install them into `~/.local/bin` and enable the user background service:

```bash
install -Dm755 ./dist/oraculo-host ~/.local/bin/oraculo-host
install -Dm755 ./dist/oraculo-cli ~/.local/bin/oraculo-cli
oraculo-host install-service
```

### 3. Pair your Android phone or secondary computer

Generate a single-use pairing QR code in the host terminal:

```bash
oraculo-host pair
```

Open the Oráculo Android app, tap **Novo pareamento**, and scan the QR code displayed in the terminal (or paste the `oraculo://pair?...` URI manually). The pairing token is valid for 5 minutes and burns automatically after first use or after 5 failed attempts.

### 4. Optional: unattended input and pre-login screen capture on Linux

In a standard Wayland session, screen capture and remote input work through `xdg-desktop-portal` without elevated privileges. If you also want `/dev/uinput` virtual input and DRM/KMS screen capture on the login/lock screen, add your user to the `input` group and grant `cap_sys_admin` to the host binary:

```bash
sudo usermod -aG input "$USER"
sudo setcap cap_sys_admin+ep ~/.local/bin/oraculo-host
```

## Host CLI reference

```text
oraculo-host serve                          Run the daemon in the foreground
oraculo-host pair [--ttl-secs 300]          Generate a single-use pairing QR code and URI
oraculo-host id                             Print the host EndpointId
oraculo-host devices                        List paired devices
oraculo-host revoke <ENDPOINT_ID>           Revoke a paired device immediately
oraculo-host permissions                    Show current subsystem permissions (settings.toml)
oraculo-host set-permission <PERM> <on|off> Toggle terminal, files, input, screen, or commands-edit
oraculo-host wol [--mac AA:BB:CC:DD:EE:FF]  Inspect or set the MAC address advertised for Wake-on-LAN
oraculo-host commands                       List pre-authorized quick commands (commands.toml)
oraculo-host install-service                Install and start ~/.config/systemd/user/oraculo-host.service
oraculo-host uninstall-service              Stop and remove the user systemd service
oraculo-host service-status                 Show systemd service and DRM/uinput capability status
```

## Security design

Every connection uses QUIC with TLS 1.3 and mutual Ed25519 endpoint authentication (`iroh`, ALPN `oraculo/p2p/1`). Secret keys (`host.key`, `client.key`), paired peer registries (`trusted_peers.json`), and portal restore tokens are created with `0600` permissions on Unix.

Unpaired peers attempting to open functional streams have their QUIC connection terminated immediately (`401 unpaired`). Revoking a device (`oraculo-host revoke <ID>`) or disabling a subsystem (`oraculo-host set-permission <subsystem> off`) drops active streams in real time without restarting the daemon.

The remote file manager is confined to the canonicalized user home directory (`~`). Path resolution blocks `..` traversal, external symlinks, and access to sensitive paths inside `~`, including `.config/oraculo`, `.local/share/oraculo`, `.ssh`, `.gnupg`, `.local/share/keyrings`, `.config/autostart`, and shell startup files (`.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.zprofile`).

## Architecture

The repository is structured as a Rust workspace paired with a native Android application:

| Component | Path | Description |
| :--- | :--- | :--- |
| `oraculo-proto` | `crates/oraculo-proto` | Wire protocol messages, 4-byte big-endian length-prefixed `postcard` framing (4 MiB max frame), `StreamKind` multiplexing headers, and `oraculo://pair` URI parser. |
| `oraculo-host` | `crates/oraculo-host` | Desktop daemon (`server.rs`), PipeWire/GStreamer/DRM screen streamer (`screen.rs`), Portal/`uinput`/`enigo` input controller (`input.rs`), `portable-pty` shell (`pty.rs`), and home sandbox (`files.rs`). |
| `oraculo-cli` | `crates/oraculo-cli` | Desktop command-line client for pairing, RTT measurement, telemetry inspection, remote shell, and command execution. |
| `oraculo-mobile-core` | `crates/oraculo-mobile-core` | Rust library compiled via `cargo-ndk` and exposed to Kotlin through `uniffi`. Implements the QUIC connection pool, `vt100` terminal parser, H.264 Annex-B frame queue, and Wake-on-LAN sender. |
| `android` | `android/` | Jetpack Compose (Material 3) Android app with `SurfaceView` + hardware `MediaCodec` H.264 decoding and a persistent foreground session service (`OraculoSessionService`). |

## Building from source

The repository includes Podman build scripts that run the Rust compiler, GStreamer development headers, Android SDK, and Android NDK inside reproducible containers while caching dependencies in Podman volumes.

```bash
# Run all workspace unit tests
./scripts/build.sh test

# Compile optimized desktop binaries into ./dist/oraculo-host and ./dist/oraculo-cli
./scripts/build.sh release

# Cross-compile liboraculo_mobile_core.so (arm64-v8a), generate UniFFI Kotlin bindings, and build ./dist/oraculo-debug.apk
./scripts/build-android.sh debug
```

If you prefer building the desktop binaries directly on the host without Podman, run `cargo build --release --workspace` with Rust 1.85+ (`edition = "2024"`).

## License

Licensed under the [Apache License, Version 2.0](licenses/LICENSE) ([Third-Party Notices](licenses/NOTICE)).
