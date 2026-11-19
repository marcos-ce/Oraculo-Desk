use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use iroh::endpoint::{
    Connection, IdleTimeout, QuicTransportConfig, RecvStream, SendStream, VarInt, presets,
};
use iroh::{Endpoint, EndpointId, SecretKey};
use oraculo_proto::pairing::{
    ALPN_PAIR_CODE, compute_pair_binding_digest, derive_short_code_secrets, encode_pairing_uri,
    format_short_code_from_entropy,
};
use oraculo_proto::wol::{DEFAULT_WOL_PORT, sanitize_wol_info, send_magic_packets};
use oraculo_proto::{
    ALPN, CodePairRequest, CodePairResponse, ControlRequest, ControlResponse, NodeRole,
    PairRequest, PairResponse, StreamKind, read_frame, read_stream_kind, write_frame,
};
use qrcode::QrCode;
use qrcode::render::unicode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock, mpsc, watch};

const PAIRING_TTL: Duration = Duration::from_secs(180);
const MAX_PAIRING_ATTEMPTS: u8 = 5;

#[derive(Parser)]
#[command(
    name = "oraculo-bridge",
    version,
    about = "Ponte Wake-on-LAN leve e isolada para o ecossistema Oráculo"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Inicia a ponte Wake-on-LAN em primeiro plano (padrão).
    Serve {
        /// Exibe imediatamente um QR Code / código de pareamento.
        #[arg(long)]
        pair: bool,
        /// Nome amigável exibido no celular para esta ponte.
        #[arg(long)]
        name: Option<String>,
    },
    /// Lista os celulares autorizados a usar esta ponte.
    Peers,
    /// Revoga o acesso de um celular pelo EndpointId.
    Revoke { endpoint_id: String },
    /// Exibe identidade e caminhos da ponte.
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedClient {
    endpoint_id: String,
    name: String,
    paired_at_unix: u64,
}

struct BridgePaths {
    key_file: PathBuf,
    peers_file: PathBuf,
    lock_file: PathBuf,
}

impl BridgePaths {
    fn discover() -> Result<Self> {
        let base = ProjectDirs::from("com", "oraculo", "oraculo-bridge")
            .context("Não foi possível determinar diretório de dados do oraculo-bridge")?;
        let data_dir = base.data_local_dir().to_path_buf();
        fs::create_dir_all(&data_dir)?;
        Ok(Self {
            key_file: data_dir.join("bridge.key"),
            peers_file: data_dir.join("trusted_peers.json"),
            lock_file: data_dir.join("bridge.lock"),
        })
    }
}

#[derive(Clone)]
struct TrustedClientsStore {
    path: PathBuf,
    clients: Arc<RwLock<HashMap<String, TrustedClient>>>,
    last_stamp: Arc<RwLock<Option<(u64, SystemTime)>>>,
}

impl TrustedClientsStore {
    fn load(path: PathBuf) -> Result<Self> {
        let map = load_clients_map(&path)?;
        let stamp = file_stamp(&path);
        Ok(Self {
            path,
            clients: Arc::new(RwLock::new(map)),
            last_stamp: Arc::new(RwLock::new(stamp)),
        })
    }

    async fn count(&self) -> usize {
        self.clients.read().await.len()
    }

    async fn is_trusted(&self, endpoint_id: &str) -> bool {
        self.clients.read().await.contains_key(endpoint_id)
    }

    async fn add_or_update(&self, endpoint_id: String, name: String) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut guard = self.clients.write().await;
        let mut draft = guard.clone();
        draft.insert(
            endpoint_id.clone(),
            TrustedClient {
                endpoint_id,
                name,
                paired_at_unix: now,
            },
        );
        self.persist_locked(&draft)?;
        *guard = draft;
        Ok(())
    }

    async fn revoke(&self, endpoint_id: &str) -> Result<bool> {
        let mut guard = self.clients.write().await;
        if !guard.contains_key(endpoint_id) {
            return Ok(false);
        }
        let mut draft = guard.clone();
        draft.remove(endpoint_id);
        self.persist_locked(&draft)?;
        *guard = draft;
        Ok(true)
    }

    async fn sync_from_disk(&self) -> Result<Vec<String>> {
        let disk_stamp = file_stamp(&self.path);
        if *self.last_stamp.read().await == disk_stamp {
            return Ok(Vec::new());
        }
        let next = load_clients_map(&self.path)?;
        let mut guard = self.clients.write().await;
        let removed: Vec<String> = guard
            .keys()
            .filter(|id| !next.contains_key(*id))
            .cloned()
            .collect();
        *guard = next;
        *self.last_stamp.write().await = file_stamp(&self.path);
        Ok(removed)
    }

    fn persist_locked(&self, map: &HashMap<String, TrustedClient>) -> Result<()> {
        let mut list: Vec<_> = map.values().cloned().collect();
        list.sort_by(|a, b| a.paired_at_unix.cmp(&b.paired_at_unix));
        let json = serde_json::to_string_pretty(&list)?;
        write_private_file(&self.path, json.as_bytes())?;
        Ok(())
    }
}

