# AGENTS.md - Guide for Autonomous Coding Agents

This file defines the architecture, build commands, and invariants of **Oraculo-Desk** (`https://github.com/marcos-ce/Oraculo-Desk`) for autonomous coding agents (Antigravity, Claude Code, Cursor, GitHub Copilot, Codex, Windsurf, Aider).

## 1. Build and verification commands (mandatory container workflow)

Do not run `cargo` or `gradle` directly on the host machine. The repository provides Podman scripts that mount cached volumes (`oraculo-cargo-cache`, `oraculo-target-cache`, `oraculo-gradle-cache`) and include the exact Rust toolchain (`edition = "2024"`), GStreamer development headers, Android SDK 34, and Android NDK:

```bash
# Typecheck all Rust crates in the workspace
./scripts/build.sh check

# Run all workspace unit tests (oraculo-proto, oraculo-host, oraculo-mobile-core)
./scripts/build.sh test

# Build optimized desktop binaries into ./dist/oraculo-host and ./dist/oraculo-cli
./scripts/build.sh release

# Cross-compile liboraculo_mobile_core.so (arm64-v8a), generate UniFFI Kotlin bindings, and build ./dist/oraculo-debug.apk
./scripts/build-android.sh debug
```

Before finishing any code change, run `./scripts/build.sh test` and, if `crates/oraculo-proto`, `crates/oraculo-mobile-core`, or `android/` changed, run `./scripts/build-android.sh debug`.

## 2. Workspace map and responsibility boundaries

- `crates/oraculo-proto` (`src/lib.rs`, `src/pairing.rs`):
  - Wire framing: 4-byte big-endian length prefix followed by `postcard`-serialized payload. `MAX_FRAME_SIZE` is `4 * 1024 * 1024` bytes (4 MiB).
  - Multiplexing: every bidirectional QUIC stream starts with a 1-byte `StreamKind` discriminant (`Pair = 1`, `Control = 2`, `Terminal = 3`, `Input = 4`, `Screen = 5`, `Files = 6`).
  - Protocol compatibility: adding fields or enum variants in `oraculo-proto` requires updating both `crates/oraculo-host` and `crates/oraculo-mobile-core`, then regenerating Kotlin bindings via `./scripts/build-android.sh bindings` or `./scripts/build-android.sh debug`.

- `crates/oraculo-host`:
  - `src/main.rs`: CLI entry point (`serve`, `pair`, `id`, `devices`, `revoke`, `permissions`, `set-permission`, `wol`, `commands`, `install-service`, `uninstall-service`, `service-status`).
  - `src/server.rs`: `iroh` QUIC endpoint listener (ALPN `oraculo/p2p/1`), peer authorization via `TrustedPeersStore`, active connection registry (`ConnectionRegistry`), and stream dispatcher.
  - `src/screen.rs`: screen capture and H.264 Annex-B streaming. Supports Wayland `xdg-desktop-portal` (`ashpd`) + PipeWire + GStreamer (`nvh264enc`, `vah264enc`, `vaapih264enc`, `openh264enc`, `x264enc`), DRM/KMS framebuffer fallback (`/dev/dri/card*` with `CAP_SYS_ADMIN`), and `xcap` fallback for X11/Windows.
  - `src/input.rs`: remote mouse, touch, and keyboard injection via Wayland `RemoteDesktop` portal, Linux `/dev/uinput` virtual devices (absolute tablet `0..65535`, relative mouse, and keyboard), or `enigo` fallback.
  - `src/pty.rs`: interactive shell sessions using `portable-pty`.
  - `src/files.rs`: home-directory (`~`) sandboxed file listing, download, and atomic upload.
  - `src/commands.rs`: execution of pre-authorized quick commands from `commands.toml` with a 30s timeout and 256 KiB output limit per stream.
  - `src/config.rs`: atomic persistence for `host.key`, `trusted_peers.json`, `settings.toml`, and `commands.toml`.

- `crates/oraculo-mobile-core` (`src/lib.rs`):
  - Exposes `OraculoMobileClient` to Kotlin via `#[uniffi::export]`.
  - Manages a reusable QUIC connection pool per `endpoint_id`, automatic stream retry on stale connections, `vt100` terminal emulation, low-latency `ScreenSession` frame queueing, file transfers, and UDP Wake-on-LAN broadcast.
  - Never edit generated files under `android/app/src/main/java/uniffi/` by hand; they are overwritten by `uniffi-bindgen`.

- `android/app/src/main/java/com/oraculo/app`:
  - `MainActivity.kt`: host list, QR code pairing (`QrScannerScreen.kt`), manual URI pairing, and navigation.
  - `HostControlScreen.kt`: connected host dashboard with tabs (`Terminal`, `Comandos`, `Sistema`, `Arquivos`), RTT latency indicator, power actions, and `HostSessionNotificationEffect`.
  - `ScreenStreamingScreen.kt` & `ScreenVideoDecoder.kt`: hardware `MediaCodec` (`video/avc`) decoding onto a `SurfaceView`, direct touch and relative trackpad input, pinch-to-zoom, monitor switcher, resolution/FPS selectors, and landscape fullscreen mode.
  - `FilesTab.kt`: remote file browser, image/text previewer, Android `Downloads` exporter, and file uploader.
  - `OraculoSessionService.kt`: Android `specialUse` foreground service displaying a persistent notification while a host session is active.

## 3. Critical invariants (do not break)

1. **GStreamer scaling and H.264 stream format (`crates/oraculo-host/src/screen.rs`)**:
   - Keep `videoscale method=lanczos` in the GStreamer pipeline args. Do not use `method=bicubic` (invalid enum value in GStreamer 1.24+ `videoscale`, which breaks 720p/480p/360p scaling).
   - Output caps must remain `video/x-h264,stream-format=byte-stream,alignment=au` wrapped in `gdppay` so `normalize_h264_au` can extract SPS (`NAL 7`) and PPS (`NAL 8`) and prepend them to every IDR keyframe (`NAL 5`) for Android `MediaCodec`.
   - When sending a terminal `ScreenHostMsg::Error` on a QUIC stream, wait briefly before `send.finish()` (`iroh_screen_error_frame_arrives_before_finish` test) so QUIC `RESET_STREAM` on drop does not discard the error frame in transit.

2. **File sandbox and secret protection (`crates/oraculo-host/src/files.rs` & `config.rs`)**:
   - All remote file operations must resolve inside `canonical_home` (`~`) via `resolve_existing_in_home` or `resolve_upload_target`.
   - Never weaken `is_protected_rel_str` / `is_protected_in_home`, which block access to `.config/oraculo`, `.local/share/oraculo`, `.ssh`, `.gnupg`, `.local/share/keyrings`, `.config/autostart`, and shell startup files (`.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.zprofile`).
   - Private files (`host.key`, `trusted_peers.json`, `screencast_restore.token`, `client.key`) must be created with Unix mode `0600`.

3. **Connection and PTY lifecycle (`crates/oraculo-host/src/server.rs` & `pty.rs`)**:
   - Untrusted peers opening non-`Pair` streams or failing `PairRequest` must have their QUIC connection closed immediately.
   - Revoking a peer or turning off a subsystem permission in `settings.toml` must terminate active streams immediately via `watch::Receiver<HostPermissions>`.
   - `handle_terminal_stream` must always call `killer.kill()` when exiting the loop so no orphan shell processes remain on the host.
