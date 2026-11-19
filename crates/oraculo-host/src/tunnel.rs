use crate::config::HostSettings;
use anyhow::{Context, Result, bail};
use iroh::endpoint::{RecvStream, SendStream};
use oraculo_proto::{TunnelOpenReq, TunnelOpenResp, TunnelTarget, read_frame, write_frame};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tracing::{info, warn};

pub const DEFAULT_RDP_PORT: u16 = 3389;
const TUNNEL_REQ_TIMEOUT: Duration = Duration::from_secs(5);
const LOCAL_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg_attr(target_os = "linux", allow(dead_code))]
const PROBE_TIMEOUT: Duration = Duration::from_millis(150);
const FIREWALL_CACHE_TTL: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RdpStatus {
    pub listening: bool,
    pub port: u16,
    pub backend_installed: bool,
    pub backend_label: String,
    pub firewall_rule_blocked: bool,
    pub exposed_on_lan: bool,
    pub can_harden_firewall: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FirewallInspection {
    firewall_rule_blocked: bool,
    can_harden_firewall: bool,
}

#[derive(Debug, Clone, Copy)]
struct CachedFirewallInspection {
    port: u16,
    captured_at: Instant,
    result: FirewallInspection,
}

fn firewall_cache() -> &'static Mutex<Option<CachedFirewallInspection>> {
    static CACHE: OnceLock<Mutex<Option<CachedFirewallInspection>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_rdp_firewall_cache() {
    if let Ok(mut guard) = firewall_cache().lock() {
        *guard = None;
    }
}

pub fn effective_rdp_port(settings: &HostSettings) -> u16 {
    if settings.rdp_port == 0 {
        DEFAULT_RDP_PORT
    } else {
        settings.rdp_port
    }
}

pub fn resolve_target_port(settings: &HostSettings, target: TunnelTarget) -> Result<u16, String> {
    let allowed = effective_rdp_port(settings);
    match target {
        TunnelTarget::LocalRdp { port } if port == 0 || port == allowed => Ok(allowed),
        TunnelTarget::LocalRdp { port } => Err(format!(
            "Porta solicitada ({port}) diferente da porta RDP configurada ({allowed})"
        )),
    }
}

pub async fn is_local_rdp_listening(port: u16) -> bool {
    let effective = if port == 0 { DEFAULT_RDP_PORT } else { port };
    #[cfg(target_os = "linux")]
    {
        // Lê `/proc/net/tcp{,6}` sem abrir conexão TCP descartável a cada poll,
        // evitando flood de handshakes TLS abortados no gnome-remote-desktop-daemon.
        tokio::task::spawn_blocking(move || has_proc_net_listener(effective, ListenerScope::LocalReachable))
            .await
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "linux"))]
    {
        let target = SocketAddrV4::new(Ipv4Addr::LOCALHOST, effective);
        tokio::time::timeout(PROBE_TIMEOUT, TcpStream::connect(target))
            .await
            .is_ok_and(|res| res.is_ok())
    }
}

pub async fn inspect_rdp_status(port: u16) -> RdpStatus {
    let effective = if port == 0 { DEFAULT_RDP_PORT } else { port };
    let listening = is_local_rdp_listening(effective).await;

    let (non_loopback_listener, fw) = tokio::task::spawn_blocking(move || {
        (
            has_non_loopback_listener(effective),
            inspect_firewall_cached(effective),
        )
    })
    .await
    .unwrap_or((
        listening,
        FirewallInspection {
            firewall_rule_blocked: false,
            can_harden_firewall: false,
        },
    ));

    #[cfg(target_os = "linux")]
    let (backend_installed, backend_label) = {
        let has_grd = Path::new("/usr/bin/grdctl").exists()
            || Path::new("/usr/libexec/gnome-remote-desktop-daemon").exists();
        (has_grd, "GNOME Remote Desktop".to_string())
    };

    #[cfg(target_os = "windows")]
    let (backend_installed, backend_label) = (true, "Windows RDP".to_string());

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let (backend_installed, backend_label) = (false, "RDP".to_string());

    let exposed_on_lan = listening && non_loopback_listener && !fw.firewall_rule_blocked;

    RdpStatus {
        listening,
        port: effective,
        backend_installed,
        backend_label,
        firewall_rule_blocked: fw.firewall_rule_blocked,
        exposed_on_lan,
        can_harden_firewall: fw.can_harden_firewall,
    }
}