fn load_clients_map(path: &Path) -> Result<HashMap<String, TrustedClient>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = fs::read_to_string(path)?;
    let list: Vec<TrustedClient> =
        serde_json::from_str(&raw).with_context(|| format!("JSON inválido em {}", path.display()))?;
    Ok(list
        .into_iter()
        .map(|c| (c.endpoint_id.clone(), c))
        .collect())
}

fn file_stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    Some((meta.len(), modified))
}

#[derive(Default)]
struct ConnectionRegistry {
    next_seq: AtomicU64,
    active: Mutex<HashMap<String, Vec<(u64, Connection)>>>,
}

impl ConnectionRegistry {
    async fn register(&self, peer_id: &str, conn: &Connection) -> u64 {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let mut guard = self.active.lock().await;
        guard
            .entry(peer_id.to_string())
            .or_default()
            .push((seq, conn.clone()));
        seq
    }

    async fn unregister(&self, peer_id: &str, seq: u64) {
        let mut guard = self.active.lock().await;
        if let Some(entries) = guard.get_mut(peer_id) {
            entries.retain(|(s, _)| *s != seq);
            if entries.is_empty() {
                guard.remove(peer_id);
            }
        }
    }

    async fn close_peer(&self, peer_id: &str, reason: &[u8]) {
        let conns = self.active.lock().await.remove(peer_id).unwrap_or_default();
        for (_, c) in conns {
            c.close(VarInt::from_u32(401), reason);
        }
    }

    async fn close_all(&self) {
        let all: Vec<_> = self.active.lock().await.drain().collect();
        for (_, conns) in all {
            for (_, c) in conns {
                c.close(VarInt::from_u32(503), b"bridge shutdown");
            }
        }
    }
}

struct ActivePairing {
    secret: String,
    auth_token: [u8; 32],
    expires_at: Instant,
    failed_attempts: u8,
    stop_tx: watch::Sender<bool>,
}

struct IssuedPairing {
    uri: String,
    short_code: String,
    rendezvous_secret_key: [u8; 32],
    stop_rx: watch::Receiver<bool>,
}

struct BridgePairingManager {
    active: Mutex<Option<ActivePairing>>,
}

