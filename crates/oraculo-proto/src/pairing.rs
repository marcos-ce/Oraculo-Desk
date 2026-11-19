use anyhow::{Result, bail};

const URI_PREFIX: &str = "oraculo://pair?";

/// ALPN exclusivo do endpoint efêmero de pareamento por código curto (`XXXX-XXXX`).
pub const ALPN_PAIR_CODE: &[u8] = b"oraculo/pair-code/1";

/// Alfabeto sem caracteres ambíguos (`0/O/1/I` removidos) — 32 símbolos = 5 bits/caractere (`40 bits` em 8 chars).
pub const SHORT_CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Iterações sequenciais de `blake3::keyed_hash` para desacelerar força bruta offline/online.
const CODE_KDF_ROUNDS: u32 = 65_536;

/// Segredos derivados deterministicamente de um código curto `XXXX-XXXX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedCodeSecrets {
    pub normalized_code: String,
    pub rendezvous_secret_key: [u8; 32],
    pub auth_token: [u8; 32],
}

/// Gera um código legível `XXXX-XXXX` sem viés de módulo a partir de 8 bytes CSPRNG (`256 = 8 * 32`).
pub fn format_short_code_from_entropy(entropy: &[u8; 8]) -> String {
    let mut out = String::with_capacity(9);
    for (idx, &byte) in entropy.iter().enumerate() {
        if idx == 4 {
            out.push('-');
        }
        out.push(SHORT_CODE_ALPHABET[(byte & 0x1F) as usize] as char);
    }
    out
}

/// Valida e normaliza um código digitado pelo usuário para o formato canônico `XXXX-XXXX`.
pub fn normalize_short_code(raw: &str) -> Result<String> {
    let cleaned: Vec<u8> = raw
        .trim()
        .bytes()
        .filter(|b| !matches!(b, b'-' | b' ' | b'\t' | b'\n' | b'\r'))
        .map(|b| b.to_ascii_uppercase())
        .collect();

    if cleaned.len() != 8 {
        bail!("Código de pareamento inválido: informe os 8 caracteres (ex.: K7P4-M9X2)");
    }

    for &ch in &cleaned {
        if !SHORT_CODE_ALPHABET.contains(&ch) {
            bail!(
                "Caractere '{}' inválido no código de pareamento (não usamos 0, O, 1 ou I)",
                ch as char
            );
        }
    }

    let mut formatted = String::with_capacity(9);
    for (idx, &ch) in cleaned.iter().enumerate() {
        if idx == 4 {
            formatted.push('-');
        }
        formatted.push(ch as char);
    }
    Ok(formatted)
}

/// Deriva a chave do endpoint efêmero de encontro e o token de prova a partir do código curto.
pub fn derive_short_code_secrets(raw_code: &str) -> Result<DerivedCodeSecrets> {
    let normalized_code = normalize_short_code(raw_code)?;
    let mut state = blake3::derive_key("oraculo/pair-code/v1/seed", normalized_code.as_bytes());
    for round in 0..CODE_KDF_ROUNDS {
        state = *blake3::keyed_hash(&state, &round.to_le_bytes()).as_bytes();
    }

    let rendezvous_secret_key =
        *blake3::keyed_hash(&state, b"oraculo/pair-code/v1/rendezvous-ed25519").as_bytes();
    let auth_token = *blake3::keyed_hash(&state, b"oraculo/pair-code/v1/client-auth").as_bytes();

    Ok(DerivedCodeSecrets {
        normalized_code,
        rendezvous_secret_key,
        auth_token,
    })
}

/// Calcula o resumo criptográfico assinado pela `host.key` permanente para impedir MITM na sala efêmera.
pub fn compute_pair_binding_digest(
    rendezvous_id: &[u8; 32],
    client_id: &[u8; 32],
    host_id: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("oraculo/pair-code/v1/transcript-bind");
    hasher.update(rendezvous_id);
    hasher.update(client_id);
    hasher.update(host_id);
    *hasher.finalize().as_bytes()
}

/// Dados extraídos de um QR Code ou URI de pareamento do Oráculo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPayload {
    pub endpoint_id: String,
    pub secret: String,
    pub host_name: String,
    pub relay_url: Option<String>,
}

