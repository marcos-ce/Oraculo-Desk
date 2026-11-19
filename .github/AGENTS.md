# AGENTS.md - Guide for Autonomous Coding Agents

This file defines the architecture, build commands, and critical invariants of **Oráculo** (`https://github.com/marcos-ce/Oraculo-Desk`) for autonomous coding agents.

## 1. Build and Verification Commands (Mandatory Container Workflow)

Do not run `cargo` or `gradle` directly on the host machine. Use the repository Podman scripts with cached volumes (`oraculo-cargo-cache`, `oraculo-target-cache`, `oraculo-gradle-cache`), which provide the pinned Rust toolchain (`edition = "2024"`), Android SDK 35 (`minSdk = 26`, `targetSdk = 35`), and Android NDK:

```bash
# Typecheck all Rust crates in the workspace
./scripts/build.sh check

# Run all workspace unit tests (all 6 crates)
./scripts/build.sh test

# Build optimized desktop binaries into ./dist/{oraculo-host,oraculo-desktop,oraculo-bridge,oraculo-cli}
./scripts/build.sh release

# Cross-compile liboraculo_mobile_core.so (arm64-v8a), generate UniFFI Kotlin bindings, and build ./dist/oraculo-debug.apk
./scripts/build-android.sh debug

# Audit Material 3 token parity (Android <-> Desktop), i18n keys, and AutoMirrored icons
python3 .agents/skills/oraculo-m3-design/scripts/check_design_tokens.py --strict
```

Before finishing any code change, run `./scripts/build.sh test` and, whenever `crates/oraculo-proto`, `crates/oraculo-mobile-core`, or `android/` changes, run `./scripts/build-android.sh debug`.

## 2. Workspace Map and Responsibility Boundaries

- `crates/oraculo-proto` (`src/lib.rs`, `src/pairing.rs`, `src/wol.rs`, `src/mcu.rs`, `src/i18n.rs`):
  - Transport ALPN: `b"oraculo/1"`.
  - Wire framing: 4-byte big-endian length prefix followed by `postcard`-serialized payload. `MAX_FRAME_SIZE` is `4 * 1024 * 1024` bytes (`4 MiB`), `FILE_CHUNK_SIZE` is `128 KiB`.
  - Multiplexing: every bidirectional QUIC stream starts with a 1-byte `StreamKind` discriminant (`Pair = 1`, `Control = 2`, `Terminal = 3`, `Input = 4`, `Files = 5`, `Screen = 6`, `Tunnel = 7`).
  - Pairing constants: `PAIRING_TTL` is `180s` (3 minutes), `MAX_PAIRING_ATTEMPTS` is `5`.
  - Protocol compatibility: changing wire types in `oraculo-proto` requires updating `crates/oraculo-host` and `crates/oraculo-mobile-core`, then regenerating Kotlin bindings via `./scripts/build-android.sh bindings` or `./scripts/build-android.sh debug`.

- `crates/oraculo-host`:
  - `src/main.rs`: CLI entry points: `serve [--pair]` (default), `peers`, `revoke <ENDPOINT_ID>`, `permissions [FLAGS]`, `info`, `monitors`, `wol`.
  - `src/server.rs`: `iroh` QUIC endpoint listener, `host.lock` single-instance guard, stdin `'p'` pairing trigger, `TrustedPeersStore`, `ConnectionRegistry`, and stream dispatcher.
  - `src/screen.rs`: Linux/Wayland screen capture via `xdg-desktop-portal` (`ScreenCast`, `ashpd`) + PipeWire + `gst-launch-1.0` subprocess (`nvh264enc` or `openh264enc`), reading `/sys/class/drm/*/status` only to count active monitors. Non-Linux builds use a stub backend.
  - `src/input.rs`: Linux/Wayland remote mouse and keyboard injection via `xdg-desktop-portal` (`RemoteDesktop`, `ashpd`) supporting X11 keysyms, Unicode hex fallback, and GNOME keyboard layout detection. Non-Linux builds use a stub backend.
  - `src/portal.rs`: persistent per-monitor-topology portal authorization vault (`portal_tokens.json`) and GNOME lock-screen detection.
  - `src/tunnel.rs`: local RDP loopback tunnel (`127.0.0.1` / `[::1]`) and `firewalld` LAN exposure checks.
  - `src/pty.rs`: interactive and persistent multi-session shells via `portable-pty`.
  - `src/files.rs`: home-directory (`~`) sandboxed file listing, download, and atomic upload (`MAX_UPLOAD_BYTES = 2 GiB`).
  - `src/commands.rs`: quick commands from `commands.toml` (`COMMAND_TIMEOUT = 30s`, `MAX_COMMAND_OUTPUT_BYTES = 256 KiB` per stream).
  - `src/wol.rs`: NIC detection, Magic Packet sender, and NetworkManager/ethtool WoL activation.
  - `src/control_ipc.rs`: authenticated local HTTP IPC bridging `oraculo-host` and `oraculo-desktop`.
  - `src/config.rs`: atomic persistence for `host.key`, `trusted_peers.json`, `portal_tokens.json`, `remote_desktop_token`, `screencast_token`, `settings.toml`, and `commands.toml`.

