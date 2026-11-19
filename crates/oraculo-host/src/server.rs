use crate::commands::run_quick_command;
use crate::config::{
    HostPaths, SettingsStore, TrustedPeersStore, delete_command, load_or_create_commands,
    local_hostname, save_command,
};
use crate::control_ipc::{DaemonControlSnapshot, start_daemon_control_server};
use crate::files::{discover_home_root, handle_files_stream, list_directory};
use crate::input::{InputController, handle_input_stream};
use crate::pairing::{IssuedPairing, PAIRING_TTL, PairingManager, render_qr_svg};
use crate::portal::{
    PortalAuthorizeOutcome, PortalCoordinator, PortalReadiness, ensure_portal_app_registered,
};
use crate::pty::{TerminalSessionManager, handle_terminal_stream};
use crate::screen::{ScreenController, handle_screen_stream};
use crate::tunnel::{RdpStatus, effective_rdp_port, handle_tunnel_stream, inspect_rdp_status};
use crate::wol::resolve_host_wol_info;
use anyhow::Result;
use iroh::endpoint::{
    Connection, IdleTimeout, QuicTransportConfig, RecvStream, SendStream, VarInt, presets,
};
use iroh::{Endpoint, SecretKey};
use oraculo_proto::pairing::{ALPN_PAIR_CODE, compute_pair_binding_digest};
use oraculo_proto::wol::{DEFAULT_WOL_PORT, sanitize_wol_info, send_magic_packets};
use oraculo_proto::{
    ALPN, CodePairRequest, CodePairResponse, ControlRequest, ControlResponse, HostPermissionsWire,
    NodeRole, PairRequest, PairResponse, StreamKind, read_frame, read_stream_kind, write_frame,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, mpsc, watch};
use tracing::{info, warn};

const QUIC_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(4);
const QUIC_MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const PORTAL_RELEASE_GRACE: Duration = Duration::from_secs(15);
const UNTRUSTED_STREAM_TIMEOUT: Duration = Duration::from_secs(8);
const PAIR_READ_TIMEOUT: Duration = Duration::from_secs(10);

fn build_host_transport_config() -> QuicTransportConfig {
    let mut builder = QuicTransportConfig::builder().keep_alive_interval(QUIC_KEEP_ALIVE_INTERVAL);
    if let Ok(idle) = IdleTimeout::try_from(QUIC_MAX_IDLE_TIMEOUT) {
        builder = builder.max_idle_timeout(Some(idle));
    }
    builder.build()
}

fn is_explicit_close_reason(reason: Option<iroh::endpoint::ConnectionError>) -> bool {
    matches!(
        reason,
        Some(iroh::endpoint::ConnectionError::LocallyClosed)
            | Some(iroh::endpoint::ConnectionError::ApplicationClosed(_))
    )
}

#[derive(Default)]
struct ConnectionRegistry {
    next_seq: AtomicU64,
    generation: AtomicU64,
    active: RwLock<HashMap<String, Vec<(u64, Connection)>>>,
}

impl ConnectionRegistry {
    fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    async fn register(&self, peer_id: &str, conn: &Connection) -> u64 {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.active
            .write()
            .await
            .entry(peer_id.to_string())
            .or_default()
            .push((seq, conn.clone()));
        seq
    }

    async fn unregister(&self, peer_id: &str, seq: u64) {
        let mut guard = self.active.write().await;
        if let Some(list) = guard.get_mut(peer_id) {
            list.retain(|(s, _)| *s != seq);
            if list.is_empty() {
                guard.remove(peer_id);
            }
        }
    }

    async fn close_peer(&self, peer_id: &str) {
        let conns = self.active.write().await.remove(peer_id).unwrap_or_default();
        let reason = b"peer revoked";
        for (_, conn) in conns {
            conn.close(VarInt::from_u32(403), reason);
        }
    }

    async fn disconnect_peer(&self, peer_id: &str) -> bool {
        let conns = self.active.write().await.remove(peer_id).unwrap_or_default();
        if conns.is_empty() {
            return false;
        }
        let reason = b"peer disconnected";
        for (_, conn) in conns {
            conn.close(VarInt::from_u32(410), reason);
        }
        true
    }

    async fn close_all(&self, code: u32, reason: &[u8]) {
        let mut guard = self.active.write().await;
        let all: Vec<_> = guard.drain().collect();
        drop(guard);
        for (_, conns) in all {
            for (_, conn) in conns {
                conn.close(VarInt::from_u32(code), reason);
            }
        }
    }

    async fn active_peer_ids(&self) -> Vec<String> {
        let guard = self.active.read().await;
        guard
            .iter()
            .filter(|(_, list)| list.iter().any(|(_, c)| c.close_reason().is_none()))
            .map(|(id, _)| id.clone())
            .collect()
    }
}

