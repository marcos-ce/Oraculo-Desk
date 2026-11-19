mod bridge;

use bridge::MobileWolBridge;
use iroh::endpoint::{
    Connection, ConnectionError, IdleTimeout, QuicTransportConfig, RecvStream, SendStream, VarInt,
    presets,
};
use iroh::tls::CaTlsConfig;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayUrl, SecretKey, Signature};
use oraculo_proto::mcu::{
    McuBridgeConfig, decode_mcu_pairing_uri, decode_mcu_secret, encode_mcu_secret_b64url,
    ping_mcu_bridge, relay_wake_via_mcu, sanitize_mcu_name,
};
use oraculo_proto::pairing::{
    ALPN_PAIR_CODE, compute_pair_binding_digest, decode_pairing_uri, derive_short_code_secrets,
};
use oraculo_proto::wol::{
    DEFAULT_WOL_PORT, normalize_wol_mac, sanitize_wol_info, send_magic_packets,
};
use oraculo_proto::{
    ALPN, CodePairRequest, CodePairResponse, ControlRequest, ControlResponse, FILE_CHUNK_SIZE,
    FileClientMsg, FileHostMsg, FileTransferRequest, HostPermissionsWire, InputEvent, InputInitAck,
    MouseButton, NodeRole, PairRequest, PairResponse, RemoteKey, ScreenClientMsg, ScreenHostMsg,
    ScreenPreset, ScreenStart, StreamKind, TerminalClientMsg, TerminalHostMsg, TerminalInit,
    TerminalSessionSummary, TunnelOpenReq, TunnelOpenResp, TunnelTarget, WolInfo, read_frame,
    write_frame, write_stream_kind,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
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

static MOBILE_UI_LANG_EN: AtomicBool = AtomicBool::new(false);

fn current_mobile_lang() -> oraculo_proto::i18n::UiLang {
    if MOBILE_UI_LANG_EN.load(Ordering::Acquire) {
        oraculo_proto::i18n::UiLang::En
    } else {
        oraculo_proto::i18n::UiLang::Pt
    }
}

fn localize_mobile(msg: impl AsRef<str>) -> String {
    oraculo_proto::i18n::localize_text(msg.as_ref(), current_mobile_lang())
}

impl From<anyhow::Error> for OraculoError {
    fn from(err: anyhow::Error) -> Self {
        Self::Failure {
            reason: localize_mobile(format!("{err:#}")),
        }
    }
}

fn err_msg(msg: impl Into<String>) -> OraculoError {
    OraculoError::Failure {
        reason: localize_mobile(msg.into()),
    }
}

fn default_wol_port() -> u16 {
    DEFAULT_WOL_PORT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, uniffi::Enum)]
pub enum DeviceRole {
    #[default]
    Computer,
    WolBridge,
}

