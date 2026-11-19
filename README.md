# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](licenses/LICENSE)

*Read this in [Português (Brasil)](README.pt-BR.md).*

Remote desktop, terminal, and file manager connecting your Android phone to your Linux desktop over end-to-end encrypted QUIC (Iroh). No accounts and no router port forwarding required.

## What it does

- Stream your desktop screen to your phone (H.264, hardware decoded) with multi-monitor switching
- Control mouse and keyboard remotely (touchpad and direct touch)
- Open persistent multi-tab terminal sessions that stay alive across reconnects
- Run and manage pre-configured quick commands with one tap
- Browse, download, and upload files (confined to your home directory)
- Unlock GDM / Remote Login screens over an encrypted local RDP tunnel (`127.0.0.1:3389`)
- Wake computers remotely via Wake-on-LAN on your LAN or through Network bridges (`oraculo-bridge`, a spare Android device, or Cardputer/ESP32), covered in the [Complete Wake-on-LAN Guide](docs/wake-on-lan.md)

The desktop companion app (`oraculo-desktop`) provides a native GUI (GTK4/libadwaita + WebKitGTK on Linux, Edge WebView2 on Windows) for pairing, live permissions, monitor authorization, quick commands, Wake-on-LAN, autostart, and local RDP firewall status.

The Android app uses a Material 3 UI with PT/EN toggle, light/dark themes, optional **Biometric or PIN lock**, pinch-to-zoom, and four tabs: Touchpad, Terminal, Commands, and Files.

## Download