/// Controlador programático do daemon Iroh usado pelo aplicativo Desktop.
#[derive(Clone)]
pub struct HostControlHandle {
    endpoint: Endpoint,
    state: ServerState,
    shutdown_tx: watch::Sender<bool>,
    #[cfg(target_os = "linux")]
    _instance_lock: Arc<std::fs::File>,
}

/// Dados de pareamento prontos para renderização na GUI Desktop.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuiPairingView {
    pub uri: String,
    pub short_code: String,
    pub qr_svg: String,
    pub remaining_secs: u64,
}

impl HostControlHandle {
    pub fn endpoint_id(&self) -> String {
        self.endpoint.id().to_string()
    }

    pub fn host_name(&self) -> &str {
        &self.state.host_name
    }

    pub fn paths(&self) -> &HostPaths {
        &self.state.paths
    }

    pub fn trusted_peers(&self) -> &TrustedPeersStore {
        &self.state.trusted_peers
    }

    pub fn settings(&self) -> &SettingsStore {
        &self.state.settings
    }

    pub fn relay_url(&self) -> Option<String> {
        current_relay_url(&self.endpoint)
    }

    pub async fn active_peer_ids(&self) -> Vec<String> {
        self.state.connections.active_peer_ids().await
    }

    pub async fn revoke_peer(&self, endpoint_id: &str) -> Result<bool> {
        let removed = self.state.trusted_peers.revoke(endpoint_id).await?;
        if removed {
            self.state.connections.close_peer(endpoint_id).await;
            self.state.terminal.kill_for_peer(endpoint_id).await;
        }
        Ok(removed)
    }

    pub async fn disconnect_peer(&self, endpoint_id: &str) -> bool {
        self.state.connections.disconnect_peer(endpoint_id).await
    }

    pub async fn cancel_pairing(&self) {
        self.state.pairing.cancel_active().await;
    }

    pub async fn current_pairing(&self) -> Option<GuiPairingView> {
        let snap = self.state.pairing.current_status().await?;
        let qr_svg = render_qr_svg(&snap.uri).unwrap_or_default();
        Some(GuiPairingView {
            uri: snap.uri,
            short_code: snap.short_code,
            qr_svg,
            remaining_secs: snap.remaining_secs,
        })
    }

    pub async fn issue_pairing(&self) -> GuiPairingView {
        let relay_url = current_relay_url(&self.endpoint);
        let issued = self
            .state
            .pairing
            .issue_silent(self.endpoint.id(), &self.state.host_name, relay_url.as_deref())
            .await;
        let qr_svg = render_qr_svg(&issued.uri).unwrap_or_default();
        let view = GuiPairingView {
            uri: issued.uri.clone(),
            short_code: issued.short_code.clone(),
            qr_svg,
            remaining_secs: PAIRING_TTL.as_secs(),
        };

        let main_ep = self.endpoint.clone();
        let state_clone = self.state.clone();
        tokio::spawn(async move {
            if let Err(err) = run_code_rendezvous_listener(issued, main_ep, state_clone).await {
                warn!("Encerramento do canal de código curto: {err:#}");
            }
        });

        view
    }

    pub async fn portal_readiness(&self) -> PortalReadiness {
        self.state.portal.readiness().await
    }

    pub async fn authorize_portals(&self) -> Result<PortalAuthorizeOutcome> {
        let cur = self.state.settings.current().await;
        self.state
            .portal
            .authorize_interactive(cur.allow_input, cur.allow_screen)
            .await
    }

    pub async fn rdp_status(&self) -> RdpStatus {
        let cur = self.state.settings.current().await;
        inspect_rdp_status(effective_rdp_port(&cur)).await
    }

    pub async fn collect_snapshot(&self) -> DaemonControlSnapshot {
        let (trusted_peers, active_peer_ids, settings, pairing, portal, rdp) = tokio::join!(
            self.state.trusted_peers.list(),
            self.active_peer_ids(),
            self.state.settings.current(),
            self.current_pairing(),
            self.portal_readiness(),
            self.rdp_status(),
        );
        let cmd_path = self.state.paths.commands_file.clone();
        let commands = tokio::task::spawn_blocking(move || load_or_create_commands(&cmd_path))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default()
            .into_iter()
            .map(|c| c.info())
            .collect();

        DaemonControlSnapshot {
            host_name: self.state.host_name.clone(),
            endpoint_id: self.endpoint.id().to_string(),
            relay_url: current_relay_url(&self.endpoint),
            active_peer_ids,
            trusted_peers,
            pairing,
            settings,
            portal,
            rdp,
            commands,
        }
    }

    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        self.state.pairing.cancel_active().await;
        self.state.terminal.shutdown().await;
        self.state
            .connections
            .close_all(503, b"host shutdown")
            .await;
        self.state.portal.release_active_session().await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), self.endpoint.close()).await;
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

