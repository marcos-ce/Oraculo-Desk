use iroh::endpoint::{Connection, IdleTimeout, QuicTransportConfig, RecvStream, SendStream, presets};
use iroh::tls::CaTlsConfig;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayUrl, SecretKey};
use oraculo_proto::pairing::decode_pairing_uri;
use oraculo_proto::{
    ALPN, ControlRequest, ControlResponse, FILE_CHUNK_SIZE, FileClientMsg, FileHostMsg,
    FileTransferRequest, InputEvent, InputInitAck, MouseButton, PairRequest, PairResponse,
    RemoteKey, ScreenClientMsg, ScreenHostMsg, ScreenPreset, ScreenStart, StreamKind,
    TerminalClientMsg, TerminalHostMsg, TerminalInit, read_frame, write_frame, write_stream_kind,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, mpsc, watch};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum OraculoError {
    #[error("{reason}")]
    Failure { reason: String },
}

impl From<anyhow::Error> for OraculoError {
    fn from(err: anyhow::Error) -> Self {
        Self::Failure {
            reason: format!("{err:#}"),
        }
    }
}

fn err_msg(msg: impl Into<String>) -> OraculoError {
    OraculoError::Failure {
        reason: msg.into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct SavedHostInfo {
    pub endpoint_id: String,
    pub host_name: String,
    pub relay_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct QuickCommandItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub command_line: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CommandRunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ConnectionPing {
    pub host_name: String,
    pub rtt_ms: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RemoteFileEntry {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_unix: u64,
    pub mime_hint: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RemoteDirectoryListing {
    pub current_path: String,
    pub home_label: String,
    pub parent_path: Option<String>,
    pub quick_paths: Vec<RemoteFileEntry>,
    pub entries: Vec<RemoteFileEntry>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RemoteFileTransferResult {
    pub file_name: String,
    pub bytes_transferred: u64,
    pub remote_rel_path: String,
    pub mime_hint: String,
}

#[uniffi::export(with_foreign)]
pub trait FileTransferObserver: Send + Sync + std::fmt::Debug {
    fn on_progress(&self, transferred: u64, total: u64);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TerminalColorKind {
    Default,
    Indexed,
    Rgb,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TerminalSpan {
    pub text: String,
    pub fg_kind: TerminalColorKind,
    pub fg_val: u32,
    pub bg_kind: TerminalColorKind,
    pub bg_val: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub inverse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TerminalLine {
    pub spans: Vec<TerminalSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TerminalScreenSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub title: String,
    pub bell_count: u64,
    pub plain_text: String,
    pub lines: Vec<TerminalLine>,
}

#[derive(Default)]
struct PtyxisCallbacks {
    title: String,
    bell_count: u64,
}

impl vt100::Callbacks for PtyxisCallbacks {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bell_count = self.bell_count.wrapping_add(1);
    }

    fn visual_bell(&mut self, _: &mut vt100::Screen) {
        self.bell_count = self.bell_count.wrapping_add(1);
    }

    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).trim().to_string();
    }
}

/// Callback implementado em Kotlin para receber atualizações da tela ANSI do terminal.
#[uniffi::export(with_foreign)]
pub trait TerminalObserver: Send + Sync + std::fmt::Debug {
    fn on_screen_updated(&self, snapshot: TerminalScreenSnapshot);
    fn on_session_ended(&self, exit_code: i32);
    fn on_error(&self, message: String);
}

enum TerminalCmd {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Close,
}

#[derive(uniffi::Object)]
pub struct MobileTerminalSession {
    tx: mpsc::UnboundedSender<TerminalCmd>,
    parser: Arc<std::sync::Mutex<vt100::Parser<PtyxisCallbacks>>>,
}

#[uniffi::export]
impl MobileTerminalSession {
    pub fn send_input(&self, data: Vec<u8>) -> Result<(), OraculoError> {
        self.tx
            .send(TerminalCmd::Input(data))
            .map_err(|_| err_msg("Sessão de terminal já encerrada"))
    }

    pub fn send_text(&self, text: String) -> Result<(), OraculoError> {
        self.send_input(text.into_bytes())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), OraculoError> {
        let (cols, rows, nz_cols, nz_rows) = clamp_terminal_size(cols, rows);
        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(nz_rows, nz_cols);
        }
        self.tx
            .send(TerminalCmd::Resize { cols, rows })
            .map_err(|_| err_msg("Sessão de terminal já encerrada"))
    }

    pub fn close_session(&self) {
        let _ = self.tx.send(TerminalCmd::Close);
    }
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum MobileMouseButton {
    Left,
    Middle,
    Right,
}

impl From<MobileMouseButton> for MouseButton {
    fn from(value: MobileMouseButton) -> Self {
        match value {
            MobileMouseButton::Left => Self::Left,
            MobileMouseButton::Middle => Self::Middle,
            MobileMouseButton::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum MobileRemoteKey {
    Backspace,
    Enter,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    Super,
    Control,
    Alt,
    Shift,
    F4,
    F5,
    F11,
    VolumeMute,
    VolumeDown,
    VolumeUp,
    PlayPause,
    MediaNext,
    MediaPrev,
}

impl From<MobileRemoteKey> for RemoteKey {
    fn from(value: MobileRemoteKey) -> Self {
        match value {
            MobileRemoteKey::Backspace => Self::Backspace,
            MobileRemoteKey::Enter => Self::Enter,
            MobileRemoteKey::Tab => Self::Tab,
            MobileRemoteKey::Escape => Self::Escape,
            MobileRemoteKey::Up => Self::Up,
            MobileRemoteKey::Down => Self::Down,
            MobileRemoteKey::Left => Self::Left,
            MobileRemoteKey::Right => Self::Right,
            MobileRemoteKey::Home => Self::Home,
            MobileRemoteKey::End => Self::End,
            MobileRemoteKey::Delete => Self::Delete,
            MobileRemoteKey::Super => Self::Super,
            MobileRemoteKey::Control => Self::Control,
            MobileRemoteKey::Alt => Self::Alt,
            MobileRemoteKey::Shift => Self::Shift,
            MobileRemoteKey::F4 => Self::F4,
            MobileRemoteKey::F5 => Self::F5,
            MobileRemoteKey::F11 => Self::F11,
            MobileRemoteKey::VolumeMute => Self::VolumeMute,
            MobileRemoteKey::VolumeDown => Self::VolumeDown,
            MobileRemoteKey::VolumeUp => Self::VolumeUp,
            MobileRemoteKey::PlayPause => Self::PlayPause,
            MobileRemoteKey::MediaNext => Self::MediaNext,
            MobileRemoteKey::MediaPrev => Self::MediaPrev,
        }
    }
}

/// Callback implementado em Kotlin para notificar quando o canal de entrada remota cair.
#[uniffi::export(with_foreign)]
pub trait InputObserver: Send + Sync + std::fmt::Debug {
    fn on_session_closed(&self, reason: String);
}

enum InputCmd {
    Event(InputEvent),
    Close,
}

#[derive(uniffi::Object)]
pub struct MobileInputSession {
    tx: mpsc::UnboundedSender<InputCmd>,
    alive: Arc<AtomicBool>,
}

#[uniffi::export]
impl MobileInputSession {
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn move_pointer(&self, dx: f64, dy: f64) -> Result<(), OraculoError> {
        self.push(InputEvent::Move { dx, dy })
    }

    pub fn click_button(&self, button: MobileMouseButton) -> Result<(), OraculoError> {
        self.push(InputEvent::Click {
            button: button.into(),
        })
    }

    pub fn press_button(&self, button: MobileMouseButton, pressed: bool) -> Result<(), OraculoError> {
        self.push(InputEvent::Button {
            button: button.into(),
            pressed,
        })
    }

    pub fn scroll_pointer(&self, dx: f64, dy: f64) -> Result<(), OraculoError> {
        self.push(InputEvent::Scroll { dx, dy })
    }

    pub fn send_text(&self, text: String) -> Result<(), OraculoError> {
        if text.is_empty() {
            return Ok(());
        }
        self.push(InputEvent::Text(text))
    }

    pub fn send_key(&self, key: MobileRemoteKey) -> Result<(), OraculoError> {
        self.push(InputEvent::Key(key.into()))
    }

    pub fn press_key(&self, key: MobileRemoteKey, pressed: bool) -> Result<(), OraculoError> {
        self.push(InputEvent::KeyState {
            key: key.into(),
            pressed,
        })
    }

    pub fn close_session(&self) {
        self.alive.store(false, Ordering::Release);
        let _ = self.tx.send(InputCmd::Close);
    }
}

impl MobileInputSession {
    fn push(&self, event: InputEvent) -> Result<(), OraculoError> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(err_msg("Sessão de touchpad encerrada"));
        }
        self.tx
            .send(InputCmd::Event(event))
            .map_err(|_| err_msg("Sessão de touchpad encerrada"))
    }
}

const MAX_CLIENT_VIDEO_DELTAS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MobileScreenPreset {
    Balanced720p30,
    Smooth1080p60,
    Crisp1080p30,
}

impl From<MobileScreenPreset> for ScreenPreset {
    fn from(value: MobileScreenPreset) -> Self {
        match value {
            MobileScreenPreset::Balanced720p30 => Self::Balanced720p30,
            MobileScreenPreset::Smooth1080p60 => Self::Smooth1080p60,
            MobileScreenPreset::Crisp1080p30 => Self::Crisp1080p30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScreenStreamInfo {
    pub width: u16,
    pub height: u16,
    pub fps: u8,
    pub encoder: String,
    pub monitor_index: u8,
    pub monitor_count: u8,
    pub available_monitors: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScreenVideoFrame {
    pub seq: u64,
    pub pts_ms: u64,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

/// Callback implementado em Kotlin para receber estado e frames H.264 Annex-B em ordem sequencial.
#[uniffi::export(with_foreign)]
pub trait ScreenObserver: Send + Sync + std::fmt::Debug {
    fn on_stream_ready(&self, info: ScreenStreamInfo);
    fn on_video_frame(&self, frame: ScreenVideoFrame);
    fn on_stream_notice(&self, message: String);
    fn on_stream_error(&self, message: String);
    fn on_stream_closed(&self, reason: String);
}

enum ScreenCmd {
    Control(ScreenClientMsg),
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClientScreenEvent {
    Ready(ScreenStreamInfo),
    Frame(ScreenVideoFrame),
    Notice(String),
    Error(String),
}

enum ClientQueuePop {
    Event(ClientScreenEvent),
    Closed(Option<String>),
}

/// Fila anti-latência no cliente: garante entrega ordenada `Ready -> Keyframe -> Deltas`,
/// descartando backlog de vídeo caso o decodificador do celular atrase.
struct ClientScreenQueue {
    inner: StdMutex<ClientScreenQueueInner>,
    condvar: Condvar,
}

#[derive(Default)]
struct ClientScreenQueueInner {
    events: VecDeque<ClientScreenEvent>,
    waiting_for_keyframe: bool,
    closed: bool,
    close_reason: Option<String>,
    dropped_frames: u64,
}

impl ClientScreenQueue {
    fn new() -> Self {
        Self {
            inner: StdMutex::new(ClientScreenQueueInner {
                waiting_for_keyframe: true,
                ..Default::default()
            }),
            condvar: Condvar::new(),
        }
    }

    fn push_ready(&self, info: ScreenStreamInfo) {
        let Ok(mut g) = self.inner.lock() else {
            return;
        };
        if g.closed {
            return;
        }
        g.events
            .retain(|ev| matches!(ev, ClientScreenEvent::Ready(_) | ClientScreenEvent::Notice(_)));
        g.waiting_for_keyframe = true;
        g.events.push_back(ClientScreenEvent::Ready(info));
        self.condvar.notify_one();
    }

    fn push_notice(&self, message: String) {
        let Ok(mut g) = self.inner.lock() else {
            return;
        };
        if g.closed {
            return;
        }
        g.events.push_back(ClientScreenEvent::Notice(message));
        self.condvar.notify_one();
    }

    fn push_frame(&self, frame: ScreenVideoFrame) {
        let Ok(mut g) = self.inner.lock() else {
            return;
        };
        if g.closed {
            return;
        }
        if frame.keyframe {
            let had_frames = g
                .events
                .iter()
                .filter(|ev| matches!(ev, ClientScreenEvent::Frame(_)))
                .count() as u64;
            if had_frames > 0 {
                g.dropped_frames += had_frames;
                g.events.retain(|ev| {
                    matches!(ev, ClientScreenEvent::Ready(_) | ClientScreenEvent::Notice(_))
                });
            }
            g.waiting_for_keyframe = false;
        } else {
            if g.waiting_for_keyframe {
                g.dropped_frames += 1;
                return;
            }
            let pending_frames = g
                .events
                .iter()
                .filter(|ev| matches!(ev, ClientScreenEvent::Frame(_)))
                .count();
            if pending_frames >= MAX_CLIENT_VIDEO_DELTAS {
                g.events.retain(|ev| {
                    matches!(ev, ClientScreenEvent::Ready(_) | ClientScreenEvent::Notice(_))
                });
                g.waiting_for_keyframe = true;
                g.dropped_frames += pending_frames as u64 + 1;
                return;
            }
        }

        g.events.push_back(ClientScreenEvent::Frame(frame));
        self.condvar.notify_one();
    }

    fn push_error_and_close(&self, message: String) {
        let Ok(mut g) = self.inner.lock() else {
            return;
        };
        if g.closed {
            return;
        }
        g.events
            .retain(|ev| matches!(ev, ClientScreenEvent::Ready(_) | ClientScreenEvent::Notice(_)));
        g.events.push_back(ClientScreenEvent::Error(message));
        g.closed = true;
        g.close_reason = None;
        self.condvar.notify_all();
    }

    fn close_with_reason(&self, reason: Option<String>) {
        if let Ok(mut g) = self.inner.lock() {
            if !g.closed {
                g.closed = true;
                g.close_reason = reason;
                g.events.clear();
            } else if reason.is_none() {
                g.close_reason = None;
                g.events.clear();
            }
        }
        self.condvar.notify_all();
    }

    fn pop_blocking(&self) -> ClientQueuePop {
        let Ok(mut g) = self.inner.lock() else {
            return ClientQueuePop::Closed(None);
        };
        loop {
            if let Some(ev) = g.events.pop_front() {
                return ClientQueuePop::Event(ev);
            }
            if g.closed {
                return ClientQueuePop::Closed(g.close_reason.take());
            }
            match self.condvar.wait(g) {
                Ok(guard) => g = guard,
                Err(_) => return ClientQueuePop::Closed(None),
            }
        }
    }
}

#[derive(uniffi::Object)]
pub struct MobileScreenSession {
    tx: mpsc::UnboundedSender<ScreenCmd>,
    stop_tx: watch::Sender<bool>,
    queue: Arc<ClientScreenQueue>,
    alive: Arc<AtomicBool>,
}

#[uniffi::export]
impl MobileScreenSession {
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn request_keyframe(&self) -> Result<(), OraculoError> {
        self.push_cmd(ScreenClientMsg::RequestKeyframe)
    }

    pub fn switch_monitor(&self) -> Result<(), OraculoError> {
        self.push_cmd(ScreenClientMsg::SwitchMonitor)
    }

    pub fn reauthorize_monitors(&self) -> Result<(), OraculoError> {
        self.push_cmd(ScreenClientMsg::ReauthorizeMonitors)
    }

    pub fn set_preset(&self, preset: MobileScreenPreset) -> Result<(), OraculoError> {
        self.push_cmd(ScreenClientMsg::Reconfigure(ScreenStart::from_preset(
            preset.into(),
        )))
    }

    pub fn reconfigure(
        &self,
        max_width: u16,
        max_height: u16,
        fps: u8,
        bitrate_kbps: u32,
    ) -> Result<(), OraculoError> {
        let cfg = ScreenStart {
            max_width,
            max_height,
            fps,
            bitrate_kbps,
        }
        .sanitized();
        self.push_cmd(ScreenClientMsg::Reconfigure(cfg))
    }

    pub fn close_session(&self) {
        self.alive.store(false, Ordering::Release);
        let _ = self.stop_tx.send(true);
        let _ = self.tx.send(ScreenCmd::Close);
        self.queue.close_with_reason(None);
    }
}

impl MobileScreenSession {
    fn push_cmd(&self, msg: ScreenClientMsg) -> Result<(), OraculoError> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(err_msg("Sessão de streaming de tela encerrada"));
        }
        self.tx
            .send(ScreenCmd::Control(msg))
            .map_err(|_| err_msg("Sessão de streaming de tela encerrada"))
    }
}

#[derive(uniffi::Object)]
pub struct OraculoMobileClient {
    data_dir: PathBuf,
    runtime: Runtime,
    endpoint: Mutex<Option<Endpoint>>,
    connections: Arc<Mutex<HashMap<String, Connection>>>,
}

#[uniffi::export]
impl OraculoMobileClient {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Self, OraculoError> {
        let dir = PathBuf::from(&data_dir);
        fs::create_dir_all(&dir).map_err(|e| err_msg(format!("Falha ao criar data_dir: {e}")))?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| err_msg(format!("Falha ao iniciar runtime async: {e}")))?;

        Ok(Self {
            data_dir: dir,
            runtime,
            endpoint: Mutex::new(None),
            connections: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn list_saved_hosts(&self) -> Result<Vec<SavedHostInfo>, OraculoError> {
        load_saved_hosts(&self.hosts_file())
    }

    pub fn remove_saved_host(&self, endpoint_id: String) -> Result<(), OraculoError> {
        let mut hosts = load_saved_hosts(&self.hosts_file())?;
        hosts.retain(|h| h.endpoint_id != endpoint_id);
        save_hosts(&self.hosts_file(), &hosts)?;
        self.disconnect(endpoint_id);
        Ok(())
    }

    pub fn pair_by_uri(
        &self,
        uri: String,
        device_name: String,
    ) -> Result<SavedHostInfo, OraculoError> {
        self.runtime.block_on(async {
            let payload = decode_pairing_uri(&uri)?;
            let host_id = EndpointId::from_str(&payload.endpoint_id)
                .map_err(|e| err_msg(format!("EndpointId inválido: {e}")))?;
            let addr = build_addr(host_id, payload.relay_url.as_deref())?;

            let endpoint = self.ensure_endpoint().await?;
            let conn = endpoint
                .connect(addr, ALPN)
                .await
                .map_err(|e| err_msg(format!("Falha na conexão de pareamento: {e}")))?;

            let (mut send, mut recv) = conn
                .open_bi()
                .await
                .map_err(|e| err_msg(format!("Falha ao abrir stream de pareamento: {e}")))?;

            write_stream_kind(&mut send, StreamKind::Pair).await?;
            write_frame(
                &mut send,
                &PairRequest {
                    secret: payload.secret,
                    device_name,
                },
            )
            .await?;
            send.finish()
                .map_err(|e| err_msg(format!("Erro ao finalizar envio: {e}")))?;

            let resp: PairResponse = read_frame(&mut recv)
                .await?
                .ok_or_else(|| err_msg("Host encerrou conexão antes de responder"))?;

            if !resp.accepted {
                return Err(err_msg(format!("Pareamento recusado: {}", resp.message)));
            }

            let info = SavedHostInfo {
                endpoint_id: payload.endpoint_id,
                host_name: resp.host_name,
                relay_url: payload.relay_url,
            };

            let mut hosts = load_saved_hosts(&self.hosts_file())?;
            hosts.retain(|h| h.endpoint_id != info.endpoint_id);
            hosts.push(info.clone());
            save_hosts(&self.hosts_file(), &hosts)?;

            self.connections
                .lock()
                .await
                .insert(info.endpoint_id.clone(), conn);

            Ok(info)
        })
    }

    pub fn ping(&self, endpoint_id: String) -> Result<ConnectionPing, OraculoError> {
        self.runtime.block_on(async {
            let conn = self.get_or_connect(&endpoint_id).await?;
            match measure_ping_on_conn(&conn).await {
                Ok(ping) => Ok(ping),
                Err(_) => {
                    // Reconecta uma vez e mede somente o round-trip real da nova conexão.
                    remove_stale_connection(&self.connections, &endpoint_id, &conn).await;
                    let fresh = self.get_or_connect(&endpoint_id).await?;
                    measure_ping_on_conn(&fresh).await
                }
            }
        })
    }

    pub fn list_commands(&self, endpoint_id: String) -> Result<Vec<QuickCommandItem>, OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(&endpoint_id, &ControlRequest::ListCommands)
                .await?
            {
                ControlResponse::Commands(items) => Ok(map_quick_commands(items)),
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!("Resposta inesperada: {other:?}"))),
            }
        })
    }

    pub fn save_command(
        &self,
        endpoint_id: String,
        id: Option<String>,
        label: String,
        description: String,
        command_line: String,
    ) -> Result<Vec<QuickCommandItem>, OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(
                    &endpoint_id,
                    &ControlRequest::SaveCommand {
                        id,
                        label,
                        description,
                        command_line,
                    },
                )
                .await?
            {
                ControlResponse::Commands(items) => Ok(map_quick_commands(items)),
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!("Resposta inesperada: {other:?}"))),
            }
        })
    }

    pub fn delete_command(
        &self,
        endpoint_id: String,
        command_id: String,
    ) -> Result<Vec<QuickCommandItem>, OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(
                    &endpoint_id,
                    &ControlRequest::DeleteCommand { id: command_id },
                )
                .await?
            {
                ControlResponse::Commands(items) => Ok(map_quick_commands(items)),
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!("Resposta inesperada: {other:?}"))),
            }
        })
    }

    pub fn run_command(
        &self,
        endpoint_id: String,
        command_id: String,
    ) -> Result<CommandRunResult, OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(&endpoint_id, &ControlRequest::RunCommand { id: command_id })
                .await?
            {
                ControlResponse::CommandResult(out) => Ok(CommandRunResult {
                    exit_code: out.exit_code,
                    stdout: out.stdout,
                    stderr: out.stderr,
                }),
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!("Resposta inesperada: {other:?}"))),
            }
        })
    }

    pub fn list_files(
        &self,
        endpoint_id: String,
        rel_path: String,
        show_hidden: bool,
    ) -> Result<RemoteDirectoryListing, OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(
                    &endpoint_id,
                    &ControlRequest::ListFiles {
                        rel_path,
                        show_hidden,
                    },
                )
                .await?
            {
                ControlResponse::DirectoryListing(listing) => Ok(RemoteDirectoryListing {
                    current_path: listing.current_path,
                    home_label: listing.home_label,
                    parent_path: listing.parent_path,
                    quick_paths: map_file_entries(listing.quick_paths),
                    entries: map_file_entries(listing.entries),
                }),
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!("Resposta inesperada: {other:?}"))),
            }
        })
    }

    pub fn download_file(
        &self,
        endpoint_id: String,
        remote_rel_path: String,
        dest_local_path: String,
        observer: Arc<dyn FileTransferObserver>,
    ) -> Result<RemoteFileTransferResult, OraculoError> {
        self.runtime.block_on(async {
            let dest_path = PathBuf::from(&dest_local_path);
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| err_msg(format!("Falha ao preparar pasta local: {e}")))?;
            }
            let tmp_path = PathBuf::from(format!("{dest_local_path}.oraculo.part"));

            let (mut send, mut recv) = self.open_bi_stream(&endpoint_id).await?;
            write_stream_kind(&mut send, StreamKind::Files).await?;
            write_frame(
                &mut send,
                &FileTransferRequest::Download {
                    rel_path: remote_rel_path.clone(),
                },
            )
            .await?;
            send.finish()
                .map_err(|e| err_msg(format!("Erro ao finalizar requisição: {e}")))?;

            let (file_name, total_size, mime_hint) =
                match read_frame::<_, FileHostMsg>(&mut recv).await? {
                    Some(FileHostMsg::DownloadStart {
                        file_name,
                        total_size,
                        mime_hint,
                    }) => (file_name, total_size, mime_hint),
                    Some(FileHostMsg::Error(msg)) => return Err(err_msg(msg)),
                    Some(other) => {
                        return Err(err_msg(format!(
                            "Resposta inesperada ao iniciar download: {other:?}"
                        )));
                    }
                    None => return Err(err_msg("Host encerrou conexão antes do download")),
                };

            let mut out_file = tokio::fs::File::create(&tmp_path)
                .await
                .map_err(|e| err_msg(format!("Falha ao criar arquivo local: {e}")))?;

            let mut transferred: u64 = 0;
            let mut last_notify = Instant::now();
            observer.on_progress(0, total_size);

            let res: Result<RemoteFileTransferResult, OraculoError> = async {
                loop {
                    match read_frame::<_, FileHostMsg>(&mut recv).await? {
                        Some(FileHostMsg::Chunk(bytes)) => {
                            out_file
                                .write_all(&bytes)
                                .await
                                .map_err(|e| err_msg(format!("Erro ao gravar download: {e}")))?;
                            transferred += bytes.len() as u64;
                            if last_notify.elapsed() >= Duration::from_millis(60) {
                                observer.on_progress(transferred, total_size);
                                last_notify = Instant::now();
                            }
                        }
                        Some(FileHostMsg::Done {
                            bytes_transferred,
                            saved_rel_path,
                        }) => {
                            out_file
                                .flush()
                                .await
                                .map_err(|e| err_msg(format!("Erro ao finalizar arquivo: {e}")))?;
                            drop(out_file);
                            tokio::fs::rename(&tmp_path, &dest_path)
                                .await
                                .map_err(|e| err_msg(format!("Erro ao salvar download: {e}")))?;
                            let total = total_size.max(bytes_transferred);
                            observer.on_progress(bytes_transferred, total);
                            return Ok(RemoteFileTransferResult {
                                file_name,
                                bytes_transferred,
                                remote_rel_path: saved_rel_path,
                                mime_hint,
                            });
                        }
                        Some(FileHostMsg::Error(msg)) => return Err(err_msg(msg)),
                        Some(other) => {
                            return Err(err_msg(format!("Mensagem inesperada: {other:?}")));
                        }
                        None => return Err(err_msg("Download interrompido antes da conclusão")),
                    }
                }
            }
            .await;

            if res.is_err() {
                let _ = tokio::fs::remove_file(&tmp_path).await;
            }
            res
        })
    }

    pub fn upload_file(
        &self,
        endpoint_id: String,
        source_local_path: String,
        remote_dir_rel_path: String,
        file_name: String,
        overwrite: bool,
        observer: Arc<dyn FileTransferObserver>,
    ) -> Result<RemoteFileTransferResult, OraculoError> {
        self.runtime.block_on(async {
            let mut src_file = tokio::fs::File::open(&source_local_path)
                .await
                .map_err(|e| err_msg(format!("Falha ao abrir arquivo para envio: {e}")))?;
            let meta = src_file
                .metadata()
                .await
                .map_err(|e| err_msg(format!("Falha ao ler tamanho do arquivo: {e}")))?;
            let total_size = meta.len();

            let (mut send, mut recv) = self.open_bi_stream(&endpoint_id).await?;
            write_stream_kind(&mut send, StreamKind::Files).await?;
            write_frame(
                &mut send,
                &FileTransferRequest::Upload {
                    dir_rel_path: remote_dir_rel_path,
                    file_name,
                    total_size,
                    overwrite,
                },
            )
            .await?;

            let resolved_name = match read_frame::<_, FileHostMsg>(&mut recv).await? {
                Some(FileHostMsg::UploadReady { resolved_name }) => resolved_name,
                Some(FileHostMsg::Error(msg)) => return Err(err_msg(msg)),
                Some(other) => {
                    return Err(err_msg(format!(
                        "Resposta inesperada ao iniciar envio: {other:?}"
                    )));
                }
                None => return Err(err_msg("Host encerrou conexão antes do envio")),
            };

            let mut buf = vec![0u8; FILE_CHUNK_SIZE];
            let mut sent: u64 = 0;
            let mut last_notify = Instant::now();
            observer.on_progress(0, total_size);

            loop {
                let n = src_file
                    .read(&mut buf)
                    .await
                    .map_err(|e| err_msg(format!("Erro ao ler arquivo local: {e}")))?;
                if n == 0 {
                    break;
                }
                write_frame(&mut send, &FileClientMsg::Chunk(buf[..n].to_vec())).await?;
                sent += n as u64;
                if last_notify.elapsed() >= Duration::from_millis(60) {
                    observer.on_progress(sent, total_size);
                    last_notify = Instant::now();
                }
            }

            write_frame(&mut send, &FileClientMsg::Finish).await?;
            send.finish()
                .map_err(|e| err_msg(format!("Erro ao finalizar envio: {e}")))?;

            match read_frame::<_, FileHostMsg>(&mut recv).await? {
                Some(FileHostMsg::Done {
                    bytes_transferred,
                    saved_rel_path,
                }) => {
                    let total = total_size.max(bytes_transferred);
                    observer.on_progress(bytes_transferred, total);
                    Ok(RemoteFileTransferResult {
                        file_name: resolved_name,
                        bytes_transferred,
                        remote_rel_path: saved_rel_path,
                        mime_hint: String::new(),
                    })
                }
                Some(FileHostMsg::Error(msg)) => Err(err_msg(msg)),
                Some(other) => Err(err_msg(format!("Resposta inesperada no upload: {other:?}"))),
                None => Err(err_msg("Host não confirmou conclusão do envio")),
            }
        })
    }

    pub fn open_terminal(
        &self,
        endpoint_id: String,
        cols: u16,
        rows: u16,
        observer: Arc<dyn TerminalObserver>,
    ) -> Result<Arc<MobileTerminalSession>, OraculoError> {
        let (cols, rows, nz_cols, nz_rows) = clamp_terminal_size(cols, rows);

        self.runtime.block_on(async {
            let (mut send, mut recv) = self.open_bi_stream(&endpoint_id).await?;

            write_stream_kind(&mut send, StreamKind::Terminal).await?;
            write_frame(&mut send, &TerminalInit { cols, rows }).await?;

            let parser = Arc::new(std::sync::Mutex::new(vt100::Parser::new_with_callbacks(
                nz_rows,
                nz_cols,
                200,
                PtyxisCallbacks::default(),
            )));
            let (tx, mut rx) = mpsc::unbounded_channel::<TerminalCmd>();

            let parser_reader = parser.clone();
            let observer_reader = observer.clone();
            tokio::spawn(async move {
                // Coalesce burst PTY output (~50fps cap) while keeping single keystrokes instant (0ms).
                const SNAPSHOT_INTERVAL: Duration = Duration::from_millis(20);
                let mut last_emit = Instant::now() - SNAPSHOT_INTERVAL;
                let mut dirty = false;
                let flush_sleep = tokio::time::sleep(SNAPSHOT_INTERVAL);
                tokio::pin!(flush_sleep);

                loop {
                    tokio::select! {
                        frame = read_frame::<_, TerminalHostMsg>(&mut recv) => {
                            match frame {
                                Ok(Some(TerminalHostMsg::Output(bytes))) => {
                                    let emit_now = last_emit.elapsed() >= SNAPSHOT_INTERVAL;
                                    let snapshot = if let Ok(mut p) = parser_reader.lock() {
                                        p.process(&bytes);
                                        if emit_now {
                                            Some(snapshot_from_parser(&mut p))
                                        } else {
                                            None
                                        }
                                    } else {
                                        continue;
                                    };
                                    if let Some(snap) = snapshot {
                                        last_emit = Instant::now();
                                        dirty = false;
                                        observer_reader.on_screen_updated(snap);
                                    } else if !dirty {
                                        let remaining = SNAPSHOT_INTERVAL.saturating_sub(last_emit.elapsed());
                                        flush_sleep
                                            .as_mut()
                                            .reset(tokio::time::Instant::now() + remaining);
                                        dirty = true;
                                    }
                                }
                                Ok(Some(TerminalHostMsg::Exit(code))) => {
                                    if dirty {
                                        if let Ok(mut p) = parser_reader.lock() {
                                            observer_reader.on_screen_updated(snapshot_from_parser(&mut p));
                                        }
                                    }
                                    observer_reader.on_session_ended(code);
                                    break;
                                }
                                Ok(Some(TerminalHostMsg::Error(msg))) => {
                                    if dirty {
                                        if let Ok(mut p) = parser_reader.lock() {
                                            observer_reader.on_screen_updated(snapshot_from_parser(&mut p));
                                        }
                                    }
                                    observer_reader.on_error(msg);
                                    break;
                                }
                                Ok(None) => {
                                    if dirty {
                                        if let Ok(mut p) = parser_reader.lock() {
                                            observer_reader.on_screen_updated(snapshot_from_parser(&mut p));
                                        }
                                    }
                                    observer_reader.on_session_ended(0);
                                    break;
                                }
                                Err(err) => {
                                    observer_reader.on_error(format!("Erro no terminal: {err:#}"));
                                    break;
                                }
                            }
                        }
                        _ = &mut flush_sleep, if dirty => {
                            dirty = false;
                            let snapshot = parser_reader
                                .lock()
                                .ok()
                                .map(|mut p| snapshot_from_parser(&mut p));
                            if let Some(snap) = snapshot {
                                last_emit = Instant::now();
                                observer_reader.on_screen_updated(snap);
                            }
                        }
                    }
                }
            });

            tokio::spawn(async move {
                let mut send: SendStream = send;
                while let Some(cmd) = rx.recv().await {
                    match cmd {
                        TerminalCmd::Input(bytes) => {
                            if write_frame(&mut send, &TerminalClientMsg::Input(bytes))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        TerminalCmd::Resize { cols, rows } => {
                            if write_frame(&mut send, &TerminalClientMsg::Resize { cols, rows })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        TerminalCmd::Close => break,
                    }
                }
                let _ = send.finish();
            });

            Ok(Arc::new(MobileTerminalSession { tx, parser }))
        })
    }

    pub fn open_input(
        &self,
        endpoint_id: String,
        observer: Arc<dyn InputObserver>,
    ) -> Result<Arc<MobileInputSession>, OraculoError> {
        self.runtime.block_on(async {
            let (conn, mut send, mut recv) = self.open_bi_stream_with_conn(&endpoint_id).await?;

            write_stream_kind(&mut send, StreamKind::Input).await?;

            let ack = tokio::time::timeout(
                Duration::from_secs(45),
                read_frame::<_, InputInitAck>(&mut recv),
            )
            .await
            .map_err(|_| err_msg("Tempo esgotado aguardando permissão de controle remoto no PC"))?
            .map_err(OraculoError::from)?
            .ok_or_else(|| err_msg("Servidor encerrou canal de entrada antes da confirmação"))?;

            if !ack.ok {
                return Err(err_msg(ack.message));
            }

            let alive = Arc::new(AtomicBool::new(true));
            let alive_task = Arc::clone(&alive);
            let connections = Arc::clone(&self.connections);
            let (tx, mut rx) = mpsc::unbounded_channel::<InputCmd>();

            let (close_tx, mut close_rx) = tokio::sync::oneshot::channel::<(String, bool)>();
            let watch_conn = conn.clone();
            let watcher_task = tokio::spawn(async move {
                let mut buf = [0u8; 32];
                let (reason, evict_conn) = tokio::select! {
                    conn_err = watch_conn.closed() => (format!("Conexão encerrada: {conn_err}"), true),
                    read_res = recv.read(&mut buf) => match read_res {
                        Ok(Some(_)) => ("Stream de entrada encerrado pelo host".to_string(), false),
                        Ok(None) => ("Host encerrou o canal de entrada".to_string(), false),
                        Err(e) => (
                            format!("Falha no canal de entrada: {e}"),
                            watch_conn.close_reason().is_some(),
                        ),
                    },
                };
                let _ = close_tx.send((reason, evict_conn));
            });

            tokio::spawn(async move {
                let mut send: SendStream = send;
                let mut pending: Option<InputCmd> = None;
                let mut close_reason: Option<(String, bool)> = None;

                loop {
                    let cmd = match pending.take() {
                        Some(item) => item,
                        None => tokio::select! {
                            biased;
                            reason = &mut close_rx => {
                                close_reason = Some(
                                    reason.unwrap_or_else(|_| ("Conexão de entrada encerrada".to_string(), false))
                                );
                                break;
                            }
                            maybe = rx.recv() => match maybe {
                                Some(item) => item,
                                None => break,
                            },
                        },
                    };

                    let mut event = match cmd {
                        InputCmd::Event(ev) => ev,
                        InputCmd::Close => break,
                    };

                    // Agrupa movimentos/rolagens enfileirados no mesmo ciclo para reduzir latência.
                    while let Ok(queued) = rx.try_recv() {
                        match (&mut event, queued) {
                            (
                                InputEvent::Move { dx, dy },
                                InputCmd::Event(InputEvent::Move { dx: nx, dy: ny }),
                            )
                            | (
                                InputEvent::Scroll { dx, dy },
                                InputCmd::Event(InputEvent::Scroll { dx: nx, dy: ny }),
                            ) => {
                                *dx += nx;
                                *dy += ny;
                            }
                            (_, other) => {
                                pending = Some(other);
                                break;
                            }
                        }
                    }

                    if let Err(err) = write_frame(&mut send, &event).await {
                        close_reason = Some((
                            format!("Falha ao enviar entrada: {err}"),
                            conn.close_reason().is_some(),
                        ));
                        break;
                    }
                }

                alive_task.store(false, Ordering::Release);
                watcher_task.abort();
                rx.close();
                let _ = send.finish();

                if let Some((reason, evict_conn)) = close_reason {
                    if evict_conn || conn.close_reason().is_some() {
                        remove_stale_connection(&connections, &endpoint_id, &conn).await;
                    }
                    observer.on_session_closed(reason);
                }
            });

            Ok(Arc::new(MobileInputSession { tx, alive }))
        })
    }

    pub fn open_screen(
        &self,
        endpoint_id: String,
        preset: MobileScreenPreset,
        observer: Arc<dyn ScreenObserver>,
    ) -> Result<Arc<MobileScreenSession>, OraculoError> {
        self.runtime.block_on(async {
            let (conn, mut send, mut recv) = self.open_bi_stream_with_conn(&endpoint_id).await?;

            write_stream_kind(&mut send, StreamKind::Screen).await?;
            write_frame(&mut send, &ScreenStart::from_preset(preset.into())).await?;

            let alive = Arc::new(AtomicBool::new(true));
            let queue = Arc::new(ClientScreenQueue::new());
            let (tx, mut rx) = mpsc::unbounded_channel::<ScreenCmd>();
            let (stop_tx, mut stop_rx) = watch::channel(false);

            let dispatch_queue = Arc::clone(&queue);
            let dispatch_observer = Arc::clone(&observer);
            let dispatch_alive = Arc::clone(&alive);
            let dispatch_connections = Arc::clone(&self.connections);
            let dispatch_conn = conn.clone();
            let dispatch_endpoint = endpoint_id.clone();

            tokio::task::spawn_blocking(move || {
                loop {
                    match dispatch_queue.pop_blocking() {
                        ClientQueuePop::Event(ClientScreenEvent::Ready(info)) => {
                            dispatch_observer.on_stream_ready(info);
                        }
                        ClientQueuePop::Event(ClientScreenEvent::Frame(frame)) => {
                            dispatch_observer.on_video_frame(frame);
                        }
                        ClientQueuePop::Event(ClientScreenEvent::Notice(msg)) => {
                            dispatch_observer.on_stream_notice(msg);
                        }
                        ClientQueuePop::Event(ClientScreenEvent::Error(msg)) => {
                            dispatch_alive.store(false, Ordering::Release);
                            dispatch_observer.on_stream_error(msg);
                            break;
                        }
                        ClientQueuePop::Closed(reason) => {
                            dispatch_alive.store(false, Ordering::Release);
                            if let Some(reason_msg) = reason {
                                if dispatch_conn.close_reason().is_some() {
                                    tokio::runtime::Handle::current().block_on(
                                        remove_stale_connection(
                                            &dispatch_connections,
                                            &dispatch_endpoint,
                                            &dispatch_conn,
                                        ),
                                    );
                                }
                                dispatch_observer.on_stream_closed(reason_msg);
                            }
                            break;
                        }
                    }
                }
            });

            let recv_queue = Arc::clone(&queue);
            let recv_alive = Arc::clone(&alive);
            let recv_stop_tx = stop_tx.clone();
            let recv_conn = conn.clone();
            tokio::spawn(async move {
                let mut unexpected_close: Option<String> = None;

                loop {
                    tokio::select! {
                        biased;
                        changed = stop_rx.changed() => {
                            if changed.is_err() || *stop_rx.borrow() {
                                break;
                            }
                        }
                        conn_err = recv_conn.closed() => {
                            unexpected_close = Some(format!("Conexão encerrada: {conn_err}"));
                            break;
                        }
                        msg = read_frame::<_, ScreenHostMsg>(&mut recv) => {
                            match msg {
                                Ok(Some(ScreenHostMsg::Ready {
                                    width,
                                    height,
                                    fps,
                                    encoder,
                                    monitor_index,
                                    monitor_count,
                                    available_monitors,
                                })) => {
                                    recv_queue.push_ready(ScreenStreamInfo {
                                        width,
                                        height,
                                        fps,
                                        encoder,
                                        monitor_index,
                                        monitor_count,
                                        available_monitors,
                                    });
                                }
                                Ok(Some(ScreenHostMsg::Frame { seq, pts_ms, keyframe, data })) => {
                                    recv_queue.push_frame(ScreenVideoFrame {
                                        seq,
                                        pts_ms,
                                        keyframe,
                                        data,
                                    });
                                }
                                Ok(Some(ScreenHostMsg::Notice(msg))) => {
                                    recv_queue.push_notice(msg);
                                }
                                Ok(Some(ScreenHostMsg::Error(err_msg_str))) => {
                                    recv_alive.store(false, Ordering::Release);
                                    let _ = recv_stop_tx.send(true);
                                    recv_queue.push_error_and_close(err_msg_str);
                                    return;
                                }
                                Ok(None) => {
                                    unexpected_close = Some("Host encerrou o streaming de tela".into());
                                    break;
                                }
                                Err(err) => {
                                    unexpected_close = Some(format!("Stream de vídeo encerrado: {err:#}"));
                                    break;
                                }
                            }
                        }
                    }
                }

                recv_alive.store(false, Ordering::Release);
                let _ = recv_stop_tx.send(true);
                recv_queue.close_with_reason(unexpected_close);
            });

            let write_alive = Arc::clone(&alive);
            tokio::spawn(async move {
                let mut send: SendStream = send;
                while let Some(cmd) = rx.recv().await {
                    match cmd {
                        ScreenCmd::Control(msg) => {
                            if write_frame(&mut send, &msg).await.is_err() {
                                break;
                            }
                        }
                        ScreenCmd::Close => {
                            let _ = write_frame(&mut send, &ScreenClientMsg::Stop).await;
                            break;
                        }
                    }
                }
                write_alive.store(false, Ordering::Release);
                rx.close();
                let _ = send.finish();
            });

            Ok(Arc::new(MobileScreenSession {
                tx,
                stop_tx,
                queue,
                alive,
            }))
        })
    }

    pub fn disconnect(&self, endpoint_id: String) {
        self.runtime.block_on(async {
            if let Some(conn) = self.connections.lock().await.remove(&endpoint_id) {
                conn.close(0u32.into(), b"disconnected");
            }
        });
    }
}

async fn measure_ping_on_conn(conn: &Connection) -> Result<ConnectionPing, OraculoError> {
    let result = tokio::time::timeout(Duration::from_secs(4), async {
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| err_msg(format!("Erro ao abrir stream de ping: {e}")))?;
        write_stream_kind(&mut send, StreamKind::Control).await?;

        // Mede apenas a ida e volta do pacote Ping/Pong, sem incluir handshake ou reconexão.
        let started = Instant::now();
        write_frame(&mut send, &ControlRequest::Ping).await?;
        send.finish()
            .map_err(|e| err_msg(format!("Erro ao fechar envio do ping: {e}")))?;

        match read_frame::<_, ControlResponse>(&mut recv)
            .await?
            .ok_or_else(|| err_msg("Host fechou stream de ping sem resposta"))?
        {
            ControlResponse::Pong { host_name } => Ok(ConnectionPing {
                host_name,
                rtt_ms: started.elapsed().as_millis() as u64,
            }),
            ControlResponse::Error(msg) => Err(err_msg(msg)),
            other => Err(err_msg(format!("Resposta inesperada ao ping: {other:?}"))),
        }
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(err_msg("Tempo esgotado aguardando ping do host")),
    }
}

async fn remove_stale_connection(
    connections: &Mutex<HashMap<String, Connection>>,
    endpoint_id: &str,
    conn: &Connection,
) {
    let mut guard = connections.lock().await;
    if let Some(cached) = guard.get(endpoint_id) {
        if cached.stable_id() == conn.stable_id() || cached.close_reason().is_some() {
            guard.remove(endpoint_id);
        }
    }
}

impl OraculoMobileClient {
    fn key_file(&self) -> PathBuf {
        self.data_dir.join("client.key")
    }

    fn hosts_file(&self) -> PathBuf {
        self.data_dir.join("saved_hosts.json")
    }

    async fn ensure_endpoint(&self) -> Result<Endpoint, OraculoError> {
        let mut guard = self.endpoint.lock().await;
        if let Some(ep) = guard.as_ref() {
            return Ok(ep.clone());
        }

        let secret_key = load_or_create_secret_key(&self.key_file())?;
        let mut transport_builder =
            QuicTransportConfig::builder().keep_alive_interval(Duration::from_secs(5));
        if let Ok(idle) = IdleTimeout::try_from(Duration::from_secs(120)) {
            transport_builder = transport_builder.max_idle_timeout(Some(idle));
        }

        // Usa raízes Mozilla embutidas (webpki-roots) para não exigir hooks JNI extras no Android.
        let ep = Endpoint::builder(presets::N0)
            .transport_config(transport_builder.build())
            .ca_tls_config(CaTlsConfig::embedded())
            .secret_key(secret_key)
            .bind()
            .await
            .map_err(|e| err_msg(format!("Falha ao iniciar Iroh Endpoint: {e}")))?;

        let _ = tokio::time::timeout(Duration::from_secs(6), ep.online()).await;
        *guard = Some(ep.clone());
        Ok(ep)
    }

    async fn get_or_connect(&self, endpoint_id: &str) -> Result<Connection, OraculoError> {
        {
            let mut conns = self.connections.lock().await;
            if let Some(conn) = conns.get(endpoint_id) {
                if conn.close_reason().is_none() {
                    return Ok(conn.clone());
                }
                conns.remove(endpoint_id);
            }
        }

        let hosts = load_saved_hosts(&self.hosts_file())?;
        let relay_url = hosts
            .into_iter()
            .find(|h| h.endpoint_id == endpoint_id)
            .and_then(|h| h.relay_url);

        let host_id = EndpointId::from_str(endpoint_id)
            .map_err(|e| err_msg(format!("EndpointId inválido: {e}")))?;
        let addr = build_addr(host_id, relay_url.as_deref())?;

        let endpoint = self.ensure_endpoint().await?;
        let conn = tokio::time::timeout(Duration::from_secs(12), endpoint.connect(addr, ALPN))
            .await
            .map_err(|_| err_msg("Tempo esgotado tentando conectar ao host"))?
            .map_err(|e| err_msg(format!("Não foi possível conectar ao host: {e}")))?;

        self.connections
            .lock()
            .await
            .insert(endpoint_id.to_string(), conn.clone());
        Ok(conn)
    }

    async fn open_bi_stream_with_conn(
        &self,
        endpoint_id: &str,
    ) -> Result<(Connection, SendStream, RecvStream), OraculoError> {
        let conn = self.get_or_connect(endpoint_id).await?;
        match tokio::time::timeout(Duration::from_secs(6), conn.open_bi()).await {
            Ok(Ok((send, recv))) => Ok((conn, send, recv)),
            _ => {
                // Reconecta uma vez caso a conexão em cache tenha expirado após bloqueio/troca de rede.
                remove_stale_connection(&self.connections, endpoint_id, &conn).await;
                let fresh = self.get_or_connect(endpoint_id).await?;
                let (send, recv) = tokio::time::timeout(Duration::from_secs(8), fresh.open_bi())
                    .await
                    .map_err(|_| err_msg("Tempo esgotado ao abrir canal de comunicação"))?
                    .map_err(|e| err_msg(format!("Erro ao abrir stream: {e}")))?;
                Ok((fresh, send, recv))
            }
        }
    }

    async fn open_bi_stream(
        &self,
        endpoint_id: &str,
    ) -> Result<(SendStream, RecvStream), OraculoError> {
        let (_conn, send, recv) = self.open_bi_stream_with_conn(endpoint_id).await?;
        Ok((send, recv))
    }

    async fn control_rpc(
        &self,
        endpoint_id: &str,
        req: &ControlRequest,
    ) -> Result<ControlResponse, OraculoError> {
        let timeout_dur = match req {
            ControlRequest::RunCommand { .. } => Duration::from_secs(45),
            _ => Duration::from_secs(12),
        };

        let result = tokio::time::timeout(timeout_dur, async {
            let (mut send, mut recv) = self.open_bi_stream(endpoint_id).await?;
            write_stream_kind(&mut send, StreamKind::Control).await?;
            write_frame(&mut send, req).await?;
            send.finish()
                .map_err(|e| err_msg(format!("Erro ao fechar envio: {e}")))?;

            read_frame(&mut recv)
                .await?
                .ok_or_else(|| err_msg("Host fechou stream de controle sem resposta"))
        })
        .await;

        match result {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(err)) => {
                self.connections.lock().await.remove(endpoint_id);
                Err(err)
            }
            Err(_) => {
                self.connections.lock().await.remove(endpoint_id);
                Err(err_msg("Tempo esgotado aguardando resposta do host"))
            }
        }
    }
}