/// Codifica os dados de pareamento em uma URI compacta para QR Code.
pub fn encode_pairing_uri(
    endpoint_id: &str,
    secret: &str,
    host_name: &str,
    relay_url: Option<&str>,
) -> String {
    let mut uri = format!(
        "{URI_PREFIX}id={}&secret={}&name={}",
        percent_encode(endpoint_id),
        percent_encode(secret),
        percent_encode(host_name),
    );
    if let Some(relay) = relay_url.filter(|s| !s.is_empty()) {
        uri.push_str("&relay=");
        uri.push_str(&percent_encode(relay));
    }
    uri
}

/// Decodifica uma URI `oraculo://pair?...` lida do QR Code.
pub fn decode_pairing_uri(uri: &str) -> Result<PairingPayload> {
    let query = uri
        .trim()
        .strip_prefix(URI_PREFIX)
        .ok_or_else(|| anyhow::anyhow!("URI de pareamento inválida: esperado prefixo {URI_PREFIX}"))?;

    let mut endpoint_id = None;
    let mut secret = None;
    let mut host_name = None;
    let mut relay_url = None;

    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = percent_decode(v)?;
            match k {
                "id" => endpoint_id = Some(decoded),
                "secret" => secret = Some(decoded),
                "name" => host_name = Some(decoded),
                "relay" if !decoded.is_empty() => relay_url = Some(decoded),
                _ => {}
            }
        }
    }

    let endpoint_id = endpoint_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Parâmetro 'id' ausente na URI de pareamento"))?;
    let secret = secret
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Parâmetro 'secret' ausente na URI de pareamento"))?;
    let host_name = host_name
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "oraculo-host".to_string());

    if endpoint_id.len() < 16 || secret.len() < 8 {
        bail!("URI de pareamento com credenciais curtas demais");
    }

    Ok(PairingPayload {
        endpoint_id,
        secret,
        host_name,
        relay_url,
    })
}

pub(crate) fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if is_unreserved {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(hex_nibble(byte >> 4));
            out.push(hex_nibble(byte & 0x0F));
        }
    }
    out
}

pub(crate) fn percent_decode(input: &str) -> Result<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                bail!("Sequência percent-encoded truncada");
            }
            let hi = from_hex_digit(bytes[i + 1])?;
            let lo = from_hex_digit(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8(out).map_err(|e| anyhow::anyhow!("UTF-8 inválido na URI: {e}"))
}

fn hex_nibble(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

fn from_hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("Caractere hexadecimal inválido na URI"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_pairing_uri() {
        let uri = encode_pairing_uri(
            "abc123def4567890abc123def4567890",
            "deadbeefcafebabe",
            "Notebook Marcos (Fedora)",
            Some("https://use1-1.relay.iroh.network./"),
        );
        let parsed = decode_pairing_uri(&uri).unwrap();
        assert_eq!(parsed.endpoint_id, "abc123def4567890abc123def4567890");
        assert_eq!(parsed.secret, "deadbeefcafebabe");
        assert_eq!(parsed.host_name, "Notebook Marcos (Fedora)");
        assert_eq!(
            parsed.relay_url.as_deref(),
            Some("https://use1-1.relay.iroh.network./")
        );
    }

    #[test]
    fn reject_invalid_prefix() {
        assert!(decode_pairing_uri("https://example.com").is_err());
    }

    #[test]
    fn short_code_normalization_and_deterministic_kdf() {
        let code = format_short_code_from_entropy(&[0, 1, 2, 3, 4, 5, 6, 31]);
        assert_eq!(code.len(), 9);
        assert_eq!(&code[4..5], "-");

        let d1 = derive_short_code_secrets("k7p4 m9x2").unwrap();
        let d2 = derive_short_code_secrets("K7P4-M9X2").unwrap();
        assert_eq!(d1, d2);
        assert_ne!(d1.rendezvous_secret_key, d1.auth_token);

        assert!(normalize_short_code("K7P4-0000").is_err());
        assert!(normalize_short_code("CURTO").is_err());
    }
}