/// Inicia o daemon Iroh em segundo plano e retorna um [`HostControlHandle`] para a interface Desktop.
pub async fn start_embedded_server(
    secret_key: SecretKey,
    paths: HostPaths,
    trusted_peers: TrustedPeersStore,
    settings: SettingsStore,
) -> Result<HostControlHandle> {
    #[cfg(target_os = "linux")]
    let instance_lock = Arc::new(acquire_instance_lock(
        &paths.secret_key_file.with_file_name("host.lock"),
    )?);

    let endpoint = Endpoint::builder(presets::N0)
        .transport_config(build_host_transport_config())
        .secret_key(secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;

    let ep_online = endpoint.clone();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(6), ep_online.online()).await;
    });

    let host_name = local_hostname();
    let home_root = discover_home_root()?;
    let portal = Arc::new(PortalCoordinator::from_host_paths(&paths));

    let state = ServerState {
        host_secret_key: secret_key,
        host_name,
        paths: paths.clone(),
        home_root,
        trusted_peers,
        settings,
        connections: Arc::new(ConnectionRegistry::default()),
        pairing: Arc::new(PairingManager::new()),
        input: Arc::new(InputController::with_portal(portal.clone())),
        screen: Arc::new(ScreenController::with_portal(portal.clone())),
        terminal: Arc::new(TerminalSessionManager::new()),
        portal,
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let _sync_handle = spawn_state_sync_task(state.clone(), shutdown_rx);

    let accept_ep = endpoint.clone();
    let accept_state = state.clone();
    let mut accept_shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = accept_shutdown_rx.changed() => break,
                incoming = accept_ep.accept() => {
                    let Some(incoming) = incoming else {
                        break;
                    };
                    let conn_state = accept_state.clone();
                    tokio::spawn(async move {
                        if let Ok(conn) = incoming.await {
                            if let Err(err) = handle_connection(conn, conn_state).await {
                                warn!("Conexão encerrada com erro: {err:#}");
                            }
                        }
                    });
                }
            }
        }
    });

    Ok(HostControlHandle {
        endpoint,
        state,
        shutdown_tx,
        #[cfg(target_os = "linux")]
        _instance_lock: instance_lock,
    })
}

#[derive(Clone)]
struct ServerState {
    host_secret_key: SecretKey,
    host_name: String,
    paths: HostPaths,
    home_root: PathBuf,
    trusted_peers: TrustedPeersStore,
    settings: SettingsStore,
    connections: Arc<ConnectionRegistry>,
    pairing: Arc<PairingManager>,
    input: Arc<InputController>,
    screen: Arc<ScreenController>,
    terminal: Arc<TerminalSessionManager>,
    portal: Arc<PortalCoordinator>,
}

fn spawn_state_sync_task(
    state: ServerState,
    mut shutdown_rx: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    let mut settings_rx = state.settings.subscribe();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match state.trusted_peers.sync_from_disk().await {
                        Ok(removed) => {
                            for peer_id in removed {
                                warn!("Dispositivo revogado em disco; encerrando sessões de {peer_id}");
                                state.connections.close_peer(&peer_id).await;
                                state.terminal.kill_for_peer(&peer_id).await;
                            }
                        }
                        Err(err) => warn!("Falha ao sincronizar trusted_peers.json: {err:#}"),
                    }
                    if let Err(err) = state.settings.sync_from_disk().await {
                        warn!("Falha ao sincronizar settings.toml: {err:#}");
                    }
                }
                changed = settings_rx.changed() => {
                    if changed.is_ok() && !settings_rx.borrow_and_update().allow_terminal {
                        state
                            .terminal
                            .kill_all_with_reason(Some("Terminal remoto desativado pelo host"))
                            .await;
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    })
}

