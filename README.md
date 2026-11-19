# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

*Leia em [Português (Brasil)](README.pt-BR.md).*

**Oráculo** is a desktop and Android remote-control app that connects your phone to your Linux computer over end-to-end encrypted QUIC (`iroh`) — no accounts, no cloud logins, and no router port forwarding.

## Features

- **Live screen streaming & remote control:** hardware-decoded H.264 video with multi-monitor switching, touchpad mode, direct touch, and pinch-to-zoom.
- **Persistent multi-tab terminal:** interactive shell sessions that stay alive across reconnects.
- **Quick commands:** run and edit one-tap shortcuts synced between desktop and phone.
- **Home-confined file manager:** browse, preview, download, and upload files strictly inside your home directory (`~`).
- **Encrypted Remote Login unlock:** unlock GDM / lock screens over a local encrypted RDP tunnel (`127.0.0.1:3389`) with built-in LAN firewall checks.
- **Wake-on-LAN:** power on your computer on your local network, or from anywhere using a bridge (`oraculo-bridge`, a spare Android device, or an ESP32 / M5Stack Cardputer).
- **Native Material Design 3 apps:** GTK4/libadwaita + WebKitGTK desktop companion on Linux and Jetpack Compose app on Android, both with Light/Dark themes and PT-BR/EN localization.

## Download & Install

