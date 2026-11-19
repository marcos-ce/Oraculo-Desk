use crate::config::{
    HostPaths, HostSettings, TrustedPeer, delete_command, load_or_create_commands,
    normalize_wol_mac, save_command, write_private_file,
};
use crate::portal::{PortalAuthorizeOutcome, PortalReadiness};
use crate::server::{GuiPairingView, HostControlHandle};
use crate::tunnel::RdpStatus;
use anyhow::{Context, Result};
use iroh::SecretKey;
use oraculo_proto::QuickCommandInfo;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::warn;

const CONTROL_CONNECT_TIMEOUT: Duration = Duration::from_millis(450);
const CONTROL_IO_TIMEOUT: Duration = Duration::from_secs(95);
const MAX_CONTROL_BODY_BYTES: usize = 64 * 1024;
const HTTP_HEAD_TIMEOUT: Duration = Duration::from_secs(4);
const HTTP_BODY_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_HTTP_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_HTTP_HEADER_TOTAL_BYTES: usize = 24 * 1024;
const MAX_HTTP_HEADER_LINES: usize = 64;

pub struct ParsedHttpHead {
    pub method: String,
    pub target: String,
    pub host: String,
    pub token: String,
    pub content_length: usize,
}

pub enum HttpHeadError {
    Empty,
    BadRequest,
    HeadersTooLarge,
}

async fn read_bounded_http_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> std::result::Result<Option<String>, HttpHeadError> {
    let mut line = String::new();
    let n = {
        let mut limited = reader.take((limit + 1) as u64);
        limited.read_line(&mut line).await.map_err(|_| HttpHeadError::BadRequest)?
    };
    if n == 0 {
        return Ok(None);
    }
    if line.len() > limit || (line.len() == limit && !line.ends_with('\n')) {
        return Err(HttpHeadError::HeadersTooLarge);
    }
    Ok(Some(line))
}