/// Inicia o daemon Iroh do Oráculo para aceitar pareamentos e conexões confiáveis.
pub async fn run_server(
    secret_key: SecretKey,
    paths: HostPaths,
    trusted_peers: TrustedPeersStore,
    settings: SettingsStore,
    pair_on_start: bool,
) -> Result<()> {
    #[cfg(target_os = "linux")]
    let instance_lock = Arc::new(acquire_instance_lock(
        &paths.secret_key_file.with_file_name("host.lock"),
    )?);

    #[cfg(target_os = "linux")]
    ensure_portal_app_registered().await;

    let endpoint = Endpoint::builder(presets::N0)
        .transport_config(build_host_transport_config())
        .secret_key(secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;

    // Aguarda conexão inicial ao relay/discovery para publicar o endereço antes de aceitar pareamentos.
    if tokio::time::timeout(Duration::from_secs(6), endpoint.online())
        .await
        .is_err()
    {
        warn!("Tempo esgotado aguardando relay online; operando com descoberta local/assíncrona.");
    }

    let endpoint_id = endpoint.id();
    let host_name = local_hostname();
    let trusted_count = trusted_peers.count().await;
    let home_root = discover_home_root()?;
    let portal = Arc::new(PortalCoordinator::from_host_paths(&paths));

    let state = ServerState {
        host_secret_key: secret_key,
        host_name: host_name.clone(),
        paths: paths.clone(),
        home_root: home_root.clone(),
        trusted_peers,
        settings,
        connections: Arc::new(ConnectionRegistry::default()),
        pairing: Arc::new(PairingManager::new()),
        input: Arc::new(InputController::with_portal(portal.clone())),
        screen: Arc::new(ScreenController::with_portal(portal.clone())),
        terminal: Arc::new(TerminalSessionManager::new()),
        portal,
    };

    let relay_url = current_relay_url(&endpoint);
    println!("Oráculo Host ativo!");
    println!("  Hostname:          {host_name}");
    println!("  EndpointId:        {endpoint_id}");
    if let Some(relay) = &relay_url {
        println!("  Relay ativo:       {relay}");
    }
    println!("  Peers confiáveis:  {trusted_count}");
    println!("  Arquivos (sandbox):{}", home_root.display());
    println!("  Comandos rápidos:  {}", paths.commands_file.display());
    println!("  Permissões/WoL:    {}", paths.settings_file.display());
    let initial_settings = state.settings.current().await;
    if !initial_settings.wol_enabled {
        println!(
            "  Wake-on-LAN:       desativado (rode 'oraculo-host wol --enable' para ativar)"
        );
    } else if let Some(wol) = resolve_host_wol_info(&initial_settings) {
        let macs_label = wol.macs.join(", ");
        let bc_label = wol.broadcast.as_deref().unwrap_or("255.255.255.255");
        println!("  Wake-on-LAN:       ativo ({macs_label} · broadcast {bc_label})");
    } else {
        println!("  Wake-on-LAN:       sem placa física detectada");
    }
    println!("  Dica: digite 'p' + Enter para gerar um novo QR Code / código de pareamento (Ctrl+C para sair).");

    if pair_on_start {
        issue_and_spawn_pairing(&endpoint, &state).await;
    }

    spawn_stdin_qr_listener(endpoint.clone(), state.clone());
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let control_handle = HostControlHandle {
        endpoint: endpoint.clone(),
        state: state.clone(),
        shutdown_tx: shutdown_tx.clone(),
        #[cfg(target_os = "linux")]
        _instance_lock: instance_lock,
    };
    let _control_guard = match start_daemon_control_server(control_handle).await {
        Ok(guard) => Some(guard),
        Err(err) => {
            warn!("Não foi possível abrir canal de controle local do daemon: {err:#}");
            None
        }
    };

    let sync_task = spawn_state_sync_task(state.clone(), shutdown_rx);

    let mut loop_shutdown_rx = shutdown_tx.subscribe();
    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else {
                    break;
                };
                let conn_state = state.clone();
                tokio::spawn(async move {
                    match incoming.await {
                        Ok(conn) => {
                            if let Err(err) = handle_connection(conn, conn_state).await {
                                warn!("Conexão encerrada com erro: {err:#}");
                            }
                        }
                        Err(err) => warn!("Falha no handshake Iroh: {err:#}"),
                    }
                });
            }
            _ = async {
                loop {
                    if *loop_shutdown_rx.borrow() {
                        break;
                    }
                    if loop_shutdown_rx.changed().await.is_err() {
                        std::future::pending::<()>().await;
                    }
                }
            } => {
                println!("\nEncerrando Oráculo Host...");
                break;
            }
            _ = wait_shutdown_signal() => {
                println!("\nEncerrando Oráculo Host...");
                break;
            }
        }
    }

    drop(_control_guard);
    let _ = shutdown_tx.send(true);
    state.terminal.shutdown().await;
    state
        .connections
        .close_all(503, b"host shutdown")
        .await;
    state.portal.release_active_session().await;
    let _ = tokio::time::timeout(Duration::from_millis(500), sync_task).await;
    let _ = tokio::time::timeout(Duration::from_millis(800), endpoint.close()).await;
    Ok(())
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
    endpoint
        .addr()
        .relay_urls()
        .next()
        .map(ToString::to_string)
}

fn spawn_stdin_qr_listener(endpoint: Endpoint, state: ServerState) {
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();

    // Usa thread OS dedicada para não prender o pool bloqueante do Tokio durante o shutdown.
    let _ = std::thread::Builder::new()
        .name("oraculo-stdin".to_string())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut reader = BufReader::new(stdin.lock());
            let mut line = String::new();
            while let Ok(bytes) = reader.read_line(&mut line) {
                if bytes == 0 {
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
            issue_and_spawn_pairing(&endpoint, &state).await;
        }
    });
}