fn inspect_firewall_cached(port: u16) -> FirewallInspection {
    if let Ok(guard) = firewall_cache().lock() {
        if let Some(cached) = *guard {
            if cached.port == port && cached.captured_at.elapsed() < FIREWALL_CACHE_TTL {
                return cached.result;
            }
        }
    }

    let fresh = inspect_firewall_live(port);
    if let Ok(mut guard) = firewall_cache().lock() {
        *guard = Some(CachedFirewallInspection {
            port,
            captured_at: Instant::now(),
            result: fresh,
        });
    }
    fresh
}

#[cfg(target_os = "linux")]
fn inspect_firewall_live(port: u16) -> FirewallInspection {
    let state_out = crate::config::host_std_command("firewall-cmd")
        .arg("--state")
        .output();
    let Ok(state_out) = state_out else {
        return FirewallInspection {
            firewall_rule_blocked: false,
            can_harden_firewall: false,
        };
    };
    if !state_out.status.success()
        || String::from_utf8_lossy(&state_out.stdout).trim() != "running"
    {
        return FirewallInspection {
            firewall_rule_blocked: false,
            can_harden_firewall: false,
        };
    }

    let zones = active_interface_zones();
    let blocked = if zones.is_empty() {
        is_zone_blocking_rdp(None, port)
    } else {
        zones.iter().all(|z| is_zone_blocking_rdp(Some(z.as_str()), port))
    };

    let has_pkexec =
        Path::new("/.flatpak-info").exists() || Path::new("/usr/bin/pkexec").exists();
    FirewallInspection {
        firewall_rule_blocked: blocked,
        can_harden_firewall: !blocked && has_pkexec,
    }
}

#[cfg(target_os = "linux")]
fn active_interface_zones() -> Vec<String> {
    let Ok(out) = crate::config::host_std_command("firewall-cmd")
        .arg("--get-active-zones")
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_active_interface_zones(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "linux")]
fn firewall_cmd_for_zone(zone: Option<&str>) -> std::process::Command {
    let mut cmd = crate::config::host_std_command("firewall-cmd");
    if let Some(z) = zone {
        cmd.args(["--zone", z]);
    }
    cmd
}

#[cfg(target_os = "linux")]
fn is_zone_blocking_rdp(zone: Option<&str>, port: u16) -> bool {
    if let Ok(out) = firewall_cmd_for_zone(zone).arg("--list-rich-rules").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if rich_rules_block_port(&text, port) {
                return true;
            }
        }
    }

    if let Ok(out) = firewall_cmd_for_zone(zone).arg("--list-ports").output() {
        if out.status.success()
            && ports_list_exposes_tcp_port(&String::from_utf8_lossy(&out.stdout), port)
        {
            return false;
        }
    }

    let port_open = firewall_cmd_for_zone(zone)
        .arg(format!("--query-port={port}/tcp"))
        .output()
        .is_ok_and(|o| o.status.success());
    if port_open {
        return false;
    }

    let rdp_service = firewall_cmd_for_zone(zone)
        .arg("--list-services")
        .output()
        .is_ok_and(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .any(|s| matches!(s, "rdp" | "ms-wbt-server"))
        });
    if rdp_service {
        return false;
    }

    let target_accept = firewall_cmd_for_zone(zone)
        .arg("--get-target")
        .output()
        .is_ok_and(|o| {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .eq_ignore_ascii_case("ACCEPT")
        });

    !target_accept
}

