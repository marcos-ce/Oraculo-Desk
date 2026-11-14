# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](licenses/LICENSE)
[![Rust](https://img.shields.io/badge/rust-edition%202024-orange.svg)](Cargo.toml)
[![Android](https://img.shields.io/badge/android-API%2026%2B%20(arm64)-3ddc84.svg)](android/app/build.gradle.kts)

*Read this in [Português (Brasil)](README.pt-BR.md).*

**Oráculo** is a peer-to-peer remote desktop, interactive terminal, and home-sandboxed file access suite written in **Rust** and **Kotlin** (Jetpack Compose). Connections run directly over encrypted QUIC streams powered by [iroh](https://github.com/n0-computer/iroh) (`ALPN: oraculo/1`), using `Ed25519` peer identities without manual port forwarding or central relay accounts.

---

## Key Capabilities

- **Zero-Config P2P Transport (`iroh`):** NAT hole-punching, encrypted QUIC streams, and `Ed25519` identity verification against a local allowlist (`trusted_peers.json`).
- **Low-Latency Wayland Screen Streaming (Linux):** Captures monitors through `xdg-desktop-portal` (`ScreenCast`) + PipeWire and encodes H.264 Annex-B via GStreamer (`nvh264enc` with automatic CPU fallback to `openh264enc`) with `videoscale method=lanczos`, decoded in hardware (`MediaCodec`) on Android.
- **Wayland Remote Input (Linux):** Pointer movement, clicks, scroll, X11 keysyms, Unicode fallback, and GNOME keyboard layout mapping via `xdg-desktop-portal` (`RemoteDesktop`).
- **Interactive PTY Terminal:** Cross-platform shell sessions backed by `portable-pty` on the host and `vt100` emulation on Android.
- **Quick Commands:** Execute pre-configured tasks from `commands.toml` (with a `30s` timeout and `256 KiB` per-stream output cap) or create/delete entries remotely when `commands_edit` is enabled.
- **Sandboxed Home File Browser:** Browse, preview, download, and upload files up to `2 GiB`, strictly confined to the host user's home directory (`~`) with symlink and traversal guards.
- **Bilingual Android Client (`arm64-v8a`):** Material 3 UI with instant **PT / EN** language toggle, Dark/Light/System theme selector, 4 control tabs (**Touchpad**, **Terminal**, **Commands**, **Files**), quality presets (`1080p30`, `1080p60`, `720p30`), pinch-to-zoom, RTT latency indicator, and foreground session service.

---

## Platform Support

| Component | Target | Current Status |
| :--- | :--- | :--- |
| **Control, PTY Terminal, Quick Commands, Files** | Linux, macOS, Windows | Cross-platform Rust core |
| **Screen Streaming & Remote Input Host** | Linux (Wayland + PipeWire + GStreamer) | Supported via `xdg-desktop-portal` |
| **Screen Streaming & Remote Input Host** | Windows / non-Linux | Stub (`Windows DXGI backend planned`) |
| **Mobile App (`Oráculo`)** | Android 8.0+ (API 26+, `arm64-v8a`) | Supported |
| **Terminal CLI (`oraculo-cli`)** | Linux / macOS / Windows | Supported |

> **Runtime requirement on Linux host:** Screen streaming launches `gst-launch-1.0` with `gdppay`, PipeWire GStreamer plugins, and an H.264 encoder (`nvh264enc` or `openh264enc`). Make sure `xdg-desktop-portal` and PipeWire are running in your Wayland session.

---

## Workspace Architecture

| Component | Path | Responsibility |
| :--- | :--- | :--- |
| **Wire Protocol** | [`crates/oraculo-proto`](crates/oraculo-proto) | `postcard`-framed protocol (`4 MiB` max frame, `128 KiB` file chunks) and stream discriminants (`Pair=1`, `Control=2`, `Terminal=3`, `Input=4`, `Files=5`, `Screen=6`). |
| **Host Daemon** | [`crates/oraculo-host`](crates/oraculo-host) | QUIC server, single-use QR pairing (`3 min` TTL), peer authorization, portal screen/input, PTY, commands, and file sandbox. |
| **Mobile Core (FFI)** | [`crates/oraculo-mobile-core`](crates/oraculo-mobile-core) | Pooled QUIC client (`OraculoMobileClient`), `vt100` terminal parser, H.264 frame queue, file transfers, and UniFFI bindings. |
| **CLI Client** | [`crates/oraculo-cli`](crates/oraculo-cli) | Desktop/terminal client for pairing, pinging, listing/running quick commands, and interactive shells. |
| **Android App** | [`android/`](android/) | Kotlin + Jetpack Compose client with QR pairing, `SurfaceView` `MediaCodec` player, touchpad, terminal, commands, and files UI. |

---

## Quick Start

### 1. Start the Host Daemon

Start `oraculo-host` and display a pairing QR Code (valid for **3 minutes**):

```bash
oraculo-host serve --pair
```

While `oraculo-host` is running interactively, press `p` + `Enter` at any time to issue a fresh pairing token.

### 2. Pair a Device

- **Android App:** Tap **Pair new computer** (`Parear novo computador` in PT-BR) and scan the QR Code printed by the host, or paste the `oraculo://pair?...` URI manually.
- **Desktop CLI (`oraculo-cli`):**
  ```bash
  oraculo-cli pair "<PASTE_PAIRING_URI>" --name "my-laptop"
  oraculo-cli ping
  oraculo-cli shell
  ```

### 3. Manage Host & Permissions

```bash
# List trusted peers
oraculo-host peers

# Revoke a peer (immediately closes its active streams)
oraculo-host revoke <ENDPOINT_ID>

# Inspect or update subsystem permissions & Wake-on-LAN settings
oraculo-host permissions --screen true --input true --terminal true
oraculo-host permissions --files-read true --files-write false --commands-edit false

# Show host EndpointId and config/data file locations
oraculo-host info

# Pre-authorize monitors via the desktop portal picker for multi-monitor switching
oraculo-host monitors
```

All `oraculo-cli` commands (`pair`, `ping`, `commands`, `run`, `shell`) accept `--host <ENDPOINT_ID>` to override the saved default host.

---

## Security Model

- **Mutual Cryptographic Identity:** Peers authenticate using `Ed25519` keys generated by `iroh`; unknown keys can only open `StreamKind::Pair`.
- **Hardened Pairing:** Tokens expire in **180 seconds**, are single-use, automatically burn after **5 failed attempts**, and verify secrets in constant time.
- **Instant Revocation & Toggles:** Removing a peer via `oraculo-host revoke` or disabling a capability via `oraculo-host permissions` terminates active streams immediately.
- **Home Directory Sandbox:** Remote file operations resolve canonical paths inside `~`, block `..` traversal and symlink escapes, and deny access to `.config/oraculo`, `.local/share/oraculo`, `appdata/roaming/oraculo`, `appdata/local/oraculo`, `.ssh`, `.gnupg`, `.local/share/keyrings`, `.config/autostart`, and shell startup files (`.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.zprofile`).
- **Private Storage:** Host/client key material (`host.key`, `trusted_peers.json`, `remote_desktop_token`, `screencast_token`, `client.key`) is written atomically with Unix mode `0600`.

See [`.github/SECURITY.md`](.github/SECURITY.md) for vulnerability reporting guidelines.

---

## Building from Source

Builds use Podman containers (`Containerfile` and `Containerfile.android`) with cached volumes for Rust (`edition = "2024"`), Android SDK 35 (`minSdk 26`), NDK, and UniFFI bindings:

```bash
# Check workspace compilation
./scripts/build.sh check

# Run workspace unit tests (oraculo-proto, oraculo-host, oraculo-mobile-core)
./scripts/build.sh test

# Build optimized desktop binaries into ./dist/oraculo-host and ./dist/oraculo-cli
./scripts/build.sh release

# Cross-compile arm64-v8a core, regenerate UniFFI bindings, and build ./dist/oraculo-debug.apk
./scripts/build-android.sh debug
```

---

## License

Licensed under the **Apache License, Version 2.0**. See [`licenses/LICENSE`](licenses/LICENSE) and [`licenses/NOTICE`](licenses/NOTICE) for details and third-party notices.
