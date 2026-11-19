# Platform support & known limitations

*Leia em [Português (Brasil)](compatibility.pt-BR.md).*

Oráculo is an active **beta**. The primary tested setup is **GNOME (Wayland / Fedora)** + **Android (`arm64-v8a`)**.

| Capability | Linux | Windows | macOS | Android | iOS |
| :--- | :---: | :---: | :---: | :---: | :---: |
| Desktop companion app (`oraculo-desktop`) | ✅ | in progress | — | — | — |
| Terminal, commands, files, Wake-on-LAN | ✅ | in progress | experimental | ✅ (client) | planned |
| Screen streaming & remote input | ✅ (Wayland) | next update | planned | ✅ (client) | planned |

## Linux desktop environments

Oráculo uses standard **XDG Desktop Portal** (`ScreenCast` + `RemoteDesktop`) and **PipeWire** for screen streaming and remote input.

- **GNOME on Wayland (Fedora):** primary development and test target.
- **Other Wayland desktops** implementing both portals (such as **KDE Plasma 6**) should also work, but have had limited real-world testing.
- **X11 sessions** (such as Cinnamon or XFCE on X11) support the desktop app, terminal, commands, files, and Wake-on-LAN — **without** screen streaming or remote input.

## Locked screen / GDM login on Wayland

Inside an unlocked desktop session, screen streaming and input run natively inside the Android app. At the GDM sign-in screen or when locked behind a password, Wayland restricts portal capture. Oráculo unlocks it through its encrypted local RDP tunnel paired with an Android RDP client (`aFreeRDP` or `Windows App`). A built-in RDP viewer is not available yet.

## Other limitations

- Desktop builds: Linux `x86_64` only. Android builds: `arm64-v8a`, Android 8.0+.
- The ESP32 / Cardputer bridge firmware has had limited real-world testing.
- Waking a computer from outside your home network requires a bridge ([guide](wake-on-lan.md)).

## Roadmap

- **Windows support (next priority):** native screen capture, remote input, packaging, and end-to-end testing.
- **Broader Linux compatibility:** KDE Plasma, wlroots/COSMIC compositors, optional X11 fallback.
- **Later:** built-in RDP unlock viewer on Android and an iOS client sharing the same Rust core.

Bug reports and compatibility feedback via [Issues](https://github.com/marcos-ce/Oraculo-Desk/issues) are very welcome.
