use crate::config::HostSettings;
use anyhow::{Context, Result, bail};
use iroh::endpoint::{RecvStream, SendStream};
use oraculo_proto::{
    TerminalClientMsg, TerminalHostMsg, TerminalInit, TerminalSessionSummary, read_frame,
    write_frame,
};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, broadcast, mpsc, watch};

const MAX_RUNNING_PERSISTENT_SESSIONS: usize = 4;
const MAX_STORED_PERSISTENT_SESSIONS: usize = 6;
const RING_BUFFER_BYTES: usize = 256 * 1024;
const ENDED_SESSION_TTL_SECS: u64 = 10 * 60;

struct SessionBufferState {
    total_bytes: u64,
    ring: VecDeque<u8>,
    ended: Option<i32>,
}

struct PersistentSession {
    id: String,
    label: String,
    owner_peer_id: String,
    created_unix: u64,
    last_activity_unix: AtomicU64,
    ended_unix: AtomicU64,
    attached_clients: AtomicU32,
    cols: AtomicU16,
    rows: AtomicU16,
    master: StdMutex<Box<dyn MasterPty + Send>>,
    in_tx: StdMutex<Option<mpsc::Sender<Vec<u8>>>>,
    killer: StdMutex<Option<Box<dyn ChildKiller + Send + Sync>>>,
    output_tx: broadcast::Sender<(u64, Vec<u8>)>,
    buffer: StdMutex<SessionBufferState>,
    stop_tx: watch::Sender<bool>,
    stop_reason: StdMutex<Option<String>>,
}

impl PersistentSession {
    fn is_running(&self) -> bool {
        self.buffer
            .lock()
            .map(|b| b.ended.is_none())
            .unwrap_or(false)
    }

    fn summary(&self) -> TerminalSessionSummary {
        let exit_code = self.buffer.lock().ok().and_then(|b| b.ended);
        TerminalSessionSummary {
            id: self.id.clone(),
            label: self.label.clone(),
            running: exit_code.is_none(),
            exit_code,
            attached_clients: self.attached_clients.load(Ordering::Acquire),
            created_unix: self.created_unix,
            last_activity_unix: self.last_activity_unix.load(Ordering::Acquire),
            cols: self.cols.load(Ordering::Acquire),
            rows: self.rows.load(Ordering::Acquire),
        }
    }

    async fn send_input(&self, bytes: Vec<u8>) -> bool {
        let sender = self
            .in_tx
            .lock()
            .ok()
            .and_then(|g| g.as_ref().cloned());
        let Some(tx) = sender else {
            return false;
        };
        self.last_activity_unix.store(now_unix(), Ordering::Release);
        tx.send(bytes).await.is_ok()
    }

    fn resize(&self, cols: u16, rows: u16) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let prev_cols = self.cols.swap(cols, Ordering::AcqRel);
        let prev_rows = self.rows.swap(rows, Ordering::AcqRel);
        if prev_cols == cols && prev_rows == rows {
            return;
        }
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    fn subscribe_and_snapshot(&self) -> (broadcast::Receiver<(u64, Vec<u8>)>, Vec<u8>, u64, Option<i32>) {
        // Inscreve no canal antes de ler o ringbuffer sob mutex para nunca perder nem duplicar chunks.
        let rx = self.output_tx.subscribe();
        let Ok(mut state) = self.buffer.lock() else {
            return (rx, Vec::new(), 0, Some(-1));
        };
        let snapshot = state.ring.make_contiguous().to_vec();
        (rx, snapshot, state.total_bytes, state.ended)
    }

    async fn kill_with_reason(&self, reason: Option<String>) {
        if let Some(msg) = reason {
            if let Ok(mut guard) = self.stop_reason.lock() {
                if guard.is_none() {
                    *guard = Some(msg);
                }
            }
        }
        let _ = self.stop_tx.send(true);
        if let Ok(mut tx_guard) = self.in_tx.lock() {
            *tx_guard = None;
        }
        let killer_opt = self.killer.lock().ok().and_then(|mut g| g.take());
        if let Some(mut killer) = killer_opt {
            let _ = tokio::task::spawn_blocking(move || killer.kill()).await;
        }
        if let Ok(mut buf) = self.buffer.lock() {
            if buf.ended.is_none() {
                buf.ended = Some(-1);
                self.ended_unix.store(now_unix(), Ordering::Release);
            }
        }
    }
}

