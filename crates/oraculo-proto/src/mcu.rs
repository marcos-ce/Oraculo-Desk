use crate::pairing::{percent_decode, percent_encode};
use crate::wol::{
    DEFAULT_WOL_PORT, MAX_WOL_MACS, normalize_wol_mac, parse_safe_lan_broadcast_v4,
};
use anyhow::{Context, Result, bail};
use ring::hmac::{HMAC_SHA256, Key, sign, verify};
use ring::rand::{SecureRandom, SystemRandom};
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, lookup_host};
use tokio_rustls::TlsConnector;

/// Prefixo padrão do QR Code e link de pareamento de pontes MCU (Cardputer / ESP32).
pub const MCU_URI_PREFIX: &str = "oraculo://mcu-bridge?";

/// Broker MQTT público com TLS padrão caso o usuário não configure um privado.
pub const DEFAULT_MCU_BROKER_HOST: &str = "broker.emqx.io";

/// Porta padrão obrigatória de MQTT sobre TLS (nunca usamos MQTT 1883 em texto puro).
pub const DEFAULT_MCU_MQTT_TLS_PORT: u16 = 8883;

/// Prefixo base dos tópicos MQTT da ponte MCU v1.
pub const MCU_TOPIC_PREFIX: &str = "oraculo/v1/mcu";

/// Limite máximo para pacotes MQTT recebidos do broker (protege memória no celular e na MCU).
const MAX_MQTT_PACKET_LEN: usize = 16 * 1024;

/// Teto seguro para drenar e descartar pacotes MQTT retidos acima de `MAX_MQTT_PACKET_LEN` sem encerrar a sessão.
const MAX_MQTT_DRAIN_LEN: usize = 2 * 1024 * 1024;

/// Tempo limite por tentativa completa (TLS + MQTT + Challenge-Response + ACK) com a ponte MCU.
const MCU_OPERATION_TIMEOUT: Duration = Duration::from_secs(7);

const B64URL_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Configuração validada de uma ponte MCU (Cardputer / ESP32) pareada via MQTT TLS.
#[derive(Clone, PartialEq, Eq)]
pub struct McuBridgeConfig {
    pub bridge_id: String,
    pub secret: [u8; 32],
    pub broker_host: String,
    pub broker_port: u16,
    pub name: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl fmt::Debug for McuBridgeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McuBridgeConfig")
            .field("bridge_id", &self.bridge_id)
            .field("secret", &"<redacted>")
            .field("broker_host", &self.broker_host)
            .field("broker_port", &self.broker_port)
            .field("name", &self.name)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Resultado de um ping autenticado (`Challenge-Response`) até uma ponte MCU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McuPingResult {
    pub bridge_name: String,
    pub rtt_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct McuPacket {
    op: String,
    req: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nonce: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ct: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sig: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sent: Option<u32>,
}

/// Codifica um segredo de 32 bytes em Base64url sem `=` (43 caracteres compactos para QR Code).
pub fn encode_mcu_secret_b64url(secret: &[u8; 32]) -> String {
    encode_base64url_no_pad(secret)
}

/// Decodifica um segredo de ponte MCU aceitando Base64url sem padding (43 chars) ou Hex (64 chars).
pub fn decode_mcu_secret(raw: &str) -> Result<[u8; 32]> {
    let trimmed = raw.trim();
    if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        let decoded = hex::decode(trimmed).context("Segredo hexadecimal inválido")?;
        return decoded
            .try_into()
            .map_err(|_| anyhow::anyhow!("Segredo MCU deve ter exatamente 32 bytes"));
    }
    let decoded = decode_base64url_no_pad(trimmed)?;
    decoded
        .try_into()
        .map_err(|_| anyhow::anyhow!("Segredo MCU deve ter exatamente 32 bytes (256 bits)"))
}

/// Sanitiza o nome de exibição da ponte MCU removendo caracteres de controle e limitando tamanho.
pub fn sanitize_mcu_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(48)
        .collect();
    if cleaned.is_empty() {
        "Cardputer / ESP32".to_string()
    } else {
        cleaned
    }
}

/// Valida o `bridge_id` da ponte MCU (16 a 32 dígitos hexadecimais normalizados em minúsculas).
pub fn validate_mcu_bridge_id(raw: &str) -> Result<String> {
    let clean = raw.trim().to_ascii_lowercase();
    if !(16..=32).contains(&clean.len()) || !clean.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        bail!("Identificador da ponte MCU inválido (esperado 16 a 32 caracteres hexadecimais)");
    }
    Ok(clean)
}

/// Valida o hostname de um broker MQTT TLS bloqueando localhost, endereços especiais e caracteres inseguros.
pub fn validate_broker_host(raw: &str) -> Result<String> {
    let clean = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if clean.is_empty() || clean.len() > 253 {
        bail!("Endereço do broker MQTT inválido");
    }
    if clean.bytes().any(|b| {
        b.is_ascii_control()
            || b.is_ascii_whitespace()
            || matches!(b, b'/' | b'\\' | b'@' | b'?' | b'#' | b':')
    }) {
        bail!("Hostname do broker MQTT contém caracteres inválidos");
    }
    if clean == "localhost" || clean.ends_with(".localhost") {
        bail!("Broker MQTT apontando para localhost é bloqueado por segurança");
    }
    if let Ok(ip) = clean.parse::<IpAddr>() {
        if is_disallowed_broker_ip(ip) {
            bail!("Endereço IP de broker MQTT bloqueado por segurança ({clean})");
        }
    }
    Ok(clean)
}

fn is_disallowed_broker_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_unspecified() || v4.is_link_local() || v4.is_multicast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_disallowed_broker_ip(IpAddr::V4(v4)))
        }
    }
}

