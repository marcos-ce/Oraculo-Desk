use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use iroh::endpoint::{Connection, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayUrl, SecretKey, Signature};
use oraculo_proto::pairing::{
    ALPN_PAIR_CODE, compute_pair_binding_digest, decode_pairing_uri, derive_short_code_secrets,
};
use oraculo_proto::wol::{
    DEFAULT_WOL_PORT, normalize_wol_mac, sanitize_wol_info, send_magic_packets,
};
use oraculo_proto::{
    ALPN, CodePairRequest, CodePairResponse, ControlRequest, ControlResponse, PairRequest,
    PairResponse, StreamKind, TerminalClientMsg, TerminalHostMsg, TerminalInit, WolInfo,
    read_frame, write_frame, write_stream_kind,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

#[derive(Parser)]
#[command(name = "oraculo-cli", about = "Cliente CLI do Oráculo (pareamento, Wake-on-LAN, comandos e shell PTY)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pareia com um host usando o código rápido (XXXX-XXXX) ou a URI do QR Code
    Pair {
        /// Código de 8 caracteres (ex.: K7P4-M9X2) ou URI oraculo://pair?...
        uri: String,
        #[arg(long, default_value = "oraculo-cli")]
        name: String,
    },
    /// Testa conectividade e mede latência até o host
    Ping {
        #[arg(long)]
        host: Option<String>,
    },
    /// Lista comandos rápidos configurados no host
    Commands {
        #[arg(long)]
        host: Option<String>,
    },
    /// Executa um comando rápido no host
    Run {
        id: String,
        #[arg(long)]
        host: Option<String>,
    },
    /// Abre uma sessão interativa de terminal PTY no host
    Shell {
        #[arg(long)]
        host: Option<String>,
    },
    /// Envia pacote Wake-on-LAN (Magic Packet UDP) para ligar o host
    Wake {
        /// Endereço MAC explícito (usa o salvo no pareamento/ping se omitido)
        #[arg(long)]
        mac: Option<String>,
        /// IP/domínio de destino ou broadcast customizado
        #[arg(long)]
        target: Option<String>,
        /// Porta UDP de destino (padrão: 9)
        #[arg(long, default_value_t = DEFAULT_WOL_PORT)]
        port: u16,
        /// EndpointId opcional de outro dispositivo Oráculo online na mesma casa/rede (Ponte P2P)
        #[arg(long)]
        bridge: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct SavedHost {
    endpoint_id: String,
    host_name: String,
    #[serde(default)]
    relay_url: Option<String>,
    #[serde(default)]
    wol_macs: Vec<String>,
    #[serde(default)]
    wol_broadcast: Option<String>,
}

struct ClientPaths {
    key_file: PathBuf,
    default_host_file: PathBuf,
}

impl ClientPaths {
    fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "oraculo", "oraculo-cli")
            .context("Não foi possível determinar diretório de dados do cliente")?;
        let data_dir = dirs.data_local_dir();
        fs::create_dir_all(data_dir)?;
        Ok(Self {
            key_file: data_dir.join("client.key"),
            default_host_file: data_dir.join("default_host.json"),
        })
    }
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = ClientPaths::discover()?;

    if let Command::Wake {
        mac,
        target,
        port,
        bridge: None,
    } = &cli.command
    {
        return run_wake_command(
            None,
            &paths.default_host_file,
            mac.clone(),
            target.clone(),
            *port,
            None,
        )
        .await;
    }

    let secret_key = load_or_create_secret_key(&paths.key_file)?;

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .bind()
        .await?;

    let _ = tokio::time::timeout(Duration::from_secs(6), endpoint.online()).await;

    let res = run_command(&endpoint, &paths, cli.command).await;
    endpoint.close().await;
    res
}

