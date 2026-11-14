# Security Policy

## Supported Versions

Security fixes target the active `main` branch and latest tagged release of **Oráculo**.

| Version | Supported |
| :--- | :--- |
| `v0.1.x` / `main` | Yes |

## Security Architecture Summary

Oráculo is designed to minimize attack surface without central accounts:

- **End-to-End Encrypted Transport:** All network traffic uses `iroh` QUIC (TLS 1.3) with `Ed25519` peer authentication (`ALPN: oraculo/1`). Unknown peers are rejected on every non-pairing stream.
- **Short-Lived Pairing:** Pairing tokens expire after **180 seconds**, are single-use, automatically invalidate after **5 failed attempts**, and verify secrets using constant-time comparison.
- **Least-Privilege Permissions & Revocation:** Subsystems (`terminal`, `files_read`, `files_write`, `input`, `screen`, `commands_run`, `commands_edit`) can be toggled independently in `settings.toml` (`oraculo-host permissions`). Revoking a peer (`oraculo-host revoke <ENDPOINT_ID>`) closes its active sessions immediately.
- **Home Directory Sandbox:** Remote file operations resolve canonical paths under the user's home directory, block `..` traversal and symlink escapes, cap uploads at `2 GiB`, and exclude sensitive configuration, credential, and shell startup directories/files.
- **Restricted File Permissions:** Host/client key material (`host.key`, `trusted_peers.json`, `remote_desktop_token`, `screencast_token`, `client.key`) is stored atomically with Unix mode `0600`.

## Reporting a Vulnerability

Please **do not** open public GitHub issues for security vulnerabilities or suspected sandbox/auth bypasses.

Instead:
1. Open a **Private Security Advisory** on GitHub (`Security` -> `Advisories` -> `Report a vulnerability`) for [`marcos-ce/Oraculo-Desk`](https://github.com/marcos-ce/Oraculo-Desk/security/advisories/new).
2. Include the affected crate/component, reproduction steps, environment details (OS, Wayland compositor / Android version), and any suggested mitigation.

Valid reports will be acknowledged and prioritized as soon as possible.
