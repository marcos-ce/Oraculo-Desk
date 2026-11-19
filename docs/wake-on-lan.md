# Complete Wake-on-LAN (WoL) Guide for Oráculo

*Leia em [Português (Brasil)](wake-on-lan.pt-BR.md).*

**Wake-on-LAN (WoL)** lets you power on a shut-down or suspended computer remotely from the Oráculo Android app or `oraculo-cli`.

This guide explains how WoL works internally in Oráculo, how to configure **your OS, NIC drivers, BIOS/UEFI, and router**, how **Network bridges** wake your computer from outside your home network, the **real security model**, and step-by-step troubleshooting.

---

## Table of Contents

1. [How it works in Oráculo](#1-how-it-works-in-oráculo)
2. [The 4 wake paths (LAN and remote)](#2-the-4-wake-paths-lan-and-remote)
3. [Checking and enabling WoL on your computer](#3-checking-and-enabling-wol-on-your-computer)
4. [Preparing the OS, network, and BIOS/UEFI](#4-preparing-the-os-network-and-biosuefi)
5. [Sleep vs. Power-off and Ethernet vs. Wi-Fi (WoWLAN)](#5-sleep-vs-power-off-and-ethernet-vs-wi-fi-wowlan)
6. [What happens after wake (autostart and login screen)](#6-what-happens-after-wake-autostart-and-login-screen)
7. [Security: is it dangerous? Why is it disabled by default?](#7-security-is-it-dangerous-why-is-it-disabled-by-default)
8. [Troubleshooting](#8-troubleshooting)

---

## 1. How it works in Oráculo

Unlike live P2P QUIC sessions (which require a running host daemon), a powered-off or suspended machine runs no OS — only the network adapter stays powered in standby listening for a standard IEEE/AMD **Magic Packet**.

- **Packet layout:** `102`-byte UDP frame containing `6` `0xFF` bytes followed by the target MAC address repeated `16` times.
- **Ports & bursts:** Oráculo sends a `3`-packet burst spaced `40 ms` apart to both the configured UDP port (default `9` / Discard) and fallback port (`7` / Echo) to mitigate Wi-Fi DTIM packet loss.
- **Automatic MAC & subnet discovery:** once you connect to a running computer, the host syncs up to `6` physical MAC addresses (prioritizing active wired interfaces and including hardware permanent MACs `permaddr` when Wi-Fi MAC randomization is active) along with the local subnet broadcast address (e.g., `192.168.18.255`).
- **Manual override:** inside the Wake-on-LAN dialog on Android, you can edit the **MAC address** manually or expand **Advanced options (DDNS & port)** to target a custom external host/IP and UDP port.

---

## 2. The 4 wake paths (LAN and remote)

When you tap **Wake computer** in the Android app, Magic Packets are dispatched in parallel across all available paths:

### A. Direct home Wi-Fi broadcast (no bridge needed)
When your phone is on the same home Wi-Fi/LAN as your computer, Oráculo sends the UDP Magic Packet directly to the saved subnet broadcast (`192.168.x.255`) and `255.255.255.255`.

### B. Android Bridge (a spare phone/tablet at home)
Away from home (4G/5G or another network), UDP broadcasts cannot cross home routers or CGNAT. Any spare Android device kept on your home Wi-Fi can act as a bridge using the **same Oráculo APK**:
1. On the spare phone, open the **`☰`** menu → enable **Wake-on-LAN Bridge Mode**.
2. Tap **Code** on the active bridge banner (or **Generate bridge pairing code** in the **`☰`** menu) to generate an **8-character pairing code (`XXXX-XXXX`)** valid for 3 minutes. *(The Android bridge displays only the short pairing code — no QR Code.)*
3. On your main phone, tap **Code** / **Enter pairing code** (or **Pair with code** in the **`☰`** menu) and type that 8-character code. It will appear automatically under **Network bridges**.

### C. Lightweight PC / Raspberry Pi / Linux / Termux Bridge (`oraculo-bridge`)
Standalone binary ([`crates/oraculo-bridge/src/main.rs`](../crates/oraculo-bridge/src/main.rs)) compiled strictly for relaying WoL without any screen, terminal, input, or file-access code:

```bash
# First launch: automatically prints a UTF-8 QR code and short code
oraculo-bridge

# Or force a new pairing code / set a custom display name
oraculo-bridge serve --pair --name "Home-Pi"

# Manage authorized phones and inspect paths (~/.local/share/oraculo-bridge/)
oraculo-bridge peers
oraculo-bridge revoke <ENDPOINT_ID>
oraculo-bridge info
```

### D. Embedded ESP32 / M5Stack Cardputer Bridge (`oraculo_wol_esp32.ino`)
Single-file firmware in [`firmware/oraculo-wol-esp32/oraculo_wol_esp32.ino`](../firmware/oraculo-wol-esp32/oraculo_wol_esp32.ino) (~0.3 W powered off a router USB port):
- **Hardware support:** M5Stack Cardputer (display + on-screen QR Code + `Q`, `Enter`, `P`, `R` keys) and generic ESP32 / ESP32-C3 / ESP32-S2 / ESP32-S3 boards (WPA2-protected local Wi-Fi setup portal `Oraculo-Ponte-XXXX` at `http://192.168.4.1` with a per-session random 10-character password shown on screen and Serial + `115200 baud` Serial CLI). The HTTP portal never exposes the pairing secret — pair via the Cardputer QR Code or Serial `uri` command after saving Wi-Fi.
- **Transport:** strictly **MQTT over TLS (`8883`)** (`broker.emqx.io` default or custom private broker) with NTP-backed X.509 certificate verification.
- See [`firmware/oraculo-wol-esp32/README.md`](../firmware/oraculo-wol-esp32/README.md#english) for flashing and setup instructions.

> **Design rule:** bridges are household-wide rather than bound to a single computer. Once paired on your main phone, any bridge under **Network bridges** can wake all saved computers on that home LAN.

---

## 3. Checking and enabling WoL on your computer

### In Oráculo Desktop (GUI)
1. Open `oraculo-desktop` and select the **Access** tab.
2. Scroll to **Wake-on-LAN**:
   - **Allow Wake-on-LAN** toggles `wol_enabled` in `settings.toml` (enabled by default for paired devices).
   - **Network interfaces** lists detected Ethernet/Wi-Fi adapters and NetworkManager WoL profiles.
   - Click **Enable on network card** if any profile still needs Magic Packet enabled.

### Via CLI (`oraculo-host wol`)
```bash
# Inspect detected NICs, advertised MACs, broadcast, and NetworkManager profiles
oraculo-host wol

# Enable in settings.toml and configure wake-on-lan = magic in NetworkManager profiles
oraculo-host wol --enable

# Override MAC or broadcast manually (pass "" to restore auto-detection)
oraculo-host wol --mac AA:BB:CC:DD:EE:FF --broadcast 192.168.1.255

# Stop sharing WoL metadata with paired peers
oraculo-host wol --disable
```

### From another terminal (`oraculo-cli wake`)
```bash
# Wake the default saved computer using its synced MAC and subnet broadcast
oraculo-cli wake

# Wake over LAN and also relay via a paired P2P WoL bridge
oraculo-cli wake --bridge <BRIDGE_ENDPOINT_ID>

# Target an explicit MAC, broadcast/DDNS address, or UDP port (default: 9)
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9

# Wake a specific MAC through a paired P2P WoL bridge
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --bridge <BRIDGE_ENDPOINT_ID>
```

---

## 4. Preparing the OS, network, and BIOS/UEFI

### Linux (Fedora / Ubuntu / Arch with NetworkManager)
1. **Enable Magic Packet on your NetworkManager profile:**
   ```bash
   nmcli connection show
   nmcli connection modify "Wired connection 1" 802-3-ethernet.wake-on-lan magic
   nmcli device reapply eno1
   ```
   *(Replace `"Wired connection 1"` and `eno1` with your profile and interface names.)*
2. **Verify the live NIC driver state** (with Ethernet cable plugged in):
   ```bash
   nmcli -f 802-3-ethernet.wake-on-lan connection show "Wired connection 1"
   sudo ethtool eno1 | grep -i wake-on
   ```
   - `Wake-on: g` = Magic Packet active in hardware.
   - `Wake-on: d` = disabled in the driver.
3. **Check kernel PCI wakeup:**
   ```bash
   cat /sys/class/net/eno1/device/power/wakeup
   ```
   Must report `enabled`.

### BIOS / UEFI (critical on laptops and many desktops)
1. Enter UEFI setup on boot (`F2` / `Del`).
2. Under **Advanced** → **APM Configuration** / **Power Management**:
   - Set **`ErP Ready` / `ErP Support` = `Disabled`** (commonly enabled on laptops, cutting all standby power to Ethernet in S5).
   - Set **`Power On By PCI-E` / `Wake on LAN` / `Resume by Onboard LAN` = `Enabled`**.
3. On laptops, keep the **AC charger plugged in** when testing S5 power-off: most laptops cut Ethernet standby power on battery.
4. **LED test:** with the computer powered off, charger connected, and RJ-45 cable plugged into your router, confirm the **RJ-45 link LEDs stay lit or blinking**. If they turn completely dark, BIOS/ErP cut NIC power in S5.

### Windows
1. **Device Manager → Network adapters → Ethernet → Properties**:
   - **Power Management:** check *Allow this device to wake the computer* and *Only allow a magic packet to wake the computer*.
   - **Advanced:** enable *Wake on Magic Packet* (and *Shutdown Wake-On-Lan* if listed).
2. **Control Panel → Power Options → Choose what the power buttons do**: disable **Turn on fast startup**, which places Windows into hybrid S4 where many NICs ignore WoL.

---

## 5. Sleep vs. Power-off and Ethernet vs. Wi-Fi (WoWLAN)

| Machine power state | Wired Ethernet | Wi-Fi (WoWLAN) |
| :--- | :--- | :--- |
| **Sleep / Suspend** (`systemctl suspend`) | **Very reliable.** Usually works even before tuning S5 BIOS settings. | **Works on supported hardware** when `802-11-wireless.wake-on-wlan = magic` is enabled and router DTIM keeps association alive. |
| **Full Power-off** (ACPI S5) | **Requires BIOS + AC power:** `ErP Ready = Disabled`, `Wake-on: g` retained on shutdown, and standby RJ-45 LED activity. | **Rarely supported** on laptops/desktops because Wi-Fi radios lose power in S5. |

---

## 6. What happens after wake (autostart and login screen)

1. **Background host service at boot (`oraculo-host.service`):**
   - Enabling **Launch at startup** on Linux installs `~/.config/systemd/user/oraculo-host.service` (`WantedBy=default.target`) and enables **`loginctl enable-linger`**.
   - This starts `oraculo-host serve` automatically on boot **before anyone signs into GNOME**, without popping up a desktop window.
2. **Automatic reconnect polling:**
   - After sending Magic Packets, [`WakeOnLanDialog`](../android/app/src/main/java/com/oraculo/app/WakeOnLanDialog.kt) probes the host (`8s` per probe inside a `55s` window) and opens the control screen as soon as `oraculo-host` comes online.
3. **Unlocking past the boot login screen (GDM):**
   - Before signing into a GNOME Wayland session, `xdg-desktop-portal` is not running yet, so **Open screen** and **Touchpad** cannot capture or inject into GDM directly.
   - To unlock from cold boot, use **Unlock via remote login (RDP)** (or the **Remote login (RDP)** lock button in the top bar) in Oráculo, which opens an encrypted QUIC tunnel to `127.0.0.1:3389`. On Fedora/GNOME this requires enabling **Remote Login** (`gnome-remote-desktop.service`, system mode — separate from user *Desktop Sharing*) in **Settings → System → Remote Desktop**, and an Android RDP client (`aFreeRDP` or `Windows App`).
   - To keep port `3389` unreachable from the LAN while still accessible over the encrypted tunnel, click **Block on LAN** in `oraculo-desktop` or run `sudo ./packaging/linux/harden-rdp-firewall.sh 3389`.

---

## 7. Security: is it dangerous? Why is it disabled by default?

### Is it dangerous?
- **WoL does not grant access:** a Magic Packet only acts like pressing the physical power button. It cannot bypass OS passwords, disk encryption (LUKS/BitLocker), files, or terminal access.
- **Practical risk:** anyone **already on your local network** (or on the internet if you dangerously forward UDP port `9` on your router) who knows your MAC address can power the machine on — potentially wasting power or overheating a laptop if left closed inside a sleeve/bag on AC power.
- **Why Oráculo avoids router port forwarding:** using an authenticated P2P/TLS **Network bridge** inside your home keeps UDP port `9` completely closed to the public internet.

### Security controls built into Oráculo
- **Explicit sharing:** MAC addresses are only shared with paired peers when `wol_enabled = true`.
- **Anti-SSRF & anti-DNS-beacon filtering ([`crates/oraculo-proto/src/wol.rs`](../crates/oraculo-proto/src/wol.rs)):** `sanitize_wol_info` and `parse_safe_lan_broadcast_v4` only store private/CGNAT/link-local/broadcast IPv4 literals — untrusted public IPs or domain names are stripped.
- **Isolated `oraculo-bridge`:** contains zero screen, input, terminal, or file code and keeps private keys in `~/.local/share/oraculo-bridge/` with mode `0600`.
- **End-to-end hardened MCU bridges (`Cardputer / ESP32`):**
  - Strictly **MQTT over TLS (`8883`)**, never plaintext `1883`.
  - Per-device `256`-bit secret generated from hardware TRNG (`esp_fill_random`) and stored in NVS.
  - **WPA2 setup portal without secret exposure:** `Oraculo-Ponte-XXXX` generates a random 10-character WPA2 password per session (displayed on screen and Serial) and never prints the pairing secret or MQTT credentials on HTTP pages.
  - **Single-use `nonce` (`25s` TTL) Challenge-Response**, `HMAC-SHA256` encrypted payload stream (target MACs never travel in plaintext through the broker), and authenticated `ACK`.

### Why is it disabled by default on many computers?
1. **ErP / EuP standby regulations:** require powered-off machines to draw under `0.5 W`–`1 W`, prompting manufacturers (especially laptop vendors) to enable `ErP Ready` in BIOS and cut Ethernet power on shutdown.
2. **Battery drain on laptops:** keeping NIC circuitry powered on battery reduces unplugged standby life.
3. **Preventing accidental wakes:** legacy wake flags triggered on arbitrary network traffic; modern drivers default to off until **Magic Packet (`g`)** is explicitly configured.

---

## 8. Troubleshooting

### 1. Wakes from Sleep (`systemctl suspend`) but not from Power-off (`poweroff`)
- Keep the AC adapter plugged in and the Ethernet cable seated before shutting down.
- Check the **RJ-45 LEDs** while powered off: if completely dark, enter BIOS and set `ErP Ready = Disabled` and `Wake on LAN / Power On By PCI-E = Enabled`.
- Verify `sudo ethtool eno1 | grep -i Wake-on` shows `Wake-on: g`.

### 2. Cable LEDs stay lit while powered off, but phone on Wi-Fi fails to wake the computer
- Check if your router isolates Wi-Fi clients from wired LAN ports (**AP / Client Isolation**) or drops `255.255.255.255` broadcasts from Wi-Fi.
- Ensure the saved MAC in Android is your **wired Ethernet MAC**, not a randomized Wi-Fi MAC, or place a wired/main-LAN **`oraculo-bridge`** / **ESP32** on the same switch.

### 3. Computer powered on, but Android reports *"Signal sent, but computer hasn't responded yet"*
- Boot + disk unlock + `network-online.target` may exceed `55s`.
- Verify the service runs on boot without login:
  ```bash
  systemctl --user status oraculo-host.service --no-pager
  loginctl show-user "$USER" -p Linger
  ```
  Both `Active: active (running)` and `Linger=yes` should be present.

### 4. MCU Bridge (`Cardputer / ESP32`) shows TLS errors or never responds
- Ensure the home Wi-Fi network allows outbound NTP (`UDP 123`) and MQTT TLS (`TCP 8883`); TLS validation fails without valid NTP time sync.
- Press **`Q`** on Cardputer to check Wi-Fi/NTP/TLS status, or hold **BOOT** for `3s` (or press **`P`**) to reopen `Oraculo-Ponte-XXXX` at `http://192.168.4.1` using the per-session WPA2 password shown on the display / Serial Monitor (`115200 baud`).

### 5. Rotating MCU credentials (`R` key) broke pairing
- Pressing **`R`** on Cardputer rotates `bridge_id` and `secret`, immediately invalidating previous QR codes. Delete the old bridge entry in Android and re-pair with the new QR code (or Serial `uri`).