#[cfg(target_os = "windows")]
fn inspect_firewall_live(port: u16) -> FirewallInspection {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let rule_name = format!("Oraculo Block External RDP ({port})");
    let ps = format!(
        "if (Get-NetFirewallRule -DisplayName '{rule_name}' -ErrorAction SilentlyContinue | Where-Object {{ $_.Enabled -eq 'True' -and $_.Action -eq 'Block' }}) {{ exit 0 }} else {{ exit 1 }}"
    );
    let blocked = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| o.status.success());

    FirewallInspection {
        firewall_rule_blocked: blocked,
        can_harden_firewall: !blocked,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn inspect_firewall_live(_port: u16) -> FirewallInspection {
    FirewallInspection {
        firewall_rule_blocked: false,
        can_harden_firewall: false,
    }
}

/// Bloqueia a porta RDP (`3389` ou configurada) para tráfego externo/LAN mantendo `127.0.0.1` acessível ao túnel.
pub fn harden_rdp_firewall(port: u16) -> Result<()> {
    let effective = if port == 0 { DEFAULT_RDP_PORT } else { port };

    #[cfg(target_os = "linux")]
    {
        if !Path::new("/.flatpak-info").exists() && !Path::new("/usr/bin/pkexec").exists() {
            bail!("pkexec não está instalado neste sistema para aplicar regras no firewall");
        }
        let rich_rule = format!("rule port port=\"{effective}\" protocol=\"tcp\" reject");
        let valid_zones: Vec<String> = active_interface_zones()
            .into_iter()
            .filter(|z| {
                !z.is_empty()
                    && z.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
            })
            .collect();
        let out = if valid_zones.is_empty() {
            crate::config::host_std_command("pkexec")
                .args([
                    "sh",
                    "-c",
                    "firewall-cmd --permanent --add-rich-rule=\"$1\" && firewall-cmd --reload",
                    "oraculo-firewall",
                    &rich_rule,
                ])
                .output()
                .context("Falha ao invocar pkexec para configurar firewalld")?
        } else {
            let mut cmd = crate::config::host_std_command("pkexec");
            cmd.args([
                "sh",
                "-c",
                "rule=\"$1\"; shift; for z in \"$@\"; do firewall-cmd --permanent --zone=\"$z\" --add-rich-rule=\"$rule\" || exit 1; done; firewall-cmd --reload",
                "oraculo-firewall",
                &rich_rule,
            ]);
            for z in &valid_zones {
                cmd.arg(z);
            }
            cmd.output()
                .context("Falha ao invocar pkexec para configurar firewalld")?
        };

        if !out.status.success() {
            let code = out.status.code().unwrap_or(1);
            if matches!(code, 126 | 127) {
                bail!("Autorização de administrador cancelada.");
            }
            let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            bail!(
                "Não foi possível atualizar o firewall{}",
                if err_msg.is_empty() {
                    String::new()
                } else {
                    format!(": {err_msg}")
                }
            );
        }
        invalidate_rdp_firewall_cache();
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let rule_name = format!("Oraculo Block External RDP ({effective})");
        let netsh_cmd = format!(
            "/c netsh advfirewall firewall delete rule name=\"{rule_name}\" >nul 2>&1 & netsh advfirewall firewall add rule name=\"{rule_name}\" dir=in action=block protocol=TCP localport={effective} interfacetype=lan,wireless,ras profile=any"
        );
        let escaped = netsh_cmd.replace('\'', "''");
        let ps = format!(
            "Start-Process -FilePath 'cmd.exe' -ArgumentList '{escaped}' -Verb RunAs -Wait -WindowStyle Hidden"
        );
        let out = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &ps,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .context("Falha ao solicitar elevação UAC para o Windows Firewall")?;
        if !out.status.success() {
            bail!("Autorização de administrador cancelada ou recusada no Windows.");
        }
        invalidate_rdp_firewall_cache();
        if !inspect_firewall_live(effective).firewall_rule_blocked {
            bail!("A regra de bloqueio RDP não foi aplicada no Firewall do Windows.");
        }
        return Ok(());
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = effective;
        bail!("Bloqueio automático de firewall disponível apenas no Linux e Windows")
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn has_proc_net_listener(port: u16, scope: ListenerScope) -> bool {
    #[cfg(target_os = "linux")]
    {
        let v4 = std::fs::read_to_string("/proc/net/tcp")
            .map(|s| proc_net_tcp_has_listener(&s, port, false, scope))
            .unwrap_or(false);
        let v6 = std::fs::read_to_string("/proc/net/tcp6")
            .map(|s| proc_net_tcp_has_listener(&s, port, true, scope))
            .unwrap_or(false);
        v4 || v6
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (port, scope);
        true
    }
}

fn has_non_loopback_listener(port: u16) -> bool {
    has_proc_net_listener(port, ListenerScope::NonLoopback)
}

#[derive(Clone, Copy)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum ListenerScope {
    NonLoopback,
    LocalReachable,
}

fn parse_proc_net_ipv4(ip_hex: &str) -> Option<IpAddr> {
    if ip_hex.len() != 8 {
        return None;
    }
    let raw = u32::from_str_radix(ip_hex, 16).ok()?;
    Some(IpAddr::V4(Ipv4Addr::from(raw.to_ne_bytes())))
}

fn parse_proc_net_ipv6(ip_hex: &str) -> Option<IpAddr> {
    if ip_hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for idx in 0..4 {
        let start = idx * 8;
        let word = u32::from_str_radix(&ip_hex[start..start + 8], 16).ok()?;
        bytes[idx * 4..idx * 4 + 4].copy_from_slice(&word.to_ne_bytes());
    }
    Some(IpAddr::V6(Ipv6Addr::from(bytes)))
}

fn is_ip_loopback_or_v4_mapped(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

fn is_ip_local_reachable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_unspecified(),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| v4.is_loopback() || v4.is_unspecified())
        }
    }
}