/// Codifica a configuração de uma ponte MCU em uma URI compacta `oraculo://mcu-bridge?...`.
pub fn encode_mcu_pairing_uri(cfg: &McuBridgeConfig) -> String {
    let secret_b64 = encode_mcu_secret_b64url(&cfg.secret);
    let mut uri = format!(
        "{MCU_URI_PREFIX}id={}&s={}&b={}&n={}",
        percent_encode(&cfg.bridge_id),
        percent_encode(&secret_b64),
        percent_encode(&cfg.broker_host),
        percent_encode(&sanitize_mcu_name(&cfg.name)),
    );
    if cfg.broker_port != DEFAULT_MCU_MQTT_TLS_PORT {
        uri.push_str(&format!("&p={}", cfg.broker_port));
    }
    if let Some(user) = cfg.username.as_deref().filter(|s| !s.is_empty()) {
        uri.push_str("&u=");
        uri.push_str(&percent_encode(user));
        if let Some(pass) = cfg.password.as_deref().filter(|s| !s.is_empty()) {
            uri.push_str("&pw=");
            uri.push_str(&percent_encode(pass));
        }
    }
    uri
}

/// Decodifica e valida estritamente uma URI `oraculo://mcu-bridge?...`.
pub fn decode_mcu_pairing_uri(uri: &str) -> Result<McuBridgeConfig> {
    let trimmed = uri.trim();
    let query = trimmed
        .strip_prefix(MCU_URI_PREFIX)
        .or_else(|| trimmed.strip_prefix("ORACULO://MCU-BRIDGE?"))
        .ok_or_else(|| anyhow::anyhow!("URI MCU inválida: esperado prefixo {MCU_URI_PREFIX}"))?;

    let mut bridge_id = None;
    let mut secret = None;
    let mut broker_host = None;
    let mut broker_port = DEFAULT_MCU_MQTT_TLS_PORT;
    let mut name = None;
    let mut username = None;
    let mut password = None;

    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = percent_decode(v)?;
            match k {
                "id" => bridge_id = Some(validate_mcu_bridge_id(&decoded)?),
                "s" | "secret" => secret = Some(decode_mcu_secret(&decoded)?),
                "b" | "broker" => broker_host = Some(validate_broker_host(&decoded)?),
                "p" | "port" if !decoded.is_empty() => {
                    let parsed: u16 = decoded
                        .parse()
                        .context("Porta TLS MQTT inválida na URI MCU")?;
                    if parsed == 0 {
                        bail!("Porta TLS MQTT inválida na URI MCU");
                    }
                    broker_port = parsed;
                }
                "n" | "name" if !decoded.is_empty() => name = Some(sanitize_mcu_name(&decoded)),
                "u" | "user" if !decoded.is_empty() => {
                    if decoded.len() > 128 || decoded.chars().any(|c| c.is_control()) {
                        bail!("Usuário MQTT inválido na URI MCU");
                    }
                    username = Some(decoded);
                }
                "pw" | "pass" if !decoded.is_empty() => {
                    if decoded.len() > 128 || decoded.chars().any(|c| c.is_control()) {
                        bail!("Senha MQTT inválida na URI MCU");
                    }
                    password = Some(decoded);
                }
                _ => {}
            }
        }
    }

    Ok(McuBridgeConfig {
        bridge_id: bridge_id.ok_or_else(|| anyhow::anyhow!("Parâmetro 'id' ausente na URI MCU"))?,
        secret: secret.ok_or_else(|| anyhow::anyhow!("Parâmetro 's' (segredo) ausente na URI MCU"))?,
        broker_host: broker_host.unwrap_or_else(|| DEFAULT_MCU_BROKER_HOST.to_string()),
        broker_port,
        name: name.unwrap_or_else(|| "Cardputer / ESP32".to_string()),
        username,
        password,
    })
}

/// Retorna os tópicos MQTT `(cmd, evt)` canônicos para uma ponte MCU.
pub fn mcu_topics(bridge_id: &str) -> (String, String) {
    (
        format!("{MCU_TOPIC_PREFIX}/{bridge_id}/cmd"),
        format!("{MCU_TOPIC_PREFIX}/{bridge_id}/evt"),
    )
}

/// Executa um ping autenticado (`Challenge-Response HMAC-SHA256`) até a ponte MCU via MQTT TLS.
pub async fn ping_mcu_bridge(cfg: &McuBridgeConfig) -> Result<McuPingResult> {
    tokio::time::timeout(MCU_OPERATION_TIMEOUT, async {
        let started = Instant::now();
        let tls = connect_mqtt_tls(cfg).await?;
        let mut res = ping_mcu_over_stream(tls, cfg).await?;
        res.rtt_ms = started.elapsed().as_millis() as u64;
        Ok(res)
    })
    .await
    .map_err(|_| anyhow::anyhow!("Tempo esgotado aguardando resposta da ponte MCU no broker MQTT"))?
}

/// Dispara Wake-on-LAN através de uma ponte MCU usando TLS + Challenge-Response + payload cifrado + ACK verificado.
pub async fn relay_wake_via_mcu(
    cfg: &McuBridgeConfig,
    macs: &[String],
    subnet_broadcast: Option<&str>,
    port: u16,
) -> Result<u32> {
    let payload_plain = format_mcu_wake_plaintext(macs, subnet_broadcast, port)?;
    tokio::time::timeout(MCU_OPERATION_TIMEOUT, async {
        let tls = connect_mqtt_tls(cfg).await?;
        wake_mcu_over_stream_with_plain(tls, cfg, &payload_plain).await
    })
    .await
    .map_err(|_| anyhow::anyhow!("Tempo esgotado aguardando confirmação (ACK) da ponte MCU"))?
}