async fn run_command(endpoint: &Endpoint, paths: &ClientPaths, cmd: Command) -> Result<()> {
    match cmd {
        Command::Pair { uri, name } => {
            let saved = if uri.trim_start().starts_with("oraculo://") {
                pair_via_qr_uri(endpoint, &uri, name).await?
            } else {
                pair_via_short_code(endpoint, &uri, name).await?
            };
            let wol_macs = saved.wol_macs.clone();
            let host_name = saved.host_name.clone();
            write_private_file(
                &paths.default_host_file,
                serde_json::to_string_pretty(&saved)?.as_bytes(),
            )?;
            println!("Pareado com '{host_name}'!");
            if !wol_macs.is_empty() {
                println!("Wake-on-LAN sincronizado: {}", wol_macs.join(", "));
            }
        }
        Command::Ping { host } => {
            let is_default_host = host.is_none();
            let target = resolve_target_id(host, &paths.default_host_file)?;
            let conn = endpoint.connect(target, ALPN).await?;
            let started = Instant::now();
            match control_rpc(&conn, &ControlRequest::Ping).await? {
                ControlResponse::Pong {
                    host_name,
                    wol,
                    role: _,
                } => {
                    println!(
                        "Pong de '{}' em {} ms",
                        host_name,
                        started.elapsed().as_millis()
                    );
                    if is_default_host && paths.default_host_file.exists() {
                        if let Ok(raw) = fs::read_to_string(&paths.default_host_file) {
                            if let Ok(mut saved) = serde_json::from_str::<SavedHost>(&raw) {
                                saved.host_name = host_name;
                                if let Some(w) = wol.and_then(sanitize_wol_info) {
                                    saved.wol_macs = w.macs;
                                    saved.wol_broadcast = w.broadcast;
                                }
                                if let Ok(json) = serde_json::to_string_pretty(&saved) {
                                    let _ = write_private_file(
                                        &paths.default_host_file,
                                        json.as_bytes(),
                                    );
                                }
                            }
                        }
                    }
                }
                ControlResponse::Error(err) => bail!("Erro do host: {err}"),
                other => bail!("Resposta inesperada: {other:?}"),
            }
        }
        Command::Commands { host } => {
            let target = resolve_target_id(host, &paths.default_host_file)?;
            let conn = endpoint.connect(target, ALPN).await?;
            match control_rpc(&conn, &ControlRequest::ListCommands).await? {
                ControlResponse::Commands(items) => {
                    if items.is_empty() {
                        println!("Nenhum comando rápido configurado.");
                    } else {
                        for item in items {
                            println!("{} — {} ({})", item.id, item.label, item.description);
                        }
                    }
                }
                ControlResponse::Error(err) => bail!("Erro do host: {err}"),
                other => bail!("Resposta inesperada: {other:?}"),
            }
        }
        Command::Run { id, host } => {
            let target = resolve_target_id(host, &paths.default_host_file)?;
            let conn = endpoint.connect(target, ALPN).await?;
            match control_rpc(&conn, &ControlRequest::RunCommand { id }).await? {
                ControlResponse::CommandResult(out) => {
                    if !out.stdout.is_empty() {
                        print!("{}", out.stdout);
                    }
                    if !out.stderr.is_empty() {
                        eprint!("{}", out.stderr);
                    }
                    println!("[exit_code={}]", out.exit_code);
                }
                ControlResponse::Error(err) => bail!("Erro do host: {err}"),
                other => bail!("Resposta inesperada: {other:?}"),
            }
        }
        Command::Shell { host } => {
            let target = resolve_target_id(host, &paths.default_host_file)?;
            let conn = endpoint.connect(target, ALPN).await?;
            run_shell_session(&conn).await?;
        }
        Command::Wake {
            mac,
            target,
            port,
            bridge,
        } => {
            run_wake_command(
                Some(endpoint),
                &paths.default_host_file,
                mac,
                target,
                port,
                bridge,
            )
            .await?;
        }
    }

    Ok(())
}

