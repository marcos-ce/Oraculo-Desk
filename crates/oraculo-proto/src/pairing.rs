use anyhow::{Result, bail};

const URI_PREFIX: &str = "oraculo://pair?";

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

fn percent_encode(input: &str) -> String {
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

fn percent_decode(input: &str) -> Result<String> {
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
}
