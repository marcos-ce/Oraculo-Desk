use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use tokio::net::{UdpSocket, lookup_host};

/// Porta UDP padrão histórica para pacotes Wake-on-LAN (Discard).
pub const DEFAULT_WOL_PORT: u16 = 9;

/// Porta UDP secundária tradicional para Wake-on-LAN (Echo).
pub const FALLBACK_WOL_PORT: u16 = 7;

/// Limite máximo de MACs aceitos/sincronizados por host para evitar abuso de memória ou amplificação UDP.
pub const MAX_WOL_MACS: usize = 6;

/// Quantidade de rajadas enviadas por acionamento para mitigar perda em Wi-Fi (DTIM).
const BURST_COUNT: usize = 3;

/// Intervalo entre rajadas consecutivas do Magic Packet.
const BURST_DELAY: Duration = Duration::from_millis(40);

/// Metadados de Wake-on-LAN anunciados pelo host para clientes pareados.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WolInfo {
    /// Endereços MAC físicos normalizados (`AA:BB:CC:DD:EE:FF`), em ordem de prioridade.
    #[serde(default)]
    pub macs: Vec<String>,
    /// Endereço de broadcast da sub-rede local do host (ex.: `192.168.1.255`).
    #[serde(default)]
    pub broadcast: Option<String>,
}

/// Extrai os 6 bytes de um endereço MAC, rejeitando formatos inválidos, nulo ou broadcast.
pub fn parse_wol_mac_bytes(raw: &str) -> Result<[u8; 6]> {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.') && !c.is_whitespace())
        .collect();

    if clean.len() != 12 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("Endereço MAC WoL inválido (use formato AA:BB:CC:DD:EE:FF)");
    }

    let mut bytes = [0u8; 6];
    for (idx, slot) in bytes.iter_mut().enumerate() {
        let pos = idx * 2;
        *slot = u8::from_str_radix(&clean[pos..pos + 2], 16)
            .context("Falha ao converter octeto hexadecimal do MAC")?;
    }

    if bytes == [0u8; 6] || bytes == [0xFFu8; 6] {
        bail!("Endereço MAC WoL reservado/inválido ({raw})");
    }

    Ok(bytes)
}

/// Normaliza um endereço MAC para o formato canônico `AA:BB:CC:DD:EE:FF`.
pub fn normalize_wol_mac(raw: &str) -> Result<String> {
    let b = parse_wol_mac_bytes(raw)?;
    Ok(format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        b[0], b[1], b[2], b[3], b[4], b[5]
    ))
}

/// Verifica se uma string representa estritamente um IPv4 literal de sub-rede local/privada ou broadcast.
/// Bloqueia nomes de domínio (evitando DNS beacon) e IPs públicos roteáveis na internet (evitando SSRF UDP).
pub fn parse_safe_lan_broadcast_v4(raw: &str) -> Option<Ipv4Addr> {
    let ip: Ipv4Addr = raw.trim().parse().ok()?;
    let oct = ip.octets();
    let is_cgnat = oct[0] == 100 && (64..=127).contains(&oct[1]);
    if ip.is_private() || ip.is_broadcast() || ip.is_link_local() || is_cgnat {
        Some(ip)
    } else {
        None
    }
}

/// Higieniza metadados `WolInfo` recebidos pela rede antes de persistir no cliente.
pub fn sanitize_wol_info(raw: WolInfo) -> Option<WolInfo> {
    let mut macs = Vec::new();
    for item in raw.macs {
        if let Ok(norm) = normalize_wol_mac(&item) {
            if !macs.contains(&norm) {
                macs.push(norm);
                if macs.len() >= MAX_WOL_MACS {
                    break;
                }
            }
        }
    }
    if macs.is_empty() {
        return None;
    }

    let broadcast = raw
        .broadcast
        .as_deref()
        .and_then(parse_safe_lan_broadcast_v4)
        .map(|ip| ip.to_string());

    Some(WolInfo { macs, broadcast })
}

/// Constrói o Magic Packet padrão IEEE/AMD de 102 bytes (`6x 0xFF` + `16x MAC`).
pub fn build_magic_packet(mac: &[u8; 6]) -> [u8; 102] {
    let mut packet = [0xFFu8; 102];
    for i in 0..16 {
        let offset = 6 + i * 6;
        packet[offset..offset + 6].copy_from_slice(mac);
    }
    packet
}

