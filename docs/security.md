# Security model

*Leia em [Português (Brasil)](security.pt-BR.md).*

- **End-to-end encryption:** all connections use mutually authenticated QUIC (TLS 1.3) with Ed25519 keys via `iroh`. Peers connect directly whenever possible; when NAT or firewalls block direct paths, `iroh` routes encrypted packets through public `presets::N0` relays that never hold decryption keys. The optional ESP32 bridge uses MQTT over TLS (`8883`) with single-use challenge-response nonces and `HMAC-SHA256` encrypted payloads.
- **Short-lived pairing & instant revocation:** pairing codes are single-use, expire in 3 minutes, and burn after 5 failed attempts. Turning off any permission or removing a device closes active sessions immediately.
- **App lock (Android):** optional biometric or PIN authentication before opening or waking a saved computer and when returning to an active session.
- **File sandbox:** file browsing and transfers are strictly confined to your home directory (`~`) with traversal and symlink protection, blocking `.ssh`, `.gnupg`, keyrings, shell startup files, `systemd/user`, `autostart`, `environment.d`, and Oráculo's own config/data directories. Private keys and tokens are stored with Unix mode `0600`.
- **RDP firewall isolation:** when system Remote Login is enabled for lock-screen unlock, **Oráculo Desktop** warns if port `3389` is exposed on the LAN and provides a **Block** button (backed by [`packaging/linux/harden-rdp-firewall.sh`](../packaging/linux/harden-rdp-firewall.sh)) so only the encrypted `127.0.0.1` tunnel can reach it.

Vulnerability reports are handled privately via [`.github/SECURITY.md`](../.github/SECURITY.md).
