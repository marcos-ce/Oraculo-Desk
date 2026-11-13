use crate::commands::run_quick_command;
use crate::config::{
    HostPaths, SettingsStore, TrustedPeersStore, delete_command, load_or_create_commands,
    local_hostname, save_command,
};
use crate::files::{discover_home_root, handle_files_stream, list_directory};
use crate::input::{InputController, handle_input_stream};
use crate::pairing::PairingManager;
use crate::pty::handle_terminal_stream;
use crate::screen::{ScreenController, handle_screen_stream};
use anyhow::Result;
use iroh::endpoint::{
    Connection, IdleTimeout, QuicTransportConfig, RecvStream, SendStream, VarInt, presets,
};
use iroh::{Endpoint, SecretKey};
use oraculo_proto::{
    ALPN, ControlRequest, ControlResponse, PairRequest, PairResponse, StreamKind, read_frame,
    read_stream_kind, write_frame,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, mpsc, watch};
use tracing::{info, warn};

#[derive(Default)]
struct ConnectionRegistry {
    next_seq: AtomicU64,
    active: RwLock<HashMap<String, Vec<(u64, Connection)>>>,
}

impl ConnectionRegistry {
    async fn register(&self, peer_id: &str, conn: &Connection) -> u64 {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
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
}

#[derive(Clone)]
struct ServerState {
    host_name: String,
    paths: HostPaths,
    home_root: PathBuf,
    trusted_peers: TrustedPeersStore,
    settings: SettingsStore,
    connections: Arc<ConnectionRegistry>,
    pairing: Arc<PairingManager>,
    input: Arc<InputController>,
    screen: Arc<ScreenController>,
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
    let _instance_lock = acquire_instance_lock(&paths.secret_key_file.with_file_name("host.lock"))?;

    let mut transport_builder =
        QuicTransportConfig::builder().keep_alive_interval(Duration::from_secs(5));
    if let Ok(idle) = IdleTimeout::try_from(Duration::from_secs(120)) {
        transport_builder = transport_builder.max_idle_timeout(Some(idle));
    }

    let endpoint = Endpoint::builder(presets::N0)
        .transport_config(transport_builder.build())
        .secret_key(secret_key)
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

    let state = ServerState {
        host_name: host_name.clone(),
        paths: paths.clone(),
        home_root: home_root.clone(),
        trusted_peers,
        settings,
        connections: Arc::new(ConnectionRegistry::default()),
        pairing: Arc::new(PairingManager::new()),
        input: Arc::new(InputController::new(paths.remote_desktop_token_file.clone())),
        screen: Arc::new(ScreenController::new(paths.screencast_token_file.clone())),
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
    println!("  Dica: digite 'p' + Enter para gerar um novo QR Code de pareamento (Ctrl+C para sair).");

    if pair_on_start {
        state
            .pairing
            .issue_and_print(endpoint_id, &state.host_name, relay_url.as_deref())
            .await;
    }

    spawn_stdin_qr_listener(endpoint.clone(), state.clone());
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let sync_state = state.clone();
    let sync_task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match sync_state.trusted_peers.sync_from_disk().await {
                        Ok(removed) => {
                            for peer_id in removed {
                                warn!("Dispositivo revogado em disco; encerrando conexões de {peer_id}");
                                sync_state.connections.close_peer(&peer_id).await;
                            }
                        }
                        Err(err) => warn!("Falha ao sincronizar trusted_peers.json: {err:#}"),
                    }
                    if let Err(err) = sync_state.settings.sync_from_disk().await {
                        warn!("Falha ao sincronizar settings.toml: {err:#}");
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    });

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
            _ = wait_shutdown_signal() => {
                println!("\nEncerrando Oráculo Host...");
                break;
            }
        }
    }

    let _ = shutdown_tx.send(true);
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
            let relay_url = current_relay_url(&endpoint);
            state
                .pairing
                .issue_and_print(endpoint.id(), &state.host_name, relay_url.as_deref())
                .await;
        }
    });
}

async fn handle_connection(conn: Connection, state: ServerState) -> Result<()> {
    let remote_id = conn.remote_id().to_string();
    let conn_seq = state.connections.register(&remote_id, &conn).await;
    info!("Conexão estabelecida com {remote_id}");

    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };

        let kind = match read_stream_kind(&mut recv).await {
            Ok(kind) => kind,
            Err(err) => {
                warn!("Cabeçalho de stream inválido de {remote_id}: {err:#}");
                continue;
            }
        };

        if kind == StreamKind::Pair {
            let state_clone = state.clone();
            let id_clone = remote_id.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_pair_stream(send, recv, id_clone, state_clone).await {
                    warn!("Erro no pareamento: {err:#}");
                }
            });
            continue;
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
                tokio::spawn(async move {
                    if let Err(err) = handle_terminal_stream(send, recv, settings_rx).await {
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
            StreamKind::Pair => unreachable!(),
        }
    }

    state.connections.unregister(&remote_id, conn_seq).await;
    Ok(())
}

fn commands_io_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
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
) -> Result<()> {
    let Some(req): Option<PairRequest> = read_frame(&mut recv).await? else {
        return Ok(());
    };

    let response = if state.pairing.consume_if_valid(&req.secret).await {
        let device_name = sanitize_device_name(&req.device_name);
        state
            .trusted_peers
            .add_or_update(remote_id.clone(), device_name.clone())
            .await?;
        println!("Novo dispositivo pareado: {device_name} ({remote_id})");
        PairResponse {
            accepted: true,
            host_name: state.host_name.clone(),
            message: "Pareado com sucesso".to_string(),
        }
    } else {
        warn!("Tentativa de pareamento rejeitada de {remote_id}");
        PairResponse {
            accepted: false,
            host_name: state.host_name.clone(),
            message: "Token de pareamento inválido ou expirado".to_string(),
        }
    };

    write_frame(&mut send, &response).await?;
    send.finish()?;
    if !response.accepted && !state.trusted_peers.is_trusted(&remote_id).await {
        state.connections.close_peer(&remote_id).await;
    }
    Ok(())
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
                                ControlResponse::CommandResult(run_quick_command(&spec).await)
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
                    let _guard = commands_io_lock().lock().await;
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
                    let _guard = commands_io_lock().lock().await;
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
        };

        write_frame(&mut send, &resp).await?;
    }

    send.finish()?;
    Ok(())
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