/// Resolve um destino (`IP`, `IP:porta`, `dominio` ou `dominio:porta`) em `SocketAddr`s.
pub async fn resolve_wol_target(raw: &str, default_port: u16) -> Result<Vec<SocketAddr>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("Endereço de destino Wake-on-LAN vazio");
    }

    if let Ok(sock) = trimmed.parse::<SocketAddr>() {
        return Ok(vec![sock]);
    }

    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, default_port)]);
    }

    let host_port = if has_explicit_port(trimmed) {
        trimmed.to_string()
    } else {
        format!("{trimmed}:{default_port}")
    };

    let resolved = tokio::time::timeout(Duration::from_secs(4), lookup_host(&host_port))
        .await
        .map_err(|_| anyhow::anyhow!("Tempo esgotado resolvendo destino WoL '{trimmed}'"))?
        .with_context(|| format!("Não foi possível resolver o destino WoL '{trimmed}'"))?;

    let addrs: Vec<SocketAddr> = resolved.collect();
    if addrs.is_empty() {
        bail!("Nenhum endereço IP encontrado para '{trimmed}'");
    }
    Ok(addrs)
}

fn has_explicit_port(input: &str) -> bool {
    if input.starts_with('[') {
        return input.contains("]:");
    }
    match input.rsplit_once(':') {
        Some((left, right)) => {
            !left.contains(':') && !right.is_empty() && right.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

fn push_unique_addr(list: &mut Vec<SocketAddr>, addr: SocketAddr) {
    if !list.contains(&addr) {
        list.push(addr);
    }
}

/// Dispara rajadas do Magic Packet para todos os MACs informados usando broadcast local e/ou alvo customizado.
pub async fn send_magic_packets(
    macs: &[String],
    subnet_broadcast: Option<&str>,
    custom_target: Option<&str>,
    port: u16,
) -> Result<usize> {
    let effective_port = if port == 0 { DEFAULT_WOL_PORT } else { port };
    let mut targets: Vec<SocketAddr> = Vec::new();

    if let Some(custom) = custom_target.map(str::trim).filter(|s| !s.is_empty()) {
        for addr in resolve_wol_target(custom, effective_port).await? {
            push_unique_addr(&mut targets, addr);
        }
    }

    if let Some(bc_ip) = subnet_broadcast.and_then(parse_safe_lan_broadcast_v4) {
        push_unique_addr(&mut targets, SocketAddr::new(IpAddr::V4(bc_ip), effective_port));
        if effective_port != FALLBACK_WOL_PORT {
            push_unique_addr(
                &mut targets,
                SocketAddr::new(IpAddr::V4(bc_ip), FALLBACK_WOL_PORT),
            );
        }
    }

    push_unique_addr(
        &mut targets,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), effective_port),
    );
    if effective_port != FALLBACK_WOL_PORT {
        push_unique_addr(
            &mut targets,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), FALLBACK_WOL_PORT),
        );
    }

    send_magic_packets_to_addrs(macs, &targets).await
}