fn map_quick_commands(items: Vec<oraculo_proto::QuickCommandInfo>) -> Vec<QuickCommandItem> {
    items
        .into_iter()
        .map(|c| QuickCommandItem {
            id: c.id,
            label: c.label,
            description: c.description,
            command_line: c.command_line,
        })
        .collect()
}

fn map_file_entries(items: Vec<oraculo_proto::FileEntryInfo>) -> Vec<RemoteFileEntry> {
    items
        .into_iter()
        .map(|e| RemoteFileEntry {
            name: e.name,
            rel_path: e.rel_path,
            is_dir: e.is_dir,
            size: e.size,
            modified_unix: e.modified_unix,
            mime_hint: e.mime_hint,
        })
        .collect()
}

const MAX_SCROLLBACK_SNAPSHOT_ROWS: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
struct CellStyleKey {
    fg_kind: TerminalColorKind,
    fg_val: u32,
    bg_kind: TerminalColorKind,
    bg_val: u32,
    bold: bool,
    italic: bool,
    underline: bool,
    dim: bool,
    inverse: bool,
}

fn convert_vt_color(color: vt100::Color) -> (TerminalColorKind, u32) {
    match color {
        vt100::Color::Default => (TerminalColorKind::Default, 0),
        vt100::Color::Idx(idx) => (TerminalColorKind::Indexed, idx as u32),
        vt100::Color::Rgb(r, g, b) => (
            TerminalColorKind::Rgb,
            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
        ),
    }
}