async fn issue_and_spawn_pairing(main_endpoint: &Endpoint, state: &ServerState) {
    let relay_url = current_relay_url(main_endpoint);
    let issued = state
        .pairing
        .issue_and_print(main_endpoint.id(), &state.host_name, relay_url.as_deref())
        .await;

    let main_ep = main_endpoint.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(err) = run_code_rendezvous_listener(issued, main_ep, state_clone).await {
            warn!("Encerramento do canal de código curto: {err:#}");
        }
    });
}

async fn run_code_rendezvous_listener(
    mut issued: IssuedPairing,
    main_endpoint: Endpoint,
    state: ServerState,
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
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
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
                    conn.close(VarInt::from_u32(400), b"stream timeout");
                    continue;
                };
                let Ok(Ok(Some(req))) = tokio::time::timeout(
                    Duration::from_secs(5),
                    read_frame::<_, CodePairRequest>(&mut recv),
                )
                .await
                else {
                    conn.close(VarInt::from_u32(400), b"invalid request");
                    continue;
                };

                if state.pairing.consume_code_token(&req.auth_token).await {
                    let remote_client = conn.remote_id();
                    let device_name = sanitize_device_name(&req.device_name);
                    state
                        .trusted_peers
                        .add_or_update(remote_client.to_string(), device_name.clone())
                        .await?;

                    let host_id = state.host_secret_key.public();
                    let digest = compute_pair_binding_digest(
                        rendezvous_ep.id().as_bytes(),
                        remote_client.as_bytes(),
                        host_id.as_bytes(),
                    );
                    let sig = state.host_secret_key.sign(&digest).to_bytes().to_vec();
                    let current_settings = state.settings.current().await;

                    let resp = CodePairResponse {
                        accepted: true,
                        message: "Pareado por código com sucesso".to_string(),
                        host_endpoint_id: host_id.to_string(),
                        host_name: state.host_name.clone(),
                        relay_url: current_relay_url(&main_endpoint),
                        wol: resolve_host_wol_info(&current_settings),
                        host_signature: sig,
                        role: NodeRole::Computer,
                    };
                    let _ = write_frame(&mut send, &resp).await;
                    let _ = send.finish();
                    println!("Novo dispositivo pareado via código: {device_name} ({remote_client})");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    break;
                } else {
                    let resp = CodePairResponse {
                        accepted: false,
                        message: "Código de pareamento inválido ou expirado".to_string(),
                        host_endpoint_id: String::new(),
                        host_name: String::new(),
                        relay_url: None,
                        wol: None,
                        host_signature: Vec::new(),
                        role: NodeRole::Computer,
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

async fn handle_connection(conn: Connection, state: ServerState) -> Result<()> {
    let remote_id = conn.remote_id().to_string();
    let mut registered_seq: Option<u64> = if state.trusted_peers.is_trusted(&remote_id).await {
        Some(state.connections.register(&remote_id, &conn).await)
    } else {
        None
    };
    info!("Conexão estabelecida com {remote_id}");

    loop {
        let is_trusted_now = registered_seq.is_some()
            || state.trusted_peers.is_trusted(&remote_id).await;
        if is_trusted_now && registered_seq.is_none() {
            registered_seq = Some(state.connections.register(&remote_id, &conn).await);
        }

        let accepted = if is_trusted_now {
            conn.accept_bi().await.ok()
        } else {
            match tokio::time::timeout(UNTRUSTED_STREAM_TIMEOUT, conn.accept_bi()).await {
                Ok(Ok(streams)) => Some(streams),
                Ok(Err(_)) => None,
                Err(_) => {
                    warn!("Conexão não pareada ociosa encerrada por timeout: {remote_id}");
                    conn.close(iroh::endpoint::VarInt::from_u32(408), b"pair timeout");
                    None
                }
            }
        };
        let Some((send, mut recv)) = accepted else {
            break;
        };

        let kind = match tokio::time::timeout(
            UNTRUSTED_STREAM_TIMEOUT,
            read_stream_kind(&mut recv),
        )
        .await
        {
            Ok(Ok(kind)) => kind,
            Ok(Err(err)) => {
                warn!("Cabeçalho de stream inválido de {remote_id}: {err:#}");
                if !is_trusted_now {
                    conn.close(iroh::endpoint::VarInt::from_u32(401), b"invalid stream");
                    break;
                }
                continue;
            }
            Err(_) => {
                warn!("Timeout lendo tipo de stream de {remote_id}");
                if !is_trusted_now {
                    conn.close(iroh::endpoint::VarInt::from_u32(408), b"stream timeout");
                    break;
                }
                continue;
            }
        };

        if kind == StreamKind::Pair {
            let state_clone = state.clone();
            let id_clone = remote_id.clone();
            let conn_clone = conn.clone();
            let pair_ok = handle_pair_stream(send, recv, id_clone, state_clone, conn_clone).await;
            match pair_ok {
                Ok(true) => {
                    if registered_seq.is_none() && state.trusted_peers.is_trusted(&remote_id).await {
                        registered_seq = Some(state.connections.register(&remote_id, &conn).await);
                    }
                    continue;
                }
                Ok(false) => break,
                Err(err) => {
                    warn!("Erro no pareamento: {err:#}");
                    if !state.trusted_peers.is_trusted(&remote_id).await {
                        conn.close(iroh::endpoint::VarInt::from_u32(401), b"pair error");
                        break;
                    }
                    continue;
                }
            }
        }

        if !state.trusted_peers.is_trusted(&remote_id).await {
            warn!("Acesso negado para peer não pareado: {remote_id}");
            if kind == StreamKind::Control {
                let mut send = send;
                let _ = write_frame(
                    &mut send,
                    &ControlResponse::Error("Dispositivo não pareado".into()),
                )
                .await;
                let _ = send.finish();
            }
            conn.close(iroh::endpoint::VarInt::from_u32(401), b"unpaired");
            break;
        }
        if registered_seq.is_none() {
            registered_seq = Some(state.connections.register(&remote_id, &conn).await);
        }

        match kind {
            StreamKind::Control => {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_control_stream(send, recv, state_clone).await {
                        warn!("Erro no stream de controle: {err:#}");
                    }
                });
            }
            StreamKind::Terminal => {
                let settings_rx = state.settings.subscribe();
                let manager = state.terminal.clone();
                let peer = remote_id.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_terminal_stream(send, recv, peer, manager, settings_rx).await
                    {
                        warn!("Erro na sessão de terminal: {err:#}");
                    }
                });
            }
            StreamKind::Input => {
                let input = state.input.clone();
                let settings_rx = state.settings.subscribe();
                tokio::spawn(async move {
                    if let Err(err) = handle_input_stream(send, recv, input, settings_rx).await {
                        warn!("Erro na sessão de entrada remota: {err:#}");
                    }
                });
            }
            StreamKind::Files => {
                let home_root = state.home_root.clone();
                let settings_rx = state.settings.subscribe();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_files_stream(send, recv, home_root, settings_rx).await
                    {
                        warn!("Erro no stream de arquivos: {err:#}");
                    }
                });
            }
            StreamKind::Screen => {
                let screen = state.screen.clone();
                let settings_rx = state.settings.subscribe();
                tokio::spawn(async move {
                    if let Err(err) = handle_screen_stream(send, recv, screen, settings_rx).await {
                        warn!("Erro na sessão de streaming de tela: {err:#}");
                    }
                });
            }
            StreamKind::Tunnel => {
                let settings_rx = state.settings.subscribe();
                tokio::spawn(async move {
                    if let Err(err) = handle_tunnel_stream(send, recv, settings_rx).await {
                        warn!("Erro no túnel RDP: {err:#}");
                    }
                });
            }
            StreamKind::Pair => unreachable!(),
        }
    }

    if let Some(conn_seq) = registered_seq {
        state.connections.unregister(&remote_id, conn_seq).await;
        if state.connections.active_peer_ids().await.is_empty() {
            if is_explicit_close_reason(conn.close_reason()) {
                state.portal.release_active_session().await;
            } else {
                // Em quedas transitórias de rede (ex.: Wi-Fi <-> 4G no celular), mantém a sessão do
                // portal aquecida por alguns segundos para retomada instantânea se o cliente reconectar.
                let gen_before = state.connections.current_generation();
                let connections = Arc::clone(&state.connections);
                let portal = Arc::clone(&state.portal);
                tokio::spawn(async move {
                    tokio::time::sleep(PORTAL_RELEASE_GRACE).await;
                    if connections.current_generation() == gen_before
                        && connections.active_peer_ids().await.is_empty()
                    {
                        portal.release_active_session().await;
                    }
                });
            }
        }
    }
    Ok(())
}