/// Envia rajadas do Magic Packet para uma lista explícita de `SocketAddr`s.
pub async fn send_magic_packets_to_addrs(macs: &[String], targets: &[SocketAddr]) -> Result<usize> {
    if targets.is_empty() {
        bail!("Nenhum endereço de destino configurado para Wake-on-LAN");
    }

    let mut seen_macs: Vec<[u8; 6]> = Vec::new();
    for raw in macs {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = parse_wol_mac_bytes(trimmed)?;
        if !seen_macs.contains(&parsed) {
            seen_macs.push(parsed);
            if seen_macs.len() >= MAX_WOL_MACS {
                break;
            }
        }
    }

    if seen_macs.is_empty() {
        bail!("Nenhum endereço MAC configurado para Wake-on-LAN");
    }

    let packets: Vec<[u8; 102]> = seen_macs.iter().map(build_magic_packet).collect();

    let needs_v4 = targets.iter().any(SocketAddr::is_ipv4);
    let needs_v6 = targets.iter().any(SocketAddr::is_ipv6);

    let sock_v4 = if needs_v4 {
        let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
            .await
            .context("Falha ao abrir socket UDP IPv4 para Wake-on-LAN")?;
        let _ = s.set_broadcast(true);
        Some(s)
    } else {
        None
    };

    let sock_v6 = if needs_v6 {
        UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).await.ok()
    } else {
        None
    };

    let mut sent = 0usize;
    let mut last_err: Option<std::io::Error> = None;

    for burst in 0..BURST_COUNT {
        if burst > 0 {
            tokio::time::sleep(BURST_DELAY).await;
        }
        for packet in &packets {
            for target in targets {
                let res = match target {
                    SocketAddr::V4(_) => match &sock_v4 {
                        Some(s) => s.send_to(packet, target).await,
                        None => continue,
                    },
                    SocketAddr::V6(_) => match &sock_v6 {
                        Some(s) => s.send_to(packet, target).await,
                        None => continue,
                    },
                };
                match res {
                    Ok(_) => sent += 1,
                    Err(err) => last_err = Some(err),
                }
            }
        }
    }

    if sent == 0 {
        if let Some(err) = last_err {
            bail!("Falha ao enviar pacote Wake-on-LAN na rede: {err}");
        }
        bail!("Não foi possível transmitir nenhum pacote Wake-on-LAN");
    }

    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_packet_layout_is_rfc_compliant() {
        let mac = parse_wol_mac_bytes("aa-bb-cc-11-22-33").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0x11, 0x22, 0x33]);
        assert_eq!(
            normalize_wol_mac("aa.bb.cc.11.22.33").unwrap(),
            "AA:BB:CC:11:22:33"
        );

        let pkt = build_magic_packet(&mac);
        assert_eq!(pkt.len(), 102);
        assert_eq!(&pkt[..6], &[0xFF; 6]);
        for i in 0..16 {
            let start = 6 + i * 6;
            assert_eq!(&pkt[start..start + 6], &mac);
        }
    }

    #[test]
    fn rejects_invalid_zero_and_broadcast_macs() {
        assert!(normalize_wol_mac("invalid").is_err());
        assert!(normalize_wol_mac("00:00:00:00:00:00").is_err());
        assert!(normalize_wol_mac("ff:ff:ff:ff:ff:ff").is_err());
    }

    #[test]
    fn sanitizes_untrusted_wol_info_blocking_dns_beacon_and_public_ips() {
        let untrusted = WolInfo {
            macs: vec![
                "lixo".into(),
                "00:00:00:00:00:00".into(),
                "aa:bb:cc:dd:ee:01".into(),
                "AA-BB-CC-DD-EE-01".into(),
                "aa:bb:cc:dd:ee:02".into(),
                "aa:bb:cc:dd:ee:03".into(),
                "aa:bb:cc:dd:ee:04".into(),
                "aa:bb:cc:dd:ee:05".into(),
                "aa:bb:cc:dd:ee:06".into(),
                "aa:bb:cc:dd:ee:07".into(),
            ],
            broadcast: Some("tracker.evil.example.com".into()),
        };
        let clean = sanitize_wol_info(untrusted).unwrap();
        assert_eq!(clean.macs.len(), MAX_WOL_MACS);
        assert_eq!(clean.macs[0], "AA:BB:CC:DD:EE:01");
        assert!(clean.broadcast.is_none());

        assert!(parse_safe_lan_broadcast_v4("8.8.8.8").is_none());
        assert!(parse_safe_lan_broadcast_v4("1.1.1.1").is_none());
        assert_eq!(
            parse_safe_lan_broadcast_v4("192.168.18.255").map(|ip| ip.to_string()),
            Some("192.168.18.255".to_string())
        );
    }

    #[tokio::test]
    async fn udp_loopback_receives_magic_packet_burst() {
        let listener = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let target = listener.local_addr().unwrap();

        let macs = vec![
            "AA:BB:CC:DD:EE:01".to_string(),
            "aa-bb-cc-dd-ee-02".to_string(),
        ];
        let count = send_magic_packets_to_addrs(&macs, &[target]).await.unwrap();
        assert_eq!(count, BURST_COUNT * 2);

        let mut buf = [0u8; 128];
        let (len1, _) = listener.recv_from(&mut buf).await.unwrap();
        assert_eq!(len1, 102);
        assert_eq!(&buf[..6], &[0xFF; 6]);
        assert_eq!(&buf[6..12], &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]);

        let (len2, _) = listener.recv_from(&mut buf).await.unwrap();
        assert_eq!(len2, 102);
        assert_eq!(&buf[6..12], &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x02]);
    }
}