fn extract_styled_row(
    screen: &vt100::Screen,
    row: u16,
    cols: u16,
    preserve_through_col: Option<u16>,
) -> (TerminalLine, String, bool) {
    let mut last_used_col: Option<u16> = None;
    let mut has_visible_content = false;

    for col in 0..cols {
        if let Some(cell) = screen.cell(row, col) {
            if cell.is_wide_continuation() {
                continue;
            }
            let has_text = cell.has_contents() && !cell.contents().trim().is_empty();
            let has_bg = cell.bgcolor() != vt100::Color::Default || cell.inverse();
            if has_text || has_bg {
                last_used_col = Some(col);
                has_visible_content = true;
            } else if cell.has_contents() {
                last_used_col = Some(col);
            }
        }
    }

    let limit_col = match (last_used_col, preserve_through_col) {
        (Some(a), Some(b)) => a.max(b).min(cols.saturating_sub(1)),
        (Some(a), None) => a,
        (None, Some(b)) => b.min(cols.saturating_sub(1)),
        (None, None) => return (TerminalLine { spans: Vec::new() }, String::new(), false),
    };

    let mut spans: Vec<TerminalSpan> = Vec::new();
    let mut plain = String::new();
    let mut active_key: Option<CellStyleKey> = None;
    let mut active_text = String::new();

    for col in 0..=limit_col {
        let Some(cell) = screen.cell(row, col) else {
            continue;
        };
        if cell.is_wide_continuation() {
            continue;
        }
        let ch = if cell.has_contents() {
            cell.contents()
        } else {
            " "
        };
        plain.push_str(ch);

        let (fg_kind, fg_val) = convert_vt_color(cell.fgcolor());
        let (bg_kind, bg_val) = convert_vt_color(cell.bgcolor());
        let key = CellStyleKey {
            fg_kind,
            fg_val,
            bg_kind,
            bg_val,
            bold: cell.bold(),
            italic: cell.italic(),
            underline: cell.underline(),
            dim: cell.dim(),
            inverse: cell.inverse(),
        };

        if active_key == Some(key) {
            active_text.push_str(ch);
        } else {
            if let Some(prev) = active_key.take() {
                spans.push(TerminalSpan {
                    text: std::mem::take(&mut active_text),
                    fg_kind: prev.fg_kind,
                    fg_val: prev.fg_val,
                    bg_kind: prev.bg_kind,
                    bg_val: prev.bg_val,
                    bold: prev.bold,
                    italic: prev.italic,
                    underline: prev.underline,
                    dim: prev.dim,
                    inverse: prev.inverse,
                });
            }
            active_key = Some(key);
            active_text.push_str(ch);
        }
    }

    if let Some(prev) = active_key {
        spans.push(TerminalSpan {
            text: active_text,
            fg_kind: prev.fg_kind,
            fg_val: prev.fg_val,
            bg_kind: prev.bg_kind,
            bg_val: prev.bg_val,
            bold: prev.bold,
            italic: prev.italic,
            underline: prev.underline,
            dim: prev.dim,
            inverse: prev.inverse,
        });
    }

    let plain_trimmed = plain.trim_end().to_string();
    (TerminalLine { spans }, plain_trimmed, has_visible_content)
}