/// Formata e valida o payload em claro (`<port>|<broadcast ou ->|<MAC1,MAC2,...>`) antes da cifragem.
pub fn format_mcu_wake_plaintext(
    macs: &[String],
    subnet_broadcast: Option<&str>,
    port: u16,
) -> Result<String> {
    let mut clean_macs = Vec::new();
    for raw in macs {
        if raw.trim().is_empty() {
            continue;
        }
        let norm = normalize_wol_mac(raw)?;
        if !clean_macs.contains(&norm) {
            clean_macs.push(norm);
            if clean_macs.len() >= MAX_WOL_MACS {
                break;
            }
        }
    }
    if clean_macs.is_empty() {
        bail!("Nenhum endereço MAC válido para enviar à ponte MCU");
    }

    let effective_port = if port == 0 { DEFAULT_WOL_PORT } else { port };
    let bc = subnet_broadcast
        .and_then(parse_safe_lan_broadcast_v4)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "-".to_string());

    Ok(format!("{effective_port}|{bc}|{}", clean_macs.join(",")))
}

/// Testa o fluxo de ping sobre qualquer stream `AsyncRead + AsyncWrite` já conectado (TLS em produção ou TCP em testes).
pub async fn ping_mcu_over_stream<S>(mut stream: S, cfg: &McuBridgeConfig) -> Result<McuPingResult>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let started = Instant::now();
    let (_cmd_topic, evt_topic) = mcu_topics(&cfg.bridge_id);
    mqtt_handshake(&mut stream, cfg, &evt_topic).await?;

    let (req_hex, chal) = perform_challenge_step(&mut stream, cfg).await?;
    let _ = req_hex;
    let _ = stream.write_all(&mqtt_disconnect_packet()).await;
    let _ = stream.flush().await;

    Ok(McuPingResult {
        bridge_name: chal
            .name
            .as_deref()
            .map(sanitize_mcu_name)
            .unwrap_or_else(|| cfg.name.clone()),
        rtt_ms: started.elapsed().as_millis() as u64,
    })
}

/// Dispara WoL autenticado sobre qualquer stream `AsyncRead + AsyncWrite` já conectado.
pub async fn wake_mcu_over_stream<S>(
    stream: S,
    cfg: &McuBridgeConfig,
    macs: &[String],
    subnet_broadcast: Option<&str>,
    port: u16,
) -> Result<u32>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let plain = format_mcu_wake_plaintext(macs, subnet_broadcast, port)?;
    wake_mcu_over_stream_with_plain(stream, cfg, &plain).await
}

async fn wake_mcu_over_stream_with_plain<S>(
    mut stream: S,
    cfg: &McuBridgeConfig,
    payload_plain: &str,
) -> Result<u32>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (cmd_topic, evt_topic) = mcu_topics(&cfg.bridge_id);
    mqtt_handshake(&mut stream, cfg, &evt_topic).await?;

    let (req_hex, chal) = perform_challenge_step(&mut stream, cfg).await?;
    let nonce_hex = chal
        .nonce
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Desafio da ponte MCU sem nonce"))?;

    let ct_bytes = apply_hmac_keystream(&cfg.secret, nonce_hex, payload_plain.as_bytes());
    let ct_hex = hex::encode(ct_bytes);
    let wake_sig = compute_wake_sig(&cfg.secret, &cfg.bridge_id, &req_hex, nonce_hex, &ct_hex);

    let wake_pkt = McuPacket {
        op: "wake".to_string(),
        req: req_hex.clone(),
        nonce: Some(nonce_hex.to_string()),
        ct: Some(ct_hex),
        sig: Some(wake_sig),
        name: None,
        ok: None,
        sent: None,
    };
    let wake_json = serde_json::to_vec(&wake_pkt)?;
    stream
        .write_all(&mqtt_publish_packet(&cmd_topic, &wake_json))
        .await?;
    stream.flush().await?;

    loop {
        let pkt = wait_for_evt_json(&mut stream, &evt_topic).await?;
        if pkt.req != req_hex || pkt.op != "ack" {
            continue;
        }
        let ack_nonce = pkt.nonce.as_deref().unwrap_or("");
        if ack_nonce != nonce_hex {
            // Ignora respostas forjadas ou de outro desafio no broker público.
            continue;
        }
        let sent = pkt.sent.unwrap_or(0);
        let sig_hex = pkt.sig.as_deref().unwrap_or("");
        let expected_input = ack_sig_input(&cfg.bridge_id, &req_hex, nonce_hex, sent);
        if !verify_hmac_hex(&cfg.secret, expected_input.as_bytes(), sig_hex) {
            // Ignora pacote com assinatura HMAC inválida (evita DoS por spoofing em broker público).
            continue;
        }
        if pkt.ok != Some(true) || sent == 0 {
            bail!("A ponte MCU recusou ou não conseguiu enviar o Magic Packet");
        }
        let _ = stream.write_all(&mqtt_disconnect_packet()).await;
        let _ = stream.flush().await;
        return Ok(sent);
    }
}

async fn perform_challenge_step<S>(stream: &mut S, cfg: &McuBridgeConfig) -> Result<(String, McuPacket)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (cmd_topic, evt_topic) = mcu_topics(&cfg.bridge_id);
    let mut req_bytes = [0u8; 8];
    SystemRandom::new()
        .fill(&mut req_bytes)
        .map_err(|_| anyhow::anyhow!("Falha ao gerar req aleatório"))?;
    let req_hex = hex::encode(req_bytes);

    let chal_req = McuPacket {
        op: "chal".to_string(),
        req: req_hex.clone(),
        nonce: None,
        ct: None,
        sig: None,
        name: None,
        ok: None,
        sent: None,
    };
    let json = serde_json::to_vec(&chal_req)?;
    stream
        .write_all(&mqtt_publish_packet(&cmd_topic, &json))
        .await?;
    stream.flush().await?;

    loop {
        let pkt = wait_for_evt_json(stream, &evt_topic).await?;
        if pkt.req != req_hex || pkt.op != "chal" {
            continue;
        }
        let Some(nonce_hex) = pkt
            .nonce
            .as_deref()
            .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            continue;
        };
        let Some(sig_hex) = pkt.sig.as_deref() else {
            continue;
        };
        let input = chal_sig_input(&cfg.bridge_id, &req_hex, nonce_hex);
        if !verify_hmac_hex(&cfg.secret, input.as_bytes(), sig_hex) {
            // Ignora desafio forjado no broker público e segue aguardando a resposta autêntica da MCU.
            continue;
        }
        return Ok((req_hex, pkt));
    }
}

