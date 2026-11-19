use iroh::{EndpointId, SecretKey};
use oraculo_proto::pairing::{
     derive_short_code_secrets, encode_pairing_uri, format_short_code_from_entropy,
};
use qrcode::QrCode;
use qrcode::render::unicode;
use std::io::IsTerminal;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, watch};
use tracing::warn;

pub const PAIRING_TTL: Duration = Duration::from_secs(180);
const MAX_PAIRING_ATTEMPTS: u8 = 5;

struct ActivePairing {
    uri: String,
    short_code: String,
    secret: String,
    auth_token: [u8; 32],
    expires_at: Instant,
    failed_attempts: u8,
    stop_tx: watch::Sender<bool>,
}

/// Resumo de uma janela de pareamento ativa (para exibição reativa na GUI Desktop).
#[derive(Debug, Clone)]
pub struct PairingStatusSnapshot {
    pub uri: String,
    pub short_code: String,
    pub remaining_secs: u64,
}

/// Dados emitidos ao abrir uma janela de pareamento (QR Code + Código Curto `XXXX-XXXX`).
#[cfg_attr(not(test), allow(dead_code))]
pub struct IssuedPairing {
    pub uri: String,
    pub short_code: String,
    pub rendezvous_secret_key: [u8; 32],
    pub stop_rx: watch::Receiver<bool>,
}

/// Gerencia segredos efêmeros de uso único compartilhados entre QR Code e Código Curto.
pub struct PairingManager {
    active: Mutex<Option<ActivePairing>>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    /// Gera um novo segredo temporário + código curto `XXXX-XXXX` sem imprimir no terminal (usado pela GUI).
    pub async fn issue_silent(
        &self,
        endpoint_id: EndpointId,
        host_name: &str,
        relay_url: Option<&str>,
    ) -> IssuedPairing {
        let random_bytes = SecretKey::generate().to_bytes();
        let secret = hex::encode(&random_bytes[..16]);
        let code_entropy: [u8; 8] = random_bytes[16..24].try_into().expect("8 bytes");
        let short_code = format_short_code_from_entropy(&code_entropy);
        let derived = derive_short_code_secrets(&short_code)
            .expect("código gerado internamente sempre válido");

        let uri = encode_pairing_uri(&endpoint_id.to_string(), &secret, host_name, relay_url);
        let (stop_tx, stop_rx) = watch::channel(false);

        {
            let mut guard = self.active.lock().await;
            if let Some(prev) = guard.take() {
                let _ = prev.stop_tx.send(true);
            }
            *guard = Some(ActivePairing {
                uri: uri.clone(),
                short_code: short_code.clone(),
                secret,
                auth_token: derived.auth_token,
                expires_at: Instant::now() + PAIRING_TTL,
                failed_attempts: 0,
                stop_tx,
            });
        }

        IssuedPairing {
            uri,
            short_code,
            rendezvous_secret_key: derived.rendezvous_secret_key,
            stop_rx,
        }
    }

    /// Cancela imediatamente qualquer sessão de pareamento ativa (usado no shutdown limpo).
    pub async fn cancel_active(&self) {
        if let Some(prev) = self.active.lock().await.take() {
            let _ = prev.stop_tx.send(true);
        }
    }

    /// Consulta se ainda há uma sessão de pareamento ativa e quantos segundos restam.
    pub async fn current_status(&self) -> Option<PairingStatusSnapshot> {
        let mut guard = self.active.lock().await;
        let active = guard.as_ref()?;
        let now = Instant::now();
        if now >= active.expires_at {
            if let Some(expired) = guard.take() {
                let _ = expired.stop_tx.send(true);
            }
            return None;
        }
        Some(PairingStatusSnapshot {
            uri: active.uri.clone(),
            short_code: active.short_code.clone(),
            remaining_secs: active.expires_at.saturating_duration_since(now).as_secs(),
        })
    }

    /// Gera um novo segredo temporário + código curto `XXXX-XXXX` (TTL de 3 min) e imprime no terminal.
    pub async fn issue_and_print(
        &self,
        endpoint_id: EndpointId,
        host_name: &str,
        relay_url: Option<&str>,
    ) -> IssuedPairing {
        let issued = self.issue_silent(endpoint_id, host_name, relay_url).await;

        println!("\n=== PAREAMENTO ORÁCULO (válido por 3 min / uso único) ===");
        if let Ok(qr) = QrCode::new(issued.uri.as_bytes()) {
            let rendered = qr
                .render::<unicode::Dense1x2>()
                .quiet_zone(true)
                .dark_color(unicode::Dense1x2::Dark)
                .light_color(unicode::Dense1x2::Light)
                .build();
            print_high_contrast_qr(&rendered);
        }
        println!("\n  Código rápido:  {}", issued.short_code);
        if std::io::stdout().is_terminal() {
            println!("  CLI / App:      oraculo-cli pair {}", issued.short_code);
        }
        println!("=========================================================\n");

        issued
    }