fn snapshot_from_parser(parser: &mut vt100::Parser<PtyxisCallbacks>) -> TerminalScreenSnapshot {
    let title = parser.callbacks().title.clone();
    let bell_count = parser.callbacks().bell_count;

    let screen = parser.screen_mut();
    let (nz_rows, nz_cols) = screen.size();
    let rows = nz_rows.get();
    let cols = nz_cols.get();
    let (raw_cur_row, cur_col) = screen.cursor_position();
    let cursor_visible = !screen.hide_cursor();
    let alternate = screen.alternate_screen();

    let mut lines: Vec<TerminalLine> = Vec::new();
    let mut plain_lines: Vec<String> = Vec::new();
    let mut prepended_sb: usize = 0;

    if !alternate {
        screen.set_scrollback(usize::MAX);
        let sb_total = screen.scrollback();
        let mut remaining = sb_total.min(MAX_SCROLLBACK_SNAPSHOT_ROWS);
        while remaining > 0 {
            screen.set_scrollback(remaining);
            let take = remaining.min(rows as usize);
            for r in 0..(take as u16) {
                let (line, plain, _) = extract_styled_row(screen, r, cols, None);
                lines.push(line);
                plain_lines.push(plain);
                prepended_sb += 1;
            }
            remaining -= take;
        }
        screen.set_scrollback(0);
    }

    let mut viewport_rows: Vec<(TerminalLine, String, bool)> = Vec::with_capacity(rows as usize);
    let mut last_nonempty_viewport: u16 = 0;
    for r in 0..rows {
        let preserve_col = if cursor_visible && r == raw_cur_row {
            Some(cur_col)
        } else {
            None
        };
        let entry = extract_styled_row(screen, r, cols, preserve_col);
        if entry.2 {
            last_nonempty_viewport = r;
        }
        viewport_rows.push(entry);
    }

    let keep_until = if alternate {
        rows.saturating_sub(1)
    } else if cursor_visible {
        last_nonempty_viewport.max(raw_cur_row).min(rows.saturating_sub(1))
    } else {
        last_nonempty_viewport.min(rows.saturating_sub(1))
    };

    for (line, plain, _) in viewport_rows.into_iter().take((keep_until as usize) + 1) {
        lines.push(line);
        plain_lines.push(plain);
    }

    let cursor_row = ((prepended_sb + raw_cur_row as usize).min(lines.len().saturating_sub(1))) as u16;
    let plain_text = plain_lines.join("\n").trim_end().to_string();

    TerminalScreenSnapshot {
        cols,
        rows,
        cursor_row,
        cursor_col: cur_col.min(cols.saturating_sub(1)),
        cursor_visible,
        title,
        bell_count,
        plain_text,
        lines,
    }
}