- `crates/oraculo-desktop`:
  - `src/main.rs`, `src/ipc.rs`, `src/webview.rs`, `src/ui.html`: native Material Design 3 desktop companion app (GTK4/libadwaita + WebKitGTK via runtime `dlopen` with GTK3 fallback on Linux; Edge WebView2 on Windows) serving local `Google Sans` assets and bilingual `PT`/`EN` UI.

- `crates/oraculo-bridge` (`src/main.rs`):
  - Lightweight standalone Wake-on-LAN P2P relay daemon with zero screen/input/terminal/file code.

- `crates/oraculo-cli` (`src/main.rs`):
  - Terminal client CLI (`pair`, `ping`, `commands`, `run`, `shell`, `wake`).

- `crates/oraculo-mobile-core` (`src/lib.rs`, `src/bridge.rs`):
  - Exposes `OraculoMobileClient` (including `set_language`, RDP tunnel, and `MobileWolBridge`) to Kotlin via `#[uniffi::export]`.
  - Manages pooled QUIC connections per `EndpointId`, retry on stale connections, `vt100` terminal emulation, H.264 Annex-B normalization (`SPS`/`PPS` prepended to `IDR` keyframes), file transfers, and UDP/MCU Wake-on-LAN.
  - Never edit generated files under `android/app/src/main/java/uniffi/` by hand; `uniffi-bindgen` overwrites them.

- `android/app/src/main/java/com/oraculo/app`:
  - `OraculoApplication.kt`: process-scoped `OraculoMobileClient` singleton and locale sync.
  - `MainActivity.kt`: host list, empty state (`JijiCatIllustration`), QR scanner (`QrScannerScreen.kt`), manual pairing dialog, biometric/PIN lock, WoL bridge mode, and theme & `PT`/`EN` language switcher.
  - `HostControlScreen.kt`: connected dashboard with 4 tabs (`Touchpad`, `Terminal`, `Commands`, `Files`), RTT latency badge, RDP unlock tunnel dialog, and `OraculoSessionService` lifecycle.
  - `ScreenStreamingScreen.kt` & `ScreenVideoDecoder.kt`: hardware `MediaCodec` (`video/avc`) rendering onto `SurfaceView`, relative trackpad & direct touch input, pinch-to-zoom, quality presets (`1080p30`, `1080p60`, `720p30`), monitor switcher, and fullscreen landscape mode.
  - `RemoteInputDeck.kt`, `FilesTab.kt`, `WakeOnLanDialog.kt`, `OraculoSessionService.kt` (Android 15 `onTimeout` + coordinated session/WoL FGS), and `ui/Theme.kt`.

- `firmware/oraculo-wol-esp32/`:
  - Optional ESP32 / M5Stack Cardputer Wake-on-LAN bridge firmware with WPA2 per-session setup SoftAP and MQTT/TLS `HMAC-SHA256` challenge-response.

## 3. Critical Invariants (Do Not Break)

1. **GStreamer Scaling and H.264 Stream Format (`crates/oraculo-host/src/screen.rs`):**
   - Keep `videoscale method=lanczos` in GStreamer pipeline args. Do not use `method=bicubic` (invalid enum in GStreamer 1.24+, which breaks scaled resolutions).
   - Output caps must remain `video/x-h264,stream-format=byte-stream,alignment=au` wrapped in `gdppay` so `normalize_h264_au` can extract SPS (`NAL 7`) and PPS (`NAL 8`) and prepend them to every IDR keyframe (`NAL 5`) for Android `MediaCodec`.
   - When sending a terminal `ScreenHostMsg::Error` on a QUIC stream, wait briefly before finishing (`iroh_screen_error_frame_arrives_before_finish`) so QUIC `RESET_STREAM` on drop does not discard the frame in transit.

2. **File Sandbox and Secret Protection (`crates/oraculo-host/src/files.rs` & `config.rs`):**
   - Every remote file path must resolve strictly inside `canonical_home` (`~`) via `resolve_existing_in_home` or `resolve_upload_target`.
   - Never weaken `is_protected_rel_str` / `is_protected_in_home`, which block Oráculo's own config/data directories (host, bridge, cli), `.ssh`, `.gnupg`, `.local/share/keyrings`, `.config/autostart`, `.config/systemd/user`, `.config/environment.d`, `.config/fish`, `.bashrc`, `.bash_profile`, `.bash_login`, `.bash_logout`, `.profile`, `.zshrc`, `.zprofile`, and `.zshenv`.
   - Private files (`host.key`, `bridge.key`, `trusted_peers.json`, `portal_tokens.json`, `remote_desktop_token`, `screencast_token`, `client.key`) must use Unix mode `0600`.

3. **Connection and PTY Lifecycle (`crates/oraculo-host/src/server.rs` & `pty.rs`):**
   - Untrusted peers opening non-`Pair` streams or failing `PairRequest` must have their QUIC connection closed immediately and never enter `ConnectionRegistry`.
   - Revoking a peer or turning off a subsystem permission in `settings.toml` must terminate active streams immediately via `watch::Receiver<HostPermissions>`.
   - `handle_terminal_stream` must always call `killer.kill()` on exit so no orphan shell processes remain on the host.