impl From<NodeRole> for DeviceRole {
    fn from(value: NodeRole) -> Self {
        match value {
            NodeRole::Computer => Self::Computer,
            NodeRole::WolBridge => Self::WolBridge,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SavedHostInfo {
    pub endpoint_id: String,
    pub host_name: String,
    #[serde(default)]
    pub relay_url: Option<String>,
    #[serde(default)]
    pub role: DeviceRole,
    #[serde(default)]
    pub wol_mac: Option<String>,
    #[serde(default)]
    pub wol_extra_macs: Vec<String>,
    #[serde(default)]
    pub wol_broadcast: Option<String>,
    #[serde(default)]
    pub wol_target_override: Option<String>,
    #[serde(default = "default_wol_port")]
    pub wol_port: u16,
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
    pub role: DeviceRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct MobileHostPermissions {
    pub allow_screen: bool,
    pub allow_input: bool,
    pub allow_terminal: bool,
    pub allow_files_read: bool,
    pub allow_files_write: bool,
    pub allow_commands_run: bool,
    pub allow_commands_edit: bool,
    pub allow_rdp_tunnel: bool,
}

impl From<HostPermissionsWire> for MobileHostPermissions {
    fn from(w: HostPermissionsWire) -> Self {
        Self {
            allow_screen: w.allow_screen,
            allow_input: w.allow_input,
            allow_terminal: w.allow_terminal,
            allow_files_read: w.allow_files_read,
            allow_files_write: w.allow_files_write,
            allow_commands_run: w.allow_commands_run,
            allow_commands_edit: w.allow_commands_edit,
            allow_rdp_tunnel: w.allow_rdp_tunnel,
        }
    }
}

#[derive(uniffi::Object)]
pub struct MobileTunnelSession {
    local_port: u16,
    remote_rdp_port: u16,
    stop_tx: watch::Sender<bool>,
    alive: Arc<AtomicBool>,
}

#[uniffi::export]
impl MobileTunnelSession {
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub fn local_address(&self) -> String {
        format!("127.0.0.1:{}", self.local_port)
    }

    pub fn remote_rdp_port(&self) -> u16 {
        self.remote_rdp_port
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn close_tunnel(&self) {
        self.alive.store(false, Ordering::Release);
        let _ = self.stop_tx.send(true);
    }
}

impl Drop for MobileTunnelSession {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        let _ = self.stop_tx.send(true);
    }
}

#[uniffi::export(with_foreign)]
pub trait HostStateObserver: Send + Sync + std::fmt::Debug {
    fn on_state(&self, host_name: String, permissions: MobileHostPermissions);
    fn on_disconnected(&self, code: u32, reason: String);
}

#[derive(uniffi::Object)]
pub struct MobileHostStateWatcher {
    stop_tx: watch::Sender<bool>,
}

#[uniffi::export]
impl MobileHostStateWatcher {
    pub fn close_watcher(&self) {
        let _ = self.stop_tx.send(true);
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WakeDispatchResult {
    pub direct_packets: u32,
    pub relayed_via: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MobileTerminalSessionInfo {
    pub id: String,
    pub label: String,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub attached_clients: u32,
    pub created_unix: u64,
    pub last_activity_unix: u64,
    pub cols: u16,
    pub rows: u16,
}

impl From<TerminalSessionSummary> for MobileTerminalSessionInfo {
    fn from(s: TerminalSessionSummary) -> Self {
        Self {
            id: s.id,
            label: s.label,
            running: s.running,
            exit_code: s.exit_code,
            attached_clients: s.attached_clients,
            created_unix: s.created_unix,
            last_activity_unix: s.last_activity_unix,
            cols: s.cols,
            rows: s.rows,
        }
    }
}

/// Callback implementado em Kotlin para receber atualizações da tela ANSI do terminal.
#[uniffi::export(with_foreign)]
pub trait TerminalObserver: Send + Sync + std::fmt::Debug {
    fn on_session_ready(
        &self,
        session_id: String,
        label: String,
        persistent: bool,
        reconnected: bool,
    );
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
    endpoint: Arc<Mutex<Option<Endpoint>>>,
    connections: Arc<Mutex<HashMap<String, Connection>>>,
    connect_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    network_dirty: Arc<AtomicBool>,
    consecutive_connect_failures: Arc<AtomicU32>,
    last_recycle_at: Arc<StdMutex<Option<Instant>>>,
    bridge: Arc<MobileWolBridge>,
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
            endpoint: Arc::new(Mutex::new(None)),
            connections: Arc::new(Mutex::new(HashMap::new())),
            connect_locks: Arc::new(Mutex::new(HashMap::new())),
            network_dirty: Arc::new(AtomicBool::new(false)),
            consecutive_connect_failures: Arc::new(AtomicU32::new(0)),
            last_recycle_at: Arc::new(StdMutex::new(None)),
            bridge: Arc::new(MobileWolBridge::default()),
        })
    }

    /// Define o idioma de exibição (`"pt"` ou `"en"`) para localizar erros e avisos vindos do núcleo Rust/host.
    pub fn set_language(&self, lang: String) {
        let is_en = lang.trim().to_ascii_lowercase().starts_with("en");
        MOBILE_UI_LANG_EN.store(is_en, Ordering::Release);
    }

    /// Pré-aquece o endpoint Iroh (socket UDP + relay) em segundo plano antes do primeiro toque no host.
    pub fn warm_up(&self) {
        let ep_slot = Arc::clone(&self.endpoint);
        let key_file = self.key_file();
        self.runtime.spawn(async move {
            let _ = get_or_init_endpoint(&ep_slot, &key_file).await;
        });
    }

    /// Notifica o endpoint Iroh sobre troca de rede no Android (Wi-Fi <-> dados móveis)
    /// e limpa conexões em cache presas à interface anterior.
    pub fn notify_network_change(&self) {
        self.network_dirty.store(true, Ordering::Release);
        let ep_slot = Arc::clone(&self.endpoint);
        let conns_slot = Arc::clone(&self.connections);
        let key_file = self.key_file();
        self.runtime.spawn(async move {
            {
                let mut guard = conns_slot.lock().await;
                guard.clear();
            }

            let maybe_ep = { ep_slot.lock().await.clone() };
            if let Some(ep) = maybe_ep {
                ep.network_change().await;
                let _ = tokio::time::timeout(Duration::from_millis(800), ep.online()).await;
            } else {
                let _ = get_or_init_endpoint(&ep_slot, &key_file).await;
            }
        });
    }

    pub fn list_saved_hosts(&self) -> Result<Vec<SavedHostInfo>, OraculoError> {
        let mut hosts = load_saved_hosts(&self.hosts_file())?;
        if let Ok(mcu_bridges) = load_saved_mcu_bridges(&self.mcu_bridges_file()) {
            for bridge in mcu_bridges {
                hosts.push(bridge.to_saved_host_info());
            }
        }
        Ok(hosts)
    }

    pub fn remove_saved_host(&self, endpoint_id: String) -> Result<(), OraculoError> {
        if let Some(bridge_id) = endpoint_id.strip_prefix("mcu:") {
            let mut mcu_bridges = load_saved_mcu_bridges(&self.mcu_bridges_file())?;
            mcu_bridges.retain(|b| b.bridge_id != bridge_id);
            save_mcu_bridges(&self.mcu_bridges_file(), &mcu_bridges)?;
            return Ok(());
        }
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
        let trimmed = uri.trim();
        if trimmed.len() >= 20 && trimmed[..20].eq_ignore_ascii_case("oraculo://mcu-bridge") {
            return self.pair_mcu_bridge_uri(trimmed);
        }
        if !trimmed.starts_with("oraculo://") {
            return self.pair_by_code(uri, device_name);
        }
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

            let info = self.persist_paired_host(
                payload.endpoint_id,
                &resp.host_name,
                payload.relay_url,
                resp.role.into(),
                resp.wol,
            )?;

            self.connections
                .lock()
                .await
                .insert(info.endpoint_id.clone(), conn);

            Ok(info)
        })
    }

    pub fn pair_by_code(
        &self,
        code: String,
        device_name: String,
    ) -> Result<SavedHostInfo, OraculoError> {
        if code.trim_start().starts_with("oraculo://") {
            return self.pair_by_uri(code, device_name);
        }
        self.runtime.block_on(async {
            let derived = derive_short_code_secrets(&code)?;
            let rendezvous_id = SecretKey::from_bytes(&derived.rendezvous_secret_key).public();
            let endpoint = self.ensure_endpoint().await?;

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

            let conn = conn_opt.ok_or_else(|| {
                err_msg(
                    "Código não encontrado ou expirado. Confira os 8 caracteres e gere um novo código com 'p' no PC.",
                )
            })?;

            let (mut send, mut recv) = conn
                .open_bi()
                .await
                .map_err(|e| err_msg(format!("Falha ao abrir canal de código: {e}")))?;

            write_frame(
                &mut send,
                &CodePairRequest {
                    auth_token: derived.auth_token,
                    device_name,
                },
            )
            .await?;
            send.finish()
                .map_err(|e| err_msg(format!("Erro ao finalizar envio do código: {e}")))?;

            let resp: CodePairResponse = tokio::time::timeout(
                Duration::from_secs(8),
                read_frame(&mut recv),
            )
            .await
            .map_err(|_| err_msg("Tempo esgotado aguardando confirmação do código"))??
            .ok_or_else(|| err_msg("Host encerrou conexão antes de confirmar o código"))?;

            conn.close(0u32.into(), b"pair done");

            if !resp.accepted {
                return Err(err_msg(format!("Pareamento recusado: {}", resp.message)));
            }

            let real_host_id = EndpointId::from_str(&resp.host_endpoint_id)
                .map_err(|e| err_msg(format!("Identidade do host inválida: {e}")))?;
            let sig_bytes: [u8; 64] = resp
                .host_signature
                .as_slice()
                .try_into()
                .map_err(|_| err_msg("Assinatura Ed25519 do host ausente ou malformada"))?;
            let sig = Signature::from_bytes(&sig_bytes);
            let digest = compute_pair_binding_digest(
                rendezvous_id.as_bytes(),
                endpoint.id().as_bytes(),
                real_host_id.as_bytes(),
            );
            real_host_id
                .verify(&digest, &sig)
                .map_err(|_| err_msg("Assinatura criptográfica do host inválida (conexão bloqueada)"))?;

            let info = self.persist_paired_host(
                real_host_id.to_string(),
                &resp.host_name,
                resp.relay_url,
                resp.role.into(),
                resp.wol,
            )?;

            Ok(info)
        })
    }

    pub fn update_host_wol(
        &self,
        endpoint_id: String,
        mac: Option<String>,
        target_override: Option<String>,
        port: u16,
    ) -> Result<SavedHostInfo, OraculoError> {
        let normalized_mac = match mac.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(raw) => Some(normalize_wol_mac(raw)?),
            None => None,
        };
        let cleaned_target = target_override
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);
        let effective_port = if port == 0 { DEFAULT_WOL_PORT } else { port };

        let mut hosts = load_saved_hosts(&self.hosts_file())?;
        let host = hosts
            .iter_mut()
            .find(|h| h.endpoint_id == endpoint_id)
            .ok_or_else(|| err_msg("Computador salvo não encontrado"))?;

        if normalized_mac.is_none() {
            host.wol_extra_macs.clear();
        } else if let Some(ref primary) = normalized_mac {
            host.wol_extra_macs.retain(|m| m != primary);
        }
        host.wol_mac = normalized_mac;
        host.wol_target_override = cleaned_target;
        host.wol_port = effective_port;

        let updated = host.clone();
        save_hosts(&self.hosts_file(), &hosts)?;
        Ok(updated)
    }