fn sanitize_device_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "dispositivo-sem-nome".to_string()
    } else {
        cleaned
    }
}

async fn handle_pair_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    remote_id: String,
    state: ServerState,
    conn: Connection,
) -> Result<bool> {
    let req_opt: Option<PairRequest> = match tokio::time::timeout(
        PAIR_READ_TIMEOUT,
        read_frame(&mut recv),
    )
    .await
    {
        Ok(Ok(opt)) => opt,
        Ok(Err(err)) => {
            if !state.trusted_peers.is_trusted(&remote_id).await {
                conn.close(iroh::endpoint::VarInt::from_u32(401), b"invalid pair");
            }
            return Err(err);
        }
        Err(_) => {
            if !state.trusted_peers.is_trusted(&remote_id).await {
                conn.close(iroh::endpoint::VarInt::from_u32(408), b"pair timeout");
            }
            return Ok(false);
        }
    };
    let Some(req) = req_opt else {
        if !state.trusted_peers.is_trusted(&remote_id).await {
            conn.close(iroh::endpoint::VarInt::from_u32(401), b"empty pair");
            return Ok(false);
        }
        return Ok(true);
    };

    let response = if state.pairing.consume_if_valid(&req.secret).await {
        let device_name = sanitize_device_name(&req.device_name);
        state
            .trusted_peers
            .add_or_update(remote_id.clone(), device_name.clone())
            .await?;
        let current_settings = state.settings.current().await;
        println!("Novo dispositivo pareado: {device_name} ({remote_id})");
        PairResponse {
            accepted: true,
            host_name: state.host_name.clone(),
            message: "Pareado com sucesso".to_string(),
            wol: resolve_host_wol_info(&current_settings),
            role: NodeRole::Computer,
        }
    } else {
        warn!("Tentativa de pareamento rejeitada de {remote_id}");
        PairResponse {
            accepted: false,
            host_name: state.host_name.clone(),
            message: "Token de pareamento inválido ou expirado".to_string(),
            wol: None,
            role: NodeRole::Computer,
        }
    };

    write_frame(&mut send, &response).await?;
    send.finish()?;
    if !response.accepted && !state.trusted_peers.is_trusted(&remote_id).await {
        tokio::time::sleep(Duration::from_millis(40)).await;
        conn.close(iroh::endpoint::VarInt::from_u32(401), b"pair rejected");
        return Ok(false);
    }
    Ok(true)
}

