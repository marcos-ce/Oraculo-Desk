use crate::{OraculoError, err_msg, sanitize_host_name, write_private_file};
use iroh::endpoint::{VarInt, presets};
use iroh::tls::CaTlsConfig;
use iroh::{Endpoint, SecretKey};
use oraculo_proto::pairing::{
    ALPN_PAIR_CODE, compute_pair_binding_digest, derive_short_code_secrets,
    format_short_code_from_entropy,
};
use oraculo_proto::wol::{DEFAULT_WOL_PORT, sanitize_wol_info, send_magic_packets};
use oraculo_proto::{
    CodePairRequest, CodePairResponse, ControlRequest, ControlResponse, NodeRole, StreamKind,
    read_frame, read_stream_kind, write_frame,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::watch;

const BRIDGE_CODE_TTL: Duration = Duration::from_secs(180);

pub struct MobileWolBridge {
    running: AtomicBool,
    stop_tx: StdMutex<Option<watch::Sender<bool>>>,
    code_stop_tx: StdMutex<Option<watch::Sender<bool>>>,
}

impl Default for MobileWolBridge {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            stop_tx: StdMutex::new(None),
            code_stop_tx: StdMutex::new(None),
        }
    }
}

impl MobileWolBridge {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
        if let Ok(mut guard) = self.stop_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(true);
            }
        }
        if let Ok(mut guard) = self.code_stop_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(true);
            }
        }
    }

    pub fn start(&self, endpoint: Endpoint, data_dir: PathBuf, label: String) {
        self.stop();
        let (tx, mut stop_rx) = watch::channel(false);
        if let Ok(mut guard) = self.stop_tx.lock() {
            *guard = Some(tx);
        }
        self.running.store(true, Ordering::Release);

        let peers_path = bridge_peers_file(&data_dir);
        let bridge_label = format_bridge_label(&label);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    changed = stop_rx.changed() => {
                        if changed.is_err() || *stop_rx.borrow() {
                            break;
                        }
                    }
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else {
                            break;
                        };
                        let peers_file = peers_path.clone();
                        let host_title = bridge_label.clone();
                        let mut conn_stop_rx = stop_rx.clone();
                        tokio::spawn(async move {
                            let Ok(Ok(conn)) = tokio::time::timeout(Duration::from_secs(6), incoming).await else {
                                return;
                            };
                            if *conn_stop_rx.borrow() {
                                conn.close(VarInt::from_u32(503), b"bridge stopped");
                                return;
                            }
                            let remote_id = conn.remote_id().to_string();
                            let trusted = load_bridge_peers(&peers_file).contains(&remote_id);
                            if !trusted {
                                conn.close(VarInt::from_u32(401), b"unpaired bridge peer");
                                return;
                            }

                            loop {
                                let accepted = tokio::select! {
                                    biased;
                                    changed = conn_stop_rx.changed() => {
                                        if changed.is_err() || *conn_stop_rx.borrow() {
                                            conn.close(VarInt::from_u32(503), b"bridge stopped");
                                            break;
                                        }
                                        continue;
                                    }
                                    res = conn.accept_bi() => res,
                                };
                                let Ok((mut send, mut recv)) = accepted else {
                                    break;
                                };
                                let host_title_stream = host_title.clone();
                                let mut stream_stop_rx = conn_stop_rx.clone();
                                tokio::spawn(async move {
                                    let Ok(kind) = read_stream_kind(&mut recv).await else {
                                        return;
                                    };
                                    if kind != StreamKind::Control {
                                        let _ = send.finish();
                                        return;
                                    }
                                    loop {
                                        let next = tokio::select! {
                                            biased;
                                            changed = stream_stop_rx.changed() => {
                                                if changed.is_err() || *stream_stop_rx.borrow() {
                                                    break;
                                                }
                                                continue;
                                            }
                                            res = read_frame::<_, ControlRequest>(&mut recv) => res,
                                        };
                                        let Ok(Some(req)) = next else {
                                            break;
                                        };
                                        let resp = match req {
                                            ControlRequest::Ping => ControlResponse::Pong {
                                                host_name: host_title_stream.clone(),
                                                wol: None,
                                                role: NodeRole::WolBridge,
                                            },
                                            ControlRequest::RelayWakeOnLan { wol } => {
                                                if let Some(clean) = sanitize_wol_info(wol) {
                                                    match send_magic_packets(
                                                        &clean.macs,
                                                        clean.broadcast.as_deref(),
                                                        None,
                                                        DEFAULT_WOL_PORT,
                                                    )
                                                    .await
                                                    {
                                                        Ok(sent) => ControlResponse::WakeDispatched {
                                                            packets_sent: sent as u32,
                                                        },
                                                        Err(err) => ControlResponse::Error(format!(
                                                            "Falha ao disparar WoL pela ponte: {err:#}"
                                                        )),
                                                    }
                                                } else {
                                                    ControlResponse::Error(
                                                        "Pacote WoL inválido recebido pela ponte".into(),
                                                    )
                                                }
                                            }
                                            _ => ControlResponse::Error(
                                                "Este dispositivo atua apenas como Ponte Wake-on-LAN".into(),
                                            ),
                                        };
                                        if write_frame(&mut send, &resp).await.is_err() {
                                            break;
                                        }
                                    }
                                    let _ = send.finish();
                                });
                            }
                        });
                    }
                }
            }
        });
    }

    pub async fn issue_pairing_code(
        &self,
        main_endpoint: Endpoint,
        client_secret_key: SecretKey,
        data_dir: PathBuf,
        label: String,
    ) -> Result<String, OraculoError> {
        if let Ok(mut guard) = self.code_stop_tx.lock() {
            if let Some(prev) = guard.take() {
                let _ = prev.send(true);
            }
        }

        let random_bytes = SecretKey::generate().to_bytes();
        let entropy: [u8; 8] = random_bytes[..8].try_into().expect("8 bytes");
        let short_code = format_short_code_from_entropy(&entropy);
        let derived = derive_short_code_secrets(&short_code)?;

        let rendezvous_key = SecretKey::from_bytes(&derived.rendezvous_secret_key);
        let rendezvous_ep = Endpoint::builder(presets::N0)
            .ca_tls_config(CaTlsConfig::embedded())
            .secret_key(rendezvous_key)
            .alpns(vec![ALPN_PAIR_CODE.to_vec()])
            .bind()
            .await
            .map_err(|e| err_msg(format!("Falha ao abrir sala de pareamento da ponte: {e}")))?;

        let ep_online = rendezvous_ep.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(6), ep_online.online()).await;
        });

        let (tx, mut stop_rx) = watch::channel(false);
        if let Ok(mut guard) = self.code_stop_tx.lock() {
            *guard = Some(tx);
        }

        let peers_path = bridge_peers_file(&data_dir);
        let bridge_title = format_bridge_label(&label);
        let expected_token = derived.auth_token;
        let relay_url = main_endpoint
            .addr()
            .relay_urls()
            .next()
            .map(ToString::to_string);

        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + BRIDGE_CODE_TTL;
            let mut attempts = 0u8;

            loop {
                tokio::select! {
                    biased;
                    changed = stop_rx.changed() => {
                        if changed.is_err() || *stop_rx.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep_until(deadline) => break,
                    incoming = rendezvous_ep.accept() => {
                        let Some(incoming) = incoming else {
                            break;
                        };
                        let Ok(Ok(conn)) = tokio::time::timeout(Duration::from_secs(6), incoming).await else {
                            continue;
                        };
                        let Ok(Ok((mut send, mut recv))) =
                            tokio::time::timeout(Duration::from_secs(5), conn.accept_bi()).await
                        else {
                            conn.close(VarInt::from_u32(400), b"timeout");
                            continue;
                        };
                        let Ok(Ok(Some(req))) = tokio::time::timeout(
                            Duration::from_secs(5),
                            read_frame::<_, CodePairRequest>(&mut recv),
                        )
                        .await
                        else {
                            conn.close(VarInt::from_u32(400), b"invalid");
                            continue;
                        };

                        if constant_time_eq(&req.auth_token, &expected_token) {
                            let remote_client = conn.remote_id();
                            if let Err(err) = add_bridge_peer(&peers_path, remote_client.to_string()) {
                                let resp = CodePairResponse {
                                    accepted: false,
                                    message: format!("Falha ao salvar dispositivo autorizado: {err}"),
                                    host_endpoint_id: String::new(),
                                    host_name: String::new(),
                                    relay_url: None,
                                    wol: None,
                                    host_signature: Vec::new(),
                                    role: NodeRole::WolBridge,
                                };
                                let _ = write_frame(&mut send, &resp).await;
                                let _ = send.finish();
                                tokio::time::sleep(Duration::from_millis(80)).await;
                                conn.close(VarInt::from_u32(500), b"persist failed");
                                continue;
                            }

                            let bridge_id = client_secret_key.public();
                            let digest = compute_pair_binding_digest(
                                rendezvous_ep.id().as_bytes(),
                                remote_client.as_bytes(),
                                bridge_id.as_bytes(),
                            );
                            let sig = client_secret_key.sign(&digest).to_bytes().to_vec();

                            let resp = CodePairResponse {
                                accepted: true,
                                message: "Ponte Wake-on-LAN pareada com sucesso".into(),
                                host_endpoint_id: bridge_id.to_string(),
                                host_name: bridge_title.clone(),
                                relay_url: relay_url.clone(),
                                wol: None,
                                host_signature: sig,
                                role: NodeRole::WolBridge,
                            };
                            let _ = write_frame(&mut send, &resp).await;
                            let _ = send.finish();
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            break;
                        } else {
                            attempts = attempts.saturating_add(1);
                            let resp = CodePairResponse {
                                accepted: false,
                                message: "Código da ponte inválido ou expirado".into(),
                                host_endpoint_id: String::new(),
                                host_name: String::new(),
                                relay_url: None,
                                wol: None,
                                host_signature: Vec::new(),
                                role: NodeRole::WolBridge,
                            };
                            let _ = write_frame(&mut send, &resp).await;
                            let _ = send.finish();
                            tokio::time::sleep(Duration::from_millis(120)).await;
                            conn.close(VarInt::from_u32(403), b"invalid code");
                            if attempts >= 5 {
                                break;
                            }
                        }
                    }
                }
            }

            let _ = tokio::time::timeout(Duration::from_millis(600), rendezvous_ep.close()).await;
        });

        Ok(short_code)
    }
}

fn format_bridge_label(raw: &str) -> String {
    sanitize_host_name(raw)
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

fn bridge_peers_file(data_dir: &Path) -> PathBuf {
    data_dir.join("bridge_trusted_peers.json")
}

fn load_bridge_peers(path: &Path) -> HashSet<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return HashSet::new();
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn add_bridge_peer(path: &Path, endpoint_id: String) -> Result<(), OraculoError> {
    let mut set = load_bridge_peers(path);
    set.insert(endpoint_id);
    let mut sorted: Vec<String> = set.into_iter().collect();
    sorted.sort();
    let json = serde_json::to_string_pretty(&sorted)
        .map_err(|e| err_msg(format!("Falha ao serializar peers da ponte: {e}")))?;
    write_private_file(path, json.as_bytes(), "bridge_trusted_peers.json")
}
