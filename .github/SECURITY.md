# Security Policy

## Supported Versions

Security fixes target the active `main` branch and the latest release of **Oráculo**.

| Version | Supported |
| :--- | :--- |
| `v0.2.x` / `main` | Yes |
| `v0.1.x` | No |

## Security Architecture Summary

Oráculo is designed to minimize attack surface without central accounts or exposed router ports:

- **End-to-End Encrypted Transport:** All network traffic uses `iroh` QUIC (TLS 1.3) with `Ed25519` mutual peer authentication (`ALPN: oraculo/1`). When direct NAT traversal is blocked, public `presets::N0` relays forward encrypted packets without holding decryption keys. Unknown peers are disconnected immediately on non-pairing streams.
- **Short-Lived Pairing:** Pairing tokens expire after **180 seconds**, are single-use, automatically burn after **5 failed attempts**, and verify secrets using constant-time comparison.
- **Least-Privilege Permissions & Instant Revocation:** Subsystems (`allow_screen`, `allow_input`, `allow_terminal`, `allow_files_read`, `allow_files_write`, `allow_commands_run`, `allow_commands_edit`, `allow_rdp_tunnel`, `wol_enabled`) can be toggled independently in the **Oráculo Desktop** app or `settings.toml`. Turning off a permission or revoking a peer terminates active streams immediately.
- **Biometric / PIN App Lock (Android):** Optional lock in the Android app requires device authentication before opening or waking a saved computer and when returning to an active session.
- **Home Directory Sandbox:** Remote file operations resolve canonical paths strictly under the user's home directory (`~`), block `..` traversal and symlink escapes, cap uploads at `2 GiB`, and deny access to sensitive configuration, credential, systemd, autostart, and shell startup paths.
- **Restricted File Permissions:** Private keys, peer lists, and Wayland portal tokens (`host.key`, `bridge.key`, `trusted_peers.json`, `portal_tokens.json`, `remote_desktop_token`, `screencast_token`, `client.key`) are written atomically with Unix mode `0600`.
- **Local RDP Tunnel & Firewall Isolation:** Remote unlock tunnels strictly to loopback RDP (`127.0.0.1` / `[::1]`). `oraculo-desktop` detects LAN exposure on the RDP port and provides one-click firewall isolation (`packaging/linux/harden-rdp-firewall.sh`).
- **Wake-on-LAN Bridges & ESP32 Firmware:** `oraculo-bridge` compiles only WoL relay logic with no screen/input/terminal/file code. The optional ESP32 / Cardputer bridge uses per-session WPA2 passwords for its setup SoftAP, never exposes secrets in its local setup HTML, and authenticates remote wake commands over MQTT/TLS with single-use challenge-response nonces and `HMAC-SHA256` encrypted payloads.

## Reporting a Vulnerability

Please **do not** open public GitHub issues for security vulnerabilities or suspected sandbox/auth bypasses.

Instead:
1. Open a **Private Security Advisory** on GitHub (`Security` -> `Advisories` -> `Report a vulnerability`) for [`marcos-ce/Oraculo-Desk`](https://github.com/marcos-ce/Oraculo-Desk/security/advisories/new).
2. Include the affected crate/component, reproduction steps, environment details (OS, Wayland compositor / Android version / board model), and any suggested mitigation.

Valid reports will be acknowledged and prioritized as soon as possible.