Download the latest release assets from the [Releases](https://github.com/marcos-ce/Oraculo-Desk/releases) page:

| Platform | File | Requirements |
| :--- | :--- | :--- |
| **Linux (x86_64)** | `oraculo-v*-linux-x86_64.tar.gz` | Wayland + PipeWire, WebKitGTK (`webkitgtk6.0` or `webkit2gtk-4.1`), GStreamer H.264 |
| **Android** | `oraculo-v*-android-arm64.apk` | Android 8.0+ (`arm64-v8a`) |
| **Checksums** | `SHA256SUMS-*.txt` | Optional integrity check (`sha256sum -c SHA256SUMS-*.txt`) |

### Linux Desktop

Extract the archive and install the app and launcher icon for your user:

```bash
tar xzf oraculo-v*-linux-x86_64.tar.gz
install -Dm 755 oraculo-desktop oraculo-host oraculo-bridge oraculo-cli -t ~/.local/bin/
install -Dm 644 com.oraculo.Desktop.desktop -t ~/.local/share/applications/
install -Dm 644 com.oraculo.Desktop.svg -t ~/.local/share/icons/hicolor/scalable/apps/
```

For screen streaming on Fedora (Wayland + PipeWire):

```bash
sudo dnf install gstreamer1-plugins-base pipewire-gstreamer gstreamer1-plugin-openh264
```

NVIDIA GPUs automatically use `nvh264enc` when available; otherwise Oráculo falls back to `openh264enc`.

### Android

Transfer the `.apk` to your phone and install it (allow installing from unknown sources if prompted).

## Getting Started

1. **Open Oráculo on your computer:** launch **Oráculo** from your application menu (or run `oraculo-desktop`). A single-use QR Code and 8-character pairing code appear on the **Home** tab.
2. **Pair your phone:** open the **Oráculo** Android app, tap **Pair device**, and scan the QR Code — or tap **Enter pairing code** to type the 8-character code (e.g. `K7P4-M9X2`).
3. **Control & configure from the apps:**
   - **Desktop app tabs:**
     - **Home:** pair new phones and view, disconnect, or remove authorized devices.
     - **Access:** toggle live permissions (screen, mouse/keyboard, terminal, RDP login, files, commands), authorize Wayland monitors once for unattended access, isolate the RDP firewall port, and enable Wake-on-LAN.
     - **Commands:** create, edit, duplicate, or delete quick command shortcuts.
     - **System:** toggle autostart, switch theme (`Auto` / `Light` / `Dark`) and language (`PT` / `EN`), or quit the background service completely.
   - **Android app tabs:** switch between **Touchpad** (with live screen streaming), **Terminal**, **Commands**, and **Files**, or enable optional **Biometric / PIN lock** in the `☰` menu.

Closing the desktop window frees UI memory while keeping the background service active; reopening **Oráculo** brings the window back.

## Wake-on-LAN

On the same Wi-Fi/LAN, the Android app sends Wake-on-LAN Magic Packets directly. To wake your computer from outside your home network, pair one of three bridge options:

- **Dedicated Linux / Raspberry Pi / Termux bridge** (`oraculo-bridge`)
- **Spare Android phone** (**Wake-on-LAN Bridge Mode** in the app's `☰` menu)
- **ESP32 / M5Stack Cardputer** ([`firmware/oraculo-wol-esp32`](firmware/oraculo-wol-esp32/README.md))

See the [Wake-on-LAN Guide](docs/wake-on-lan.md) for setup details.

## Security

- **End-to-end encryption:** all connections use mutually authenticated QUIC (TLS 1.3) with Ed25519 keys via `iroh`. Peers connect directly whenever possible; when NAT or firewalls block direct paths, `iroh` routes encrypted packets through public `presets::N0` relays that never hold decryption keys. The optional ESP32 bridge uses MQTT over TLS (`8883`) with single-use challenge-response nonces and `HMAC-SHA256` encrypted payloads.
- **Short-lived pairing & instant revocation:** pairing codes are single-use, expire in 3 minutes, and burn after 5 failed attempts. Turning off any permission or removing a device closes active sessions immediately.
- **App lock (Android):** optional biometric or PIN authentication before opening or waking a saved computer and when returning to an active session.
- **File sandbox:** file browsing and transfers are strictly confined to your home directory (`~`) with traversal and symlink protection, blocking `.ssh`, `.gnupg`, keyrings, shell startup files, `systemd/user`, `autostart`, `environment.d`, and Oráculo's own config/data directories. Private keys and tokens are stored with Unix mode `0600`.
- **RDP firewall isolation:** when system Remote Login is enabled for lock-screen unlock, **Oráculo Desktop** warns if port `3389` is exposed on the LAN and provides a **Block** button (backed by [`packaging/linux/harden-rdp-firewall.sh`](packaging/linux/harden-rdp-firewall.sh)) so only the encrypted `127.0.0.1` tunnel can reach it.

Vulnerability reports are handled privately via [`.github/SECURITY.md`](.github/SECURITY.md).

## Platform Support

| Capability | Linux | Windows | macOS | Android | iOS |
| :--- | :---: | :---: | :---: | :---: | :---: |
| Desktop companion app (`oraculo-desktop`) | ✅ | experimental | — | — | — |
| Terminal, commands, files, Wake-on-LAN | ✅ | experimental | experimental | ✅ (client) | planned |
| Screen streaming & remote input | ✅ (Wayland) | planned | planned | ✅ (client) | planned |

- **Locked screen / initial GDM login on Wayland:** inside an unlocked desktop session, screen streaming and input run natively inside the Android app. When the computer is at the GDM sign-in screen or locked behind a password, Wayland restricts portal capture; Oráculo unlocks it through its encrypted local RDP tunnel paired with an Android RDP client (`aFreeRDP` or `Windows App`).

<details>
<summary><strong>Advanced usage: Headless host, CLI client, and standalone WoL bridge</strong></summary>

All desktop features can also be operated without a GUI for headless Linux machines, scripts, or SSH-free administration.

### Desktop app CLI flags
```bash
oraculo-desktop              # open the GUI (embeds host or attaches to running daemon)
oraculo-desktop --background # start silently in the background
oraculo-desktop --quit       # stop the GUI and background service completely
```

### Headless host daemon (`oraculo-host`)
```bash
oraculo-host serve --pair             # start daemon and print QR + 8-character code (press 'p' for a new code)
oraculo-host peers                    # list paired devices
oraculo-host revoke <ENDPOINT_ID>     # revoke a device immediately
oraculo-host permissions              # inspect current settings.toml permissions
oraculo-host permissions \
  --screen true --input true --terminal true \
  --files-read true --files-write false \
  --commands-run true --commands-edit false \
  --rdp-tunnel true --rdp-port 3389 \
  --wol-enabled true --wol-mac "" --wol-broadcast ""
oraculo-host info                     # print EndpointId and config paths
oraculo-host monitors                 # choose monitors for multi-monitor streaming
oraculo-host wol --enable             # enable Wake-on-LAN / Magic Packet on physical NICs
```

### Terminal client (`oraculo-cli`)
```bash
oraculo-cli pair "<CODE_OR_PAIRING_URI>" --name "laptop"
oraculo-cli ping                      # accepts --host <ENDPOINT_ID>
oraculo-cli commands                  # accepts --host <ENDPOINT_ID>
oraculo-cli run <COMMAND_ID>          # accepts --host <ENDPOINT_ID>
oraculo-cli shell                     # accepts --host <ENDPOINT_ID>
oraculo-cli wake                      # wake default saved computer on LAN
oraculo-cli wake --bridge <BRIDGE_ID> # wake via a paired WoL bridge
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9
```

### Standalone Wake-on-LAN bridge (`oraculo-bridge`)
```bash
oraculo-bridge                                   # first run prints QR code and short code
oraculo-bridge serve --pair --name "Home-Pi"     # custom label + fresh pairing code
oraculo-bridge peers                             # list authorized phones
oraculo-bridge revoke <ENDPOINT_ID>              # revoke a phone
oraculo-bridge info                              # show bridge EndpointId and data paths
```

</details>

## Building from Source

Builds run inside reproducible [Podman](https://podman.io/) containers with the pinned Rust toolchain, Android SDK 35, and NDK:

```bash
git clone https://github.com/marcos-ce/Oraculo-Desk.git
cd Oraculo-Desk

./scripts/build.sh test          # run workspace tests
./scripts/build.sh release       # build host, desktop, bridge, cli → ./dist/
./scripts/build-android.sh debug # build Android APK → ./dist/
```

## License

Licensed under the [Apache License 2.0](LICENSE). Third-party notices and embedded font licenses are listed in [`NOTICE`](NOTICE).