    pub fn wake_host(&self, endpoint_id: String) -> Result<WakeDispatchResult, OraculoError> {
        if endpoint_id.starts_with("mcu:") {
            return Err(err_msg(
                "Este aparelho atua como Ponte Wake-on-LAN e não pode ser ligado por si mesmo",
            ));
        }
        let hosts = load_saved_hosts(&self.hosts_file())?;
        let host = hosts
            .iter()
            .find(|h| h.endpoint_id == endpoint_id)
            .cloned()
            .ok_or_else(|| err_msg("Computador salvo não encontrado"))?;
        if host.role == DeviceRole::WolBridge {
            return Err(err_msg(
                "Este aparelho atua como Ponte Wake-on-LAN e não pode ser ligado por si mesmo",
            ));
        }

        let mut macs: Vec<String> = Vec::new();
        if let Some(primary) = host.wol_mac.clone().filter(|s| !s.trim().is_empty()) {
            macs.push(primary);
        }
        for extra in &host.wol_extra_macs {
            if !extra.trim().is_empty() && !macs.contains(extra) {
                macs.push(extra.clone());
            }
        }
        if macs.is_empty() {
            return Err(err_msg(
                "Nenhum endereço MAC configurado para Wake-on-LAN neste computador",
            ));
        }

        let mut bridge_candidates: Vec<SavedHostInfo> = hosts
            .into_iter()
            .filter(|h| h.endpoint_id != endpoint_id)
            .collect();
        bridge_candidates.sort_by_key(|peer| {
            let is_dedicated_bridge = peer.role == DeviceRole::WolBridge;
            let same_subnet =
                host.wol_broadcast.is_some() && peer.wol_broadcast == host.wol_broadcast;
            match (is_dedicated_bridge, same_subnet) {
                (true, true) => 0u8,
                (true, false) => 1u8,
                (false, true) => 2u8,
                (false, false) => 3u8,
            }
        });
        bridge_candidates.truncate(8);

        let mcu_bridges = load_saved_mcu_bridges(&self.mcu_bridges_file()).unwrap_or_default();

        self.runtime.block_on(async move {
            // 1. Disparo UDP direto (LAN Wi-Fi/Cabeada + DDNS customizado) — sempre mantido intacto.
            let direct_res = send_magic_packets(
                &macs,
                host.wol_broadcast.as_deref(),
                host.wol_target_override.as_deref(),
                host.wol_port,
            )
            .await;

            // 2. Disparo paralelo via Pontes P2P Iroh e Pontes MCU (Cardputer/ESP32 via MQTT TLS).
            let mut relayed_via = Vec::new();
            if !bridge_candidates.is_empty() || !mcu_bridges.is_empty() {
                let endpoint_opt = if bridge_candidates.is_empty() {
                    None
                } else {
                    self.ensure_endpoint().await.ok()
                };
                let relay_wol = WolInfo {
                    macs: macs.clone(),
                    broadcast: host.wol_broadcast.clone(),
                };
                let mut join_set = tokio::task::JoinSet::new();
                for peer in bridge_candidates {
                    let connections = Arc::clone(&self.connections);
                    let ep_opt = endpoint_opt.clone();
                    let req = ControlRequest::RelayWakeOnLan {
                        wol: relay_wol.clone(),
                    };
                    join_set.spawn(async move {
                        let peer_id = peer.endpoint_id;
                        let peer_name = peer.host_name;
                        let peer_relay = peer.relay_url;
                        let res = tokio::time::timeout(Duration::from_millis(4200), async {
                            let conn = {
                                let cached = {
                                    let guard = connections.lock().await;
                                    guard
                                        .get(&peer_id)
                                        .filter(|c| c.close_reason().is_none())
                                        .cloned()
                                };
                                match cached {
                                    Some(c) => c,
                                    None => {
                                        let ep = ep_opt.ok_or_else(|| err_msg("Endpoint indisponível"))?;
                                        let host_id = EndpointId::from_str(&peer_id)
                                            .map_err(|e| err_msg(format!("EndpointId inválido: {e}")))?;
                                        let addr = build_addr(host_id, peer_relay.as_deref())?;
                                        let fresh = ep
                                            .connect(addr, ALPN)
                                            .await
                                            .map_err(|e| err_msg(format!("Falha ao conectar à ponte: {e}")))?;
                                        connections.lock().await.insert(peer_id.clone(), fresh.clone());
                                        fresh
                                    }
                                }
                            };
                            let (mut send, mut recv) = conn
                                .open_bi()
                                .await
                                .map_err(|e| err_msg(format!("Erro ao abrir canal na ponte: {e}")))?;
                            write_stream_kind(&mut send, StreamKind::Control).await?;
                            write_frame(&mut send, &req).await?;
                            send.finish()
                                .map_err(|e| err_msg(format!("Erro ao finalizar envio à ponte: {e}")))?;
                            read_frame::<_, ControlResponse>(&mut recv)
                                .await
                                .map_err(OraculoError::from)
                        })
                        .await;
                        match res {
                            Ok(Ok(Some(ControlResponse::WakeDispatched { packets_sent })))
                                if packets_sent > 0 =>
                            {
                                Some(peer_name)
                            }
                            _ => None,
                        }
                    });
                }

                for mcu in mcu_bridges.into_iter().take(6) {
                    let macs_mcu = macs.clone();
                    let broadcast_mcu = host.wol_broadcast.clone();
                    let port_mcu = host.wol_port;
                    join_set.spawn(async move {
                        let Ok(cfg) = mcu.to_config() else {
                            return None;
                        };
                        match relay_wake_via_mcu(&cfg, &macs_mcu, broadcast_mcu.as_deref(), port_mcu)
                            .await
                        {
                            Ok(sent) if sent > 0 => Some(cfg.name),
                            _ => None,
                        }
                    });
                }

                while let Some(joined) = join_set.join_next().await {
                    if let Ok(Some(name)) = joined {
                        relayed_via.push(name);
                    }
                }
            }

            match direct_res {
                Ok(sent) => Ok(WakeDispatchResult {
                    direct_packets: sent as u32,
                    relayed_via,
                }),
                Err(_) if !relayed_via.is_empty() => Ok(WakeDispatchResult {
                    direct_packets: 0,
                    relayed_via,
                }),
                Err(direct_err) => Err(OraculoError::from(direct_err)),
            }
        })
    }

    pub fn is_wol_bridge_enabled(&self) -> bool {
        self.bridge.is_running()
    }

    pub fn set_wol_bridge_enabled(
        &self,
        enabled: bool,
        device_label: String,
    ) -> Result<(), OraculoError> {
        if !enabled {
            self.bridge.stop();
            return Ok(());
        }
        self.runtime.block_on(async {
            let endpoint = self.ensure_endpoint().await?;
            self.bridge
                .start(endpoint, self.data_dir.clone(), device_label);
            Ok(())
        })
    }

    pub fn issue_wol_bridge_code(&self, device_label: String) -> Result<String, OraculoError> {
        self.runtime.block_on(async {
            let endpoint = self.ensure_endpoint().await?;
            if !self.bridge.is_running() {
                self.bridge.start(
                    endpoint.clone(),
                    self.data_dir.clone(),
                    device_label.clone(),
                );
            }
            let secret_key = load_or_create_secret_key(&self.key_file())?;
            self.bridge
                .issue_pairing_code(endpoint, secret_key, self.data_dir.clone(), device_label)
                .await
        })
    }

    /// Sondagem rápida não-bloqueante usada após Wake-on-LAN para detectar assim que o PC iniciar.
    pub fn probe_host_online(&self, endpoint_id: String, timeout_ms: u64) -> bool {
        let cap = Duration::from_millis(timeout_ms.clamp(1500, 12000));
        self.runtime.block_on(async {
            tokio::time::timeout(cap, async {
                let conn = self.get_or_connect(&endpoint_id).await?;
                let (ping, wol) = measure_ping_on_conn(&conn).await?;
                let _ = sync_saved_host_metadata(
                    &self.hosts_file(),
                    &endpoint_id,
                    &ping.host_name,
                    Some(ping.role),
                    wol.as_ref(),
                );
                Ok::<(), OraculoError>(())
            })
            .await
            .is_ok_and(|res| res.is_ok())
        })
    }