fn clamp_terminal_size(cols: u16, rows: u16) -> (u16, u16, NonZeroU16, NonZeroU16) {
    let cols = cols.max(20);
    let rows = rows.max(8);
    let nz_cols = NonZeroU16::new(cols).unwrap_or(NonZeroU16::MIN);
    let nz_rows = NonZeroU16::new(rows).unwrap_or(NonZeroU16::MIN);
    (cols, rows, nz_cols, nz_rows)
}

fn build_addr(host_id: EndpointId, relay_url: Option<&str>) -> Result<EndpointAddr, OraculoError> {
    let mut addr = EndpointAddr::new(host_id);
    if let Some(raw) = relay_url {
        let parsed = RelayUrl::from_str(raw)
            .map_err(|e| err_msg(format!("URL de relay inválida: {e}")))?;
        addr = addr.with_relay_url(parsed);
    }
    Ok(addr)
}

static PRIVATE_WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

fn write_private_file(path: &Path, bytes: &[u8], label: &str) -> Result<(), OraculoError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| err_msg(format!("Erro ao criar diretório de {label}: {e}")))?;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = PRIVATE_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("data");
    let tmp = path.with_file_name(format!(".{file_name}.{}.{nanos}-{seq}.tmp", std::process::id()));

    let write_res = (|| -> std::io::Result<()> {
        use std::io::Write;
        #[cfg(unix)]
        let mut f = {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)?;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
            file
        };
        #[cfg(not(unix))]
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;

        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp);
        return Err(err_msg(format!("Falha ao gravar {label}: {e}")));
    }

    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        err_msg(format!("Falha ao salvar {label}: {e}"))
    })?;
    Ok(())
}