impl BridgePairingManager {
    fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    async fn issue(
        &self,
        endpoint_id: EndpointId,
        label: &str,
        relay_url: Option<&str>,
    ) -> IssuedPairing {
        let random_bytes = SecretKey::generate().to_bytes();
        let secret = hex::encode(&random_bytes[..16]);
        let code_entropy: [u8; 8] = random_bytes[16..24].try_into().expect("8 bytes");
        let short_code = format_short_code_from_entropy(&code_entropy);
        let derived = derive_short_code_secrets(&short_code).expect("código válido");
        let uri = encode_pairing_uri(&endpoint_id.to_string(), &secret, label, relay_url);
        let (stop_tx, stop_rx) = watch::channel(false);

        {
            let mut guard = self.active.lock().await;
            if let Some(prev) = guard.take() {
                let _ = prev.stop_tx.send(true);
            }
            *guard = Some(ActivePairing {
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

    async fn consume_secret(&self, candidate: &str) -> bool {
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

    async fn consume_code_token(&self, candidate: &[u8; 32]) -> bool {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_mut() else {
            return false;
        };
        if Instant::now() > active.expires_at {
            let _ = active.stop_tx.send(true);
            *guard = None;
            return false;
        }
        if constant_time_eq(&active.auth_token, candidate) {
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
                let _ = active.stop_tx.send(true);
                *guard = None;
            }
        }
    }
}

#[derive(Clone)]
struct BridgeState {
    secret_key: SecretKey,
    label: String,
    trusted: TrustedClientsStore,
    pairing: Arc<BridgePairingManager>,
    connections: Arc<ConnectionRegistry>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = BridgePaths::discover()?;

    match cli.command {
        Some(Command::Peers) => {
            let store = TrustedClientsStore::load(paths.peers_file)?;
            let mut list: Vec<_> = store.clients.read().await.values().cloned().collect();
            list.sort_by_key(|c| c.paired_at_unix);
            if list.is_empty() {
                println!("Nenhum dispositivo autorizado nesta ponte.");
            } else {
                for client in list {
                    println!("{}  ({})", client.endpoint_id, client.name);
                }
            }
        }
        Some(Command::Revoke { endpoint_id }) => {
            let store = TrustedClientsStore::load(paths.peers_file)?;
            if store.revoke(&endpoint_id).await? {
                println!("Acesso revogado: {endpoint_id}");
            } else {
                println!("EndpointId não encontrado: {endpoint_id}");
            }
        }
        Some(Command::Info) => {
            let key = load_or_create_secret_key(&paths.key_file)?;
            let store = TrustedClientsStore::load(paths.peers_file.clone())?;
            println!("Oráculo Bridge");
            println!("  EndpointId: {}", key.public());
            println!("  Chave:      {}", paths.key_file.display());
            println!("  Peers:      {} ({})", store.count().await, paths.peers_file.display());
        }
        Some(Command::Serve { pair, name }) => {
            run_bridge(paths, pair, name).await?;
        }
        None => {
            run_bridge(paths, false, None).await?;
        }
    }

    Ok(())
}

async fn run_bridge(paths: BridgePaths, pair_flag: bool, custom_name: Option<String>) -> Result<()> {
    #[cfg(unix)]
    let _instance_lock = acquire_instance_lock(&paths.lock_file)?;

    let secret_key = load_or_create_secret_key(&paths.key_file)?;
    let trusted = TrustedClientsStore::load(paths.peers_file)?;
    let label = format_bridge_label(
        custom_name
            .as_deref()
            .unwrap_or(&local_hostname()),
    );

    let mut transport_builder =
        QuicTransportConfig::builder().keep_alive_interval(Duration::from_secs(5));
    if let Ok(idle) = IdleTimeout::try_from(Duration::from_secs(120)) {
        transport_builder = transport_builder.max_idle_timeout(Some(idle));
    }

    let endpoint = Endpoint::builder(presets::N0)
        .transport_config(transport_builder.build())
        .secret_key(secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;

    let _ = tokio::time::timeout(Duration::from_secs(6), endpoint.online()).await;

    let state = BridgeState {
        secret_key,
        label: label.clone(),
        trusted,
        pairing: Arc::new(BridgePairingManager::new()),
        connections: Arc::new(ConnectionRegistry::default()),
    };

    let peers_count = state.trusted.count().await;
    let relay_url = current_relay_url(&endpoint);
    println!("Oráculo Bridge ativo!");
    println!("  Nome:         {label}");
    println!("  EndpointId:   {}", endpoint.id());
    if let Some(ref r) = relay_url {
        println!("  Relay ativo:  {r}");
    }
    println!("  Autorizados:  {peers_count}");
    println!("  Dica: digite 'p' + Enter para gerar novo QR Code / código (Ctrl+C para sair).");

    if pair_flag || peers_count == 0 {
        issue_and_print_pairing(&endpoint, &state).await;
    }

    spawn_stdin_pair_listener(endpoint.clone(), state.clone());

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let sync_state = state.clone();
    let sync_task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Ok(removed) = sync_state.trusted.sync_from_disk().await {
                        for id in removed {
                            println!("Acesso revogado em disco; desconectando {id}");
                            sync_state.connections.close_peer(&id, b"revoked").await;
                        }
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    });

    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break };
                let st = state.clone();
                tokio::spawn(async move {
                    if let Ok(conn) = incoming.await {
                        handle_connection(conn, st).await;
                    }
                });
            }
            _ = wait_shutdown_signal() => {
                println!("\nEncerrando Oráculo Bridge...");
                break;
            }
        }
    }