    pub fn ping(&self, endpoint_id: String) -> Result<ConnectionPing, OraculoError> {
        if let Some(bridge_id) = endpoint_id.strip_prefix("mcu:") {
            return self.runtime.block_on(self.ping_mcu_bridge_id(bridge_id));
        }
        self.runtime.block_on(async {
            let conn = self.get_or_connect(&endpoint_id).await?;
            let (ping, wol) = match measure_ping_on_conn(&conn).await {
                Ok(out) => out,
                Err(_) => {
                    if let Some((code @ (401 | 403), msg)) =
                        conn.close_reason().as_ref().map(classify_conn_error)
                    {
                        remove_stale_connection(&self.connections, &endpoint_id, &conn).await;
                        return Err(err_msg(format!("[{code}] {msg}")));
                    }
                    // Reconecta uma vez e mede somente o round-trip real da nova conexão.
                    remove_stale_connection(&self.connections, &endpoint_id, &conn).await;
                    let fresh = self.get_or_connect(&endpoint_id).await?;
                    match measure_ping_on_conn(&fresh).await {
                        Ok(out) => out,
                        Err(e) => {
                            if let Some((code, msg)) =
                                fresh.close_reason().as_ref().map(classify_conn_error)
                            {
                                remove_stale_connection(&self.connections, &endpoint_id, &fresh)
                                    .await;
                                return Err(err_msg(format!("[{code}] {msg}")));
                            }
                            return Err(e);
                        }
                    }
                }
            };
            let _ = sync_saved_host_metadata(
                &self.hosts_file(),
                &endpoint_id,
                &ping.host_name,
                Some(ping.role),
                wol.as_ref(),
            );
            Ok(ping)
        })
    }

    /// Verifica rapidamente se o computador está online e autorizado antes de abrir o painel de controle.
    pub fn connect_and_verify(
        &self,
        endpoint_id: String,
        timeout_ms: u64,
    ) -> Result<ConnectionPing, OraculoError> {
        if let Some(bridge_id) = endpoint_id.strip_prefix("mcu:") {
            let budget = Duration::from_millis(timeout_ms.clamp(1500, 15000));
            return self.runtime.block_on(async {
                tokio::time::timeout(budget, self.ping_mcu_bridge_id(bridge_id))
                    .await
                    .map_err(|_| err_msg("Tempo esgotado tentando alcançar a ponte MCU"))?
            });
        }
        let budget = Duration::from_millis(timeout_ms.clamp(1500, 15000));
        self.runtime.block_on(async {
            match tokio::time::timeout(budget, async {
                if let Some(cached) = self.connections.lock().await.get(&endpoint_id).cloned() {
                    if cached.close_reason().is_none() {
                        if let Ok(Ok((ping, wol))) =
                            tokio::time::timeout(Duration::from_millis(1400), measure_ping_on_conn(&cached))
                                .await
                        {
                            let _ = sync_saved_host_metadata(
                                &self.hosts_file(),
                                &endpoint_id,
                                &ping.host_name,
                                Some(ping.role),
                                wol.as_ref(),
                            );
                            return Ok(ping);
                        }
                    }
                    remove_stale_connection(&self.connections, &endpoint_id, &cached).await;
                }

                let fresh = self.get_or_connect(&endpoint_id).await?;
                match measure_ping_on_conn(&fresh).await {
                    Ok((ping, wol)) => {
                        let _ = sync_saved_host_metadata(
                            &self.hosts_file(),
                            &endpoint_id,
                            &ping.host_name,
                            Some(ping.role),
                            wol.as_ref(),
                        );
                        Ok(ping)
                    }
                    Err(e) => {
                        if let Some((code, msg)) =
                            fresh.close_reason().as_ref().map(classify_conn_error)
                        {
                            remove_stale_connection(&self.connections, &endpoint_id, &fresh).await;
                            return Err(err_msg(format!("[{code}] {msg}")));
                        }
                        remove_stale_connection(&self.connections, &endpoint_id, &fresh).await;
                        Err(e)
                    }
                }
            })
            .await
            {
                Ok(inner) => inner,
                Err(_) => Err(err_msg("Computador indisponível ou Oráculo fechado no PC")),
            }
        })
    }

    /// Encerra proativamente a conexão QUIC em cache ao sair do computador no celular.
    pub fn disconnect_host(&self, endpoint_id: String) {
        self.runtime.block_on(async {
            if let Some(conn) = self.connections.lock().await.remove(&endpoint_id) {
                conn.close(VarInt::from_u32(0), b"client disconnect");
            }
        });
    }