Go to the [Releases](https://github.com/marcos-ce/Oraculo-Desk/releases/latest) page and download both:

| | File | For |
| :--- | :--- | :--- |
| 🖥️ | `oraculo-vX.Y.Z-linux-x86_64.tar.gz` | Your Linux computer (`oraculo-desktop`, `oraculo-host`, `oraculo-bridge`, `oraculo-cli`) |
| 📱 | `oraculo-vX.Y.Z-android-arm64.apk` | Your Android phone (8.0+) |

### Install on Linux

```bash
tar xzf oraculo-v*.tar.gz
sudo install -m 755 oraculo-host oraculo-desktop oraculo-bridge oraculo-cli /usr/local/bin/
```

Screen streaming requires a Wayland session with PipeWire, GStreamer (with PipeWire plugins), and an H.264 encoder. On Fedora:

```bash
sudo dnf install gstreamer1-plugins-base pipewire-gstreamer gstreamer1-plugin-openh264
```

If you have an NVIDIA GPU, `nvh264enc` is used automatically when available (faster, lower CPU usage). Otherwise it falls back to `openh264enc`.

### Install on Android

Transfer the `.apk` to your phone and install it. You may need to allow "Install from unknown sources" in your phone's settings.

## How to use

### 1. Start Oráculo on your computer

Using the desktop companion GUI (recommended):

```bash
oraculo-desktop                       # open the companion GUI (embeds host or attaches to running daemon)
oraculo-desktop --background          # start silently in the background without opening a window
oraculo-desktop --quit                # stop the running Oráculo Desktop instance and service
```

Closing the `oraculo-desktop` window frees UI memory while keeping the background service active; launching `oraculo-desktop` again reopens the window.

Or run the headless daemon directly in a terminal:

```bash
oraculo-host serve --pair
```

A QR code and 8-character quick code appear in the terminal, valid for 3 minutes. Press `p` + Enter to generate a new one at any time.

### 2. Pair your phone

Open the Oráculo app, tap **Pair device** and scan the QR code — or tap **Enter pairing code** / **Code** (or **Pair with code** in the `☰` menu) to type the 8-character quick code (e.g. `K7P4-M9X2`).

Your phone is now connected. Use the tabs to switch between Touchpad, Terminal, Commands, and Files.

### 3. Manage devices, permissions, and Wake-on-LAN

```bash
oraculo-host peers                    # list paired devices
oraculo-host revoke <ENDPOINT_ID>     # remove a device
oraculo-host permissions              # show current settings in settings.toml
oraculo-host permissions \
  --screen true --input true --terminal true \
  --files-read true --files-write false \
  --commands-run true --commands-edit false \
  --rdp-tunnel true --rdp-port 3389 \
  --wol-enabled true --wol-mac "" --wol-broadcast ""
oraculo-host info                     # show host EndpointId and config paths
oraculo-host monitors                 # pick monitors for multi-monitor streaming
oraculo-host wol --enable             # enable Wake-on-LAN / Magic Packet on NIC
```

Pass `""` to `--wol-mac` or `--wol-broadcast` to restore automatic detection.

## CLI client (`oraculo-cli`)

There is also a terminal client for pairing, running quick commands, waking a host, and opening remote shells from another computer:

```bash
oraculo-cli pair "<CODE_OR_PAIRING_URI>" --name "my-laptop"
oraculo-cli ping
oraculo-cli commands
oraculo-cli run <COMMAND_ID>
oraculo-cli shell
oraculo-cli wake                      # wake saved default computer on LAN
oraculo-cli wake --bridge <BRIDGE_ID> # wake via a paired P2P WoL bridge
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9
```

- `ping`, `commands`, `run`, and `shell` accept `--host <ENDPOINT_ID>` to target a specific paired host.
- `pair` accepts an 8-character short code or an `oraculo://pair?...` URI (plus `--name`).
- `wake` uses the default paired host's synced MAC when `--mac` is omitted, and accepts `--mac`, `--target`, `--port` (default `9`), and `--bridge <BRIDGE_ENDPOINT_ID>`.

## Standalone Wake-on-LAN bridge (`oraculo-bridge`)

To wake your computer from outside your home network, you can run `oraculo-bridge` on an always-on Linux box, Raspberry Pi, or Termux. It is compiled strictly for relaying Wake-on-LAN with no screen, input, terminal, or file-access code:

```bash
oraculo-bridge                                   # first run prints QR code and short code
oraculo-bridge serve --pair --name "Home-Pi"     # custom label + fresh pairing code
oraculo-bridge peers                             # list authorized phones
oraculo-bridge revoke <ENDPOINT_ID>              # revoke a phone
oraculo-bridge info                              # show bridge EndpointId and data paths
```

You can also use a spare Android phone (**Wake-on-LAN Bridge Mode**) or an ESP32 / M5Stack Cardputer board. See the [Complete Wake-on-LAN Guide](docs/wake-on-lan.md).

## Security

- **End-to-end encryption & relay architecture:** connections use end-to-end encrypted, mutually authenticated QUIC (TLS 1.3) with Ed25519 keys via `iroh`. Peers connect directly whenever possible; when NAT or firewalls block direct P2P traversal, `iroh` falls back to public `presets::N0` relays to forward encrypted packets. Relays hold no decryption keys and cannot read plaintext traffic or authenticate as a trusted peer. The optional ESP32 / Cardputer Wake-on-LAN bridge uses MQTT over TLS (`8883`, defaulting to `broker.emqx.io` or a private broker) with single-use Challenge-Response nonces and `HMAC-SHA256` encrypted payloads so the broker never sees target MAC addresses or plaintext commands.
- **Pairing & instant revocation:** pairing tokens are single-use, expire in 3 minutes, and lock out after 5 failed attempts. Revoking a device or turning off any permission terminates active streams immediately.
- **Biometric or PIN app lock (Android):** optional lock in the `☰` Settings menu requires device unlock before opening or waking a saved computer and when returning to an active session.
- **File sandbox:** file access is confined to your home directory (`~`) with symlink and path-traversal protection. Sensitive paths (`.ssh`, `.gnupg`, keyrings, shell startup configs, `systemd/user`, `autostart`, `environment.d`, and Oráculo's own config/data directories) are always blocked. Private keys and tokens are stored with Unix mode `0600`.
- **Local RDP firewall hardening:** when GNOME Remote Login is enabled for unlocking via Oráculo's RDP tunnel, `oraculo-desktop` flags if port `3389` is reachable from the LAN and offers a **Block on LAN** button — or run `sudo ./packaging/linux/harden-rdp-firewall.sh [PORT]` to reject direct LAN TCP access across active `firewalld` zones while keeping `127.0.0.1` tunnel access working.

See [`.github/SECURITY.md`](.github/SECURITY.md) for vulnerability reporting.

## Building from source

If you want to build it yourself instead of using the release binaries, you need [Podman](https://podman.io/). The build scripts manage the pinned Rust toolchain, Android SDK, and NDK inside containers.

```bash
git clone https://github.com/marcos-ce/Oraculo-Desk.git
cd Oraculo-Desk

./scripts/build.sh release            # host, desktop, bridge, cli → ./dist/
./scripts/build-android.sh debug      # Android APK → ./dist/
./scripts/build.sh test               # run workspace tests
```

## Platform support

| Component | Linux | Windows | macOS | Android | iOS |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Terminal, commands, files, WoL | ✅ | ✅ (untested on Windows) | experimental | ✅ (client) | planned |
| Desktop companion GUI (`oraculo-desktop`) | ✅ (GTK4/GTK3 WebKit) | ✅ (Edge WebView2, untested) | — | — | — |
| Screen streaming + remote input | ✅ (Wayland) | stub (planned) | stub | ✅ (client) | planned |

### Beta v0.1 known limitations

- **Locked screen and initial login on Wayland (GDM):** inside an already unlocked session, screen streaming and remote mouse and keyboard work directly inside the app. After a cold boot at the GDM login screen, or when waking from sleep behind a password lock, Wayland blocks new `xdg-desktop-portal` sessions. Remote unlock in those states routes through Oráculo's encrypted tunnel on port `3389` into an external Android RDP app (`aFreeRDP` or `Windows App`). A built-in RDP renderer is left for v0.2.
- **Windows host:** terminal, commands, files, Wake-on-LAN, autostart, companion GUI, and firewall helpers already have Windows implementations in the tree, while screen capture (`screen.rs`) and remote input (`input.rs`) remain stubs. Completing and testing the Windows port comes after this Beta.
- **iPhone (iOS):** not part of Beta v0.1. The Rust core uses UniFFI and can target Swift later, but the SwiftUI client and macOS/Xcode build remain for a future milestone.

## License

[Apache License 2.0](licenses/LICENSE). Third-party notices in [`licenses/NOTICE`](licenses/NOTICE).
