use iroh::{EndpointId, SecretKey};
use oraculo_proto::pairing::encode_pairing_uri;
use qrcode::QrCode;
use qrcode::render::unicode;
use std::io::IsTerminal;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::warn;

const PAIRING_TTL: Duration = Duration::from_secs(180);
const MAX_PAIRING_ATTEMPTS: u8 = 5;

struct ActivePairing {
    secret: String,
    expires_at: Instant,
    failed_attempts: u8,
}

/// Gerencia segredos efêmeros de uso único para pareamento via QR Code.
pub struct PairingManager {
    active: Mutex<Option<ActivePairing>>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    /// Gera um novo segredo temporário (TTL de 3 min) e imprime o QR Code no terminal.
    pub async fn issue_and_print(
        &self,
        endpoint_id: EndpointId,
        host_name: &str,
        relay_url: Option<&str>,
    ) -> String {
        let random_bytes = SecretKey::generate().to_bytes();
        let secret = hex::encode(&random_bytes[..16]);

        {
            let mut guard = self.active.lock().await;
            *guard = Some(ActivePairing {
                secret: secret.clone(),
                expires_at: Instant::now() + PAIRING_TTL,
                failed_attempts: 0,
            });
        }

        let uri = encode_pairing_uri(&endpoint_id.to_string(), &secret, host_name, relay_url);

        println!("\n=== PAREAMENTO ORÁCULO (válido por 3 min / uso único) ===");
        if let Ok(qr) = QrCode::new(uri.as_bytes()) {
            let rendered = qr
                .render::<unicode::Dense1x2>()
                .quiet_zone(true)
                .dark_color(unicode::Dense1x2::Dark)
                .light_color(unicode::Dense1x2::Light)
                .build();
            print_high_contrast_qr(&rendered);
        }
        if std::io::stdout().is_terminal() {
            println!("URI (uso manual em CLI): {uri}");
        }
        println!("=========================================================\n");

        uri
    }

    /// Valida e consome imediatamente o segredo de pareamento se estiver ativo e válido.
    pub async fn consume_if_valid(&self, candidate: &str) -> bool {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_mut() else {
            return false;
        };

        if Instant::now() > active.expires_at {
            *guard = None;
            return false;
        }

        if constant_time_eq(active.secret.as_bytes(), candidate.as_bytes()) {
            *guard = None;
            return true;
        }

        active.failed_attempts = active.failed_attempts.saturating_add(1);
        if active.failed_attempts >= MAX_PAIRING_ATTEMPTS {
            warn!(
                attempts = active.failed_attempts,
                "Segredo de pareamento invalidado após excesso de tentativas incorretas"
            );
            *guard = None;
        }

        false
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn print_high_contrast_qr(rendered: &str) {
    use std::io::IsTerminal;

    if std::io::stdout().is_terminal() {
        for line in rendered.lines() {
            println!("\x1b[30;107m  {line}  \x1b[0m");
        }
    } else {
        println!("{rendered}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn secret_is_single_use() {
        let mgr = PairingManager::new();
        let id = SecretKey::generate().public();
        let uri = mgr.issue_and_print(id, "test-pc", None).await;
        let parsed = oraculo_proto::pairing::decode_pairing_uri(&uri).unwrap();

        assert!(!mgr.consume_if_valid("segredo-errado").await);
        assert!(mgr.consume_if_valid(&parsed.secret).await);
        assert!(!mgr.consume_if_valid(&parsed.secret).await);
    }

    #[tokio::test]
    async fn secret_burns_after_max_failed_attempts() {
        let mgr = PairingManager::new();
        let id = SecretKey::generate().public();
        let uri = mgr.issue_and_print(id, "test-pc", None).await;
        let parsed = oraculo_proto::pairing::decode_pairing_uri(&uri).unwrap();

        for _ in 0..MAX_PAIRING_ATTEMPTS {
            assert!(!mgr.consume_if_valid("tentativa-errada").await);
        }

        // Após atingir o limite de falhas, mesmo o segredo correto deve falhar.
        assert!(!mgr.consume_if_valid(&parsed.secret).await);
    }
}