    let _ = shutdown_tx.send(true);
    state.connections.close_all().await;
    let _ = tokio::time::timeout(Duration::from_millis(500), sync_task).await;
    let _ = tokio::time::timeout(Duration::from_millis(800), endpoint.close()).await;
    Ok(())
}

async fn handle_connection(conn: Connection, state: BridgeState) {
    let remote_id = conn.remote_id().to_string();
    let mut registered_seq: Option<u64> = if state.trusted.is_trusted(&remote_id).await {
        Some(state.connections.register(&remote_id, &conn).await)
    } else {
        None
    };

    loop {
        let is_trusted_now =
            registered_seq.is_some() || state.trusted.is_trusted(&remote_id).await;
        if is_trusted_now && registered_seq.is_none() {
            registered_seq = Some(state.connections.register(&remote_id, &conn).await);
        }

        let accepted = if is_trusted_now {
            conn.accept_bi().await.ok()
        } else {
            match tokio::time::timeout(Duration::from_secs(8), conn.accept_bi()).await {
                Ok(Ok(streams)) => Some(streams),
                Ok(Err(_)) => None,
                Err(_) => {
                    conn.close(VarInt::from_u32(408), b"pair timeout");
                    None
                }
            }
        };
        let Some((mut send, mut recv)) = accepted else {
            break;
        };
        let kind = match tokio::time::timeout(Duration::from_secs(8), read_stream_kind(&mut recv)).await {
            Ok(Ok(kind)) => kind,
            _ => {
                if !is_trusted_now {
                    conn.close(VarInt::from_u32(401), b"invalid stream");
                    break;
                }
                continue;
            }
        };

        if kind == StreamKind::Pair {
            let keep_open = handle_pair_stream(send, recv, remote_id.clone(), state.clone(), conn.clone()).await;
            if !keep_open {
                break;
            }
            if registered_seq.is_none() && state.trusted.is_trusted(&remote_id).await {
                registered_seq = Some(state.connections.register(&remote_id, &conn).await);
            }
            continue;
        }

        if !state.trusted.is_trusted(&remote_id).await {
            if kind == StreamKind::Control {
                let _ = write_frame(
                    &mut send,
                    &ControlResponse::Error("Dispositivo não pareado nesta ponte".into()),
                )
                .await;
                let _ = send.finish();
            }
            conn.close(VarInt::from_u32(401), b"unpaired");
            break;
        }
        if registered_seq.is_none() {
            registered_seq = Some(state.connections.register(&remote_id, &conn).await);
        }

        if kind != StreamKind::Control {
            let _ = send.finish();
            continue;
        }

        let st = state.clone();
        tokio::spawn(async move {
            while let Ok(Some(req)) = read_frame::<_, ControlRequest>(&mut recv).await {
                let resp = match req {
                    ControlRequest::Ping => ControlResponse::Pong {
                        host_name: st.label.clone(),
                        wol: None,
                        role: NodeRole::WolBridge,
                    },
                    ControlRequest::RelayWakeOnLan { wol } => match sanitize_wol_info(wol) {
                        Some(clean) => {
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
                                    "Falha ao disparar Magic Packet: {err:#}"
                                )),
                            }
                        }
                        None => ControlResponse::Error("Pacote WoL inválido recebido".into()),
                    },
                    _ => ControlResponse::Error(
                        "Este serviço atua apenas como Ponte Wake-on-LAN".into(),
                    ),
                };
                if write_frame(&mut send, &resp).await.is_err() {
                    break;
                }
            }
            let _ = send.finish();
        });
    }

    if let Some(seq) = registered_seq {
        state.connections.unregister(&remote_id, seq).await;
    }
}