async fn connect_mqtt_tls(
    cfg: &McuBridgeConfig,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let host = validate_broker_host(&cfg.broker_host)?;
    let addrs = resolve_safe_broker_addrs(&host, cfg.broker_port).await?;

    let mut last_err = None;
    let mut tcp_opt = None;
    for addr in addrs {
        match tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                tcp_opt = Some(stream);
                break;
            }
            Ok(Err(e)) => last_err = Some(e.to_string()),
            Err(_) => last_err = Some("timeout TCP".to_string()),
        }
    }
    let tcp = tcp_opt.ok_or_else(|| {
        anyhow::anyhow!(
            "Falha ao conectar ao broker MQTT {host}:{}: {}",
            cfg.broker_port,
            last_err.unwrap_or_else(|| "sem rota".to_string())
        )
    })?;
    let _ = tcp.set_nodelay(true);

    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls_cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("Falha ao configurar versões TLS")?
        .with_root_certificates(roots)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(tls_cfg));
    let server_name = ServerName::try_from(host)
        .map_err(|_| anyhow::anyhow!("Nome de servidor TLS inválido"))?;
    connector
        .connect(server_name, tcp)
        .await
        .context("Falha na verificação TLS do broker MQTT")
}

async fn resolve_safe_broker_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_disallowed_broker_ip(ip) {
            bail!("IP do broker MQTT bloqueado por segurança ({ip})");
        }
        return Ok(vec![SocketAddr::new(ip, port)]);
    }

    let resolved = lookup_host((host, port))
        .await
        .with_context(|| format!("Não foi possível resolver DNS do broker '{host}'"))?;

    let safe: Vec<SocketAddr> = resolved
        .filter(|addr| !is_disallowed_broker_ip(addr.ip()))
        .collect();
    if safe.is_empty() {
        bail!("DNS do broker '{host}' não retornou nenhum IP seguro/roteável");
    }
    Ok(safe)
}

async fn mqtt_handshake<S>(stream: &mut S, cfg: &McuBridgeConfig, evt_topic: &str) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut cid_rand = [0u8; 6];
    SystemRandom::new()
        .fill(&mut cid_rand)
        .map_err(|_| anyhow::anyhow!("Falha ao gerar ClientId MQTT"))?;
    let client_id = format!("ora-{}", hex::encode(cid_rand));

    let connect_bytes = mqtt_connect_packet(
        &client_id,
        cfg.username.as_deref(),
        cfg.password.as_deref(),
    )?;
    stream.write_all(&connect_bytes).await?;
    stream.flush().await?;

    let (pkt_type, _flags, body) = read_mqtt_packet(stream).await?;
    if pkt_type != 2 || body.len() < 2 {
        bail!("Resposta inesperada do broker MQTT ao CONNECT (tipo {pkt_type})");
    }
    if body[1] != 0 {
        bail!("Broker MQTT recusou conexão (código CONNACK {})", body[1]);
    }

    let sub_bytes = mqtt_subscribe_packet(1, evt_topic, 1)?;
    stream.write_all(&sub_bytes).await?;
    stream.flush().await?;

    loop {
        let (sub_type, _flags, sub_body) = read_mqtt_packet(stream).await?;
        if sub_type == 9 {
            if sub_body.len() < 3 || sub_body[0] != 0 || sub_body[1] != 1 {
                bail!("SUBACK inválido do broker MQTT");
            }
            if sub_body[2] == 0x80 {
                bail!("Broker MQTT negou inscrição no tópico da ponte MCU");
            }
            break;
        }
    }

    Ok(())
}

async fn wait_for_evt_json<S>(stream: &mut S, expected_topic: &str) -> Result<McuPacket>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let (pkt_type, flags, body) = read_mqtt_packet(stream).await?;
        match pkt_type {
            3 => {
                let qos = (flags >> 1) & 0x03;
                if body.len() < 2 {
                    continue;
                }
                let tlen = u16::from_be_bytes([body[0], body[1]]) as usize;
                let mut pos = 2 + tlen;
                if pos > body.len() {
                    continue;
                }
                let topic = std::str::from_utf8(&body[2..pos]).unwrap_or("");
                if qos > 0 {
                    if pos + 2 > body.len() {
                        continue;
                    }
                    let pid = u16::from_be_bytes([body[pos], body[pos + 1]]);
                    pos += 2;
                    if qos == 1 {
                        let _ = stream.write_all(&mqtt_puback_packet(pid)).await;
                        let _ = stream.flush().await;
                    }
                }
                if topic != expected_topic {
                    continue;
                }
                if let Ok(pkt) = serde_json::from_slice::<McuPacket>(&body[pos..]) {
                    return Ok(pkt);
                }
            }
            4 | 13 => continue,
            14 => bail!("Broker MQTT encerrou a conexão"),
            _ => continue,
        }
    }
}

/// Cifra ou decifra bytes usando fluxo de chave derivado via `HMAC-SHA256(secret, "oraculo-mcu-v1|stream|nonce|idx")`.
pub fn apply_hmac_keystream(secret: &[u8; 32], nonce_hex: &str, data: &[u8]) -> Vec<u8> {
    let key = Key::new(HMAC_SHA256, secret);
    let mut out = Vec::with_capacity(data.len());
    for (block_idx, chunk) in data.chunks(32).enumerate() {
        let label = format!("oraculo-mcu-v1|stream|{nonce_hex}|{block_idx}");
        let tag = sign(&key, label.as_bytes());
        let pad = tag.as_ref();
        for (i, &b) in chunk.iter().enumerate() {
            out.push(b ^ pad[i]);
        }
    }
    out
}