async fn run_wake_command(
    endpoint: Option<&Endpoint>,
    default_file: &Path,
    explicit_mac: Option<String>,
    explicit_target: Option<String>,
    port: u16,
    bridge_id: Option<String>,
) -> Result<()> {
    let saved = if default_file.exists() {
        fs::read_to_string(default_file)
            .ok()
            .and_then(|raw| serde_json::from_str::<SavedHost>(&raw).ok())
    } else {
        None
    };

    let macs = if let Some(raw_mac) = explicit_mac {
        vec![normalize_wol_mac(&raw_mac)?]
    } else if let Some(s) = &saved {
        if s.wol_macs.is_empty() {
            bail!(
                "O host salvo '{}' ainda não possui MAC de Wake-on-LAN registrado. Execute 'oraculo-cli ping' com o PC ligado ou informe '--mac AA:BB:CC:DD:EE:FF'.",
                s.host_name
            );
        }
        s.wol_macs.clone()
    } else {
        bail!("Nenhum host salvo. Informe '--mac AA:BB:CC:DD:EE:FF' ou pareie primeiro.");
    };

    let subnet_bc = saved.as_ref().and_then(|s| s.wol_broadcast.clone());
    let direct_res = send_magic_packets(&macs, subnet_bc.as_deref(), explicit_target.as_deref(), port).await;

    let mut bridge_sent: Option<u32> = None;
    if let (Some(ep), Some(raw_bridge)) = (endpoint, bridge_id) {
        let parsed_bridge = EndpointId::from_str(&raw_bridge).context("EndpointId da ponte inválido")?;
        let conn = ep.connect(EndpointAddr::new(parsed_bridge), ALPN).await?;
        match control_rpc(
            &conn,
            &ControlRequest::RelayWakeOnLan {
                wol: WolInfo {
                    macs: macs.clone(),
                    broadcast: subnet_bc,
                },
            },
        )
        .await?
        {
            ControlResponse::WakeDispatched { packets_sent } => {
                bridge_sent = Some(packets_sent);
            }
            ControlResponse::Error(msg) => bail!("Erro na ponte WoL: {msg}"),
            other => bail!("Resposta inesperada da ponte: {other:?}"),
        }
    }

    let host_label = saved
        .as_ref()
        .map(|s| s.host_name.as_str())
        .unwrap_or("destino manual");
    match (direct_res, bridge_sent) {
        (Ok(direct), Some(relayed)) => println!(
            "Magic Packet enviado para '{}' ({} · {} UDP locais + {} via ponte P2P).",
            host_label,
            macs.join(", "),
            direct,
            relayed
        ),
        (Err(_), Some(relayed)) => println!(
            "Magic Packet enviado para '{}' via ponte P2P ({} · {} datagramas).",
            host_label,
            macs.join(", "),
            relayed
        ),
        (Ok(direct), None) => println!(
            "Magic Packet Wake-on-LAN enviado para '{}' ({} · {} datagramas UDP).",
            host_label,
            macs.join(", "),
            direct
        ),
        (Err(err), None) => return Err(err),
    }
    Ok(())
}

async fn pair_via_qr_uri(endpoint: &Endpoint, uri: &str, name: String) -> Result<SavedHost> {
    let payload = decode_pairing_uri(uri)?;
    let host_id = EndpointId::from_str(&payload.endpoint_id).context("EndpointId inválido na URI")?;
    let addr = build_endpoint_addr(host_id, payload.relay_url.as_deref())?;

    println!("Conectando a {} ({})...", payload.host_name, host_id);
    let conn = endpoint.connect(addr, ALPN).await?;
    let (mut send, mut recv) = conn.open_bi().await?;
    write_stream_kind(&mut send, StreamKind::Pair).await?;
    write_frame(
        &mut send,
        &PairRequest {
            secret: payload.secret,
            device_name: name,
        },
    )
    .await?;
    send.finish()?;

    let resp: PairResponse = read_frame(&mut recv)
        .await?
        .context("Host encerrou o stream antes de responder ao pareamento")?;

    if !resp.accepted {
        bail!("Pareamento recusado: {}", resp.message);
    }

    let (wol_macs, wol_broadcast) = match resp.wol.and_then(sanitize_wol_info) {
        Some(w) => (w.macs, w.broadcast),
        None => (Vec::new(), None),
    };
    Ok(SavedHost {
        endpoint_id: payload.endpoint_id,
        host_name: resp.host_name,
        relay_url: payload.relay_url,
        wol_macs,
        wol_broadcast,
    })
}