async fn handle_pair_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    remote_id: String,
    state: BridgeState,
    conn: Connection,
) -> bool {
    let req_opt = tokio::time::timeout(
        Duration::from_secs(10),
        read_frame::<_, PairRequest>(&mut recv),
    )
    .await;
    let Ok(Ok(Some(req))) = req_opt else {
        if !state.trusted.is_trusted(&remote_id).await {
            conn.close(VarInt::from_u32(401), b"invalid pair");
            return false;
        }
        return true;
    };
    let secret_valid = state.pairing.consume_secret(&req.secret).await;
    let (accepted, resp) = if secret_valid {
        let name = sanitize_label(&req.device_name, "dispositivo");
        match state.trusted.add_or_update(remote_id.clone(), name.clone()).await {
            Ok(()) => {
                println!("Novo aparelho autorizado na ponte: {name} ({remote_id})");
                (
                    true,
                    PairResponse {
                        accepted: true,
                        host_name: state.label.clone(),
                        message: "Ponte Wake-on-LAN pareada com sucesso".into(),
                        wol: None,
                        role: NodeRole::WolBridge,
                    },
                )
            }
            Err(err) => (
                false,
                PairResponse {
                    accepted: false,
                    host_name: state.label.clone(),
                    message: format!("Falha ao salvar dispositivo autorizado: {err:#}"),
                    wol: None,
                    role: NodeRole::WolBridge,
                },
            ),
        }
    } else {
        (
            false,
            PairResponse {
                accepted: false,
                host_name: state.label.clone(),
                message: "Token de pareamento inválido ou expirado".into(),
                wol: None,
                role: NodeRole::WolBridge,
            },
        )
    };
    let _ = write_frame(&mut send, &resp).await;
    let _ = send.finish();
    if !accepted && !state.trusted.is_trusted(&remote_id).await {
        tokio::time::sleep(Duration::from_millis(40)).await;
        conn.close(VarInt::from_u32(401), b"pair failed");
        return false;
    }
    true
}

async fn issue_and_print_pairing(main_endpoint: &Endpoint, state: &BridgeState) {
    let relay_url = current_relay_url(main_endpoint);
    let issued = state
        .pairing
        .issue(main_endpoint.id(), &state.label, relay_url.as_deref())
        .await;

    println!("\n=== PAREAMENTO ORÁCULO BRIDGE (3 min / uso único) ===");
    if let Ok(qr) = QrCode::new(issued.uri.as_bytes()) {
        let rendered = qr
            .render::<unicode::Dense1x2>()
            .quiet_zone(true)
            .dark_color(unicode::Dense1x2::Dark)
            .light_color(unicode::Dense1x2::Light)
            .build();
        println!("{rendered}");
    }
    println!("  Código curto:  {}", issued.short_code);
    println!("=====================================================\n");

    let st = state.clone();
    let main_ep = main_endpoint.clone();
    tokio::spawn(async move {
        let _ = run_code_rendezvous(issued, main_ep, st).await;
    });
}

async fn run_code_rendezvous(
    mut issued: IssuedPairing,
    main_endpoint: Endpoint,
    state: BridgeState,
) -> Result<()> {
    let rendezvous_key = SecretKey::from_bytes(&issued.rendezvous_secret_key);
    let rendezvous_ep = Endpoint::builder(presets::N0)
        .secret_key(rendezvous_key)
        .alpns(vec![ALPN_PAIR_CODE.to_vec()])
        .bind()
        .await?;

    let ep_online = rendezvous_ep.clone();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(6), ep_online.online()).await;
    });

    let deadline = tokio::time::Instant::now() + PAIRING_TTL;
    loop {
        if *issued.stop_rx.borrow() {
            break;
        }
        tokio::select! {
            biased;
            changed = issued.stop_rx.changed() => {
                if changed.is_err() || *issued.stop_rx.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
            incoming = rendezvous_ep.accept() => {
                let Some(incoming) = incoming else { break };
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

                if state.pairing.consume_code_token(&req.auth_token).await {
                    let remote_client = conn.remote_id();
                    let name = sanitize_label(&req.device_name, "dispositivo");
                    if let Err(err) = state
                        .trusted
                        .add_or_update(remote_client.to_string(), name.clone())
                        .await
                    {
                        let resp = CodePairResponse {
                            accepted: false,
                            message: format!("Falha ao salvar dispositivo autorizado: {err:#}"),
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

                    let bridge_id = state.secret_key.public();
                    let digest = compute_pair_binding_digest(
                        rendezvous_ep.id().as_bytes(),
                        remote_client.as_bytes(),
                        bridge_id.as_bytes(),
                    );
                    let sig = state.secret_key.sign(&digest).to_bytes().to_vec();

                    let resp = CodePairResponse {
                        accepted: true,
                        message: "Ponte Wake-on-LAN pareada com sucesso".into(),
                        host_endpoint_id: bridge_id.to_string(),
                        host_name: state.label.clone(),
                        relay_url: current_relay_url(&main_endpoint),
                        wol: None,
                        host_signature: sig,
                        role: NodeRole::WolBridge,
                    };
                    let _ = write_frame(&mut send, &resp).await;
                    let _ = send.finish();
                    println!("Novo aparelho autorizado via código: {name} ({remote_client})");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    break;
                } else {
                    let resp = CodePairResponse {
                        accepted: false,
                        message: "Código inválido ou expirado".into(),
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
                    if *issued.stop_rx.borrow() {
                        break;
                    }
                }
            }
        }
    }

    let _ = tokio::time::timeout(Duration::from_millis(600), rendezvous_ep.close()).await;
    Ok(())
}

fn spawn_stdin_pair_listener(endpoint: Endpoint, state: BridgeState) {
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();
    let _ = std::thread::Builder::new()
        .name("oraculo-bridge-stdin".to_string())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin.lock());
            let mut line = String::new();
            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 {
                    break;
                }
                if line.trim().eq_ignore_ascii_case("p") && tx.send(()).is_err() {
                    break;
                }
                line.clear();
            }
        });

    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            issue_and_print_pairing(&endpoint, &state).await;
        }
    });
}

