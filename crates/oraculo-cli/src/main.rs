use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use iroh::endpoint::{Connection, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayUrl, SecretKey};
use oraculo_proto::pairing::decode_pairing_uri;
use oraculo_proto::{
    ALPN, ControlRequest, ControlResponse, PairRequest, PairResponse, StreamKind,
    TerminalClientMsg, TerminalHostMsg, TerminalInit, read_frame, write_frame, write_stream_kind,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Parser)]
#[command(name = "oraculo-cli", about = "Cliente CLI de teste do Oráculo (pareamento, comandos e shell PTY)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pareia com um host usando a URI do QR Code
    Pair {
        uri: String,
        #[arg(long, default_value = "oraculo-cli-teste")]
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
}

#[derive(Serialize, Deserialize)]
struct SavedHost {
    endpoint_id: String,
    host_name: String,
    #[serde(default)]
    relay_url: Option<String>,
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
            let payload = decode_pairing_uri(&uri)?;
            let host_id = EndpointId::from_str(&payload.endpoint_id)
                .context("EndpointId inválido na URI")?;
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

            let saved = SavedHost {
                endpoint_id: payload.endpoint_id,
                host_name: resp.host_name.clone(),
                relay_url: payload.relay_url,
            };
            write_private_file(
                &paths.default_host_file,
                serde_json::to_string_pretty(&saved)?.as_bytes(),
            )?;
            println!("Pareado com '{}' ({})", resp.host_name, resp.message);
        }
        Command::Ping { host } => {
            let target = resolve_target_id(host, &paths.default_host_file)?;
            let conn = endpoint.connect(target, ALPN).await?;
            let started = Instant::now();
            match control_rpc(&conn, &ControlRequest::Ping).await? {
                ControlResponse::Pong { host_name } => {
                    println!(
                        "Pong de '{}' em {} ms",
                        host_name,
                        started.elapsed().as_millis()
                    );
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
    }

    Ok(())
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
    write_frame(&mut send, &TerminalInit { cols, rows }).await?;

    let _raw_guard = RawModeGuard::enter()?;

    let out_task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let mut exit_code = 0;
        while let Ok(Some(msg)) = read_frame::<_, TerminalHostMsg>(&mut recv).await {
            match msg {
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

    let mut stdin = tokio::io::stdin();
    let mut buf = [0u8; 1024];

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
            n = stdin.read(&mut buf) => {
                match n {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if write_frame(&mut send, &TerminalClientMsg::Input(buf[..n].to_vec())).await.is_err() {
                            break;
                        }
                    }
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
            n = stdin.read(&mut buf) => {
                match n {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if write_frame(&mut send, &TerminalClientMsg::Input(buf[..n].to_vec())).await.is_err() {
                            break;
                        }
                    }
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