pub fn chal_sig_input(bridge_id: &str, req_hex: &str, nonce_hex: &str) -> String {
    format!("oraculo-mcu-v1|chal|{bridge_id}|{req_hex}|{nonce_hex}")
}

pub fn wake_sig_input(bridge_id: &str, req_hex: &str, nonce_hex: &str, ct_hex: &str) -> String {
    format!("oraculo-mcu-v1|wake|{bridge_id}|{req_hex}|{nonce_hex}|{ct_hex}")
}

pub fn ack_sig_input(bridge_id: &str, req_hex: &str, nonce_hex: &str, sent: u32) -> String {
    format!("oraculo-mcu-v1|ack|{bridge_id}|{req_hex}|{nonce_hex}|{sent}")
}

pub fn compute_chal_sig(secret: &[u8; 32], bridge_id: &str, req_hex: &str, nonce_hex: &str) -> String {
    hmac_sha256_hex(secret, chal_sig_input(bridge_id, req_hex, nonce_hex).as_bytes())
}

pub fn compute_wake_sig(
    secret: &[u8; 32],
    bridge_id: &str,
    req_hex: &str,
    nonce_hex: &str,
    ct_hex: &str,
) -> String {
    hmac_sha256_hex(
        secret,
        wake_sig_input(bridge_id, req_hex, nonce_hex, ct_hex).as_bytes(),
    )
}

pub fn compute_ack_sig(
    secret: &[u8; 32],
    bridge_id: &str,
    req_hex: &str,
    nonce_hex: &str,
    sent: u32,
) -> String {
    hmac_sha256_hex(
        secret,
        ack_sig_input(bridge_id, req_hex, nonce_hex, sent).as_bytes(),
    )
}

pub fn hmac_sha256_hex(secret: &[u8; 32], msg: &[u8]) -> String {
    let key = Key::new(HMAC_SHA256, secret);
    hex::encode(sign(&key, msg).as_ref())
}

pub fn verify_hmac_hex(secret: &[u8; 32], msg: &[u8], expected_hex: &str) -> bool {
    let Ok(expected) = hex::decode(expected_hex.trim()) else {
        return false;
    };
    if expected.len() != 32 {
        return false;
    }
    let key = Key::new(HMAC_SHA256, secret);
    verify(&key, msg, &expected).is_ok()
}

fn push_mqtt_str(buf: &mut Vec<u8>, s: &str) -> Result<()> {
    let len = u16::try_from(s.len()).context("Campo MQTT excede 65535 bytes")?;
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(s.as_bytes());
    Ok(())
}

fn push_remaining_len(buf: &mut Vec<u8>, mut len: usize) -> Result<()> {
    if len > 268_435_455 {
        bail!("Pacote MQTT excede limite da especificação");
    }
    loop {
        let mut digit = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            digit |= 0x80;
        }
        buf.push(digit);
        if len == 0 {
            break;
        }
    }
    Ok(())
}

fn wrap_mqtt_packet(header: u8, body: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(5 + body.len());
    out.push(header);
    push_remaining_len(&mut out, body.len())?;
    out.extend_from_slice(body);
    Ok(out)
}

pub fn mqtt_connect_packet(
    client_id: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(64);
    push_mqtt_str(&mut body, "MQTT")?;
    body.push(0x04); // MQTT 3.1.1

    // [MQTT-3.1.2-22]: Password Flag só pode ser 1 quando User Name Flag também for 1.
    let clean_user = username.filter(|s| !s.is_empty());
    let clean_pass = if clean_user.is_some() {
        password.filter(|s| !s.is_empty())
    } else {
        None
    };

    let mut flags: u8 = 0x02; // Clean Session
    if clean_user.is_some() {
        flags |= 0x80;
    }
    if clean_pass.is_some() {
        flags |= 0x40;
    }
    body.push(flags);
    body.extend_from_slice(&30u16.to_be_bytes()); // KeepAlive 30s

    push_mqtt_str(&mut body, client_id)?;
    if let Some(u) = clean_user {
        push_mqtt_str(&mut body, u)?;
    }
    if let Some(p) = clean_pass {
        push_mqtt_str(&mut body, p)?;
    }

    wrap_mqtt_packet(0x10, &body)
}

pub fn mqtt_subscribe_packet(packet_id: u16, topic: &str, qos: u8) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(4 + topic.len());
    body.extend_from_slice(&packet_id.to_be_bytes());
    push_mqtt_str(&mut body, topic)?;
    body.push(qos.min(2));
    wrap_mqtt_packet(0x82, &body)
}

pub fn mqtt_publish_packet(topic: &str, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(2 + topic.len() + payload.len());
    let _ = push_mqtt_str(&mut body, topic);
    body.extend_from_slice(payload);
    wrap_mqtt_packet(0x30, &body).unwrap_or_default()
}

pub fn mqtt_puback_packet(packet_id: u16) -> [u8; 4] {
    let [hi, lo] = packet_id.to_be_bytes();
    [0x40, 0x02, hi, lo]
}

pub fn mqtt_disconnect_packet() -> [u8; 2] {
    [0xE0, 0x00]
}