struct AttachedClientGuard(Arc<PersistentSession>);

impl Drop for AttachedClientGuard {
    fn drop(&mut self) {
        self.0.attached_clients.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Gerenciador de sessões de terminal persistentes e efêmeras no host (Linux PTY / Windows ConPTY).
pub struct TerminalSessionManager {
    sessions: Mutex<Vec<Arc<PersistentSession>>>,
    next_seq: AtomicU64,
    shutdown_tx: watch::Sender<bool>,
}

impl Default for TerminalSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalSessionManager {
    pub fn new() -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            sessions: Mutex::new(Vec::new()),
            next_seq: AtomicU64::new(1),
            shutdown_tx,
        }
    }

    pub fn subscribe_shutdown(&self) -> watch::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }

    pub async fn list_sessions(&self) -> Vec<TerminalSessionSummary> {
        let mut guard = self.sessions.lock().await;
        prune_expired_sessions(&mut guard, now_unix());
        guard.iter().map(|s| s.summary()).collect()
    }

    pub async fn kill_session(&self, session_id: &str) -> Result<bool> {
        let target = {
            let mut guard = self.sessions.lock().await;
            let pos = guard.iter().position(|s| s.id == session_id);
            pos.map(|idx| guard.remove(idx))
        };
        if let Some(session) = target {
            session
                .kill_with_reason(Some("Sessão encerrada pelo usuário".to_string()))
                .await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn kill_for_peer(&self, peer_id: &str) {
        let targets: Vec<Arc<PersistentSession>> = {
            let mut guard = self.sessions.lock().await;
            let mut removed = Vec::new();
            let mut idx = 0;
            while idx < guard.len() {
                if guard[idx].owner_peer_id == peer_id {
                    removed.push(guard.remove(idx));
                } else {
                    idx += 1;
                }
            }
            removed
        };
        for session in targets {
            session
                .kill_with_reason(Some("Dispositivo revogado no host".to_string()))
                .await;
        }
    }

    pub async fn kill_all_with_reason(&self, reason: Option<&str>) {
        let targets = {
            let mut guard = self.sessions.lock().await;
            std::mem::take(&mut *guard)
        };
        let reason_str = reason.map(ToString::to_string);
        for session in targets {
            session.kill_with_reason(reason_str.clone()).await;
        }
    }

    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        self.kill_all_with_reason(Some("Host Oráculo encerrado")).await;
    }

    async fn resolve_or_spawn_persistent(
        &self,
        init: &TerminalInit,
        owner_peer_id: &str,
    ) -> Result<(Arc<PersistentSession>, bool)> {
        let mut guard = self.sessions.lock().await;
        let now = now_unix();
        prune_expired_sessions(&mut guard, now);

        if let Some(req_id) = init.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let found = guard
                .iter()
                .find(|s| s.id == req_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Sessão persistente '{req_id}' não encontrada ou já encerrada"))?;
            return Ok((found, true));
        }

        // Sem session_id nem label explícito -> retoma a sessão ativa mais recente se houver.
        if init.label.is_none() {
            if let Some(latest_running) = guard
                .iter()
                .filter(|s| s.is_running())
                .max_by_key(|s| s.last_activity_unix.load(Ordering::Acquire))
                .cloned()
            {
                return Ok((latest_running, true));
            }
        }

        let running_count = guard.iter().filter(|s| s.is_running()).count();
        if running_count >= MAX_RUNNING_PERSISTENT_SESSIONS {
            bail!(
                "Limite de {MAX_RUNNING_PERSISTENT_SESSIONS} sessões persistentes ativas atingido. Encerre uma sessão antes de abrir outra."
            );
        }

        // Remove encerradas mais antigas se estourar a capacidade de armazenamento.
        while guard.len() >= MAX_STORED_PERSISTENT_SESSIONS {
            if let Some(ended_idx) = guard.iter().position(|s| !s.is_running()) {
                guard.remove(ended_idx);
            } else {
                break;
            }
        }

        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let id = format!("s{seq}");
        let label = sanitize_session_label(init.label.as_deref(), seq);
        let session = spawn_persistent_pty(
            id,
            label,
            owner_peer_id.to_string(),
            init.cols,
            init.rows,
        )?;
        guard.push(Arc::clone(&session));
        Ok((session, false))
    }
}