    /// Valida e consome imediatamente o segredo do QR Code (queimando também o código curto ativo).
    pub async fn consume_if_valid(&self, candidate: &str) -> bool {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_mut() else {
            return false;
        };

        if Instant::now() > active.expires_at {
            let _ = active.stop_tx.send(true);
            *guard = None;
            return false;
        }

        if constant_time_eq(active.secret.as_bytes(), candidate.as_bytes()) {
            let _ = active.stop_tx.send(true);
            *guard = None;
            return true;
        }

        Self::record_failure(&mut guard);
        false
    }

    /// Valida e consome imediatamente o token derivado do Código Curto (queimando também o QR Code ativo).
    pub async fn consume_code_token(&self, candidate_token: &[u8; 32]) -> bool {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_mut() else {
            return false;
        };

        if Instant::now() > active.expires_at {
            let _ = active.stop_tx.send(true);
            *guard = None;
            return false;
        }

        if constant_time_eq(&active.auth_token, candidate_token) {
            let _ = active.stop_tx.send(true);
            *guard = None;
            return true;
        }

        Self::record_failure(&mut guard);
        false
    }

    fn record_failure(guard: &mut Option<ActivePairing>) {
        if let Some(active) = guard.as_mut() {
            active.failed_attempts = active.failed_attempts.saturating_add(1);
            if active.failed_attempts >= MAX_PAIRING_ATTEMPTS {
                warn!(
                    attempts = active.failed_attempts,
                    "Sessão de pareamento invalidada após excesso de tentativas incorretas"
                );
                let _ = active.stop_tx.send(true);
                *guard = None;
            }
        }
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

/// Renderiza um QR Code vetorial `<svg>` em alto contraste ISO (`quiet = 4`, módulos mesclados sem falhas subpixel).
pub fn render_qr_svg(data: &str) -> Option<String> {
    let qr = QrCode::new(data.as_bytes()).ok()?;
    let width = qr.width();
    let quiet = 4usize;
    let total = width + quiet * 2;
    let colors = qr.to_colors();

    // Mescla módulos escuros contíguos na horizontal em faixas únicas para evitar frestas subpixel e serrilhado `crispEdges`.
    let mut path_data = String::with_capacity(colors.len() * 4);
    for y in 0..width {
        let mut x = 0usize;
        while x < width {
            if colors[y * width + x] == qrcode::Color::Dark {
                let start = x;
                while x < width && colors[y * width + x] == qrcode::Color::Dark {
                    x += 1;
                }
                let span = x - start;
                use std::fmt::Write;
                let _ = write!(path_data, "M{},{}h{}v1.02h-{}z", start + quiet, y + quiet, span, span);
            } else {
                x += 1;
            }
        }
    }

    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total} {total}"><rect width="{total}" height="{total}" fill="#FFFFFF"/><path d="{path_data}" fill="#000000"/></svg>"##
    ))
}

fn print_high_contrast_qr(rendered: &str) {
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
        let issued = mgr.issue_and_print(id, "test-pc", None).await;
        let parsed = oraculo_proto::pairing::decode_pairing_uri(&issued.uri).unwrap();

        assert!(!mgr.consume_if_valid("segredo-errado").await);
        assert!(mgr.consume_if_valid(&parsed.secret).await);
        assert!(*issued.stop_rx.borrow());
        assert!(!mgr.consume_if_valid(&parsed.secret).await);

        // Usar o QR Code também invalida o código curto da mesma janela.
        let code_secrets = derive_short_code_secrets(&issued.short_code).unwrap();
        assert!(!mgr.consume_code_token(&code_secrets.auth_token).await);
    }

    #[tokio::test]
    async fn short_code_token_burns_qr_and_is_single_use() {
        let mgr = PairingManager::new();
        let id = SecretKey::generate().public();
        let issued = mgr.issue_and_print(id, "test-pc", None).await;
        let parsed = oraculo_proto::pairing::decode_pairing_uri(&issued.uri).unwrap();
        let code_secrets = derive_short_code_secrets(&issued.short_code).unwrap();

        assert!(mgr.consume_code_token(&code_secrets.auth_token).await);
        assert!(*issued.stop_rx.borrow());
        assert!(!mgr.consume_code_token(&code_secrets.auth_token).await);
        assert!(!mgr.consume_if_valid(&parsed.secret).await);
    }

    #[tokio::test]
    async fn secret_burns_after_max_failed_attempts() {
        let mgr = PairingManager::new();
        let id = SecretKey::generate().public();
        let issued = mgr.issue_and_print(id, "test-pc", None).await;
        let parsed = oraculo_proto::pairing::decode_pairing_uri(&issued.uri).unwrap();

        for _ in 0..MAX_PAIRING_ATTEMPTS {
            assert!(!mgr.consume_if_valid("tentativa-errada").await);
        }

        // Após atingir o limite de falhas, mesmo o segredo correto deve falhar.
        assert!(!mgr.consume_if_valid(&parsed.secret).await);
    }
}