pub async fn read_mqtt_packet<R>(reader: &mut R) -> Result<(u8, u8, Vec<u8>)>
where
    R: AsyncRead + Unpin,
{
    let header = reader
        .read_u8()
        .await
        .context("Conexão MQTT encerrada durante leitura do cabeçalho")?;
    let pkt_type = header >> 4;
    let flags = header & 0x0F;

    let mut multiplier = 1usize;
    let mut remaining_len = 0usize;
    let mut count = 0u8;
    loop {
        if count >= 4 {
            bail!("Comprimento variável MQTT malformado");
        }
        let encoded = reader
            .read_u8()
            .await
            .context("Conexão MQTT encerrada ao ler tamanho do pacote")?;
        remaining_len += ((encoded & 0x7F) as usize) * multiplier;
        if remaining_len > MAX_MQTT_DRAIN_LEN {
            bail!("Pacote MQTT excede limite máximo de segurança");
        }
        count += 1;
        if (encoded & 0x80) == 0 {
            break;
        }
        multiplier *= 128;
    }

    if remaining_len > MAX_MQTT_PACKET_LEN {
        // Drena e descarta pacotes retidos/enormes no broker público sem abortar a sessão TLS.
        let mut left = remaining_len;
        let mut scratch = [0u8; 4096];
        while left > 0 {
            let chunk = left.min(scratch.len());
            reader
                .read_exact(&mut scratch[..chunk])
                .await
                .context("Conexão MQTT encerrada ao drenar pacote excedente")?;
            left -= chunk;
        }
        return Ok((0, 0, Vec::new()));
    }

    let mut body = vec![0u8; remaining_len];
    if remaining_len > 0 {
        reader
            .read_exact(&mut body)
            .await
            .context("Conexão MQTT encerrada antes do fim do payload")?;
    }
    Ok((pkt_type, flags, body))
}