    /// Mantém uma assinatura em tempo real das permissões e da presença ativa do host.
    pub fn watch_host_state(
        &self,
        endpoint_id: String,
        observer: Arc<dyn HostStateObserver>,
    ) -> Result<Arc<MobileHostStateWatcher>, OraculoError> {
        self.runtime.block_on(async {
            let (conn, mut send, mut recv) = self.open_bi_stream_with_conn(&endpoint_id).await?;
            write_stream_kind(&mut send, StreamKind::Control).await?;
            write_frame(&mut send, &ControlRequest::WatchState).await?;
            let _ = send.finish();

            let (stop_tx, mut stop_rx) = watch::channel(false);
            let connections = Arc::clone(&self.connections);
            let hosts_file = self.hosts_file();
            let ep_id = endpoint_id.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        biased;
                        _ = stop_rx.changed() => break,
                        conn_err = conn.closed() => {
                            remove_stale_connection(&connections, &ep_id, &conn).await;
                            let (code, msg) = classify_conn_error(&conn_err);
                            observer.on_disconnected(code, localize_mobile(msg));
                            break;
                        }
                        frame_res = read_frame::<_, ControlResponse>(&mut recv) => {
                            match frame_res {
                                Ok(Some(ControlResponse::HostState { host_name, wol, permissions })) => {
                                    let _ = sync_saved_host_metadata(
                                        &hosts_file,
                                        &ep_id,
                                        &host_name,
                                        Some(DeviceRole::Computer),
                                        wol.as_ref(),
                                    );
                                    observer.on_state(host_name, permissions.into());
                                }
                                Ok(Some(_)) => {}
                                Ok(None) | Err(_) => {
                                    if *stop_rx.borrow() {
                                        break;
                                    }
                                    // Aguarda brevemente caso `conn.closed()` traga o código exato (403/410/503).
                                    tokio::time::sleep(Duration::from_millis(60)).await;
                                    if let Some(reason) = conn.close_reason() {
                                        remove_stale_connection(&connections, &ep_id, &conn).await;
                                        let (code, msg) = classify_conn_error(&reason);
                                        observer.on_disconnected(code, localize_mobile(msg));
                                    } else {
                                        observer.on_disconnected(
                                            0,
                                            localize_mobile("Canal de estado encerrado pelo computador."),
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            Ok(Arc::new(MobileHostStateWatcher { stop_tx }))
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

    pub fn list_terminal_sessions(
        &self,
        endpoint_id: String,
    ) -> Result<Vec<MobileTerminalSessionInfo>, OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(&endpoint_id, &ControlRequest::ListTerminalSessions)
                .await?
            {
                ControlResponse::TerminalSessions(list) => {
                    Ok(list.into_iter().map(Into::into).collect())
                }
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!(
                    "Resposta inesperada ao listar sessões: {other:?}"
                ))),
            }
        })
    }

    pub fn kill_terminal_session(
        &self,
        endpoint_id: String,
        session_id: String,
    ) -> Result<(), OraculoError> {
        self.runtime.block_on(async {
            match self
                .control_rpc(
                    &endpoint_id,
                    &ControlRequest::KillTerminalSession { id: session_id },
                )
                .await?
            {
                ControlResponse::TerminalSessionClosed { .. } => Ok(()),
                ControlResponse::Error(msg) => Err(err_msg(msg)),
                other => Err(err_msg(format!(
                    "Resposta inesperada ao encerrar sessão: {other:?}"
                ))),
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
        self.open_terminal_session(endpoint_id, cols, rows, true, None, None, observer)
    }

    pub fn open_terminal_session(
        &self,
        endpoint_id: String,
        cols: u16,
        rows: u16,
        persistent: bool,
        session_id: Option<String>,
        label: Option<String>,
        observer: Arc<dyn TerminalObserver>,
    ) -> Result<Arc<MobileTerminalSession>, OraculoError> {
        let (cols, rows, nz_cols, nz_rows) = clamp_terminal_size(cols, rows);

        self.runtime.block_on(async {
            let (mut send, mut recv) = self.open_bi_stream(&endpoint_id).await?;

            write_stream_kind(&mut send, StreamKind::Terminal).await?;
            write_frame(
                &mut send,
                &TerminalInit {
                    cols,
                    rows,
                    persistent,
                    session_id,
                    label,
                },
            )
            .await?;

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
                let mut saw_first_frame = false;
                let flush_sleep = tokio::time::sleep(SNAPSHOT_INTERVAL);
                tokio::pin!(flush_sleep);

                loop {
                    tokio::select! {
                        frame = read_frame::<_, TerminalHostMsg>(&mut recv) => {
                            match frame {
                                Ok(Some(TerminalHostMsg::Ready {
                                    session_id,
                                    label,
                                    persistent,
                                    reconnected,
                                })) => {
                                    saw_first_frame = true;
                                    observer_reader.on_session_ready(
                                        session_id,
                                        label,
                                        persistent,
                                        reconnected,
                                    );
                                }
                                Ok(Some(TerminalHostMsg::Output(bytes))) => {
                                    saw_first_frame = true;
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
                                    observer_reader.on_error(localize_mobile(msg));
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
                                    let detail = if !saw_first_frame {
                                        format!(
                                            "Protocolo de terminal incompatível ou corrompido (reinicie/atualize o Oráculo no PC): {err:#}"
                                        )
                                    } else {
                                        format!("Erro no terminal: {err:#}")
                                    };
                                    observer_reader.on_error(localize_mobile(detail));
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
            // Prioridade máxima no QUIC para mouse/teclado não disputarem fila com vídeo ou arquivos.
            let _ = send.set_priority(100);

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

                    // Agrupa movimentos/rolagens numa janela curta (~12ms / ~83Hz) para evitar
                    // saturar links fracos com 120-240 pacotes/s, sem atrasar cliques ou teclas.
                    const INPUT_COALESCE_WINDOW: Duration = Duration::from_millis(12);
                    let mut coalesce_closed = false;
                    if matches!(event, InputEvent::Move { .. } | InputEvent::Scroll { .. }) {
                        let deadline = tokio::time::Instant::now() + INPUT_COALESCE_WINDOW;
                        loop {
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
                            if pending.is_some() {
                                break;
                            }
                            let sleep = tokio::time::sleep_until(deadline);
                            tokio::select! {
                                biased;
                                reason = &mut close_rx => {
                                    close_reason = Some(
                                        reason.unwrap_or_else(|_| ("Conexão de entrada encerrada".to_string(), false))
                                    );
                                    coalesce_closed = true;
                                    break;
                                }
                                _ = sleep => break,
                                maybe = rx.recv() => match maybe {
                                    Some(queued) => match (&mut event, queued) {
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
                                    },
                                    None => {
                                        coalesce_closed = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if coalesce_closed {
                        break;
                    }

                    if matches!(
                        event,
                        InputEvent::Move { dx, dy } | InputEvent::Scroll { dx, dy }
                            if !dx.is_finite() || !dy.is_finite() || (dx == 0.0 && dy == 0.0)
                    ) {
                        continue;
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
                    observer.on_session_closed(localize_mobile(reason));
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
                            dispatch_observer.on_stream_notice(localize_mobile(msg));
                        }
                        ClientQueuePop::Event(ClientScreenEvent::Error(msg)) => {
                            dispatch_alive.store(false, Ordering::Release);
                            dispatch_observer.on_stream_error(localize_mobile(msg));
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
                                dispatch_observer.on_stream_closed(localize_mobile(reason_msg));
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

    pub fn start_rdp_tunnel(
        &self,
        endpoint_id: String,
    ) -> Result<Arc<MobileTunnelSession>, OraculoError> {
        self.runtime.block_on(async {
            let (conn, mut probe_send, mut probe_recv) =
                self.open_bi_stream_with_conn(&endpoint_id).await?;

            write_stream_kind(&mut probe_send, StreamKind::Tunnel).await?;
            write_frame(
                &mut probe_send,
                &TunnelOpenReq {
                    target: TunnelTarget::LocalRdp { port: 0 },
                    probe_only: true,
                },
            )
            .await?;
            let _ = probe_send.finish();

            let probe_resp = tokio::time::timeout(
                Duration::from_secs(8),
                read_frame::<_, TunnelOpenResp>(&mut probe_recv),
            )
            .await
            .map_err(|_| err_msg("Tempo esgotado verificando acesso RDP no computador"))?
            .map_err(OraculoError::from)?
            .ok_or_else(|| err_msg("Computador encerrou canal RDP antes da resposta"))?;

            if !probe_resp.ok {
                return Err(err_msg(probe_resp.message));
            }
            let remote_rdp_port = probe_resp.resolved_port;

            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                    .await
                    .map_err(|e| err_msg(format!("Falha ao abrir porta local de túnel: {e}")))?;
            let local_port = listener
                .local_addr()
                .map_err(|e| err_msg(format!("Falha ao consultar porta local: {e}")))?
                .port();

            let (stop_tx, mut stop_rx) = watch::channel(false);
            let stop_tx_loop = stop_tx.clone();
            let alive = Arc::new(AtomicBool::new(true));
            let alive_task = Arc::clone(&alive);

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        biased;
                        _ = stop_rx.changed() => {
                            if *stop_rx.borrow() {
                                break;
                            }
                        }
                        _ = conn.closed() => {
                            break;
                        }
                        accepted = listener.accept() => {
                            let Ok((tcp, _)) = accepted else {
                                break;
                            };
                            let _ = tcp.set_nodelay(true);
                            let c = conn.clone();
                            let mut conn_stop = stop_tx_loop.subscribe();
                            tokio::spawn(async move {
                                let Ok(Ok((mut send, mut recv))) =
                                    tokio::time::timeout(Duration::from_secs(6), c.open_bi()).await
                                else {
                                    return;
                                };
                                if write_stream_kind(&mut send, StreamKind::Tunnel).await.is_err() {
                                    return;
                                }
                                if write_frame(
                                    &mut send,
                                    &TunnelOpenReq {
                                        target: TunnelTarget::LocalRdp { port: 0 },
                                        probe_only: false,
                                    },
                                )
                                .await
                                .is_err()
                                {
                                    return;
                                }
                                let Ok(Ok(Some(resp))) = tokio::time::timeout(
                                    Duration::from_secs(6),
                                    read_frame::<_, TunnelOpenResp>(&mut recv),
                                )
                                .await
                                else {
                                    return;
                                };
                                if !resp.ok {
                                    return;
                                }
                                let (mut tcp_read, mut tcp_write) = tcp.into_split();
                                let relay = async move {
                                    let c2r = async {
                                        let _ = tokio::io::copy(&mut tcp_read, &mut send).await;
                                        let _ = send.finish();
                                    };
                                    let r2c = async {
                                        let _ = tokio::io::copy(&mut recv, &mut tcp_write).await;
                                        let _ = tcp_write.shutdown().await;
                                    };
                                    tokio::join!(c2r, r2c);
                                };
                                tokio::select! {
                                    biased;
                                    _ = conn_stop.changed() => {}
                                    _ = c.closed() => {}
                                    _ = relay => {}
                                }
                            });
                        }
                    }
                }
                alive_task.store(false, Ordering::Release);
            });

            Ok(Arc::new(MobileTunnelSession {
                local_port,
                remote_rdp_port,
                stop_tx,
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

fn classify_conn_error(err: &ConnectionError) -> (u32, String) {
    match err {
        ConnectionError::ApplicationClosed(close) => {
            let code = close.error_code.into_inner() as u32;
            match code {
                401 | 403 => (
                    code,
                    "Acesso removido neste computador. Faça o pareamento novamente.".to_string(),
                ),
                410 => (410, "Sessão desconectada no computador.".to_string()),
                503 => (503, "O Oráculo foi encerrado no computador.".to_string()),
                other => {
                    let raw = String::from_utf8_lossy(&close.reason).trim().to_string();
                    let msg = if raw.is_empty() {
                        "Conexão encerrada pelo computador.".to_string()
                    } else {
                        raw
                    };
                    (other, msg)
                }
            }
        }
        other => (0, format!("Conexão perdida com o computador: {other}")),
    }
}

async fn measure_ping_on_conn(
    conn: &Connection,
) -> Result<(ConnectionPing, Option<WolInfo>), OraculoError> {
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
            ControlResponse::Pong {
                host_name,
                wol,
                role,
            } => Ok((
                ConnectionPing {
                    host_name,
                    rtt_ms: started.elapsed().as_millis() as u64,
                    role: role.into(),
                },
                wol,
            )),
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

pub(crate) fn sanitize_host_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "oraculo-host".to_string()
    } else {
        cleaned
    }
}

fn sync_saved_host_metadata(
    hosts_path: &Path,
    endpoint_id: &str,
    host_name: &str,
    role: Option<DeviceRole>,
    wol: Option<&WolInfo>,
) -> Result<(), OraculoError> {
    let mut hosts = load_saved_hosts(hosts_path)?;
    let Some(target) = hosts.iter_mut().find(|h| h.endpoint_id == endpoint_id) else {
        return Ok(());
    };

    let snapshot = target.clone();
    if !host_name.trim().is_empty() {
        target.host_name = sanitize_host_name(host_name);
    }
    if let Some(next_role) = role {
        target.role = next_role;
    }
    if let Some(info) = wol.cloned().and_then(sanitize_wol_info) {
        let mut iter = info.macs.into_iter();
        target.wol_mac = iter.next();
        target.wol_extra_macs = iter.collect();
        target.wol_broadcast = info.broadcast;
    }

    if *target != snapshot {
        save_hosts(hosts_path, &hosts)?;
    }
    Ok(())
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
    fn persist_paired_host(
        &self,
        endpoint_id: String,
        raw_host_name: &str,
        relay_url: Option<String>,
        role: DeviceRole,
        raw_wol: Option<WolInfo>,
    ) -> Result<SavedHostInfo, OraculoError> {
        let mut hosts = load_saved_hosts(&self.hosts_file())?;
        let prev = hosts
            .iter()
            .find(|h| h.endpoint_id == endpoint_id)
            .cloned();
        hosts.retain(|h| h.endpoint_id != endpoint_id);

        let (wol_mac, wol_extra_macs, wol_broadcast) = match raw_wol.and_then(sanitize_wol_info) {
            Some(w) => {
                let mut iter = w.macs.into_iter();
                let primary = iter.next();
                let extras: Vec<String> = iter.collect();
                (primary, extras, w.broadcast)
            }
            None => match &prev {
                Some(old) => (
                    old.wol_mac.clone(),
                    old.wol_extra_macs.clone(),
                    old.wol_broadcast.clone(),
                ),
                None => (None, Vec::new(), None),
            },
        };

        let effective_relay = relay_url.or_else(|| prev.as_ref().and_then(|p| p.relay_url.clone()));

        let info = SavedHostInfo {
            endpoint_id,
            host_name: sanitize_host_name(raw_host_name),
            relay_url: effective_relay,
            role,
            wol_mac,
            wol_extra_macs,
            wol_broadcast,
            wol_target_override: prev.as_ref().and_then(|p| p.wol_target_override.clone()),
            wol_port: prev.as_ref().map(|p| p.wol_port).unwrap_or(DEFAULT_WOL_PORT),
        };

        hosts.push(info.clone());
        save_hosts(&self.hosts_file(), &hosts)?;
        Ok(info)
    }

    fn key_file(&self) -> PathBuf {
        self.data_dir.join("client.key")
    }

    fn hosts_file(&self) -> PathBuf {
        self.data_dir.join("saved_hosts.json")
    }

    fn mcu_bridges_file(&self) -> PathBuf {
        self.data_dir.join("saved_mcu_bridges.json")
    }

    fn pair_mcu_bridge_uri(&self, uri: &str) -> Result<SavedHostInfo, OraculoError> {
        self.runtime.block_on(async {
            let mut cfg = decode_mcu_pairing_uri(uri)?;
            let ping = ping_mcu_bridge(&cfg).await.map_err(|e| {
                err_msg(format!(
                    "A ponte '{}' não respondeu em {}:{} ({e}). Verifique Wi-Fi e energia da ponte.",
                    cfg.name, cfg.broker_host, cfg.broker_port
                ))
            })?;
            if !ping.bridge_name.trim().is_empty() {
                cfg.name = sanitize_mcu_name(&ping.bridge_name);
            }

            let entry = SavedMcuBridge::from_config(&cfg);
            let mut list = load_saved_mcu_bridges(&self.mcu_bridges_file())?;
            list.retain(|item| item.bridge_id != entry.bridge_id);
            list.push(entry.clone());
            save_mcu_bridges(&self.mcu_bridges_file(), &list)?;
            Ok(entry.to_saved_host_info())
        })
    }

    async fn ping_mcu_bridge_id(&self, bridge_id: &str) -> Result<ConnectionPing, OraculoError> {
        let mut list = load_saved_mcu_bridges(&self.mcu_bridges_file())?;
        let idx = list
            .iter()
            .position(|b| b.bridge_id == bridge_id)
            .ok_or_else(|| err_msg("Ponte MCU salva não encontrada"))?;
        let cfg = list[idx].to_config()?;
        let res = ping_mcu_bridge(&cfg).await?;
        let synced_name = sanitize_mcu_name(&res.bridge_name);
        if list[idx].name != synced_name {
            list[idx].name = synced_name.clone();
            let _ = save_mcu_bridges(&self.mcu_bridges_file(), &list);
        }
        Ok(ConnectionPing {
            host_name: synced_name,
            rtt_ms: res.rtt_ms,
            role: DeviceRole::WolBridge,
        })
    }

    async fn ensure_endpoint(&self) -> Result<Endpoint, OraculoError> {
        get_or_init_endpoint(&self.endpoint, &self.key_file()).await
    }

    async fn host_connect_lock(&self, endpoint_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.connect_locks.lock().await;
        Arc::clone(
            locks
                .entry(endpoint_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    async fn recycle_endpoint(&self) -> Result<Endpoint, OraculoError> {
        if let Ok(mut guard) = self.last_recycle_at.lock() {
            *guard = Some(Instant::now());
        }
        self.consecutive_connect_failures.store(0, Ordering::Release);

        if self.bridge.is_running() {
            let ep = self.ensure_endpoint().await?;
            ep.network_change().await;
            let _ = tokio::time::timeout(Duration::from_millis(1200), ep.online()).await;
            return Ok(ep);
        }

        let old_ep = { self.endpoint.lock().await.take() };
        {
            self.connections.lock().await.clear();
        }
        if let Some(ep) = old_ep {
            tokio::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(2), ep.close()).await;
            });
        }
        get_or_init_endpoint(&self.endpoint, &self.key_file()).await
    }

    async fn get_or_connect(&self, endpoint_id: &str) -> Result<Connection, OraculoError> {
        if endpoint_id.starts_with("mcu:") {
            return Err(err_msg(
                "Esta ponte MCU atua apenas para Wake-on-LAN via MQTT TLS",
            ));
        }
        {
            let mut conns = self.connections.lock().await;
            if let Some(conn) = conns.get(endpoint_id) {
                if conn.close_reason().is_none() {
                    return Ok(conn.clone());
                }
                conns.remove(endpoint_id);
            }
        }

        // Serializa handshakes simultâneos para o mesmo host (watchHostState, ping, input, terminal)
        // evitando corrida de conexões paralelas que sobrescrevem o cache e derrubam streams ativos.
        let host_lock = self.host_connect_lock(endpoint_id).await;
        let _connect_guard = host_lock.lock().await;

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
        let addr_with_hint = build_addr(host_id, relay_url.as_deref())?;

        let dirty = self.network_dirty.load(Ordering::Acquire);
        let first_timeout = if dirty {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(8)
        };

        let endpoint = self.ensure_endpoint().await?;
        let first_res =
            tokio::time::timeout(first_timeout, endpoint.connect(addr_with_hint, ALPN)).await;

        let conn = match first_res {
            Ok(Ok(conn)) => conn,
            first_err => {
                let is_auth_denied = matches!(
                    &first_err,
                    Ok(Err(err)) if {
                        let msg = err.to_string();
                        msg.contains("401") || msg.contains("403")
                    }
                );
                if is_auth_denied {
                    let msg = match first_err {
                        Ok(Err(e)) => format!("Não foi possível conectar ao host: {e}"),
                        _ => "Acesso recusado pelo host".to_string(),
                    };
                    return Err(err_msg(msg));
                }

                let dirty_now = dirty || self.network_dirty.load(Ordering::Acquire);
                let failures = self
                    .consecutive_connect_failures
                    .fetch_add(1, Ordering::AcqRel)
                    + 1;
                let recycle_cooldown_ok = self
                    .last_recycle_at
                    .lock()
                    .ok()
                    .and_then(|g| *g)
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(25));
                let should_recycle = dirty_now || (failures >= 2 && recycle_cooldown_ok);
                let should_retry = should_recycle || relay_url.is_some();

                if !should_retry {
                    return match first_err {
                        Err(_) => Err(err_msg("Tempo esgotado tentando conectar ao host")),
                        Ok(Err(e)) => Err(err_msg(format!("Não foi possível conectar ao host: {e}"))),
                        Ok(Ok(_)) => unreachable!(),
                    };
                }

                let retry_ep = if should_recycle {
                    self.recycle_endpoint().await?
                } else {
                    endpoint
                };

                // Na retomada, tenta descoberta limpa por EndpointId (Pkarr/DNS) sem depender de relay antigo em cache.
                let fresh_addr = EndpointAddr::new(host_id);
                tokio::time::timeout(Duration::from_secs(8), retry_ep.connect(fresh_addr, ALPN))
                    .await
                    .map_err(|_| err_msg("Tempo esgotado tentando conectar ao host"))?
                    .map_err(|e| err_msg(format!("Não foi possível conectar ao host: {e}")))?
            }
        };

        self.network_dirty.store(false, Ordering::Release);
        self.consecutive_connect_failures.store(0, Ordering::Release);
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
        let max_attempts = if is_idempotent_control_req(req) { 2 } else { 1 };

        for attempt in 0..max_attempts {
            let result = tokio::time::timeout(timeout_dur, async {
                let (conn, mut send, mut recv) = self.open_bi_stream_with_conn(endpoint_id).await?;
                let io_res: Result<ControlResponse, OraculoError> = async {
                    write_stream_kind(&mut send, StreamKind::Control).await?;
                    write_frame(&mut send, req).await?;
                    send.finish()
                        .map_err(|e| err_msg(format!("Erro ao fechar envio: {e}")))?;

                    read_frame(&mut recv)
                        .await?
                        .ok_or_else(|| err_msg("Host fechou stream de controle sem resposta"))
                }
                .await;
                Ok::<(Connection, Result<ControlResponse, OraculoError>), OraculoError>((
                    conn, io_res,
                ))
            })
            .await;

            match result {
                Ok(Ok((_, Ok(resp)))) => return Ok(resp),
                Ok(Ok((conn, Err(err)))) => {
                    if let Some((code @ (401 | 403), msg)) =
                        conn.close_reason().as_ref().map(classify_conn_error)
                    {
                        remove_stale_connection(&self.connections, endpoint_id, &conn).await;
                        return Err(err_msg(format!("[{code}] {msg}")));
                    }
                    remove_stale_connection(&self.connections, endpoint_id, &conn).await;
                    if attempt + 1 >= max_attempts {
                        return Err(err);
                    }
                }
                Ok(Err(err)) => {
                    self.connections.lock().await.remove(endpoint_id);
                    if attempt + 1 >= max_attempts {
                        return Err(err);
                    }
                }
                Err(_) => {
                    self.connections.lock().await.remove(endpoint_id);
                    if attempt + 1 >= max_attempts {
                        return Err(err_msg("Tempo esgotado aguardando resposta do host"));
                    }
                }
            }
        }
        Err(err_msg("Tempo esgotado aguardando resposta do host"))
    }
}

fn is_idempotent_control_req(req: &ControlRequest) -> bool {
    matches!(
        req,
        ControlRequest::Ping
            | ControlRequest::ListCommands
            | ControlRequest::ListFiles { .. }
            | ControlRequest::ListTerminalSessions
    )
}

async fn get_or_init_endpoint(
    endpoint_slot: &Arc<Mutex<Option<Endpoint>>>,
    key_file: &Path,
) -> Result<Endpoint, OraculoError> {
    let ep = {
        let mut guard = endpoint_slot.lock().await;
        if let Some(ep) = guard.as_ref() {
            return Ok(ep.clone());
        }

        let secret_key = load_or_create_secret_key(key_file)?;
        let mut transport_builder =
            QuicTransportConfig::builder().keep_alive_interval(Duration::from_secs(4));
        if let Ok(idle) = IdleTimeout::try_from(Duration::from_secs(30)) {
            transport_builder = transport_builder.max_idle_timeout(Some(idle));
        }

        // Usa raízes Mozilla embutidas (webpki-roots) para não exigir hooks JNI extras no Android.
        let ep = Endpoint::builder(presets::N0)
            .transport_config(transport_builder.build())
            .ca_tls_config(CaTlsConfig::embedded())
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| err_msg(format!("Falha ao iniciar Iroh Endpoint: {e}")))?;

        // Guarda o endpoint no cache antes de aguardar relay online para não perder o socket em timeouts curtos.
        *guard = Some(ep.clone());
        ep
    };

    let _ = tokio::time::timeout(Duration::from_millis(1800), ep.online()).await;
    Ok(ep)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SavedMcuBridge {
    bridge_id: String,
    secret_b64: String,
    broker_host: String,
    broker_port: u16,
    name: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

impl SavedMcuBridge {
    fn from_config(cfg: &McuBridgeConfig) -> Self {
        Self {
            bridge_id: cfg.bridge_id.clone(),
            secret_b64: encode_mcu_secret_b64url(&cfg.secret),
            broker_host: cfg.broker_host.clone(),
            broker_port: cfg.broker_port,
            name: sanitize_mcu_name(&cfg.name),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
        }
    }

    fn to_config(&self) -> Result<McuBridgeConfig, OraculoError> {
        let secret = decode_mcu_secret(&self.secret_b64)?;
        Ok(McuBridgeConfig {
            bridge_id: self.bridge_id.clone(),
            secret,
            broker_host: self.broker_host.clone(),
            broker_port: self.broker_port,
            name: sanitize_mcu_name(&self.name),
            username: self.username.clone(),
            password: self.password.clone(),
        })
    }

    fn to_saved_host_info(&self) -> SavedHostInfo {
        SavedHostInfo {
            endpoint_id: format!("mcu:{}", self.bridge_id),
            host_name: sanitize_mcu_name(&self.name),
            relay_url: Some(format!("{}:{}", self.broker_host, self.broker_port)),
            role: DeviceRole::WolBridge,
            wol_mac: None,
            wol_extra_macs: Vec::new(),
            wol_broadcast: None,
            wol_target_override: None,
            wol_port: DEFAULT_WOL_PORT,
        }
    }
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

fn load_saved_mcu_bridges(path: &Path) -> Result<Vec<SavedMcuBridge>, OraculoError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| err_msg(format!("Falha ao ler saved_mcu_bridges.json: {e}")))?;
    serde_json::from_str(&raw)
        .map_err(|e| err_msg(format!("JSON inválido em saved_mcu_bridges.json: {e}")))
}

fn save_mcu_bridges(path: &Path, bridges: &[SavedMcuBridge]) -> Result<(), OraculoError> {
    let json = serde_json::to_string_pretty(bridges)
        .map_err(|e| err_msg(format!("Falha ao serializar pontes MCU: {e}")))?;
    write_private_file(path, json.as_bytes(), "saved_mcu_bridges.json")
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
            role: DeviceRole::Computer,
            wol_mac: None,
            wol_extra_macs: Vec::new(),
            wol_broadcast: None,
            wol_target_override: None,
            wol_port: DEFAULT_WOL_PORT,
        }];
        save_hosts(&client.hosts_file(), &sample).unwrap();
        assert_eq!(client.list_saved_hosts().unwrap().len(), 1);

        // Garante retrocompatibilidade estrita com saved_hosts.json de versões anteriores (3 campos).
        let legacy_json = r#"[{"endpoint_id":"11111111111111111111111111111111","host_name":"legado","relay_url":null}]"#;
        write_private_file(&client.hosts_file(), legacy_json.as_bytes(), "saved_hosts.json").unwrap();
        let loaded_legacy = client.list_saved_hosts().unwrap();
        assert_eq!(loaded_legacy.len(), 1);
        assert_eq!(loaded_legacy[0].role, DeviceRole::Computer);
        assert_eq!(loaded_legacy[0].wol_port, DEFAULT_WOL_PORT);
        assert!(loaded_legacy[0].wol_mac.is_none());

        let updated_wol = client
            .update_host_wol(
                "11111111111111111111111111111111".into(),
                Some("aa-bb-cc-dd-ee-ff".into()),
                Some("meu-pc.ddns.net".into()),
                40000,
            )
            .unwrap();
        assert_eq!(updated_wol.wol_mac.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        assert_eq!(updated_wol.wol_target_override.as_deref(), Some("meu-pc.ddns.net"));
        assert_eq!(updated_wol.wol_port, 40000);

        // Sync via Pong preserva wol_target_override e wol_port configurados pelo usuário.
        sync_saved_host_metadata(
            &client.hosts_file(),
            "11111111111111111111111111111111",
            "novo-nome",
            Some(DeviceRole::WolBridge),
            Some(&WolInfo {
                macs: vec!["AA:BB:CC:00:11:22".into(), "AA:BB:CC:00:11:33".into()],
                broadcast: Some("192.168.18.255".into()),
            }),
        )
        .unwrap();
        let after_sync = client.list_saved_hosts().unwrap();
        assert_eq!(after_sync[0].host_name, "novo-nome");
        assert_eq!(after_sync[0].role, DeviceRole::WolBridge);
        assert_eq!(after_sync[0].wol_mac.as_deref(), Some("AA:BB:CC:00:11:22"));
        assert_eq!(after_sync[0].wol_extra_macs, vec!["AA:BB:CC:00:11:33".to_string()]);
        assert_eq!(after_sync[0].wol_broadcast.as_deref(), Some("192.168.18.255"));
        assert_eq!(after_sync[0].wol_target_override.as_deref(), Some("meu-pc.ddns.net"));
        assert_eq!(after_sync[0].wol_port, 40000);

        client
            .remove_saved_host("11111111111111111111111111111111".into())
            .unwrap();
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
    fn saves_lists_and_removes_mcu_bridges_privately_without_leaking_secrets() {
        let dir = std::env::temp_dir().join("oraculo_mobile_mcu_test");
        let _ = fs::remove_dir_all(&dir);
        let client = OraculoMobileClient::new(dir.to_string_lossy().into_owned()).unwrap();

        let cfg = McuBridgeConfig {
            bridge_id: "0123456789abcdef".to_string(),
            secret: [0xABu8; 32],
            broker_host: "broker.emqx.io".to_string(),
            broker_port: 8883,
            name: "Cardputer Sala".to_string(),
            username: Some("demo".to_string()),
            password: Some("supersecret".to_string()),
        };
        save_mcu_bridges(&client.mcu_bridges_file(), &[SavedMcuBridge::from_config(&cfg)])
            .unwrap();

        let listed = client.list_saved_hosts().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].endpoint_id, "mcu:0123456789abcdef");
        assert_eq!(listed[0].host_name, "Cardputer Sala");
        assert_eq!(listed[0].role, DeviceRole::WolBridge);
        assert_eq!(listed[0].relay_url.as_deref(), Some("broker.emqx.io:8883"));
        let dbg = format!("{:?}", listed[0]);
        assert!(!dbg.contains("supersecret"));
        assert!(!dbg.contains("abab"));

        assert!(client.wake_host("mcu:0123456789abcdef".to_string()).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&client.mcu_bridges_file())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        client
            .remove_saved_host("mcu:0123456789abcdef".to_string())
            .unwrap();
        assert!(client.list_saved_hosts().unwrap().is_empty());
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

    #[test]
    fn idempotent_control_filter_and_per_host_lock_dedup() {
        assert!(is_idempotent_control_req(&ControlRequest::Ping));
        assert!(is_idempotent_control_req(&ControlRequest::ListCommands));
        assert!(is_idempotent_control_req(&ControlRequest::ListFiles {
            rel_path: "".into(),
            show_hidden: false,
        }));
        assert!(is_idempotent_control_req(
            &ControlRequest::ListTerminalSessions
        ));
        assert!(!is_idempotent_control_req(&ControlRequest::RunCommand {
            id: "cmd1".into(),
        }));
        assert!(!is_idempotent_control_req(&ControlRequest::DeleteCommand {
            id: "cmd1".into(),
        }));
        assert!(!is_idempotent_control_req(
            &ControlRequest::KillTerminalSession { id: "s1".into() }
        ));

        let dir = std::env::temp_dir().join("oraculo_mobile_singleflight_test");
        let _ = fs::remove_dir_all(&dir);
        let client = OraculoMobileClient::new(dir.to_string_lossy().into_owned()).unwrap();
        let (lock_a1, lock_a2, lock_b) = client.runtime.block_on(async {
            (
                client.host_connect_lock("host-a").await,
                client.host_connect_lock("host-a").await,
                client.host_connect_lock("host-b").await,
            )
        });
        assert!(Arc::ptr_eq(&lock_a1, &lock_a2));
        assert!(!Arc::ptr_eq(&lock_a1, &lock_b));
        let _ = fs::remove_dir_all(&dir);
    }
}