async fn pair_via_short_code(endpoint: &Endpoint, raw_code: &str, name: String) -> Result<SavedHost> {
    let derived = derive_short_code_secrets(raw_code)?;
    let rendezvous_id = SecretKey::from_bytes(&derived.rendezvous_secret_key).public();
    println!("Localizando sessão do código {}...", derived.normalized_code);

    let mut conn_opt: Option<Connection> = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(750)).await;
        }
        if let Ok(Ok(c)) = tokio::time::timeout(
            Duration::from_secs(8),
            endpoint.connect(EndpointAddr::new(rendezvous_id), ALPN_PAIR_CODE),
        )
        .await
        {
            conn_opt = Some(c);
            break;
        }
    }
    let conn = conn_opt.context(
        "Código não encontrado ou expirado. Verifique os 8 caracteres e gere um novo com 'p' no host.",
    )?;

    let (mut send, mut recv) = conn.open_bi().await?;
    write_frame(
        &mut send,
        &CodePairRequest {
            auth_token: derived.auth_token,
            device_name: name,
        },
    )
    .await?;
    send.finish()?;

    let resp: CodePairResponse = tokio::time::timeout(Duration::from_secs(8), read_frame(&mut recv))
        .await
        .context("Tempo esgotado aguardando confirmação do código")??
        .context("Host encerrou a conexão antes de confirmar o código")?;

    conn.close(0u32.into(), b"pair done");

    if !resp.accepted {
        bail!("Pareamento recusado: {}", resp.message);
    }

    let real_host_id =
        EndpointId::from_str(&resp.host_endpoint_id).context("EndpointId do host inválido")?;
    let sig_bytes: [u8; 64] = resp
        .host_signature
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Assinatura Ed25519 do host ausente ou malformada"))?;
    let sig = Signature::from_bytes(&sig_bytes);
    let digest = compute_pair_binding_digest(
        rendezvous_id.as_bytes(),
        endpoint.id().as_bytes(),
        real_host_id.as_bytes(),
    );
    real_host_id
        .verify(&digest, &sig)
        .map_err(|_| anyhow::anyhow!("Assinatura criptográfica do host inválida"))?;

    let (wol_macs, wol_broadcast) = match resp.wol.and_then(sanitize_wol_info) {
        Some(w) => (w.macs, w.broadcast),
        None => (Vec::new(), None),
    };

    Ok(SavedHost {
        endpoint_id: real_host_id.to_string(),
        host_name: resp.host_name,
        relay_url: resp.relay_url,
        wol_macs,
        wol_broadcast,
    })
}

async fn control_rpc(conn: &Connection, req: &ControlRequest) -> Result<ControlResponse> {
    let (mut send, mut recv) = conn.open_bi().await?;
    write_stream_kind(&mut send, StreamKind::Control).await?;
    write_frame(&mut send, req).await?;
    send.finish()?;

    read_frame(&mut recv)
        .await?
        .context("Host fechou stream de controle sem resposta")
}