fn encode_base64url_no_pad(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() * 4 + 2) / 3);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64URL_TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64URL_TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() >= 2 {
            out.push(B64URL_TABLE[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() >= 3 {
            out.push(B64URL_TABLE[(triple & 0x3F) as usize] as char);
        }
    }
    out
}

fn b64url_val(byte: u8) -> Result<u32> {
    match byte {
        b'A'..=b'Z' => Ok((byte - b'A') as u32),
        b'a'..=b'z' => Ok((byte - b'a' + 26) as u32),
        b'0'..=b'9' => Ok((byte - b'0' + 52) as u32),
        b'-' => Ok(62),
        b'_' => Ok(63),
        _ => bail!("Caractere Base64url inválido"),
    }
}

fn decode_base64url_no_pad(raw: &str) -> Result<Vec<u8>> {
    let trimmed = raw.trim().trim_end_matches('=');
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() || bytes.len() % 4 == 1 {
        bail!("Comprimento Base64url inválido");
    }

    let mut out = Vec::with_capacity((bytes.len() * 3) / 4);
    for chunk in bytes.chunks(4) {
        let v0 = b64url_val(chunk[0])?;
        let v1 = b64url_val(chunk[1])?;
        if chunk.len() == 2 {
            out.push(((v0 << 2) | (v1 >> 4)) as u8);
        } else if chunk.len() == 3 {
            let v2 = b64url_val(chunk[2])?;
            out.push(((v0 << 2) | (v1 >> 4)) as u8);
            out.push((((v1 & 0x0F) << 4) | (v2 >> 2)) as u8);
        } else {
            let v2 = b64url_val(chunk[2])?;
            let v3 = b64url_val(chunk[3])?;
            out.push(((v0 << 2) | (v1 >> 4)) as u8);
            out.push((((v1 & 0x0F) << 4) | (v2 >> 2)) as u8);
            out.push((((v2 & 0x03) << 6) | v3) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn roundtrip_compact_mcu_uri_and_security_guards() {
        let secret = [0x5Au8; 32];
        let cfg = McuBridgeConfig {
            bridge_id: "A1B2C3D4E5F60718".to_string(),
            secret,
            broker_host: "broker.emqx.io".to_string(),
            broker_port: DEFAULT_MCU_MQTT_TLS_PORT,
            name: "Cardputer".to_string(),
            username: None,
            password: None,
        };
        let uri = encode_mcu_pairing_uri(&cfg);
        assert!(uri.len() <= 130, "QR URI deve caber em versão <=6: {uri}");

        let decoded = decode_mcu_pairing_uri(&uri).unwrap();
        assert_eq!(decoded.bridge_id, "a1b2c3d4e5f60718");
        assert_eq!(decoded.secret, secret);
        assert_eq!(decoded.broker_host, "broker.emqx.io");
        assert_eq!(decoded.broker_port, 8883);
        assert_eq!(decoded.name, "Cardputer");
        assert!(!format!("{decoded:?}").contains("5a5a"));

        // Retrocompatibilidade com segredo em 64 hex chars.
        let hex_uri = format!(
            "{MCU_URI_PREFIX}id=0011223344556677&s={}&b=broker.hivemq.com&p=8884&u=alice&pw=segredo",
            hex::encode(secret)
        );
        let decoded_hex = decode_mcu_pairing_uri(&hex_uri).unwrap();
        assert_eq!(decoded_hex.secret, secret);
        assert_eq!(decoded_hex.broker_port, 8884);
        assert_eq!(decoded_hex.username.as_deref(), Some("alice"));
        assert_eq!(decoded_hex.password.as_deref(), Some("segredo"));

        // Bloqueios anti-SSRF e validação estrita.
        assert!(decode_mcu_pairing_uri("https://example.com").is_err());
        assert!(validate_broker_host("localhost").is_err());
        assert!(validate_broker_host("127.0.0.1").is_err());
        assert!(validate_broker_host("169.254.169.254").is_err());
        assert!(validate_mcu_bridge_id("curto").is_err());
    }

    #[test]
    fn keystream_roundtrip_and_hmac_verification() {
        let secret = *blake3::hash(b"oraculo-mcu-test-key").as_bytes();
        let nonce = "0123456789abcdef0123456789abcdef";
        let plain = "9|192.168.18.255|AA:BB:CC:DD:EE:FF,11:22:33:44:55:66";

        let ct = apply_hmac_keystream(&secret, nonce, plain.as_bytes());
        assert_ne!(ct.as_slice(), plain.as_bytes());
        let recovered = apply_hmac_keystream(&secret, nonce, &ct);
        assert_eq!(String::from_utf8(recovered).unwrap(), plain);

        let ct_hex = hex::encode(&ct);
        let sig = compute_wake_sig(&secret, "0011223344556677", "deadbeef01020304", nonce, &ct_hex);
        let msg = wake_sig_input("0011223344556677", "deadbeef01020304", nonce, &ct_hex);
        assert!(verify_hmac_hex(&secret, msg.as_bytes(), &sig));

        // Qualquer alteração no ciphertext ou req falha verificação constant-time.
        let tampered_msg = wake_sig_input("0011223344556677", "deadbeef01020305", nonce, &ct_hex);
        assert!(!verify_hmac_hex(&secret, tampered_msg.as_bytes(), &sig));
    }

    #[tokio::test]
    async fn full_mqtt_challenge_response_ack_and_replay_protection() {
        let secret = *blake3::hash(b"test-mcu-bridge-secret").as_bytes();
        let cfg = McuBridgeConfig {
            bridge_id: "0123456789abcdef".to_string(),
            secret,
            broker_host: "broker.emqx.io".to_string(),
            broker_port: 8883,
            name: "Cardputer Lab".to_string(),
            username: None,
            password: None,
        };
        let (expected_cmd, expected_evt) = mcu_topics(&cfg.bridge_id);

        // Simula o ciclo completo MQTT 3.1.1 + ponte MCU em memória (duplex) com controle anti-replay.
        let (client_stream, mut server_stream) = tokio::io::duplex(8192);
        let cfg_srv = cfg.clone();

        let broker_task = tokio::spawn(async move {
            // 1. CONNECT -> CONNACK
            let (t, _, _) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 1);
            server_stream.write_all(&[0x20, 0x02, 0x00, 0x00]).await.unwrap();

            // 2. SUBSCRIBE -> SUBACK
            let (t, _, sub_body) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 8);
            let tlen = u16::from_be_bytes([sub_body[2], sub_body[3]]) as usize;
            let sub_topic = std::str::from_utf8(&sub_body[4..4 + tlen]).unwrap();
            assert_eq!(sub_topic, expected_evt);
            server_stream
                .write_all(&[0x90, 0x03, sub_body[0], sub_body[1], 0x01])
                .await
                .unwrap();

            // 3. PUBLISH(chal) -> PUBLISH(chal_resp)
            let (t, _, pub_body) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 3);
            let ptlen = u16::from_be_bytes([pub_body[0], pub_body[1]]) as usize;
            assert_eq!(std::str::from_utf8(&pub_body[2..2 + ptlen]).unwrap(), expected_cmd);
            let chal_req: McuPacket = serde_json::from_slice(&pub_body[2 + ptlen..]).unwrap();
            assert_eq!(chal_req.op, "chal");

            let nonce = "a0b1c2d3e4f5061728394a5b6c7d8e9f".to_string();
            let mut active_nonces = HashSet::new();
            active_nonces.insert(nonce.clone());

            let chal_resp = McuPacket {
                op: "chal".to_string(),
                req: chal_req.req.clone(),
                nonce: Some(nonce.clone()),
                ct: None,
                sig: Some(compute_chal_sig(
                    &cfg_srv.secret,
                    &cfg_srv.bridge_id,
                    &chal_req.req,
                    &nonce,
                )),
                name: Some("Cardputer Lab".to_string()),
                ok: None,
                sent: None,
            };
            let resp_pkt = mqtt_publish_packet(&expected_evt, &serde_json::to_vec(&chal_resp).unwrap());
            server_stream.write_all(&resp_pkt).await.unwrap();

            // 4. PUBLISH(wake) -> valida HMAC + consome nonce único + envia ACK assinado
            let (t, _, wake_body) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 3);
            let wtlen = u16::from_be_bytes([wake_body[0], wake_body[1]]) as usize;
            let wake_req: McuPacket = serde_json::from_slice(&wake_body[2 + wtlen..]).unwrap();
            assert_eq!(wake_req.op, "wake");

            let wake_nonce = wake_req.nonce.as_deref().unwrap();
            let ct_hex = wake_req.ct.as_deref().unwrap();
            let wake_sig = wake_req.sig.as_deref().unwrap();
            assert!(active_nonces.contains(wake_nonce));
            let expected_wake_msg =
                wake_sig_input(&cfg_srv.bridge_id, &wake_req.req, wake_nonce, ct_hex);
            assert!(verify_hmac_hex(&cfg_srv.secret, expected_wake_msg.as_bytes(), wake_sig));
            assert!(active_nonces.remove(wake_nonce)); // Consome nonce (replay falha agora)
            assert!(!active_nonces.contains(wake_nonce));

            let ct_bytes = hex::decode(ct_hex).unwrap();
            let recovered_plain =
                String::from_utf8(apply_hmac_keystream(&cfg_srv.secret, wake_nonce, &ct_bytes)).unwrap();
            assert_eq!(recovered_plain, "9|192.168.18.255|AA:BB:CC:DD:EE:FF");

            let packets_sent = 6u32;
            let ack = McuPacket {
                op: "ack".to_string(),
                req: wake_req.req,
                nonce: Some(wake_nonce.to_string()),
                ct: None,
                sig: Some(compute_ack_sig(
                    &cfg_srv.secret,
                    &cfg_srv.bridge_id,
                    &chal_req.req,
                    wake_nonce,
                    packets_sent,
                )),
                name: None,
                ok: Some(true),
                sent: Some(packets_sent),
            };
            server_stream
                .write_all(&mqtt_publish_packet(&expected_evt, &serde_json::to_vec(&ack).unwrap()))
                .await
                .unwrap();

            // 5. DISCONNECT
            let (t, _, _) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 14);
        });

        let sent = wake_mcu_over_stream(
            client_stream,
            &cfg,
            &["aa-bb-cc-dd-ee-ff".to_string()],
            Some("192.168.18.255"),
            9,
        )
        .await
        .unwrap();
        assert_eq!(sent, 6);
        broker_task.await.unwrap();
    }

    #[tokio::test]
    async fn blocks_ipv4_mapped_ipv6_ssrf_sanitizes_connect_and_ignores_spoofed_and_oversized_mqtt() {
        assert!(is_disallowed_broker_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_disallowed_broker_ip(
            "::ffff:169.254.169.254".parse().unwrap()
        ));
        assert!(!is_disallowed_broker_ip("::ffff:8.8.8.8".parse().unwrap()));

        // [MQTT-3.1.2-22]: senha sem usuário não pode ativar Password Flag (0x40).
        let pkt = mqtt_connect_packet("ora-test", None, Some("orphan-pass")).unwrap();
        // Byte 9 = connect flags (fixed header 2 bytes + "MQTT" 6 bytes + level 1 byte = índice 9).
        assert_eq!(pkt[9] & 0xC0, 0x00);

        // Testa drenagem de pacote > 16 KiB + rejeição silenciosa de desafio/ACK forjado.
        let secret = *blake3::hash(b"test-spoof-secret").as_bytes();
        let cfg = McuBridgeConfig {
            bridge_id: "fedcba9876543210".to_string(),
            secret,
            broker_host: "broker.emqx.io".to_string(),
            broker_port: 8883,
            name: "Spoof Test".to_string(),
            username: None,
            password: None,
        };
        let (_, expected_evt) = mcu_topics(&cfg.bridge_id);
        let (client_stream, mut server_stream) = tokio::io::duplex(64 * 1024);
        let cfg_srv = cfg.clone();

        let broker_task = tokio::spawn(async move {
            let (t, _, _) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 1);
            server_stream.write_all(&[0x20, 0x02, 0x00, 0x00]).await.unwrap();

            let (t, _, sub_body) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 8);
            // Injeta pacote retained > 16 KiB antes do SUBACK para testar drenagem segura.
            let huge_payload = vec![b'X'; MAX_MQTT_PACKET_LEN + 2048];
            let huge_pkt = mqtt_publish_packet(&expected_evt, &huge_payload);
            server_stream.write_all(&huge_pkt).await.unwrap();
            server_stream
                .write_all(&[0x90, 0x03, sub_body[0], sub_body[1], 0x01])
                .await
                .unwrap();

            let (t, _, pub_body) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 3);
            let ptlen = u16::from_be_bytes([pub_body[0], pub_body[1]]) as usize;
            let chal_req: McuPacket = serde_json::from_slice(&pub_body[2 + ptlen..]).unwrap();

            // Envia primeiro um desafio forjado com assinatura errada (deve ser ignorado).
            let spoof_chal = McuPacket {
                op: "chal".to_string(),
                req: chal_req.req.clone(),
                nonce: Some("00000000000000000000000000000000".to_string()),
                ct: None,
                sig: Some("deadbeef".to_string()),
                name: Some("Attacker".to_string()),
                ok: None,
                sent: None,
            };
            server_stream
                .write_all(&mqtt_publish_packet(
                    &expected_evt,
                    &serde_json::to_vec(&spoof_chal).unwrap(),
                ))
                .await
                .unwrap();

            // Em seguida envia o desafio autêntico.
            let nonce = "11223344556677889900aabbccddeeff".to_string();
            let real_chal = McuPacket {
                op: "chal".to_string(),
                req: chal_req.req.clone(),
                nonce: Some(nonce.clone()),
                ct: None,
                sig: Some(compute_chal_sig(
                    &cfg_srv.secret,
                    &cfg_srv.bridge_id,
                    &chal_req.req,
                    &nonce,
                )),
                name: Some("Spoof Test".to_string()),
                ok: None,
                sent: None,
            };
            server_stream
                .write_all(&mqtt_publish_packet(
                    &expected_evt,
                    &serde_json::to_vec(&real_chal).unwrap(),
                ))
                .await
                .unwrap();

            let (t, _, wake_body) = read_mqtt_packet(&mut server_stream).await.unwrap();
            assert_eq!(t, 3);
            let wtlen = u16::from_be_bytes([wake_body[0], wake_body[1]]) as usize;
            let wake_req: McuPacket = serde_json::from_slice(&wake_body[2 + wtlen..]).unwrap();

            // Envia ACK forjado com nonce errado e depois ACK autêntico.
            let spoof_ack = McuPacket {
                op: "ack".to_string(),
                req: wake_req.req.clone(),
                nonce: Some("ffffffffffffffffffffffffffffffff".to_string()),
                ct: None,
                sig: Some("00".to_string()),
                name: None,
                ok: Some(true),
                sent: Some(1),
            };
            server_stream
                .write_all(&mqtt_publish_packet(
                    &expected_evt,
                    &serde_json::to_vec(&spoof_ack).unwrap(),
                ))
                .await
                .unwrap();

            let real_ack = McuPacket {
                op: "ack".to_string(),
                req: wake_req.req.clone(),
                nonce: Some(nonce.clone()),
                ct: None,
                sig: Some(compute_ack_sig(
                    &cfg_srv.secret,
                    &cfg_srv.bridge_id,
                    &wake_req.req,
                    &nonce,
                    3,
                )),
                name: None,
                ok: Some(true),
                sent: Some(3),
            };
            server_stream
                .write_all(&mqtt_publish_packet(
                    &expected_evt,
                    &serde_json::to_vec(&real_ack).unwrap(),
                ))
                .await
                .unwrap();
            let _ = read_mqtt_packet(&mut server_stream).await;
        });

        let sent = wake_mcu_over_stream(
            client_stream,
            &cfg,
            &["aa:bb:cc:dd:ee:ff".to_string()],
            Some("192.168.1.255"),
            9,
        )
        .await
        .unwrap();
        assert_eq!(sent, 3);
        broker_task.await.unwrap();
    }
}
