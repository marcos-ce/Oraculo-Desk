use crate::config::HostSettings;
use oraculo_proto::WolInfo;
use oraculo_proto::wol::{MAX_WOL_MACS, normalize_wol_mac, parse_safe_lan_broadcast_v4};

/// Interface de rede física detectada no host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedNic {
    pub name: String,
    pub mac: String,
    /// MAC permanente de fábrica (`permaddr`) quando diferente do MAC ativo clonado/randomizado.
    pub perm_mac: Option<String>,
    pub broadcast_v4: Option<String>,
    pub is_wireless: bool,
    pub is_up: bool,
    pub is_default_route: bool,
}

impl DetectedNic {
    fn priority_key(&self) -> (u8, &str) {
        let rank = match (self.is_up, self.is_wireless, self.is_default_route) {
            (true, false, true) => 0,
            (true, false, false) => 1,
            (true, true, true) => 2,
            (true, true, false) => 3,
            (false, false, _) => 4,
            (false, true, _) => 5,
        };
        (rank, self.name.as_str())
    }
}

/// Estado do Wake-on-LAN / WoWLAN em um perfil do NetworkManager (Linux).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NmProfileWolStatus {
    pub name: String,
    pub conn_type: String,
    pub device: Option<String>,
    pub magic_enabled: bool,
}

/// Resolve os metadados efetivos de Wake-on-LAN que serão anunciados aos clientes confiáveis.
/// Retorna `None` quando `wol_enabled == false` ou quando nenhum MAC válido está disponível.
pub fn resolve_host_wol_info(settings: &HostSettings) -> Option<WolInfo> {
    if !settings.wol_enabled {
        return None;
    }

    let nics = discover_nics();
    let mut macs: Vec<String> = Vec::new();

    if let Some(explicit) = settings.wol_mac.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(norm) = normalize_wol_mac(explicit) {
            macs.push(norm);
        }
    } else {
        for nic in &nics {
            for candidate in std::iter::once(&nic.mac).chain(nic.perm_mac.iter()) {
                if !macs.contains(candidate) {
                    macs.push(candidate.clone());
                    if macs.len() >= MAX_WOL_MACS {
                        break;
                    }
                }
            }
            if macs.len() >= MAX_WOL_MACS {
                break;
            }
        }
    }

    if macs.is_empty() {
        return None;
    }

    let broadcast = settings
        .wol_broadcast
        .as_deref()
        .and_then(parse_safe_lan_broadcast_v4)
        .map(|ip| ip.to_string())
        .or_else(|| {
            nics.iter()
                .filter(|n| n.is_up)
                .find_map(|n| n.broadcast_v4.clone())
        })
        .or_else(|| nics.iter().find_map(|n| n.broadcast_v4.clone()));

    Some(WolInfo { macs, broadcast })
}

#[cfg(target_os = "linux")]
pub fn discover_nics() -> Vec<DetectedNic> {
    use std::fs;
    use std::path::Path;

    let default_ifaces = linux_default_route_ifaces();
    let broadcasts = linux_ipv4_broadcasts();
    let sys_net = Path::new("/sys/class/net");

    let Ok(entries) = fs::read_dir(sys_net) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let iface_dir = entry.path();

        // Exige vínculo real com barramento de hardware (PCI/USB/plataforma), descartando lo, docker, podman, veth, tun e wg.
        if !iface_dir.join("device").exists() {
            continue;
        }

        let Ok(raw_mac) = fs::read_to_string(iface_dir.join("address")) else {
            continue;
        };
        let Ok(mac) = normalize_wol_mac(&raw_mac) else {
            continue;
        };

        let operstate = fs::read_to_string(iface_dir.join("operstate"))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let carrier_up = fs::read_to_string(iface_dir.join("carrier"))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);
        let is_up = operstate == "up" || carrier_up;
        let is_wireless =
            iface_dir.join("wireless").exists() || iface_dir.join("phy80211").exists();
        let perm_mac = linux_read_perm_mac(&name, &iface_dir, &mac);
        let is_default_route = default_ifaces.contains(&name);
        let broadcast_v4 = broadcasts.get(&name).cloned();

        out.push(DetectedNic {
            name,
            mac,
            perm_mac,
            broadcast_v4,
            is_wireless,
            is_up,
            is_default_route,
        });
    }

    out.sort_by(|a, b| a.priority_key().cmp(&b.priority_key()));
    out
}