async fn run_shell_session(conn: &Connection) -> Result<()> {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let (mut send, mut recv) = conn.open_bi().await?;
    write_stream_kind(&mut send, StreamKind::Terminal).await?;
    write_frame(&mut send, &TerminalInit::ephemeral(cols, rows)).await?;

    let _raw_guard = RawModeGuard::enter()?;

    let out_task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let mut exit_code = 0;
        while let Ok(Some(msg)) = read_frame::<_, TerminalHostMsg>(&mut recv).await {
            match msg {
                TerminalHostMsg::Ready { .. } => {}
                TerminalHostMsg::Output(bytes) => {
                    if stdout.write_all(&bytes).await.is_err() || stdout.flush().await.is_err() {
                        break;
                    }
                }
                TerminalHostMsg::Exit(code) => {
                    exit_code = code;
                    break;
                }
                TerminalHostMsg::Error(err) => {
                    let _ = stdout.write_all(format!("\r\n[erro]: {err}\r\n").as_bytes()).await;
                    break;
                }
            }
        }
        exit_code
    });

    // Thread dedicada fora do pool bloqueante do Tokio para não travar o encerramento em `stdin.read`.
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    std::thread::spawn(move || {
        use std::io::Read;
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if stdin_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    #[cfg(unix)]
    let mut sigwinch = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())?;

    let mut out_task = out_task;
    loop {
        #[cfg(unix)]
        tokio::select! {
            res = &mut out_task => {
                let code = res.unwrap_or(0);
                drop(_raw_guard);
                println!("\r\n[sessão encerrada: exit={code}]");
                break;
            }
            chunk = stdin_rx.recv() => {
                let Some(bytes) = chunk else {
                    break;
                };
                if write_frame(&mut send, &TerminalClientMsg::Input(bytes)).await.is_err() {
                    break;
                }
            }
            _ = sigwinch.recv() => {
                if let Ok((cols, rows)) = crossterm::terminal::size() {
                    let _ = write_frame(&mut send, &TerminalClientMsg::Resize { cols, rows }).await;
                }
            }
        }

        #[cfg(not(unix))]
        tokio::select! {
            res = &mut out_task => {
                let code = res.unwrap_or(0);
                drop(_raw_guard);
                println!("\r\n[sessão encerrada: exit={code}]");
                break;
            }
            chunk = stdin_rx.recv() => {
                let Some(bytes) = chunk else {
                    break;
                };
                if write_frame(&mut send, &TerminalClientMsg::Input(bytes)).await.is_err() {
                    break;
                }
            }
        }
    }

    let _ = send.finish();
    Ok(())
}

fn build_endpoint_addr(host_id: EndpointId, relay_url: Option<&str>) -> Result<EndpointAddr> {
    let mut addr = EndpointAddr::new(host_id);
    if let Some(raw_relay) = relay_url {
        let parsed = RelayUrl::from_str(raw_relay).context("URL de relay inválida")?;
        addr = addr.with_relay_url(parsed);
    }
    Ok(addr)
}

fn resolve_target_id(explicit: Option<String>, default_file: &Path) -> Result<EndpointAddr> {
    match explicit {
        Some(id) => {
            let endpoint_id = EndpointId::from_str(&id).context("EndpointId inválido")?;
            Ok(EndpointAddr::new(endpoint_id))
        }
        None => {
            if !default_file.exists() {
                bail!("Nenhum host pareado salvo. Use 'oraculo-cli pair <URI>' primeiro.");
            }
            let saved: SavedHost = serde_json::from_str(&fs::read_to_string(default_file)?)?;
            let endpoint_id =
                EndpointId::from_str(&saved.endpoint_id).context("EndpointId inválido")?;
            build_endpoint_addr(endpoint_id, saved.relay_url.as_deref())
        }
    }
}

fn write_private_file(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn load_or_create_secret_key(path: &Path) -> Result<SecretKey> {
    if path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        let raw = fs::read_to_string(path)?;
        let bytes = hex::decode(raw.trim())?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("client.key deve ter 32 bytes"))?;
        return Ok(SecretKey::from_bytes(&arr));
    }

    let key = SecretKey::generate();
    write_private_file(path, hex::encode(key.to_bytes()).as_bytes())?;
    Ok(key)
}