fn spawn_persistent_pty(
    id: String,
    label: String,
    owner_peer_id: String,
    cols: u16,
    rows: u16,
) -> Result<Arc<PersistentSession>> {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(size).context("Falha ao abrir PTY")?;
    let cmd = build_shell_command();
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .context("Falha ao iniciar shell no PTY")?;
    let killer = child.clone_killer();
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let (in_tx, mut in_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = broadcast::channel::<(u64, Vec<u8>)>(256);
    let (stop_tx, _) = watch::channel(false);

    let created_unix = now_unix();
    let session = Arc::new(PersistentSession {
        id,
        label,
        owner_peer_id,
        created_unix,
        last_activity_unix: AtomicU64::new(created_unix),
        ended_unix: AtomicU64::new(0),
        attached_clients: AtomicU32::new(0),
        cols: AtomicU16::new(cols),
        rows: AtomicU16::new(rows),
        master: StdMutex::new(pair.master),
        in_tx: StdMutex::new(Some(in_tx)),
        killer: StdMutex::new(Some(killer)),
        output_tx,
        buffer: StdMutex::new(SessionBufferState {
            total_bytes: 0,
            ring: VecDeque::with_capacity(64 * 1024),
            ended: None,
        }),
        stop_tx,
        stop_reason: StdMutex::new(None),
    });

    let session_reader = Arc::clone(&session);
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = buf[..n].to_vec();
                    session_reader
                        .last_activity_unix
                        .store(now_unix(), Ordering::Release);
                    if let Ok(mut state) = session_reader.buffer.lock() {
                        push_ring_bytes(&mut state.ring, &chunk, RING_BUFFER_BYTES);
                        state.total_bytes = state.total_bytes.saturating_add(n as u64);
                        let end_offset = state.total_bytes;
                        let _ = session_reader.output_tx.send((end_offset, chunk));
                    }
                }
                Err(_) => break,
            }
        }

        let code = child
            .wait()
            .map(|status| status.exit_code() as i32)
            .unwrap_or(0);
        if let Ok(mut tx_guard) = session_reader.in_tx.lock() {
            *tx_guard = None;
        }
        if let Ok(mut killer_guard) = session_reader.killer.lock() {
            *killer_guard = None;
        }
        if let Ok(mut state) = session_reader.buffer.lock() {
            if state.ended.is_none() {
                state.ended = Some(code);
                session_reader
                    .ended_unix
                    .store(now_unix(), Ordering::Release);
            }
        }
        let _ = session_reader.stop_tx.send(true);
    });

    tokio::task::spawn_blocking(move || {
        while let Some(bytes) = in_rx.blocking_recv() {
            if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });

    Ok(session)
}

async fn send_terminal_error(mut send: SendStream, msg: impl Into<String>) {
    let _ = write_frame(&mut send, &TerminalHostMsg::Error(msg.into())).await;
    let _ = send.finish();
    tokio::time::sleep(Duration::from_millis(60)).await;
}