#[cfg(target_os = "linux")]
fn linux_read_perm_mac(
    iface_name: &str,
    iface_dir: &std::path::Path,
    active_mac: &str,
) -> Option<String> {
    use std::fs;
    use std::process::Command;

    let assign_type = fs::read_to_string(iface_dir.join("addr_assign_type"))
        .unwrap_or_default()
        .trim()
        .to_string();
    if assign_type == "0" {
        return None;
    }

    if let Ok(raw_phy) = fs::read_to_string(iface_dir.join("phy80211/macaddress")) {
        if let Ok(norm) = normalize_wol_mac(&raw_phy) {
            if norm != active_mac {
                return Some(norm);
            }
        }
    }

    if let Ok(out) = Command::new("ip")
        .args(["-o", "link", "show", "dev", iface_name])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let tokens: Vec<&str> = text.split_whitespace().collect();
            for win in tokens.windows(2) {
                if win[0] == "permaddr" {
                    if let Ok(norm) = normalize_wol_mac(win[1]) {
                        if norm != active_mac {
                            return Some(norm);
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn linux_default_route_ifaces() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let Ok(raw) = std::fs::read_to_string("/proc/net/route") else {
        return set;
    };
    for line in raw.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 2 && cols[1] == "00000000" {
            set.insert(cols[0].to_string());
        }
    }
    set
}

#[cfg(target_os = "linux")]
fn linux_ipv4_broadcasts() -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    use std::ffi::CStr;
    use std::net::Ipv4Addr;

    struct IfAddrsGuard(*mut libc::ifaddrs);
    impl Drop for IfAddrsGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { libc::freeifaddrs(self.0) };
            }
        }
    }

    let mut map = HashMap::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 || head.is_null() {
        return map;
    }
    let _guard = IfAddrsGuard(head);

    let mut cur = head;
    while !cur.is_null() {
        let ifa = unsafe { &*cur };
        cur = ifa.ifa_next;

        if ifa.ifa_addr.is_null() || ifa.ifa_netmask.is_null() || ifa.ifa_name.is_null() {
            continue;
        }

        let family = unsafe { (*ifa.ifa_addr).sa_family as i32 };
        let mask_family = unsafe { (*ifa.ifa_netmask).sa_family as i32 };
        if family != libc::AF_INET || mask_family != libc::AF_INET {
            continue;
        }

        let name = unsafe { CStr::from_ptr(ifa.ifa_name) }
            .to_string_lossy()
            .into_owned();
        let sin_addr = unsafe { (*(ifa.ifa_addr as *const libc::sockaddr_in)).sin_addr.s_addr };
        let sin_mask = unsafe { (*(ifa.ifa_netmask as *const libc::sockaddr_in)).sin_addr.s_addr };

        let ip = Ipv4Addr::from(u32::from_be(sin_addr));
        let mask = u32::from_be(sin_mask);
        if ip.is_loopback() || ip.is_link_local() || mask == 0 || mask == u32::MAX {
            continue;
        }

        let bcast = Ipv4Addr::from(u32::from(ip) | !mask);
        map.entry(name).or_insert_with(|| bcast.to_string());
    }

    map
}