async fn handle_control_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    state: ServerState,
) -> Result<()> {
    while let Some(req) = read_frame::<_, ControlRequest>(&mut recv).await? {
        let perms = state.settings.current().await;
        let resp = match req {
            ControlRequest::Ping => ControlResponse::Pong {
                host_name: state.host_name.clone(),
                wol: resolve_host_wol_info(&perms),
                role: NodeRole::Computer,
            },
            ControlRequest::ListCommands => {
                match load_or_create_commands(&state.paths.commands_file) {
                    Ok(specs) => {
                        ControlResponse::Commands(specs.iter().map(|s| s.info()).collect())
                    }
                    Err(err) => ControlResponse::Error(format!("Erro ao ler comandos: {err:#}")),
                }
            }
            ControlRequest::RunCommand { id } => {
                if !perms.allow_commands_run {
                    ControlResponse::Error(
                        "Execução de comandos rápidos desativada pelo host".into(),
                    )
                } else {
                    match load_or_create_commands(&state.paths.commands_file) {
                        Ok(specs) => match specs.into_iter().find(|s| s.id == id) {
                            Some(spec) => {
                                let mut cmd_settings_rx = state.settings.subscribe();
                                tokio::select! {
                                    biased;
                                    _ = async {
                                        loop {
                                            if !cmd_settings_rx.borrow().allow_commands_run {
                                                break;
                                            }
                                            if cmd_settings_rx.changed().await.is_err() {
                                                std::future::pending::<()>().await;
                                            }
                                        }
                                    } => {
                                        ControlResponse::Error(
                                            "Execução de comandos rápidos desativada pelo host".into(),
                                        )
                                    }
                                    _ = send.stopped() => {
                                        return Ok(());
                                    }
                                    out = run_quick_command(&spec) => {
                                        ControlResponse::CommandResult(out)
                                    }
                                }
                            }
                            None => {
                                ControlResponse::Error(format!("Comando '{id}' não encontrado"))
                            }
                        },
                        Err(err) => {
                            ControlResponse::Error(format!("Erro ao ler comandos: {err:#}"))
                        }
                    }
                }
            }
            ControlRequest::SaveCommand {
                id,
                label,
                description,
                command_line,
            } => {
                if !perms.allow_commands_edit || !perms.allow_terminal {
                    ControlResponse::Error(
                        "Edição de comandos rápidos desativada pelo host".into(),
                    )
                } else {
                    let _guard = crate::config::commands_io_lock().lock().await;
                    match save_command(
                        &state.paths.commands_file,
                        id.as_deref(),
                        &label,
                        &description,
                        &command_line,
                    ) {
                        Ok(specs) => {
                            ControlResponse::Commands(specs.iter().map(|s| s.info()).collect())
                        }
                        Err(err) => ControlResponse::Error(format!("{err:#}")),
                    }
                }
            }
            ControlRequest::DeleteCommand { id } => {
                if !perms.allow_commands_edit || !perms.allow_terminal {
                    ControlResponse::Error(
                        "Edição de comandos rápidos desativada pelo host".into(),
                    )
                } else {
                    let _guard = crate::config::commands_io_lock().lock().await;
                    match delete_command(&state.paths.commands_file, &id) {
                        Ok(specs) => {
                            ControlResponse::Commands(specs.iter().map(|s| s.info()).collect())
                        }
                        Err(err) => ControlResponse::Error(format!("{err:#}")),
                    }
                }
            }
            ControlRequest::ListFiles {
                rel_path,
                show_hidden,
            } => {
                if !perms.allow_files_read {
                    ControlResponse::Error("Acesso a arquivos desativado pelo host".into())
                } else {
                    let home_root = state.home_root.clone();
                    match tokio::task::spawn_blocking(move || {
                        list_directory(&home_root, &rel_path, show_hidden)
                    })
                    .await
                    {
                        Ok(Ok(listing)) => ControlResponse::DirectoryListing(listing),
                        Ok(Err(err)) => ControlResponse::Error(format!("{err:#}")),
                        Err(err) => ControlResponse::Error(format!("Falha ao listar pasta: {err}")),
                    }
                }
            }
            ControlRequest::RelayWakeOnLan { wol } => {
                if !perms.wol_enabled {
                    ControlResponse::Error("Ponte Wake-on-LAN desativada neste host".into())
                } else if let Some(clean) = sanitize_wol_info(wol) {
                    let effective_bc = clean
                        .broadcast
                        .or_else(|| resolve_host_wol_info(&perms).and_then(|local| local.broadcast));
                    match send_magic_packets(
                        &clean.macs,
                        effective_bc.as_deref(),
                        None,
                        DEFAULT_WOL_PORT,
                    )
                    .await
                    {
                        Ok(sent) => ControlResponse::WakeDispatched {
                            packets_sent: sent as u32,
                        },
                        Err(err) => ControlResponse::Error(format!(
                            "Falha ao retransmitir Wake-on-LAN na LAN local: {err:#}"
                        )),
                    }
                } else {
                    ControlResponse::Error("Dados de Wake-on-LAN inválidos para retransmissão".into())
                }
            }
            ControlRequest::WatchState => {
                let mut settings_rx = state.settings.subscribe();
                let initial = settings_rx.borrow_and_update().clone();
                write_frame(
                    &mut send,
                    &ControlResponse::HostState {
                        host_name: state.host_name.clone(),
                        wol: resolve_host_wol_info(&initial),
                        permissions: permissions_wire(&initial),
                    },
                )
                .await?;
                loop {
                    tokio::select! {
                        _ = send.stopped() => break,
                        changed = settings_rx.changed() => {
                            if changed.is_err() {
                                break;
                            }
                            let next = settings_rx.borrow_and_update().clone();
                            if write_frame(
                                &mut send,
                                &ControlResponse::HostState {
                                    host_name: state.host_name.clone(),
                                    wol: resolve_host_wol_info(&next),
                                    permissions: permissions_wire(&next),
                                },
                            )
                            .await
                            .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
                return Ok(());
            }
            ControlRequest::ListTerminalSessions => {
                if !perms.allow_terminal {
                    ControlResponse::Error("Terminal remoto desativado pelo host".into())
                } else {
                    ControlResponse::TerminalSessions(state.terminal.list_sessions().await)
                }
            }
            ControlRequest::KillTerminalSession { id } => {
                if !perms.allow_terminal {
                    ControlResponse::Error("Terminal remoto desativado pelo host".into())
                } else {
                    match state.terminal.kill_session(&id).await {
                        Ok(true) => ControlResponse::TerminalSessionClosed { id },
                        Ok(false) => {
                            ControlResponse::Error(format!("Sessão '{id}' não encontrada"))
                        }
                        Err(err) => ControlResponse::Error(format!("{err:#}")),
                    }
                }
            }
        };

        write_frame(&mut send, &resp).await?;
    }

    send.finish()?;
    Ok(())
}

fn permissions_wire(perms: &crate::config::HostSettings) -> HostPermissionsWire {
    HostPermissionsWire {
        allow_screen: perms.allow_screen,
        allow_input: perms.allow_input,
        allow_terminal: perms.allow_terminal,
        allow_files_read: perms.allow_files_read,
        allow_files_write: perms.allow_files_write,
        allow_commands_run: perms.allow_commands_run,
        allow_commands_edit: perms.allow_commands_edit,
        allow_rdp_tunnel: perms.allow_rdp_tunnel,
    }
}

#[cfg(target_os = "linux")]
fn acquire_instance_lock(lock_path: &std::path::Path) -> Result<std::fs::File> {
    use std::os::fd::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            anyhow::bail!(
                "Já existe outro processo oraculo-host em execução (lock ativo em {}). Encerre a instância anterior antes de abrir outra.",
                lock_path.display()
            );
        }
        return Err(err.into());
    }
    Ok(file)
}