/// Atende uma sessão de terminal PTY (efêmera para `oraculo-cli` ou persistente para sessões mobile).
pub async fn handle_terminal_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    remote_id: String,
    manager: Arc<TerminalSessionManager>,
    mut settings_rx: watch::Receiver<HostSettings>,
) -> Result<()> {
    if !settings_rx.borrow().allow_terminal {
        send_terminal_error(send, "Terminal remoto desativado pelo host").await;
        return Ok(());
    }

    let init: TerminalInit = match read_frame(&mut recv).await? {
        Some(init) => init,
        None => return Ok(()),
    };

    if !settings_rx.borrow().allow_terminal {
        send_terminal_error(send, "Terminal remoto desativado pelo host").await;
        return Ok(());
    }

    if !init.persistent {
        return handle_ephemeral_terminal(send, recv, init, manager, settings_rx).await;
    }

    let (session, reconnected) = match manager.resolve_or_spawn_persistent(&init, &remote_id).await {
        Ok(res) => res,
        Err(err) => {
            send_terminal_error(send, format!("{err:#}")).await;
            return Ok(());
        }
    };

    session.attached_clients.fetch_add(1, Ordering::AcqRel);
    let _attached_guard = AttachedClientGuard(Arc::clone(&session));

    let mut session_stop_rx = session.stop_tx.subscribe();
    let mut daemon_shutdown_rx = manager.subscribe_shutdown();
    let (mut output_rx, snapshot, snapshot_end, ended_code) = session.subscribe_and_snapshot();

    write_frame(
        &mut send,
        &TerminalHostMsg::Ready {
            session_id: session.id.clone(),
            label: session.label.clone(),
            persistent: true,
            reconnected,
        },
    )
    .await?;

    if !snapshot.is_empty() {
        write_frame(&mut send, &TerminalHostMsg::Output(snapshot)).await?;
    }

    if let Some(code) = ended_code {
        write_frame(&mut send, &TerminalHostMsg::Exit(code)).await?;
        let _ = send.finish();
        tokio::time::sleep(Duration::from_millis(60)).await;
        return Ok(());
    }

    session.resize(init.cols, init.rows);

    let mut cursor_offset = snapshot_end;
    let mut sent_terminal_frame = false;

    // Verifica encerramento ocorrido durante o envio do Ready/snapshot antes de entrar no select!.
    if *daemon_shutdown_rx.borrow() {
        let _ = write_frame(
            &mut send,
            &TerminalHostMsg::Error("Host Oráculo encerrado".to_string()),
        )
        .await;
        let _ = send.finish();
        tokio::time::sleep(Duration::from_millis(60)).await;
        return Ok(());
    }
    if *session_stop_rx.borrow() {
        while let Ok((chunk_end, chunk)) = output_rx.try_recv() {
            if let Some(slice) = slice_new_bytes(cursor_offset, chunk_end, &chunk) {
                cursor_offset = chunk_end;
                if write_frame(&mut send, &TerminalHostMsg::Output(slice.to_vec()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
        let custom_reason = session.stop_reason.lock().ok().and_then(|g| g.clone());
        let ended = session.buffer.lock().ok().and_then(|b| b.ended);
        if let Some(reason) = custom_reason {
            let _ = write_frame(&mut send, &TerminalHostMsg::Error(reason)).await;
        } else if let Some(code) = ended {
            let _ = write_frame(&mut send, &TerminalHostMsg::Exit(code)).await;
        }
        let _ = send.finish();
        tokio::time::sleep(Duration::from_millis(60)).await;
        return Ok(());
    }

    loop {
        tokio::select! {
            biased;
            changed = settings_rx.changed() => {
                if changed.is_ok() && !settings_rx.borrow().allow_terminal {
                    let _ = write_frame(
                        &mut send,
                        &TerminalHostMsg::Error("Terminal remoto desativado pelo host".to_string()),
                    )
                    .await;
                    sent_terminal_frame = true;
                    break;
                }
            }
            _ = daemon_shutdown_rx.changed() => {
                if *daemon_shutdown_rx.borrow() {
                    let _ = write_frame(
                        &mut send,
                        &TerminalHostMsg::Error("Host Oráculo encerrado".to_string()),
                    )
                    .await;
                    sent_terminal_frame = true;
                    break;
                }
            }
            _ = session_stop_rx.changed() => {
                if *session_stop_rx.borrow() {
                    while let Ok((chunk_end, chunk)) = output_rx.try_recv() {
                        if let Some(slice) = slice_new_bytes(cursor_offset, chunk_end, &chunk) {
                            cursor_offset = chunk_end;
                            if write_frame(&mut send, &TerminalHostMsg::Output(slice.to_vec()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    let custom_reason = session.stop_reason.lock().ok().and_then(|g| g.clone());
                    let ended = session.buffer.lock().ok().and_then(|b| b.ended);
                    if let Some(reason) = custom_reason {
                        let _ = write_frame(&mut send, &TerminalHostMsg::Error(reason)).await;
                        sent_terminal_frame = true;
                    } else if let Some(code) = ended {
                        let _ = write_frame(&mut send, &TerminalHostMsg::Exit(code)).await;
                        sent_terminal_frame = true;
                    }
                    break;
                }
            }
            out = output_rx.recv() => {
                match out {
                    Ok((chunk_end, chunk)) => {
                        if let Some(slice) = slice_new_bytes(cursor_offset, chunk_end, &chunk) {
                            cursor_offset = chunk_end;
                            if write_frame(&mut send, &TerminalHostMsg::Output(slice.to_vec()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let msg = b"\r\n[... parte da sa\xc3\xadda foi condensada pela rede ...]\r\n".to_vec();
                        if write_frame(&mut send, &TerminalHostMsg::Output(msg))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            frame = read_frame::<_, TerminalClientMsg>(&mut recv) => {
                match frame {
                    Ok(Some(TerminalClientMsg::Input(bytes))) => {
                        if !session.send_input(bytes).await {
                            break;
                        }
                    }
                    Ok(Some(TerminalClientMsg::Resize { cols, rows })) => {
                        session.resize(cols, rows);
                    }
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }

    let _ = send.finish();
    if sent_terminal_frame {
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    Ok(())
}

async fn handle_ephemeral_terminal(
    mut send: SendStream,
    mut recv: RecvStream,
    init: TerminalInit,
    manager: Arc<TerminalSessionManager>,
    mut settings_rx: watch::Receiver<HostSettings>,
) -> Result<()> {
    let size = PtySize {
        rows: init.rows.max(1),
        cols: init.cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(size).context("Falha ao abrir PTY")?;
    let cmd = build_shell_command();
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .context("Falha ao iniciar shell no PTY")?;
    let mut killer = child.clone_killer();
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, mut out_rx) = mpsc::channel::<TerminalHostMsg>(64);
    let (in_tx, mut in_rx) = mpsc::channel::<Vec<u8>>(64);

    let out_tx_reader = out_tx.clone();
    let reader_task = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if out_tx_reader
                        .blocking_send(TerminalHostMsg::Output(buf[..n].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let code = child
            .wait()
            .map(|status| status.exit_code() as i32)
            .unwrap_or(0);
        let _ = out_tx_reader.blocking_send(TerminalHostMsg::Exit(code));
    });

    let writer_task = tokio::task::spawn_blocking(move || {
        while let Some(bytes) = in_rx.blocking_recv() {
            if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });

    let mut daemon_shutdown_rx = manager.subscribe_shutdown();
    let master = pair.master;
    let ready_sent = write_frame(
        &mut send,
        &TerminalHostMsg::Ready {
            session_id: "ephemeral".to_string(),
            label: "Terminal".to_string(),
            persistent: false,
            reconnected: false,
        },
    )
    .await
    .is_ok();

    let send_task = if ready_sent {
        Some(tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                let is_terminal =
                    matches!(msg, TerminalHostMsg::Exit(_) | TerminalHostMsg::Error(_));
                if write_frame(&mut send, &msg).await.is_err() {
                    break;
                }
                if is_terminal {
                    break;
                }
            }
            let _ = send.finish();
        }))
    } else {
        drop(out_rx);
        let _ = send.finish();
        None
    };

    if ready_sent && !*daemon_shutdown_rx.borrow() {
        loop {
            let msg_opt = tokio::select! {
                biased;
                changed = settings_rx.changed() => {
                    if changed.is_ok() && !settings_rx.borrow().allow_terminal {
                        let _ = out_tx
                            .send(TerminalHostMsg::Error(
                                "Terminal remoto desativado pelo host".to_string(),
                            ))
                            .await;
                        break;
                    }
                    continue;
                }
                _ = daemon_shutdown_rx.changed() => {
                    if *daemon_shutdown_rx.borrow() {
                        let _ = out_tx
                            .send(TerminalHostMsg::Error("Host Oráculo encerrado".to_string()))
                            .await;
                        break;
                    }
                    continue;
                }
                frame = read_frame::<_, TerminalClientMsg>(&mut recv) => match frame {
                    Ok(opt) => opt,
                    Err(_) => break,
                },
            };
            let Some(msg) = msg_opt else {
                break;
            };
            match msg {
                TerminalClientMsg::Input(bytes) => {
                    if in_tx.send(bytes).await.is_err() {
                        break;
                    }
                }
                TerminalClientMsg::Resize { cols, rows } => {
                    let _ = master.resize(PtySize {
                        rows: rows.max(1),
                        cols: cols.max(1),
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        }
    }

    drop(out_tx);
    drop(in_tx);
    drop(master);
    let _ = tokio::task::spawn_blocking(move || killer.kill()).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), reader_task).await;
    let _ = writer_task.await;
    if let Some(task) = send_task {
        let _ = task.await;
    }
    Ok(())
}

fn build_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string());
        CommandBuilder::new(shell)
    } else if std::path::Path::new("/.flatpak-info").exists() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new("flatpak-spawn");
        cmd.args([
            "--host",
            "--env=TERM=xterm-256color",
            "--env=COLORTERM=truecolor",
            &shell,
        ]);
        cmd
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd
    }
}

pub(crate) fn push_ring_bytes(ring: &mut VecDeque<u8>, bytes: &[u8], cap: usize) {
    if bytes.len() >= cap {
        ring.clear();
        ring.extend(&bytes[bytes.len() - cap..]);
        return;
    }
    let excess = (ring.len() + bytes.len()).saturating_sub(cap);
    for _ in 0..excess {
        ring.pop_front();
    }
    ring.extend(bytes);
}

pub(crate) fn slice_new_bytes(cursor_offset: u64, chunk_end: u64, chunk: &[u8]) -> Option<&[u8]> {
    if chunk_end <= cursor_offset {
        return None;
    }
    let chunk_start = chunk_end.saturating_sub(chunk.len() as u64);
    if chunk_start < cursor_offset {
        let skip = (cursor_offset - chunk_start) as usize;
        Some(&chunk[skip.min(chunk.len())..])
    } else {
        Some(chunk)
    }
}

pub(crate) fn sanitize_session_label(raw: Option<&str>, seq: u64) -> String {
    if let Some(raw_str) = raw {
        let clean: String = raw_str
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
            .trim()
            .chars()
            .take(32)
            .collect();
        if !clean.is_empty() {
            return clean;
        }
    }
    format!("Sessão {seq}")
}

fn prune_expired_sessions(sessions: &mut Vec<Arc<PersistentSession>>, now: u64) {
    sessions.retain(|s| {
        if s.is_running() {
            return true;
        }
        let ended = s.ended_unix.load(Ordering::Acquire);
        ended == 0 || now.saturating_sub(ended) < ENDED_SESSION_TTL_SECS
    });
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_caps_and_preserves_tail_cleanly() {
        let mut ring = VecDeque::new();
        push_ring_bytes(&mut ring, b"abcde", 8);
        push_ring_bytes(&mut ring, b"123456", 8);
        assert_eq!(ring.make_contiguous(), b"de123456");

        push_ring_bytes(&mut ring, b"0123456789", 4);
        assert_eq!(ring.make_contiguous(), b"6789");
    }

    #[test]
    fn slice_new_bytes_avoids_duplication_and_gaps() {
        assert_eq!(slice_new_bytes(10, 10, b"abcd"), None);
        assert_eq!(slice_new_bytes(10, 8, b"abcd"), None);
        assert_eq!(slice_new_bytes(10, 12, b"abcd"), Some(b"cd".as_slice()));
        assert_eq!(slice_new_bytes(10, 14, b"abcd"), Some(b"abcd".as_slice()));
    }

    #[test]
    fn sanitizes_session_labels_and_generates_fallback() {
        assert_eq!(sanitize_session_label(Some("  Agente\nCLI  "), 2), "AgenteCLI");
        assert_eq!(sanitize_session_label(Some("   "), 3), "Sessão 3");
        assert_eq!(sanitize_session_label(None, 4), "Sessão 4");
    }

    #[tokio::test]
    async fn persistent_manager_spawns_reattaches_isolates_peers_and_cleans_up() {
        let mgr = TerminalSessionManager::new();
        assert!(mgr.list_sessions().await.is_empty());

        let (s1, re1) = mgr
            .resolve_or_spawn_persistent(
                &TerminalInit {
                    cols: 80,
                    rows: 24,
                    persistent: true,
                    session_id: None,
                    label: Some("Agente".into()),
                },
                "peer_a",
            )
            .await
            .unwrap();
        assert!(!re1);
        assert_eq!(s1.label, "Agente");

        // Nova conexão sem session_id nem label retoma a sessão ativa existente.
        let (reattached, re2) = mgr
            .resolve_or_spawn_persistent(
                &TerminalInit {
                    cols: 90,
                    rows: 30,
                    persistent: true,
                    session_id: None,
                    label: None,
                },
                "peer_a",
            )
            .await
            .unwrap();
        assert!(re2);
        assert_eq!(reattached.id, s1.id);

        // Outro peer pode ter sua sessão isolada; revogar peer_a não mata sessão de peer_b.
        let (s2, _) = mgr
            .resolve_or_spawn_persistent(
                &TerminalInit {
                    cols: 80,
                    rows: 24,
                    persistent: true,
                    session_id: None,
                    label: Some("Build".into()),
                },
                "peer_b",
            )
            .await
            .unwrap();
        assert_eq!(mgr.list_sessions().await.len(), 2);

        mgr.kill_for_peer("peer_a").await;
        let left = mgr.list_sessions().await;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, s2.id);

        mgr.shutdown().await;
        assert!(mgr.list_sessions().await.is_empty());
    }
}