fn load_or_create_secret_key(path: &Path) -> Result<SecretKey, OraculoError> {
    if path.exists() {
        let raw = fs::read_to_string(path)
            .map_err(|e| err_msg(format!("Erro ao ler chave do cliente: {e}")))?;
        let bytes = hex::decode(raw.trim())
            .map_err(|e| err_msg(format!("Chave do cliente corrompida: {e}")))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| err_msg("client.key deve ter 32 bytes"))?;
        return Ok(SecretKey::from_bytes(&arr));
    }

    let key = SecretKey::generate();
    write_private_file(path, hex::encode(key.to_bytes()).as_bytes(), "client.key")?;
    Ok(key)
}

fn load_saved_hosts(path: &Path) -> Result<Vec<SavedHostInfo>, OraculoError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| err_msg(format!("Falha ao ler saved_hosts.json: {e}")))?;
    serde_json::from_str(&raw)
        .map_err(|e| err_msg(format!("JSON inválido em saved_hosts.json: {e}")))
}

fn save_hosts(path: &Path, hosts: &[SavedHostInfo]) -> Result<(), OraculoError> {
    let json = serde_json::to_string_pretty(hosts)
        .map_err(|e| err_msg(format!("Falha ao serializar hosts: {e}")))?;
    write_private_file(path, json.as_bytes(), "saved_hosts.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_lists_hosts() {
        let dir = std::env::temp_dir().join("oraculo_mobile_core_test");
        let _ = fs::remove_dir_all(&dir);
        let client = OraculoMobileClient::new(dir.to_string_lossy().into_owned()).unwrap();
        assert!(client.list_saved_hosts().unwrap().is_empty());

        let sample = vec![SavedHostInfo {
            endpoint_id: "abcd".repeat(16),
            host_name: "mr".into(),
            relay_url: Some("https://use1-1.relay.n0.iroh.link./".into()),
        }];
        save_hosts(&client.hosts_file(), &sample).unwrap();
        assert_eq!(client.list_saved_hosts().unwrap().len(), 1);

        client.remove_saved_host("abcd".repeat(16)).unwrap();
        assert!(client.list_saved_hosts().unwrap().is_empty());

        let key1 = load_or_create_secret_key(&client.key_file()).unwrap();
        let key2 = load_or_create_secret_key(&client.key_file()).unwrap();
        assert_eq!(key1.to_bytes(), key2.to_bytes());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let key_mode = fs::metadata(&client.key_file())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            let hosts_mode = fs::metadata(&client.hosts_file())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(key_mode, 0o600);
            assert_eq!(hosts_mode, 0o600);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn renders_ansi_colors_title_and_scrollback_snapshot() {
        let rows = NonZeroU16::new(4).unwrap();
        let cols = NonZeroU16::new(20).unwrap();
        let mut parser =
            vt100::Parser::new_with_callbacks(rows, cols, 50, PtyxisCallbacks::default());
        parser.process(b"\x1b]2;marcos@mr:~\x07");
        parser.process(b"line1\r\nline2\r\nline3\r\nline4\r\n\x1b[32mok\x1b[0m > ");

        let snap = snapshot_from_parser(&mut parser);
        assert_eq!(snap.title, "marcos@mr:~");
        assert!(snap.plain_text.contains("line1"));
        assert!(snap.plain_text.contains("ok >"));
        let last_line = snap.lines.last().unwrap();
        assert!(last_line.spans.iter().any(|s| {
            s.text == "ok" && s.fg_kind == TerminalColorKind::Indexed && s.fg_val == 2
        }));
        assert_eq!(snap.cursor_row as usize, snap.lines.len() - 1);
        assert_eq!(snap.cursor_col, 5);
    }

    #[test]
    fn screen_queue_condvar_delivers_ordered_events_across_threads() {
        let queue = Arc::new(ClientScreenQueue::new());
        let consumer_queue = Arc::clone(&queue);
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();

        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            loop {
                match consumer_queue.pop_blocking() {
                    ClientQueuePop::Event(ClientScreenEvent::Ready(info)) => {
                        seen.push(format!("ready:{}x{}", info.width, info.height));
                    }
                    ClientQueuePop::Event(ClientScreenEvent::Frame(frame)) => {
                        let seq = frame.seq;
                        seen.push(format!("frame:{seq}:{}", frame.keyframe));
                        if seq == 2 {
                            let _ = ack_tx.send(());
                        }
                    }
                    ClientQueuePop::Event(ClientScreenEvent::Notice(msg)) => {
                        seen.push(format!("notice:{msg}"));
                    }
                    ClientQueuePop::Event(ClientScreenEvent::Error(msg)) => {
                        seen.push(format!("err:{msg}"));
                    }
                    ClientQueuePop::Closed(reason) => {
                        seen.push(format!("closed:{reason:?}"));
                        break;
                    }
                }
            }
            seen
        });

        queue.push_ready(ScreenStreamInfo {
            width: 1280,
            height: 720,
            fps: 30,
            encoder: "nvh264enc".into(),
            monitor_index: 0,
            monitor_count: 1,
            available_monitors: 2,
        });
        queue.push_frame(ScreenVideoFrame {
            seq: 1,
            pts_ms: 0,
            keyframe: true,
            data: vec![0, 0, 0, 1, 0x65],
        });
        queue.push_frame(ScreenVideoFrame {
            seq: 2,
            pts_ms: 33,
            keyframe: false,
            data: vec![0, 0, 0, 1, 0x61],
        });
        ack_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        queue.push_error_and_close("portal negado".into());

        let seen = handle.join().unwrap();
        assert_eq!(
            seen,
            vec![
                "ready:1280x720",
                "frame:1:true",
                "frame:2:false",
                "err:portal negado",
                "closed:None"
            ]
        );
    }

    #[test]
    fn screen_queue_keeps_ready_ahead_of_fresh_keyframe() {
        let queue = ClientScreenQueue::new();
        queue.push_ready(ScreenStreamInfo {
            width: 1152,
            height: 720,
            fps: 30,
            encoder: "nvh264enc".into(),
            monitor_index: 0,
            monitor_count: 1,
            available_monitors: 1,
        });
        queue.push_frame(ScreenVideoFrame {
            seq: 0,
            pts_ms: 0,
            keyframe: false,
            data: vec![1],
        });
        queue.push_frame(ScreenVideoFrame {
            seq: 1,
            pts_ms: 33,
            keyframe: true,
            data: vec![2],
        });
        for i in 2..=(MAX_CLIENT_VIDEO_DELTAS as u64 + 2) {
            queue.push_frame(ScreenVideoFrame {
                seq: i,
                pts_ms: i * 33,
                keyframe: false,
                data: vec![i as u8],
            });
        }
        queue.push_frame(ScreenVideoFrame {
            seq: 50,
            pts_ms: 400,
            keyframe: true,
            data: vec![50],
        });

        let g = queue.inner.lock().unwrap();
        assert_eq!(g.events.len(), 2);
        assert!(matches!(
            &g.events[0],
            ClientScreenEvent::Ready(info) if info.width == 1152 && info.height == 720
        ));
        assert!(matches!(
            &g.events[1],
            ClientScreenEvent::Frame(f) if f.seq == 50 && f.keyframe
        ));
        assert!(g.dropped_frames >= MAX_CLIENT_VIDEO_DELTAS as u64);
    }
}