fn proc_net_tcp_has_listener(content: &str, port: u16, is_v6: bool, scope: ListenerScope) -> bool {
    for line in content.lines().skip(1) {
        let mut cols = line.split_whitespace();
        let _slot = cols.next();
        let Some(local_addr) = cols.next() else {
            continue;
        };
        let _remote_addr = cols.next();
        if cols.next() != Some("0A") {
            continue;
        }
        let Some((ip_hex, port_hex)) = local_addr.split_once(':') else {
            continue;
        };
        if u16::from_str_radix(port_hex, 16).ok() != Some(port) {
            continue;
        }
        let ip = if is_v6 {
            parse_proc_net_ipv6(ip_hex)
        } else {
            parse_proc_net_ipv4(ip_hex)
        };
        let Some(ip) = ip else {
            continue;
        };
        let matches = match scope {
            ListenerScope::NonLoopback => !is_ip_loopback_or_v4_mapped(ip),
            ListenerScope::LocalReachable => is_ip_local_reachable(ip),
        };
        if matches {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn proc_net_tcp_has_non_loopback_v4(content: &str, port: u16) -> bool {
    proc_net_tcp_has_listener(content, port, false, ListenerScope::NonLoopback)
}

#[allow(dead_code)]
fn proc_net_tcp_has_non_loopback_v6(content: &str, port: u16) -> bool {
    proc_net_tcp_has_listener(content, port, true, ListenerScope::NonLoopback)
}

#[allow(dead_code)]
fn parse_active_interface_zones(stdout: &str) -> Vec<String> {
    let mut zones = Vec::new();
    let mut current_zone: Option<String> = None;
    for raw in stdout.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !raw.starts_with(char::is_whitespace) {
            let name = raw
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            current_zone = if name.is_empty() { None } else { Some(name) };
        } else if let Some(rest) = trimmed.strip_prefix("interfaces:") {
            if !rest.trim().is_empty() {
                if let Some(ref z) = current_zone {
                    if !zones.contains(z) {
                        zones.push(z.clone());
                    }
                }
            }
        }
    }
    zones
}

#[allow(dead_code)]
fn rich_rule_has_positive_priority(normalized: &str) -> bool {
    for tok in normalized.split_whitespace() {
        if let Some(rest) = tok.strip_prefix("priority=") {
            let raw = rest.trim_matches(|c| c == '"' || c == '\'');
            if let Ok(val) = raw.parse::<i32>() {
                if val > 0 {
                    return true;
                }
            }
        }
    }
    false
}

#[allow(dead_code)]
fn rich_rule_blocks_port(line: &str, port: u16) -> bool {
    let normalized = line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if normalized.contains(" source ") || normalized.starts_with("source ") {
        return false;
    }
    if rich_rule_has_positive_priority(&normalized) {
        return false;
    }
    let matches_port = normalized.contains(&format!("port=\"{port}\""))
        || normalized.contains(&format!("port='{port}'"));
    let matches_tcp = normalized.contains("protocol=\"tcp\"")
        || normalized.contains("protocol='tcp'");
    let has_deny_action = normalized
        .split_whitespace()
        .any(|tok| matches!(tok, "reject" | "drop"));
    matches_port && matches_tcp && has_deny_action
}

#[allow(dead_code)]
fn rich_rules_block_port(text: &str, port: u16) -> bool {
    let mut blocks_v4 = false;
    let mut blocks_v6 = false;
    for line in text.lines() {
        if !rich_rule_blocks_port(line, port) {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let only_v4 = lower.contains("family=\"ipv4\"") || lower.contains("family='ipv4'");
        let only_v6 = lower.contains("family=\"ipv6\"") || lower.contains("family='ipv6'");
        if !only_v4 && !only_v6 {
            return true;
        }
        if only_v4 {
            blocks_v4 = true;
        }
        if only_v6 {
            blocks_v6 = true;
        }
    }
    blocks_v4 && blocks_v6
}

#[allow(dead_code)]
fn ports_list_exposes_tcp_port(stdout: &str, port: u16) -> bool {
    stdout.split_whitespace().any(|entry| {
        let Some((spec, proto)) = entry.split_once('/') else {
            return false;
        };
        if !proto.eq_ignore_ascii_case("tcp") {
            return false;
        }
        if let Some((start_s, end_s)) = spec.split_once('-') {
            match (start_s.parse::<u16>(), end_s.parse::<u16>()) {
                (Ok(start), Ok(end)) => start <= port && port <= end,
                _ => false,
            }
        } else {
            spec.parse::<u16>().ok() == Some(port)
        }
    })
}

async fn send_open_error(mut send: SendStream, msg: impl Into<String>) {
    let _ = write_frame(
        &mut send,
        &TunnelOpenResp {
            ok: false,
            message: msg.into(),
            resolved_port: 0,
        },
    )
    .await;
    let _ = send.finish();
    tokio::time::sleep(Duration::from_millis(60)).await;
}

async fn wait_until_tunnel_disabled(settings_rx: &mut watch::Receiver<HostSettings>) {
    loop {
        if !settings_rx.borrow().allow_rdp_tunnel {
            return;
        }
        if settings_rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

pub async fn handle_tunnel_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    mut settings_rx: watch::Receiver<HostSettings>,
) -> Result<()> {
    let req = match tokio::time::timeout(TUNNEL_REQ_TIMEOUT, read_frame::<_, TunnelOpenReq>(&mut recv)).await {
        Ok(Ok(Some(req))) => req,
        Ok(Ok(None)) => return Ok(()),
        Ok(Err(err)) => {
            send_open_error(send, format!("Requisição de túnel inválida: {err:#}")).await;
            return Ok(());
        }
        Err(_) => {
            send_open_error(send, "Tempo esgotado lendo requisição de túnel").await;
            return Ok(());
        }
    };

    let current_settings = settings_rx.borrow().clone();
    if !current_settings.allow_rdp_tunnel {
        send_open_error(
            send,
            "Túnel de login remoto (RDP) desativado nas permissões deste computador",
        )
        .await;
        return Ok(());
    }

    let target_port = match resolve_target_port(&current_settings, req.target) {
        Ok(port) => port,
        Err(msg) => {
            send_open_error(send, msg).await;
            return Ok(());
        }
    };

    let addr_v4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, target_port);
    let tcp = match tokio::time::timeout(LOCAL_CONNECT_TIMEOUT, TcpStream::connect(addr_v4)).await {
        Ok(Ok(stream)) => stream,
        _ => {
            let addr_v6 = std::net::SocketAddrV6::new(std::net::Ipv6Addr::LOCALHOST, target_port, 0, 0);
            match tokio::time::timeout(LOCAL_CONNECT_TIMEOUT, TcpStream::connect(addr_v6)).await {
                Ok(Ok(stream)) => stream,
                _ => {
                    send_open_error(
                        send,
                        format!(
                            "Nenhum servidor RDP ativo em 127.0.0.1:{target_port}. Ative o Login Remoto nas configurações do sistema para usar o login remoto."
                        ),
                    )
                    .await;
                    return Ok(());
                }
            }
        }
    };
    let _ = tcp.set_nodelay(true);

    write_frame(
        &mut send,
        &TunnelOpenResp {
            ok: true,
            message: if req.probe_only {
                format!("Servidor RDP acessível na porta {target_port}")
            } else {
                format!("Túnel RDP ativo (127.0.0.1:{target_port})")
            },
            resolved_port: target_port,
        },
    )
    .await?;

    if req.probe_only {
        drop(tcp);
        let _ = send.finish();
        tokio::time::sleep(Duration::from_millis(60)).await;
        return Ok(());
    }

    info!(port = target_port, "Túnel RDP local iniciado");
    let (mut tcp_read, mut tcp_write) = tcp.into_split();

    let relay_fut = async move {
        let client_to_local = async {
            let _ = tokio::io::copy(&mut recv, &mut tcp_write).await;
            let _ = tcp_write.shutdown().await;
        };
        let local_to_client = async {
            let _ = tokio::io::copy(&mut tcp_read, &mut send).await;
            let _ = send.finish();
        };
        tokio::join!(client_to_local, local_to_client);
    };

    tokio::select! {
        biased;
        _ = wait_until_tunnel_disabled(&mut settings_rx) => {
            warn!(port = target_port, "Túnel RDP encerrado imediatamente porque a permissão foi desativada");
        }
        _ = relay_fut => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::{Endpoint, EndpointAddr, endpoint::presets};
    use oraculo_proto::{ALPN, StreamKind, read_stream_kind, write_stream_kind};
    use std::net::{IpAddr, SocketAddr};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn resolve_target_port_blocks_non_rdp_localhost_ports() {
        let settings = HostSettings::default();
        assert_eq!(
            resolve_target_port(&settings, TunnelTarget::LocalRdp { port: 0 }),
            Ok(3389)
        );
        assert_eq!(
            resolve_target_port(&settings, TunnelTarget::LocalRdp { port: 3389 }),
            Ok(3389)
        );
        assert!(resolve_target_port(&settings, TunnelTarget::LocalRdp { port: 22 }).is_err());
        assert!(resolve_target_port(&settings, TunnelTarget::LocalRdp { port: 5432 }).is_err());
    }

    #[test]
    fn proc_net_and_firewall_parsers_distinguish_loopback_and_exposed_ports() {
        let lo_v4 = format!("{:08X}:0D3D", u32::from_ne_bytes([127, 0, 0, 1]));
        let lan_v4 = format!("{:08X}:0D3D", u32::from_ne_bytes([0, 0, 0, 0]));
        let only_lo_v4 = format!(
            "  sl  local_address rem_address   st\n   0: {lo_v4} 00000000:0000 0A\n"
        );
        let wildcard_v4 = format!(
            "  sl  local_address rem_address   st\n   0: {lan_v4} 00000000:0000 0A\n"
        );
        assert!(!proc_net_tcp_has_non_loopback_v4(&only_lo_v4, 3389));
        assert!(proc_net_tcp_has_non_loopback_v4(&wildcard_v4, 3389));
        assert!(proc_net_tcp_has_listener(
            &only_lo_v4,
            3389,
            false,
            ListenerScope::LocalReachable
        ));
        assert!(proc_net_tcp_has_listener(
            &wildcard_v4,
            3389,
            false,
            ListenerScope::LocalReachable
        ));
        let time_wait_v4 = format!(
            "  sl  local_address rem_address   st\n   0: {lo_v4} {lo_v4} 06\n"
        );
        assert!(!proc_net_tcp_has_listener(
            &time_wait_v4,
            3389,
            false,
            ListenerScope::LocalReachable
        ));
        let lan_only_v4 = format!(
            "  sl  local_address rem_address   st\n   0: {:08X}:0D3D 00000000:0000 0A\n",
            u32::from_ne_bytes([192, 168, 1, 10])
        );
        assert!(!proc_net_tcp_has_listener(
            &lan_only_v4,
            3389,
            false,
            ListenerScope::LocalReachable
        ));

        let lo_v6_words = [
            u32::from_ne_bytes([0, 0, 0, 0]),
            u32::from_ne_bytes([0, 0, 0, 0]),
            u32::from_ne_bytes([0, 0, 0, 0]),
            u32::from_ne_bytes([0, 0, 0, 1]),
        ];
        let lo_v6 = format!(
            "{:08X}{:08X}{:08X}{:08X}:0D3D",
            lo_v6_words[0], lo_v6_words[1], lo_v6_words[2], lo_v6_words[3]
        );
        let any_v6 = "00000000000000000000000000000000:0D3D";
        assert!(!proc_net_tcp_has_non_loopback_v6(
            &format!("hdr\n  0: {lo_v6} 0000:0000 0A\n"),
            3389
        ));
        assert!(proc_net_tcp_has_non_loopback_v6(
            &format!("hdr\n  0: {any_v6} 0000:0000 0A\n"),
            3389
        ));
        assert!(proc_net_tcp_has_listener(
            &format!("hdr\n  0: {lo_v6} 0000:0000 0A\n"),
            3389,
            true,
            ListenerScope::LocalReachable
        ));
        assert!(proc_net_tcp_has_listener(
            &format!("hdr\n  0: {any_v6} 0000:0000 0A\n"),
            3389,
            true,
            ListenerScope::LocalReachable
        ));

        let zones_out = "FedoraWorkstation (default)\n  interfaces: wlp3s0\ntrusted\n  sources: 10.89.0.0/24\n";
        assert_eq!(parse_active_interface_zones(zones_out), vec!["FedoraWorkstation"]);

        assert!(rich_rule_blocks_port(
            "rule port port=\"3389\" protocol=\"tcp\" reject",
            3389
        ));
        assert!(rich_rule_blocks_port(
            "rule family=\"ipv4\" port port='3389' protocol='tcp' drop",
            3389
        ));
        assert!(!rich_rule_blocks_port(
            "rule port port=\"3389\" protocol=\"tcp\" accept",
            3389
        ));
        assert!(!rich_rule_blocks_port(
            "rule port port=\"3390\" protocol=\"tcp\" reject",
            3389
        ));
        assert!(!rich_rule_blocks_port(
            "rule family=\"ipv4\" source address=\"1.2.3.4\" port port=\"3389\" protocol=\"tcp\" reject",
            3389
        ));
        assert!(!rich_rule_blocks_port(
            "rule priority=\"10\" port port=\"3389\" protocol=\"tcp\" reject",
            3389
        ));
        assert!(rich_rules_block_port(
            "rule port port=\"3389\" protocol=\"tcp\" reject\n",
            3389
        ));
        assert!(!rich_rules_block_port(
            "rule family=\"ipv4\" port port=\"3389\" protocol=\"tcp\" reject\n",
            3389
        ));
        assert!(rich_rules_block_port(
            "rule family=\"ipv4\" port port=\"3389\" protocol=\"tcp\" reject\nrule family=\"ipv6\" port port=\"3389\" protocol=\"tcp\" reject\n",
            3389
        ));

        assert!(ports_list_exposes_tcp_port(
            "1025-65535/tcp 1025-65535/udp",
            3389
        ));
        assert!(ports_list_exposes_tcp_port("22/tcp 3389/TCP", 3389));
        assert!(ports_list_exposes_tcp_port("1025-65535/tcp", 1025));
        assert!(ports_list_exposes_tcp_port("1025-65535/tcp", 65535));
        assert!(!ports_list_exposes_tcp_port("3389/udp 8080-8090/tcp", 3389));
        assert!(!ports_list_exposes_tcp_port("invalido", 3389));
    }

    #[tokio::test]
    async fn tunnel_bridges_localhost_tcp_and_stops_on_permission_off() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let echo_task = tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 256];
                    loop {
                        match sock.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if sock.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        });

        let mut settings = HostSettings::default();
        settings.rdp_port = port;
        let (settings_tx, settings_rx) = watch::channel(settings);

        let ep_srv = Endpoint::builder(presets::N0)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .unwrap();
        let ep_cli = Endpoint::builder(presets::N0).bind().await.unwrap();
        let srv_port = ep_srv
            .bound_sockets()
            .into_iter()
            .find(|a| a.is_ipv4())
            .unwrap()
            .port();
        let srv_addr = EndpointAddr::new(ep_srv.id())
            .with_ip_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), srv_port));

        let srv_task = tokio::spawn(async move {
            let conn = ep_srv.accept().await.unwrap().await.unwrap();
            let (send, mut recv) = conn.accept_bi().await.unwrap();
            assert_eq!(read_stream_kind(&mut recv).await.unwrap(), StreamKind::Tunnel);
            handle_tunnel_stream(send, recv, settings_rx).await.unwrap();
        });

        let cli_conn = ep_cli.connect(srv_addr, ALPN).await.unwrap();
        let (mut cli_send, mut cli_recv) = cli_conn.open_bi().await.unwrap();
        write_stream_kind(&mut cli_send, StreamKind::Tunnel)
            .await
            .unwrap();
        write_frame(
            &mut cli_send,
            &TunnelOpenReq {
                target: TunnelTarget::LocalRdp { port: 0 },
                probe_only: false,
            },
        )
        .await
        .unwrap();

        let resp: TunnelOpenResp = read_frame(&mut cli_recv).await.unwrap().unwrap();
        assert!(resp.ok);
        assert_eq!(resp.resolved_port, port);

        cli_send.write_all(b"oraculo-rdp-ping").await.unwrap();
        let mut echoed = [0u8; 16];
        cli_recv.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, b"oraculo-rdp-ping");

        // Ao desativar a permissão no host, o túnel deve encerrar imediatamente.
        settings_tx.send_modify(|s| s.allow_rdp_tunnel = false);
        tokio::time::timeout(Duration::from_secs(2), srv_task)
            .await
            .expect("túnel deve encerrar ao revogar permissão")
            .unwrap();

        echo_task.abort();
    }
}
