# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](licenses/LICENSE)

*Read this in [Português (Brasil)](README.pt-BR.md).*

Remote desktop, terminal, and file manager that connects your Android phone to your Linux desktop over encrypted P2P. No accounts, no port forwarding, no relay servers.

## What it does

- Stream your desktop screen to your phone (H.264, hardware decoded)
- Control mouse and keyboard remotely
- Open terminal sessions from your phone
- Run pre-configured commands with one tap
- Browse, download, and upload files (confined to your home directory)

The Android app has Material 3 UI with PT/EN toggle, dark/light themes, pinch-to-zoom, and four tabs: Touchpad, Terminal, Commands, and Files.

## Download

Go to the [Releases](https://github.com/marcos-ce/Oraculo-Desk/releases/latest) page and download both:

| | File | For |
| :--- | :--- | :--- |
| 🖥️ | `oraculo-vX.Y.Z-linux-x86_64.tar.gz` | Your Linux computer |
| 📱 | `oraculo-vX.Y.Z-android-arm64.apk` | Your Android phone (8.0+) |

### Install on Linux

```bash
tar xzf oraculo-v*.tar.gz
sudo install -m 755 oraculo-host oraculo-cli /usr/local/bin/
```

Screen streaming requires a Wayland session with PipeWire, GStreamer (with PipeWire plugins), and an H.264 encoder. On Fedora:

```bash
sudo dnf install gstreamer1-plugins-base pipewire-gstreamer gstreamer1-plugin-openh264
```

If you have an NVIDIA GPU, `nvh264enc` is used automatically when available (faster, lower CPU usage). Otherwise it falls back to `openh264enc`.

### Install on Android

Transfer the `.apk` to your phone and install it. You may need to allow "Install from unknown sources" in your phone's settings.

## How to use

### 1. Start the host on your computer

```bash
oraculo-host serve --pair
```

A QR code appears in the terminal, valid for 3 minutes. Press `p` + Enter to generate a new one at any time.

### 2. Pair your phone

Open the Oráculo app, tap "Pair new computer" and scan the QR code. You can also paste the `oraculo://pair?...` link manually.

That's it. Your phone is now connected. Use the tabs to switch between touchpad, terminal, commands, and file browser.

### 3. Manage devices and permissions

```bash
oraculo-host peers                    # list paired devices
oraculo-host revoke <ID>              # remove a device
oraculo-host permissions --screen true --input true --terminal true
oraculo-host permissions --files-read true --files-write false
oraculo-host info                     # show host ID and config paths
oraculo-host monitors                 # pick monitors for multi-monitor streaming
```

## CLI client

There's also a terminal client for pairing and remote shells from another computer:

```bash
oraculo-cli pair "<PAIRING_URI>" --name "my-laptop"
oraculo-cli ping
oraculo-cli shell
```

All commands accept `--host <ID>` to target a specific host.

## Security

Connections are encrypted end-to-end with QUIC (TLS 1.3) and Ed25519 keys. No data passes through third-party servers.

Pairing tokens are single-use, expire in 3 minutes, and lock out after 5 wrong attempts. Revoking a device or changing permissions kills active streams immediately.

File access is confined to your home directory with symlink and traversal protection. Sensitive directories (`.ssh`, `.gnupg`, keyrings, shell configs) are always blocked. Key material is stored with mode 0600.

See [`.github/SECURITY.md`](.github/SECURITY.md) for vulnerability reporting.

## Building from source

If you want to build it yourself instead of using the release binaries, you need [Podman](https://podman.io/). The build scripts handle the Rust toolchain, Android SDK, and NDK inside containers.

```bash
git clone https://github.com/marcos-ce/Oraculo-Desk.git
cd Oraculo-Desk

./scripts/build.sh release            # desktop binaries → ./dist/
./scripts/build-android.sh debug      # Android APK → ./dist/
./scripts/build.sh test               # run unit tests
```

## Platform support

| Component | Linux | macOS / Windows | Android |
| :--- | :--- | :--- | :--- |
| Terminal, commands, files | ✅ | ✅ | ✅ (client) |
| Screen streaming + remote input | ✅ (Wayland) | planned | ✅ (client) |

## License

[Apache License 2.0](licenses/LICENSE). Third-party notices in [`licenses/NOTICE`](licenses/NOTICE).