async fn wait_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = async {
                if let Some(ref mut s) = sigterm {
                    s.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

fn current_relay_url(endpoint: &Endpoint) -> Option<String> {
    endpoint.addr().relay_urls().next().map(ToString::to_string)
}

fn load_or_create_secret_key(path: &Path) -> Result<SecretKey> {
    if path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        let hex_key = fs::read_to_string(path)?;
        let bytes = hex::decode(hex_key.trim())?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Chave da ponte com tamanho inválido"))?;
        return Ok(SecretKey::from_bytes(&arr));
    }
    let key = SecretKey::generate();
    write_private_file(path, hex::encode(key.to_bytes()).as_bytes())?;
    Ok(key)
}

fn write_private_file(path: &Path, data: &[u8]) -> Result<()> {
    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let base_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("oraculo-bridge");
    let tmp = path.with_file_name(format!(
        ".{base_name}.{}.{nonce}-{seq}.tmp",
        std::process::id()
    ));

    let write_res: Result<()> = (|| {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    })();

    if write_res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_res
}

fn sanitize_label(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(64)
        .collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

fn format_bridge_label(raw: &str) -> String {
    sanitize_label(raw, "oraculo-bridge")
}

fn local_hostname() -> String {
    if let Ok(name) = fs::read_to_string("/proc/sys/kernel/hostname") {
        let t = name.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "oraculo-bridge".to_string())
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

#[cfg(unix)]
struct InstanceLockGuard {
    _file: fs::File,
}

#[cfg(unix)]
fn acquire_instance_lock(path: &Path) -> Result<InstanceLockGuard> {
    use std::os::unix::io::AsRawFd;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        bail!("Outra instância do oraculo-bridge já está em execução.");
    }
    Ok(InstanceLockGuard { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_label_sanitizes_and_falls_back_cleanly() {
        assert_eq!(format_bridge_label("raspberry"), "raspberry");
        assert_eq!(format_bridge_label("Ponte Sala"), "Ponte Sala");
        assert_eq!(format_bridge_label("   "), "oraculo-bridge");
    }

    #[tokio::test]
    async fn pairing_manager_is_single_use_and_burns_after_5_failures() {
        let mgr = BridgePairingManager::new();
        let ep_id = SecretKey::generate().public();
        let issued = mgr.issue(ep_id, "bridge-pi", None).await;
        let parsed = oraculo_proto::pairing::decode_pairing_uri(&issued.uri).unwrap();

        assert!(!mgr.consume_secret("wrong-secret").await);
        assert!(mgr.consume_secret(&parsed.secret).await);
        assert!(!mgr.consume_secret(&parsed.secret).await);

        let issued2 = mgr.issue(ep_id, "bridge-pi", None).await;
        let parsed2 = oraculo_proto::pairing::decode_pairing_uri(&issued2.uri).unwrap();
        for _ in 0..MAX_PAIRING_ATTEMPTS {
            assert!(!mgr.consume_code_token(&[0u8; 32]).await);
        }
        assert!(!mgr.consume_secret(&parsed2.secret).await);
    }
}