pub async fn read_http_head<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    token_header_name: &str,
) -> std::result::Result<ParsedHttpHead, HttpHeadError> {
    tokio::time::timeout(HTTP_HEAD_TIMEOUT, async {
        let Some(req_line) = read_bounded_http_line(reader, MAX_HTTP_HEADER_LINE_BYTES).await? else {
            return Err(HttpHeadError::Empty);
        };
        let parts: Vec<&str> = req_line.trim_end().split_whitespace().collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(HttpHeadError::BadRequest);
        }
        let method = parts[0].to_string();
        let target = parts[1].to_string();

        let mut host = String::new();
        let mut token = String::new();
        let mut content_length: usize = 0;
        let mut total_bytes = req_line.len();
        let mut header_lines = 0usize;

        loop {
            let Some(line) = read_bounded_http_line(reader, MAX_HTTP_HEADER_LINE_BYTES).await? else {
                break;
            };
            total_bytes += line.len();
            if total_bytes > MAX_HTTP_HEADER_TOTAL_BYTES {
                return Err(HttpHeadError::HeadersTooLarge);
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            header_lines += 1;
            if header_lines > MAX_HTTP_HEADER_LINES {
                return Err(HttpHeadError::HeadersTooLarge);
            }
            if let Some((k, v)) = trimmed.split_once(':') {
                let key = k.trim().to_ascii_lowercase();
                let val = v.trim();
                if key == "host" {
                    host = val.to_string();
                } else if key == token_header_name {
                    token = val.to_string();
                } else if key == "content-length" {
                    content_length = val.parse::<usize>().map_err(|_| HttpHeadError::BadRequest)?;
                }
            }
        }

        Ok(ParsedHttpHead {
            method,
            target,
            host,
            token,
            content_length,
        })
    })
    .await
    .map_err(|_| HttpHeadError::BadRequest)?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEndpointRecord {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonControlSnapshot {
    pub host_name: String,
    pub endpoint_id: String,
    pub relay_url: Option<String>,
    pub active_peer_ids: Vec<String>,
    pub trusted_peers: Vec<TrustedPeer>,
    pub pairing: Option<GuiPairingView>,
    pub settings: HostSettings,
    pub portal: PortalReadiness,
    pub rdp: RdpStatus,
    pub commands: Vec<QuickCommandInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsPatchDto {
    pub allow_screen: Option<bool>,
    pub allow_input: Option<bool>,
    pub allow_terminal: Option<bool>,
    pub allow_files_read: Option<bool>,
    pub allow_files_write: Option<bool>,
    pub allow_commands_run: Option<bool>,
    pub allow_commands_edit: Option<bool>,
    pub allow_rdp_tunnel: Option<bool>,
    pub wol_enabled: Option<bool>,
    pub wol_mac: Option<String>,
    pub wol_broadcast: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerActionDto {
    pub endpoint_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerActionOutcome {
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveCommandDto {
    pub id: Option<String>,
    pub label: String,
    pub description: String,
    pub command_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteCommandDto {
    pub id: String,
}

pub fn apply_settings_patch(s: &mut HostSettings, patch: &SettingsPatchDto) -> Result<()> {
    if let Some(v) = patch.allow_screen {
        s.allow_screen = v;
    }
    if let Some(v) = patch.allow_input {
        s.allow_input = v;
    }
    if let Some(v) = patch.allow_terminal {
        s.allow_terminal = v;
    }
    if let Some(v) = patch.allow_files_read {
        s.allow_files_read = v;
    }
    if let Some(v) = patch.allow_files_write {
        s.allow_files_write = v;
    }
    if let Some(v) = patch.allow_commands_run {
        s.allow_commands_run = v;
    }
    if let Some(v) = patch.allow_commands_edit {
        s.allow_commands_edit = v;
    }
    if let Some(v) = patch.allow_rdp_tunnel {
        s.allow_rdp_tunnel = v;
    }
    if let Some(v) = patch.wol_enabled {
        s.wol_enabled = v;
    }
    if let Some(ref mac) = patch.wol_mac {
        let trimmed = mac.trim();
        s.wol_mac = if trimmed.is_empty() {
            None
        } else {
            Some(normalize_wol_mac(trimmed)?)
        };
    }
    if let Some(ref bc) = patch.wol_broadcast {
        let trimmed = bc.trim();
        if trimmed.is_empty() {
            s.wol_broadcast = None;
        } else {
            let safe = oraculo_proto::wol::parse_safe_lan_broadcast_v4(trimmed)
                .ok_or_else(|| anyhow::anyhow!("Broadcast IPv4 inválido: {trimmed}"))?;
            s.wol_broadcast = Some(safe.to_string());
        }
    }
    Ok(())
}

pub struct DaemonControlGuard {
    endpoint_file: PathBuf,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for DaemonControlGuard {
    fn drop(&mut self) {
        self.task.abort();
        remove_record_if_owned(&self.endpoint_file, std::process::id());
    }
}

pub async fn start_daemon_control_server(handle: HostControlHandle) -> Result<DaemonControlGuard> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("Falha ao abrir canal de controle local do oraculo-host")?;
    let port = listener.local_addr()?.port();
    let token = hex::encode(SecretKey::generate().to_bytes());
    let endpoint_file = handle.paths().control_endpoint_file.clone();

    let rec = ControlEndpointRecord {
        port,
        token: token.clone(),
        pid: std::process::id(),
    };
    write_private_file(&endpoint_file, &serde_json::to_vec(&rec)?)?;

    let token_arc: Arc<str> = Arc::from(token);
    let task = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let h = handle.clone();
                    let t = Arc::clone(&token_arc);
                    tokio::spawn(async move {
                        if let Err(err) = handle_control_conn(stream, port, &t, h).await {
                            warn!("Erro no canal local do daemon: {err:#}");
                        }
                    });
                }
                Err(err) => {
                    warn!("Erro transitório no accept() do canal local do daemon: {err}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    });

    Ok(DaemonControlGuard {
        endpoint_file,
        task,
    })
}

async fn handle_control_conn(
    stream: TcpStream,
    expected_port: u16,
    expected_token: &str,
    handle: HostControlHandle,
) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let head = match read_http_head(&mut reader, "x-oraculo-control-token").await {
        Ok(h) => h,
        Err(HttpHeadError::Empty) => return Ok(()),
        Err(HttpHeadError::HeadersTooLarge) => {
            return write_http_json(
                reader.into_inner(),
                431,
                br#"{"error":"Request header fields too large"}"#,
            )
            .await;
        }
        Err(HttpHeadError::BadRequest) => {
            return write_http_json(
                reader.into_inner(),
                400,
                br#"{"error":"Bad request"}"#,
            )
            .await;
        }
    };
    let (method, target, host_hdr, token_hdr, content_len) = (
        head.method.as_str(),
        head.target.as_str(),
        head.host,
        head.token,
        head.content_length,
    );

    if host_hdr != format!("127.0.0.1:{expected_port}")
        || !constant_time_eq(token_hdr.as_bytes(), expected_token.as_bytes())
    {
        return write_http_json(
            reader.into_inner(),
            403,
            br#"{"error":"Unauthorized"}"#,
        )
        .await;
    }

    if content_len > MAX_CONTROL_BODY_BYTES {
        return write_http_json(
            reader.into_inner(),
            413,
            br#"{"error":"Body too large"}"#,
        )
        .await;
    }

    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        tokio::time::timeout(HTTP_BODY_TIMEOUT, reader.read_exact(&mut body))
            .await
            .context("Timeout lendo corpo da requisição de controle")??;
    }
    let stream = reader.into_inner();

    match (method, target) {
        ("GET", "/control/health") => {
            write_http_json(stream, 200, br#"{"ok":true}"#).await
        }
        ("GET", "/control/state") => {
            let snap = handle.collect_snapshot().await;
            write_json_resp(stream, 200, &snap).await
        }
        ("POST", "/control/pair/issue") => {
            let pairing = handle.issue_pairing().await;
            write_json_resp(stream, 200, &pairing).await
        }
        ("POST", "/control/pair/cancel") => {
            handle.cancel_pairing().await;
            write_http_json(stream, 200, br#"{"ok":true}"#).await
        }
        ("POST", "/control/peers/disconnect") => {
            let req: PeerActionDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_error_resp(stream, 400, &format!("JSON inválido: {e}")).await,
            };
            let changed = handle.disconnect_peer(&req.endpoint_id).await;
            write_json_resp(stream, 200, &PeerActionOutcome { changed }).await
        }
        ("POST", "/control/peers/revoke") => {
            let req: PeerActionDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_error_resp(stream, 400, &format!("JSON inválido: {e}")).await,
            };
            match handle.revoke_peer(&req.endpoint_id).await {
                Ok(changed) => write_json_resp(stream, 200, &PeerActionOutcome { changed }).await,
                Err(err) => write_error_resp(stream, 400, &format!("{err:#}")).await,
            }
        }
        ("POST", "/control/portal/authorize") => match handle.authorize_portals().await {
            Ok(outcome) => write_json_resp(stream, 200, &outcome).await,
            Err(err) => write_error_resp(stream, 400, &format!("{err:#}")).await,
        },
        ("POST", "/control/settings/patch") => {
            let patch: SettingsPatchDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_error_resp(stream, 400, &format!("JSON inválido: {e}")).await,
            };
            match handle
                .settings()
                .update(|s| apply_settings_patch(s, &patch))
                .await
            {
                Ok(updated) => write_json_resp(stream, 200, &updated).await,
                Err(err) => write_error_resp(stream, 400, &format!("{err:#}")).await,
            }
        }
        ("POST", "/control/commands/save") => {
            let req: SaveCommandDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_error_resp(stream, 400, &format!("JSON inválido: {e}")).await,
            };
            let _guard = crate::config::commands_io_lock().lock().await;
            let cmd_file = handle.paths().commands_file.clone();
            let res = tokio::task::spawn_blocking(move || {
                save_command(
                    &cmd_file,
                    req.id.as_deref(),
                    &req.label,
                    &req.description,
                    &req.command_line,
                )
            })
            .await;
            match res {
                Ok(Ok(specs)) => {
                    let infos: Vec<QuickCommandInfo> = specs.into_iter().map(|s| s.info()).collect();
                    write_json_resp(stream, 200, &infos).await
                }
                Ok(Err(err)) => write_error_resp(stream, 400, &format!("{err:#}")).await,
                Err(err) => write_error_resp(stream, 500, &format!("{err}")).await,
            }
        }
        ("POST", "/control/commands/delete") => {
            let req: DeleteCommandDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_error_resp(stream, 400, &format!("JSON inválido: {e}")).await,
            };
            let _guard = crate::config::commands_io_lock().lock().await;
            let cmd_file = handle.paths().commands_file.clone();
            let res = tokio::task::spawn_blocking(move || delete_command(&cmd_file, &req.id)).await;
            match res {
                Ok(Ok(specs)) => {
                    let infos: Vec<QuickCommandInfo> = specs.into_iter().map(|s| s.info()).collect();
                    write_json_resp(stream, 200, &infos).await
                }
                Ok(Err(err)) => write_error_resp(stream, 400, &format!("{err:#}")).await,
                Err(err) => write_error_resp(stream, 500, &format!("{err}")).await,
            }
        }
        ("POST", "/control/shutdown") => {
            let _ = write_http_json(stream, 200, br#"{"ok":true}"#).await;
            let h = handle.clone();
            tokio::spawn(async move {
                h.shutdown().await;
            });
            Ok(())
        }
        ("POST", "/control/rdp/refresh") => {
            crate::tunnel::invalidate_rdp_firewall_cache();
            write_http_json(stream, 200, br#"{"ok":true}"#).await
        }
        _ => write_error_resp(stream, 404, "Endpoint de controle não encontrado").await,
    }
}

#[derive(Clone)]
pub struct DaemonControlClient {
    endpoint: Arc<tokio::sync::RwLock<(u16, String)>>,
    paths: HostPaths,
    host_name: String,
    endpoint_id: String,
}

impl DaemonControlClient {
    pub async fn try_connect(paths: &HostPaths) -> Option<Self> {
        let raw = std::fs::read_to_string(&paths.control_endpoint_file).ok()?;
        let rec: ControlEndpointRecord = serde_json::from_str(&raw).ok()?;
        let mut client = Self {
            endpoint: Arc::new(tokio::sync::RwLock::new((rec.port, rec.token))),
            paths: paths.clone(),
            host_name: String::new(),
            endpoint_id: String::new(),
        };
        let snap = client
            .request_json::<(), DaemonControlSnapshot>("GET", "/control/state", None)
            .await
            .ok()?;
        client.host_name = snap.host_name;
        client.endpoint_id = snap.endpoint_id;
        Some(client)
    }

    async fn try_reload_endpoint(&self, prev_port: u16, prev_token: &str) -> bool {
        let Ok(raw) = tokio::fs::read_to_string(&self.paths.control_endpoint_file).await else {
            return false;
        };
        let Ok(rec) = serde_json::from_str::<ControlEndpointRecord>(&raw) else {
            return false;
        };
        if rec.port == prev_port && rec.token == prev_token {
            return false;
        }
        let mut guard = self.endpoint.write().await;
        *guard = (rec.port, rec.token);
        true
    }

    async fn request_json_once<Req: Serialize, Resp: for<'de> Deserialize<'de>>(
        port: u16,
        token: &str,
        method: &str,
        path: &str,
        body: Option<&Req>,
    ) -> std::result::Result<Resp, (bool, anyhow::Error)> {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let mut stream = tokio::time::timeout(CONTROL_CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| (true, anyhow::anyhow!("Timeout conectando ao daemon oraculo-host")))?
            .map_err(|_| (true, anyhow::anyhow!("Daemon oraculo-host não está acessível")))?;

        let payload = match body {
            Some(v) => serde_json::to_vec(v).map_err(|e| (false, e.into()))?,
            None => Vec::new(),
        };
        let header = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Oraculo-Control-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        stream
            .write_all(header.as_bytes())
            .await
            .map_err(|e| (true, e.into()))?;
        if !payload.is_empty() {
            stream
                .write_all(&payload)
                .await
                .map_err(|e| (true, e.into()))?;
        }
        stream.shutdown().await.map_err(|e| (true, e.into()))?;

        let mut raw_resp = Vec::new();
        tokio::time::timeout(CONTROL_IO_TIMEOUT, stream.read_to_end(&mut raw_resp))
            .await
            .map_err(|_| (false, anyhow::anyhow!("Timeout aguardando resposta do daemon oraculo-host")))?
            .map_err(|e| (true, e.into()))?;

        let split_pos = raw_resp
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| (true, anyhow::anyhow!("Resposta HTTP inválida do daemon")))?;
        let head = String::from_utf8_lossy(&raw_resp[..split_pos]);
        let body_bytes = &raw_resp[split_pos + 4..];

        let status_code: u16 = head
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(500);

        if !(200..300).contains(&status_code) {
            let retryable = status_code == 403;
            #[derive(Deserialize)]
            struct ErrPayload {
                error: String,
            }
            if let Ok(err) = serde_json::from_slice::<ErrPayload>(body_bytes) {
                return Err((retryable, anyhow::anyhow!("{}", err.error)));
            }
            return Err((retryable, anyhow::anyhow!("Erro do daemon ({status_code})")));
        }

        serde_json::from_slice(body_bytes)
            .context("JSON inválido retornado pelo daemon")
            .map_err(|e| (false, e))
    }

    async fn request_json<Req: Serialize, Resp: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        path: &str,
        body: Option<&Req>,
    ) -> Result<Resp> {
        let (port, token) = self.endpoint.read().await.clone();
        match Self::request_json_once(port, &token, method, path, body).await {
            Ok(resp) => Ok(resp),
            Err((true, first_err)) => {
                // Se o daemon `oraculo-host` reiniciou em outra porta/token, recarrega `control_endpoint.json`.
                if self.try_reload_endpoint(port, &token).await {
                    let (new_port, new_token) = self.endpoint.read().await.clone();
                    Self::request_json_once(new_port, &new_token, method, path, body)
                        .await
                        .map_err(|(_, e)| e)
                } else {
                    Err(first_err)
                }
            }
            Err((false, err)) => Err(err),
        }
    }
}

#[derive(Clone)]
pub enum HostBackend {
    Embedded(HostControlHandle),
    DaemonClient(DaemonControlClient),
}

impl HostBackend {
    pub fn is_external_daemon(&self) -> bool {
        matches!(self, Self::DaemonClient(_))
    }

    pub fn paths(&self) -> &HostPaths {
        match self {
            Self::Embedded(h) => h.paths(),
            Self::DaemonClient(c) => &c.paths,
        }
    }

    pub fn host_name(&self) -> &str {
        match self {
            Self::Embedded(h) => h.host_name(),
            Self::DaemonClient(c) => &c.host_name,
        }
    }

    pub async fn snapshot(&self) -> Result<DaemonControlSnapshot> {
        match self {
            Self::Embedded(h) => Ok(h.collect_snapshot().await),
            Self::DaemonClient(c) => c.request_json::<(), _>("GET", "/control/state", None).await,
        }
    }

    pub async fn issue_pairing(&self) -> Result<GuiPairingView> {
        match self {
            Self::Embedded(h) => Ok(h.issue_pairing().await),
            Self::DaemonClient(c) => {
                c.request_json("POST", "/control/pair/issue", Some(&serde_json::json!({})))
                    .await
            }
        }
    }

    pub async fn cancel_pairing(&self) -> Result<()> {
        match self {
            Self::Embedded(h) => {
                h.cancel_pairing().await;
                Ok(())
            }
            Self::DaemonClient(c) => {
                let _: serde_json::Value = c
                    .request_json("POST", "/control/pair/cancel", Some(&serde_json::json!({})))
                    .await?;
                Ok(())
            }
        }
    }

    pub async fn disconnect_peer(&self, endpoint_id: &str) -> Result<bool> {
        match self {
            Self::Embedded(h) => Ok(h.disconnect_peer(endpoint_id).await),
            Self::DaemonClient(c) => {
                let res: PeerActionOutcome = c
                    .request_json(
                        "POST",
                        "/control/peers/disconnect",
                        Some(&PeerActionDto {
                            endpoint_id: endpoint_id.to_string(),
                        }),
                    )
                    .await?;
                Ok(res.changed)
            }
        }
    }

    pub async fn revoke_peer(&self, endpoint_id: &str) -> Result<bool> {
        match self {
            Self::Embedded(h) => h.revoke_peer(endpoint_id).await,
            Self::DaemonClient(c) => {
                let res: PeerActionOutcome = c
                    .request_json(
                        "POST",
                        "/control/peers/revoke",
                        Some(&PeerActionDto {
                            endpoint_id: endpoint_id.to_string(),
                        }),
                    )
                    .await?;
                Ok(res.changed)
            }
        }
    }

    pub async fn authorize_portals(&self) -> Result<PortalAuthorizeOutcome> {
        match self {
            Self::Embedded(h) => h.authorize_portals().await,
            Self::DaemonClient(c) => {
                c.request_json(
                    "POST",
                    "/control/portal/authorize",
                    Some(&serde_json::json!({})),
                )
                .await
            }
        }
    }

    pub async fn patch_settings(&self, patch: SettingsPatchDto) -> Result<HostSettings> {
        match self {
            Self::Embedded(h) => {
                h.settings()
                    .update(|s| apply_settings_patch(s, &patch))
                    .await
            }
            Self::DaemonClient(c) => {
                c.request_json("POST", "/control/settings/patch", Some(&patch))
                    .await
            }
        }
    }

    pub async fn enable_wol_setting(&self) -> Result<HostSettings> {
        self.patch_settings(SettingsPatchDto {
            wol_enabled: Some(true),
            ..Default::default()
        })
        .await
    }

    pub async fn save_command(&self, req: SaveCommandDto) -> Result<Vec<QuickCommandInfo>> {
        match self {
            Self::Embedded(h) => {
                let _guard = crate::config::commands_io_lock().lock().await;
                let cmd_file = h.paths().commands_file.clone();
                let specs = tokio::task::spawn_blocking(move || {
                    save_command(
                        &cmd_file,
                        req.id.as_deref(),
                        &req.label,
                        &req.description,
                        &req.command_line,
                    )
                })
                .await??;
                Ok(specs.into_iter().map(|s| s.info()).collect())
            }
            Self::DaemonClient(c) => {
                c.request_json("POST", "/control/commands/save", Some(&req))
                    .await
            }
        }
    }

    pub async fn delete_command(&self, id: &str) -> Result<Vec<QuickCommandInfo>> {
        match self {
            Self::Embedded(h) => {
                let _guard = crate::config::commands_io_lock().lock().await;
                let cmd_file = h.paths().commands_file.clone();
                let id_owned = id.to_string();
                let specs = tokio::task::spawn_blocking(move || delete_command(&cmd_file, &id_owned))
                    .await??;
                Ok(specs.into_iter().map(|s| s.info()).collect())
            }
            Self::DaemonClient(c) => {
                c.request_json(
                    "POST",
                    "/control/commands/delete",
                    Some(&DeleteCommandDto { id: id.to_string() }),
                )
                .await
            }
        }
    }

    pub async fn list_commands(&self) -> Result<Vec<QuickCommandInfo>> {
        match self {
            Self::Embedded(h) => {
                let cmd_file = h.paths().commands_file.clone();
                let specs =
                    tokio::task::spawn_blocking(move || load_or_create_commands(&cmd_file))
                        .await??;
                Ok(specs.into_iter().map(|s| s.info()).collect())
            }
            Self::DaemonClient(_) => Ok(self.snapshot().await?.commands),
        }
    }

    pub async fn invalidate_rdp_cache(&self) {
        crate::tunnel::invalidate_rdp_firewall_cache();
        if let Self::DaemonClient(c) = self {
            let _: Result<serde_json::Value> = c
                .request_json("POST", "/control/rdp/refresh", Some(&serde_json::json!({})))
                .await;
        }
    }

    pub async fn shutdown(&self) {
        match self {
            Self::Embedded(h) => h.shutdown().await,
            Self::DaemonClient(c) => {
                let _: Result<serde_json::Value> = c
                    .request_json("POST", "/control/shutdown", Some(&serde_json::json!({})))
                    .await;
            }
        }
    }
}

fn remove_record_if_owned(path: &Path, pid: u32) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    if let Ok(rec) = serde_json::from_str::<ControlEndpointRecord>(&raw) {
        if rec.pid == pid {
            let _ = std::fs::remove_file(path);
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

async fn write_http_json(mut stream: TcpStream, status: u16, body: &[u8]) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

async fn write_json_resp<T: Serialize>(stream: TcpStream, status: u16, payload: &T) -> Result<()> {
    let body = serde_json::to_vec(payload)?;
    write_http_json(stream, status, &body).await
}

async fn write_error_resp(stream: TcpStream, status: u16, msg: &str) -> Result<()> {
    #[derive(Serialize)]
    struct ErrBody<'a> {
        error: &'a str,
    }
    write_json_resp(stream, status, &ErrBody { error: msg }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_http_head_rejects_oversized_and_malformed_headers() {
        let (mut client, server) = tokio::io::duplex(32 * 1024);
        let huge = "X-Pad: ".to_string() + &"a".repeat(9000);
        let req = format!("GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:80\r\n{huge}\r\n\r\n");
        tokio::spawn(async move {
            let _ = client.write_all(req.as_bytes()).await;
        });
        let mut reader = BufReader::new(server);
        assert!(matches!(
            read_http_head(&mut reader, "x-oraculo-token").await,
            Err(HttpHeadError::HeadersTooLarge)
        ));

        let (mut client2, server2) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            let _ = client2
                .write_all(b"POST /api/settings HTTP/1.1\r\nHost: 127.0.0.1:80\r\nContent-Length: nope\r\n\r\n")
                .await;
        });
        let mut reader2 = BufReader::new(server2);
        assert!(matches!(
            read_http_head(&mut reader2, "x-oraculo-token").await,
            Err(HttpHeadError::BadRequest)
        ));
    }
}