#[cfg(target_os = "windows")]
pub fn discover_nics() -> Vec<DetectedNic> {
    let Ok(output) = std::process::Command::new("getmac")
        .args(["/fo", "csv", "/nh"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let first_col = line
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        if let Ok(mac) = normalize_wol_mac(first_col) {
            let disconnected = line.to_ascii_lowercase().contains("disconnected")
                || line.to_lowercase().contains("desconectada");
            out.push(DetectedNic {
                name: format!("nic{idx}"),
                mac,
                perm_mac: None,
                broadcast_v4: None,
                is_wireless: false,
                is_up: !disconnected,
                is_default_route: idx == 0 && !disconnected,
            });
        }
    }
    out.sort_by(|a, b| a.priority_key().cmp(&b.priority_key()));
    out
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn discover_nics() -> Vec<DetectedNic> {
    Vec::new()
}

#[cfg(target_os = "linux")]
pub fn inspect_nm_wol_profiles() -> Vec<NmProfileWolStatus> {
    let Ok(out) = crate::config::host_std_command("nmcli")
        .args(["-t", "-f", "UUID,TYPE,DEVICE,NAME", "connection", "show"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    let mut statuses = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 4 {
            continue;
        }
        let (uuid, conn_type, device_raw, name) = (parts[0], parts[1], parts[2], parts[3]);
        let prop = match conn_type {
            "802-3-ethernet" => "802-3-ethernet.wake-on-lan",
            "802-11-wireless" if !device_raw.trim().is_empty() => "802-11-wireless.wake-on-wlan",
            _ => continue,
        };

        // `-f` inclui o rótulo textual `(magic)` mesmo quando `wake-on-wlan` retorna máscara hex `0x8`.
        let magic_enabled = crate::config::host_std_command("nmcli")
            .args(["-f", prop, "connection", "show", "uuid", uuid])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .to_ascii_lowercase()
                    .contains("magic")
            })
            .unwrap_or(false);

        statuses.push(NmProfileWolStatus {
            name: name.to_string(),
            conn_type: conn_type.to_string(),
            device: (!device_raw.trim().is_empty()).then(|| device_raw.to_string()),
            magic_enabled,
        });
    }
    statuses
}

#[cfg(not(target_os = "linux"))]
pub fn inspect_nm_wol_profiles() -> Vec<NmProfileWolStatus> {
    Vec::new()
}

#[cfg(target_os = "linux")]
pub fn enable_nm_magic_packet() -> Vec<std::result::Result<String, String>> {
    let Ok(out) = crate::config::host_std_command("nmcli")
        .args(["-t", "-f", "UUID,TYPE,DEVICE,NAME", "connection", "show"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 4 {
            continue;
        }
        let (uuid, conn_type, device_raw, name) = (parts[0], parts[1], parts[2], parts[3]);
        let prop = match conn_type {
            "802-3-ethernet" => "802-3-ethernet.wake-on-lan",
            "802-11-wireless" if !device_raw.trim().is_empty() => "802-11-wireless.wake-on-wlan",
            _ => continue,
        };

        match crate::config::host_std_command("nmcli")
            .args(["connection", "modify", "uuid", uuid, prop, "magic"])
            .output()
        {
            Ok(res) if res.status.success() => {
                let dev = device_raw.trim();
                let reapplied = if !dev.is_empty() {
                    crate::config::host_std_command("nmcli")
                        .args(["device", "reapply", dev])
                        .output()
                        .is_ok_and(|o| o.status.success())
                } else {
                    false
                };
                let suffix = if reapplied {
                    " · aplicado ao driver ativo"
                } else {
                    ""
                };
                results.push(Ok(format!("{name} ({prop} = magic{suffix})")));
            }
            Ok(res) => {
                let stderr = String::from_utf8_lossy(&res.stderr).trim().to_string();
                results.push(Err(format!(
                    "{name}: {stderr} (tente: sudo nmcli connection modify uuid {uuid} {prop} magic)"
                )));
            }
            Err(err) => {
                results.push(Err(format!("{name}: falha ao invocar nmcli ({err})")));
            }
        }
    }
    results
}

#[cfg(not(target_os = "linux"))]
pub fn enable_nm_magic_packet() -> Vec<std::result::Result<String, String>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_orders_wired_up_before_wifi_and_down() {
        let mut nics = vec![
            DetectedNic {
                name: "eno1".into(),
                mac: "AA:BB:CC:00:00:01".into(),
                perm_mac: None,
                broadcast_v4: None,
                is_wireless: false,
                is_up: false,
                is_default_route: false,
            },
            DetectedNic {
                name: "wlp3s0".into(),
                mac: "AA:BB:CC:00:00:02".into(),
                perm_mac: Some("AA:BB:CC:00:00:99".into()),
                broadcast_v4: Some("192.168.18.255".into()),
                is_wireless: true,
                is_up: true,
                is_default_route: true,
            },
            DetectedNic {
                name: "enp4s0".into(),
                mac: "AA:BB:CC:00:00:03".into(),
                perm_mac: None,
                broadcast_v4: Some("192.168.18.255".into()),
                is_wireless: false,
                is_up: true,
                is_default_route: true,
            },
        ];
        nics.sort_by(|a, b| a.priority_key().cmp(&b.priority_key()));
        assert_eq!(nics[0].name, "enp4s0");
        assert_eq!(nics[1].name, "wlp3s0");
        assert_eq!(nics[2].name, "eno1");
    }

    #[test]
    fn disabled_wol_never_exposes_mac() {
        let mut settings = HostSettings::default();
        settings.wol_enabled = false;
        settings.wol_mac = Some("AA:BB:CC:11:22:33".into());
        assert!(resolve_host_wol_info(&settings).is_none());

        settings.wol_enabled = true;
        settings.wol_broadcast = Some("192.168.1.255".into());
        let resolved = resolve_host_wol_info(&settings).unwrap();
        assert_eq!(resolved.macs, vec!["AA:BB:CC:11:22:33".to_string()]);
        assert_eq!(resolved.broadcast.as_deref(), Some("192.168.1.255"));
    }
}
