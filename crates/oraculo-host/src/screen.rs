use crate::config::HostSettings;
use anyhow::{Result, bail};
use iroh::endpoint::{RecvStream, SendStream};
use oraculo_proto::{
    MAX_FRAME_SIZE, ScreenClientMsg, ScreenHostMsg, ScreenStart, read_frame, write_frame,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex as StdMutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, watch};
use tracing::{debug, info, warn};

const MAX_PENDING_DELTAS: usize = 6;
const MAX_GDP_PAYLOAD_SIZE: usize = 8 * 1024 * 1024;
const KEYFRAME_RESTART_COOLDOWN: Duration = Duration::from_secs(4);

/// Unidade de acesso H.264 Annex-B pronta para transmissão no stream de vídeo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAccessUnit {
    pub seq: u64,
    pub pts_ms: u64,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

/// Metadados negociados do stream ativo antes da emissão de frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedStreamInfo {
    pub width: u16,
    pub height: u16,
    pub fps: u8,
    pub encoder: String,
    pub monitor_index: u8,
    pub monitor_count: u8,
    pub available_monitors: u8,
}

/// Fila não bloqueante com descarte anti-latência para frames de vídeo:
/// novos keyframes limpam imediatamente o backlog antigo e saturação descarta deltas
/// até o próximo keyframe para evitar artefatos visuais e acúmulo de atraso.
pub struct ScreenFrameQueue {
    state: StdMutex<QueueInner>,
    notify: Notify,
}

#[derive(Default)]
struct QueueInner {
    frames: VecDeque<EncodedAccessUnit>,
    waiting_for_keyframe: bool,
    closed: bool,
    next_seq: u64,
    dropped_frames: u64,
}

impl ScreenFrameQueue {
    pub fn new() -> Self {
        Self {
            state: StdMutex::new(QueueInner {
                waiting_for_keyframe: true,
                ..Default::default()
            }),
            notify: Notify::new(),
        }
    }

    pub fn push(&self, pts_ms: u64, keyframe: bool, data: Vec<u8>) {
        let Ok(mut inner) = self.state.lock() else {
            return;
        };
        if inner.closed {
            return;
        }

        if keyframe {
            let cleared = inner.frames.len() as u64;
            if cleared > 0 {
                inner.dropped_frames += cleared;
                inner.frames.clear();
            }
            inner.waiting_for_keyframe = false;
        } else {
            if inner.waiting_for_keyframe {
                inner.dropped_frames += 1;
                return;
            }
            if inner.frames.len() >= MAX_PENDING_DELTAS {
                let cleared = inner.frames.len() as u64 + 1;
                inner.frames.clear();
                inner.waiting_for_keyframe = true;
                inner.dropped_frames += cleared;
                debug!(
                    dropped = inner.dropped_frames,
                    "Fila de vídeo saturada; descartando deltas até o próximo keyframe"
                );
                return;
            }
        }

        let seq = inner.next_seq;
        inner.next_seq = inner.next_seq.wrapping_add(1);
        inner.frames.push_back(EncodedAccessUnit {
            seq,
            pts_ms,
            keyframe,
            data,
        });
        drop(inner);
        self.notify.notify_one();
    }

    pub fn close(&self) {
        if let Ok(mut inner) = self.state.lock() {
            inner.closed = true;
            inner.frames.clear();
        }
        self.notify.notify_waiters();
    }

    #[cfg(test)]
    pub fn dropped_frames(&self) -> u64 {
        self.state
            .lock()
            .map(|inner| inner.dropped_frames)
            .unwrap_or(0)
    }

    pub async fn pop_wait(&self) -> Option<EncodedAccessUnit> {
        loop {
            let notified = self.notify.notified();
            {
                let mut inner = self.state.lock().ok()?;
                if let Some(frame) = inner.frames.pop_front() {
                    return Some(frame);
                }
                if inner.closed {
                    return None;
                }
            }
            notified.await;
        }
    }
}

/// Controlador de captura e streaming de tela desacoplado de plataforma.
/// No Linux usa `xdg-desktop-portal` ScreenCast + GStreamer; no Windows pode acoplar DXGI/WGC + Media Foundation.
pub struct ScreenController {
    backend: PlatformScreenBackend,
    generation: AtomicU64,
    active_tx: watch::Sender<u64>,
    session_lock: Mutex<()>,
}

impl ScreenController {
    pub fn new(token_file: PathBuf) -> Self {
        let (active_tx, _rx) = watch::channel(0u64);
        Self {
            backend: PlatformScreenBackend::new(token_file),
            generation: AtomicU64::new(0),
            active_tx,
            session_lock: Mutex::new(()),
        }
    }

    fn claim_generation(&self) -> (u64, watch::Receiver<u64>) {
        let next_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        // `watch::Sender::send()` é no-op quando `receiver_count() == 0` (após `new()`).
        // `send_if_modified` atualiza o valor incondicionalmente antes do `subscribe()`.
        self.active_tx.send_if_modified(|cur| {
            if next_gen > *cur {
                *cur = next_gen;
                true
            } else {
                false
            }
        });
        let rx = self.active_tx.subscribe();
        (next_gen, rx)
    }
}

async fn send_ready(send: &mut SendStream, info: &NegotiatedStreamInfo) -> Result<()> {
    write_frame(
        send,
        &ScreenHostMsg::Ready {
            width: info.width,
            height: info.height,
            fps: info.fps,
            encoder: info.encoder.clone(),
            monitor_index: info.monitor_index,
            monitor_count: info.monitor_count,
            available_monitors: info.available_monitors,
        },
    )
    .await
}

async fn send_notice(send: &mut SendStream, message: impl Into<String>) {
    let _ = write_frame(send, &ScreenHostMsg::Notice(message.into())).await;
}

async fn recv_optional_reauth(
    rx: &mut Option<tokio::sync::oneshot::Receiver<Result<PendingReauth>>>,
) -> std::result::Result<Result<PendingReauth>, tokio::sync::oneshot::error::RecvError> {
    match rx.as_mut() {
        Some(inner) => inner.await,
        None => std::future::pending().await,
    }
}

/// Executa o assistente interativo no terminal local para pré-autorizar múltiplos monitores no portal.
pub async fn authorize_monitors_cli(token_file: &std::path::Path) -> Result<()> {
    authorize_monitors_interactive(token_file).await
}

async fn restart_capture(
    send: &mut SendStream,
    controller: &ScreenController,
    active_capture: &mut ActiveCaptureSession,
    config: ScreenStart,
) -> Result<()> {
    active_capture.stop().await;
    match controller.backend.start_capture(config).await {
        Ok(new_session) => {
            *active_capture = new_session;
            send_ready(send, &active_capture.info).await
        }
        Err(err) => {
            let _ = write_frame(send, &ScreenHostMsg::Error(format!("{err:#}"))).await;
            Err(err)
        }
    }
}

/// Atende uma sessão dedicada de streaming H.264 sobre `StreamKind::Screen`.
pub async fn handle_screen_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    controller: Arc<ScreenController>,
    mut settings_rx: watch::Receiver<HostSettings>,
) -> Result<()> {
    if !settings_rx.borrow().allow_screen {
        let _ = write_frame(
            &mut send,
            &ScreenHostMsg::Error("Espelhamento de tela desativado pelo host".to_string()),
        )
        .await;
        let _ = send.finish();
        return Ok(());
    }

    let init: ScreenStart = match read_frame::<_, ScreenStart>(&mut recv).await? {
        Some(cfg) => cfg.sanitized(),
        None => return Ok(()),
    };

    let (my_gen, mut gen_rx) = controller.claim_generation();
    let _guard = controller.session_lock.lock().await;
    if *gen_rx.borrow() != my_gen {
        return Ok(());
    }
    if !settings_rx.borrow().allow_screen {
        let _ = write_frame(
            &mut send,
            &ScreenHostMsg::Error("Espelhamento de tela desativado pelo host".to_string()),
        )
        .await;
        let _ = send.finish();
        return Ok(());
    }

    let mut config = init;
    let mut active_capture = match controller.backend.start_capture(config).await {
        Ok(session) => session,
        Err(err) => {
            let _ = write_frame(&mut send, &ScreenHostMsg::Error(format!("{err:#}"))).await;
            let _ = send.finish();
            return Err(err);
        }
    };

    send_ready(&mut send, &active_capture.info).await?;

    let mut last_keyframe_at = Instant::now();
    let mut last_keyframe_restart_at = Instant::now() - KEYFRAME_RESTART_COOLDOWN;
    let mut reauth_rx: Option<tokio::sync::oneshot::Receiver<Result<PendingReauth>>> = None;

    loop {
        tokio::select! {
            biased;
            s_changed = settings_rx.changed() => {
                if s_changed.is_ok() && !settings_rx.borrow().allow_screen {
                    let reason = "Espelhamento de tela desativado pelo host".to_string();
                    let _ = write_frame(&mut send, &ScreenHostMsg::Error(reason.clone())).await;
                    warn!("{reason}");
                    break;
                }
            }
            changed = gen_rx.changed() => {
                if changed.is_err() || *gen_rx.borrow() != my_gen {
                    info!("Sessão de streaming encerrada por nova conexão de tela");
                    break;
                }
            }
            reauth_res = recv_optional_reauth(&mut reauth_rx) => {
                reauth_rx = None;
                match reauth_res {
                    Ok(Ok(pending)) => {
                        match controller
                            .backend
                            .apply_reauthorized(config, pending, &mut active_capture)
                            .await
                        {
                            Ok(notice) => {
                                send_ready(&mut send, &active_capture.info).await?;
                                last_keyframe_at = Instant::now();
                                last_keyframe_restart_at = Instant::now();
                                if let Some(msg) = notice {
                                    send_notice(&mut send, msg).await;
                                }
                            }
                            Err(err) => {
                                warn!("Falha ao aplicar nova seleção de monitores: {err:#}");
                                send_notice(
                                    &mut send,
                                    format!("Não foi possível aplicar a troca de monitores: {err:#}"),
                                )
                                .await;
                            }
                        }
                    }
                    Ok(Err(err)) => {
                        warn!("Seleção de monitores no PC não concluída: {err:#}");
                        send_notice(
                            &mut send,
                            "Seleção no PC cancelada; mantendo o monitor atual.",
                        )
                        .await;
                    }
                    Err(_) => {
                        send_notice(
                            &mut send,
                            "Seletor de monitores encerrado sem confirmação.",
                        )
                        .await;
                    }
                }
            }
            frame = active_capture.queue.pop_wait() => {
                let Some(au) = frame else {
                    let reason = active_capture
                        .take_failure()
                        .unwrap_or_else(|| "Captura de tela encerrada inesperadamente".to_string());
                    let _ = write_frame(&mut send, &ScreenHostMsg::Error(reason.clone())).await;
                    warn!("{reason}");
                    break;
                };
                if au.keyframe {
                    last_keyframe_at = Instant::now();
                }
                if write_frame(
                    &mut send,
                    &ScreenHostMsg::Frame {
                        seq: au.seq,
                        pts_ms: au.pts_ms,
                        keyframe: au.keyframe,
                        data: au.data,
                    },
                )
                .await
                .is_err()
                {
                    break;
                }
            }
            msg = read_frame::<_, ScreenClientMsg>(&mut recv) => {
                match msg {
                    Ok(Some(ScreenClientMsg::Stop)) | Ok(None) => break,
                    Ok(Some(ScreenClientMsg::RequestKeyframe)) => {
                        let since_idr = last_keyframe_at.elapsed();
                        let since_restart = last_keyframe_restart_at.elapsed();
                        if since_idr > Duration::from_millis(1200)
                            && since_restart >= KEYFRAME_RESTART_COOLDOWN
                        {
                            info!("Reiniciando encoder sob demanda para keyframe imediato");
                            last_keyframe_restart_at = Instant::now();
                            restart_capture(&mut send, &controller, &mut active_capture, config).await?;
                            last_keyframe_at = Instant::now();
                        }
                    }
                    Ok(Some(ScreenClientMsg::Reconfigure(next_raw))) => {
                        let next = next_raw.sanitized();
                        if next != config {
                            config = next;
                            restart_capture(&mut send, &controller, &mut active_capture, config).await?;
                            last_keyframe_at = Instant::now();
                            last_keyframe_restart_at = Instant::now();
                        }
                    }
                    Ok(Some(ScreenClientMsg::SwitchMonitor)) => {
                        match controller
                            .backend
                            .switch_monitor(config, &mut active_capture)
                            .await
                        {
                            Ok(true) => {
                                send_ready(&mut send, &active_capture.info).await?;
                                last_keyframe_at = Instant::now();
                                last_keyframe_restart_at = Instant::now();
                            }
                            Ok(false) => {
                                let avail = active_capture.info.available_monitors;
                                let msg = if avail > 1 {
                                    format!(
                                        "O GNOME só liberou 1 dos {avail} monitores. Rode `oraculo-host monitors` no PC e marque todos os monitores."
                                    )
                                } else {
                                    "Apenas 1 monitor ativo detectado neste PC.".to_string()
                                };
                                send_notice(&mut send, msg).await;
                            }
                            Err(err) => {
                                let _ = write_frame(
                                    &mut send,
                                    &ScreenHostMsg::Error(format!("{err:#}")),
                                )
                                .await;
                                return Err(err);
                            }
                        }
                    }
                    Ok(Some(ScreenClientMsg::ReauthorizeMonitors)) => {
                        if reauth_rx.is_some() {
                            send_notice(
                                &mut send,
                                "A janela de seleção de monitores já está aberta no PC.",
                            )
                            .await;
                        } else if let Some(rx) = controller.backend.begin_reauthorize() {
                            reauth_rx = Some(rx);
                            send_notice(
                                &mut send,
                                "Seletor aberto no PC: marque TODOS os monitores e clique em Compartilhar.",
                            )
                            .await;
                        } else {
                            send_notice(
                                &mut send,
                                "A janela de seleção de monitores já está em uso no PC.",
                            )
                            .await;
                        }
                    }
                    Err(err) => {
                        debug!("Canal de controle de tela fechado: {err:#}");
                        break;
                    }
                }
            }
        }
    }

    active_capture.stop().await;
    let _ = send.finish();
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GdpPacket {
    Caps(String),
    Buffer { pts_ns: u64, payload: Vec<u8> },
    Other,
}

/// Leitor incremental de pacotes do GStreamer Data Protocol (`gdppay`, v1.0, cabeçalho de 62 bytes).
#[derive(Default)]
struct GdpPacketReader {
    buf: Vec<u8>,
}

impl GdpPacketReader {
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<GdpPacket>> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        let mut consumed = 0usize;

        while self.buf.len().saturating_sub(consumed) >= 62 {
            let hdr = &self.buf[consumed..consumed + 62];
            if hdr[0] != 1 {
                bail!("Versão GDP inesperada: {}.{}", hdr[0], hdr[1]);
            }
            let ptype = u16::from_be_bytes([hdr[4], hdr[5]]);
            let plen = u32::from_be_bytes([hdr[6], hdr[7], hdr[8], hdr[9]]) as usize;
            if plen > MAX_GDP_PAYLOAD_SIZE {
                bail!("Payload GDP excede limite de segurança ({plen} bytes)");
            }
            let total = 62 + plen;
            if self.buf.len() - consumed < total {
                break;
            }
            let payload = &self.buf[consumed + 62..consumed + total];
            let pkt = match ptype {
                1 => {
                    let pts_ns = u64::from_be_bytes(hdr[10..18].try_into().unwrap());
                    GdpPacket::Buffer {
                        pts_ns,
                        payload: payload.to_vec(),
                    }
                }
                2 => {
                    let text = String::from_utf8_lossy(payload)
                        .trim_end_matches('\0')
                        .to_string();
                    GdpPacket::Caps(text)
                }
                _ => GdpPacket::Other,
            };
            out.push(pkt);
            consumed += total;
        }

        if consumed > 0 {
            self.buf.drain(..consumed);
        }
        Ok(out)
    }
}

fn parse_gdp_caps_dimensions(caps: &str) -> Option<(u16, u16)> {
    let width = parse_caps_int(caps, "width=")?;
    let height = parse_caps_int(caps, "height=")?;
    let w = u16::try_from(width).ok()?;
    let h = u16::try_from(height).ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

fn parse_caps_int(caps: &str, key: &str) -> Option<u32> {
    let idx = caps.find(key)?;
    let mut rest = &caps[idx + key.len()..];
    if let Some(after_type) = rest.strip_prefix("(int)") {
        rest = after_type;
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

struct AnalyzedAu {
    keyframe: bool,
    sps_dimensions: Option<(u16, u16)>,
    data: Vec<u8>,
}

/// Valida NAL units Annex-B de um Access Unit, atualiza cache de SPS/PPS e garante que
/// todo keyframe (`IDR`, NAL tipo 5) seja precedido por SPS e PPS para decodificação imediata.
fn normalize_annex_b_au(
    raw: Vec<u8>,
    sps_cache: &mut Option<Vec<u8>>,
    pps_cache: &mut Option<Vec<u8>>,
) -> Option<AnalyzedAu> {
    let nals = scan_annex_b_nals(&raw);
    if nals.is_empty() {
        return None;
    }

    let mut has_sps = false;
    let mut has_pps = false;
    let mut has_idr = false;
    let mut sps_dimensions = None;

    for (start, sc_len, end) in nals {
        let header_off = start + sc_len;
        if header_off >= end {
            continue;
        }
        let nal_type = raw[header_off] & 0x1F;
        match nal_type {
            7 => {
                has_sps = true;
                *sps_cache = Some(raw[start..end].to_vec());
                if sps_dimensions.is_none() {
                    sps_dimensions = parse_sps_dimensions(&raw[header_off..end]);
                }
            }
            8 => {
                has_pps = true;
                *pps_cache = Some(raw[start..end].to_vec());
            }
            5 => {
                has_idr = true;
            }
            _ => {}
        }
    }

    let data = if has_idr && (!has_sps || !has_pps) {
        if let (Some(sps), Some(pps)) = (sps_cache.as_ref(), pps_cache.as_ref()) {
            let mut combined = Vec::with_capacity(raw.len() + sps.len() + pps.len());
            if !has_sps {
                combined.extend_from_slice(sps);
            }
            if !has_pps {
                combined.extend_from_slice(pps);
            }
            combined.extend_from_slice(&raw);
            combined
        } else {
            raw
        }
    } else {
        raw
    };

    if data.len() > MAX_FRAME_SIZE {
        warn!(bytes = data.len(), "Access Unit excede MAX_FRAME_SIZE; descartado");
        return None;
    }

    Some(AnalyzedAu {
        keyframe: has_idr,
        sps_dimensions,
        data,
    })
}

/// Retorna tuplas `(start_with_start_code, start_code_len, end_offset)` para cada NAL Annex-B.
fn scan_annex_b_nals(bytes: &[u8]) -> Vec<(usize, usize, usize)> {
    let mut starts = Vec::new();
    let mut i = 0usize;
    while i + 3 <= bytes.len() {
        if let Some(sc_len) = start_code_len_at(bytes, i) {
            starts.push((i, sc_len));
            i += sc_len;
        } else {
            i += 1;
        }
    }

    let mut out = Vec::with_capacity(starts.len());
    for idx in 0..starts.len() {
        let (start, sc_len) = starts[idx];
        let end = starts.get(idx + 1).map(|s| s.0).unwrap_or(bytes.len());
        out.push((start, sc_len, end));
    }
    out
}

fn start_code_len_at(bytes: &[u8], i: usize) -> Option<usize> {
    if i + 4 <= bytes.len()
        && bytes[i] == 0
        && bytes[i + 1] == 0
        && bytes[i + 2] == 0
        && bytes[i + 3] == 1
    {
        Some(4)
    } else if i + 3 <= bytes.len() && bytes[i] == 0 && bytes[i + 1] == 0 && bytes[i + 2] == 1 {
        Some(3)
    } else {
        None
    }
}

/// Extrai `(width, height)` de um NAL unit H.264 SPS (tipo 7, sem start code).
fn parse_sps_dimensions(sps_nal: &[u8]) -> Option<(u16, u16)> {
    if sps_nal.is_empty() || (sps_nal[0] & 0x1F) != 7 {
        return None;
    }
    let rbsp = ebsp_to_rbsp(&sps_nal[1..]);
    let mut r = RbspReader::new(&rbsp);

    let profile_idc = r.read_bits(8)?;
    let _constraint_flags = r.read_bits(8)?;
    let _level_idc = r.read_bits(8)?;
    let _sps_id = r.read_ue()?;

    let mut chroma_format_idc = 1u32;
    if matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        chroma_format_idc = r.read_ue()?;
        if chroma_format_idc == 3 {
            let _separate_colour_plane = r.read_bit()?;
        }
        let _bit_depth_luma = r.read_ue()?;
        let _bit_depth_chroma = r.read_ue()?;
        let _qpprime_zero_transform_bypass = r.read_bit()?;
        if r.read_bit()? == 1 {
            let count = if chroma_format_idc != 3 { 8 } else { 12 };
            for i in 0..count {
                if r.read_bit()? == 1 {
                    let size = if i < 6 { 16 } else { 64 };
                    skip_scaling_list(&mut r, size)?;
                }
            }
        }
    }

    let _log2_max_frame_num = r.read_ue()?;
    let pic_order_cnt_type = r.read_ue()?;
    if pic_order_cnt_type == 0 {
        let _log2_max_poc_lsb = r.read_ue()?;
    } else if pic_order_cnt_type == 1 {
        let _delta_pic_order_always_zero = r.read_bit()?;
        let _offset_for_non_ref = r.read_se()?;
        let _offset_for_top_to_bottom = r.read_se()?;
        let num_ref_frames_in_poc_cycle = r.read_ue()?;
        for _ in 0..num_ref_frames_in_poc_cycle {
            let _ = r.read_se()?;
        }
    }

    let _max_num_ref_frames = r.read_ue()?;
    let _gaps_in_frame_num = r.read_bit()?;
    let pic_width_in_mbs_minus1 = r.read_ue()?;
    let pic_height_in_map_units_minus1 = r.read_ue()?;
    let frame_mbs_only_flag = r.read_bit()? as u32;
    if frame_mbs_only_flag == 0 {
        let _mb_adaptive_frame_field = r.read_bit()?;
    }
    let _direct_8x8_inference = r.read_bit()?;

    let (crop_l, crop_r, crop_t, crop_b) = if r.read_bit()? == 1 {
        (r.read_ue()?, r.read_ue()?, r.read_ue()?, r.read_ue()?)
    } else {
        (0, 0, 0, 0)
    };

    let (crop_unit_x, crop_unit_y) = match chroma_format_idc {
        0 => (1, 2 - frame_mbs_only_flag),
        1 => (2, 2 * (2 - frame_mbs_only_flag)),
        2 => (2, 2 - frame_mbs_only_flag),
        _ => (1, 2 - frame_mbs_only_flag),
    };

    let raw_w = pic_width_in_mbs_minus1.checked_add(1)?.checked_mul(16)?;
    let raw_h = (2 - frame_mbs_only_flag)
        .checked_mul(pic_height_in_map_units_minus1.checked_add(1)?)?
        .checked_mul(16)?;
    let crop_x = crop_l.checked_add(crop_r)?.checked_mul(crop_unit_x)?;
    let crop_y = crop_t.checked_add(crop_b)?.checked_mul(crop_unit_y)?;

    let width = u16::try_from(raw_w.checked_sub(crop_x)?).ok()?;
    let height = u16::try_from(raw_h.checked_sub(crop_y)?).ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

fn ebsp_to_rbsp(ebsp: &[u8]) -> Vec<u8> {
    let mut rbsp = Vec::with_capacity(ebsp.len());
    let mut zeros = 0u8;
    for &b in ebsp {
        if zeros == 2 && b == 0x03 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros.saturating_add(1) } else { 0 };
        rbsp.push(b);
    }
    rbsp
}

fn skip_scaling_list(r: &mut RbspReader<'_>, size: usize) -> Option<()> {
    let mut last_scale = 8i32;
    let mut next_scale = 8i32;
    for _ in 0..size {
        if next_scale != 0 {
            let delta = r.read_se()?;
            next_scale = (last_scale + delta + 256) % 256;
        }
        last_scale = if next_scale == 0 {
            last_scale
        } else {
            next_scale
        };
    }
    Some(())
}

struct RbspReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> RbspReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    fn read_bit(&mut self) -> Option<u8> {
        let byte_idx = self.bit_pos / 8;
        let bit_idx = 7 - (self.bit_pos % 8);
        let byte = *self.data.get(byte_idx)?;
        self.bit_pos += 1;
        Some((byte >> bit_idx) & 1)
    }

    fn read_bits(&mut self, count: usize) -> Option<u32> {
        if count > 32 {
            return None;
        }
        let mut val = 0u32;
        for _ in 0..count {
            val = (val << 1) | u32::from(self.read_bit()?);
        }
        Some(val)
    }

    fn read_ue(&mut self) -> Option<u32> {
        let mut zeros = 0usize;
        while self.read_bit()? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        if zeros == 0 {
            return Some(0);
        }
        let suffix = self.read_bits(zeros)?;
        Some((1u32 << zeros) - 1 + suffix)
    }

    fn read_se(&mut self) -> Option<i32> {
        let code = self.read_ue()?;
        let signed = if code % 2 == 0 {
            -((code / 2) as i64)
        } else {
            ((code + 1) / 2) as i64
        };
        i32::try_from(signed).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GstEncoderKind {
    NvEnc,
    OpenH264,
}

impl GstEncoderKind {
    fn element_name(self) -> &'static str {
        match self {
            Self::NvEnc => "nvh264enc",
            Self::OpenH264 => "openh264enc",
        }
    }
}

fn compute_target_dimensions(source_size: Option<(u16, u16)>, cfg: ScreenStart) -> (u16, u16) {
    let cfg = cfg.sanitized();
    let Some((src_w, src_h)) = source_size.filter(|(w, h)| *w >= 2 && *h >= 2) else {
        return (cfg.max_width, cfg.max_height);
    };

    let scale = f64::min(
        1.0,
        f64::min(
            f64::from(cfg.max_width) / f64::from(src_w),
            f64::from(cfg.max_height) / f64::from(src_h),
        ),
    );
    let scaled_w = ((f64::from(src_w) * scale).round() as u16).max(2) & !1;
    let scaled_h = ((f64::from(src_h) * scale).round() as u16).max(2) & !1;
    (
        scaled_w.clamp(2, cfg.max_width),
        scaled_h.clamp(2, cfg.max_height),
    )
}

fn build_gst_pipeline_args(
    cfg: ScreenStart,
    pw_source_arg: &str,
    source_size: Option<(u16, u16)>,
    pw_fd: i32,
    out_fd: i32,
    encoder: GstEncoderKind,
) -> Vec<String> {
    let cfg = cfg.sanitized();
    let (target_w, target_h) = compute_target_dimensions(source_size, cfg);
    let needs_scale = source_size != Some((target_w, target_h));
    let mut args: Vec<String> = vec![
        "-q".into(),
        "pipewiresrc".into(),
        format!("fd={pw_fd}"),
        pw_source_arg.into(),
        "do-timestamp=true".into(),
        "keepalive-time=1000".into(),
        "min-buffers=1".into(),
        "max-buffers=2".into(),
        "!".into(),
        "videorate".into(),
        format!("max-rate={}", cfg.fps),
        "drop-only=true".into(),
        "skip-to-first=true".into(),
        "!".into(),
        // Mantém sempre o frame mais novo e descarta qualquer atraso antes do encoder.
        "queue".into(),
        "max-size-buffers=1".into(),
        "leaky=downstream".into(),
        "!".into(),
    ];

    if needs_scale {
        args.extend([
            "videoscale".into(),
            "n-threads=0".into(),
            "add-borders=false".into(),
            "method=lanczos".into(),
            "!".into(),
        ]);
    }

    args.extend([
        // Fixa largura e altura explícitas para evitar que o capsfilter negocie o limite mínimo [320, max] com o PipeWire.
        format!("video/x-raw,width={target_w},height={target_h},pixel-aspect-ratio=1/1"),
        "!".into(),
        "videoconvert".into(),
        "n-threads=0".into(),
        "!".into(),
    ]);

    let gop_frames = cfg.fps.saturating_mul(2);
    let encoded_caps = match encoder {
        GstEncoderKind::NvEnc => {
            let max_bitrate = cfg
                .bitrate_kbps
                .saturating_mul(3)
                .saturating_div(2)
                .clamp(cfg.bitrate_kbps, 20_000);
            let vbv = (cfg.bitrate_kbps / 3).clamp(600, 3000);
            args.extend([
                "nvh264enc".into(),
                "preset=p2".into(),
                "tune=ultra-low-latency".into(),
                "rc-mode=vbr".into(),
                format!("bitrate={}", cfg.bitrate_kbps),
                format!("max-bitrate={max_bitrate}"),
                format!("vbv-buffer-size={vbv}"),
                format!("gop-size={gop_frames}"),
                "zerolatency=true".into(),
                "bframes=0".into(),
                "spatial-aq=true".into(),
                "repeat-sequence-header=true".into(),
                "aud=false".into(),
                "qos=false".into(),
                "!".into(),
            ]);
            "video/x-h264,profile=high,stream-format=byte-stream,alignment=au"
        }
        GstEncoderKind::OpenH264 => {
            let bitrate_bps = cfg.bitrate_kbps.saturating_mul(1000);
            args.extend([
                "openh264enc".into(),
                format!("bitrate={bitrate_bps}"),
                format!("gop-size={gop_frames}"),
                "usage-type=screen".into(),
                "rate-control=bitrate".into(),
                "complexity=low".into(),
                "num-slices=1".into(),
                "!".into(),
                "h264parse".into(),
                "config-interval=-1".into(),
                "!".into(),
            ]);
            "video/x-h264,stream-format=byte-stream,alignment=au"
        }
    };

    args.extend([
        encoded_caps.into(),
        "!".into(),
        "gdppay".into(),
        "!".into(),
        "fdsink".into(),
        format!("fd={out_fd}"),
        "sync=false".into(),
        "async=false".into(),
        "enable-last-sample=false".into(),
        "qos=false".into(),
    ]);

    args
}

#[cfg(target_os = "linux")]
use linux::{
    PendingReauth, WaylandScreenBackend as PlatformScreenBackend, authorize_monitors_interactive,
};

#[cfg(not(target_os = "linux"))]
use stub::{
    PendingReauth, StubScreenBackend as PlatformScreenBackend, authorize_monitors_interactive,
};

#[cfg(target_os = "linux")]
use linux::CaptureSession as ActiveCaptureSession;

#[cfg(not(target_os = "linux"))]
use stub::CaptureSession as ActiveCaptureSession;

#[cfg(target_os = "linux")]
mod linux {
    use super::{
        GdpPacket, GdpPacketReader, GstEncoderKind, NegotiatedStreamInfo, ScreenFrameQueue,
        build_gst_pipeline_args, normalize_annex_b_au, parse_gdp_caps_dimensions,
    };
    use crate::config::write_private_file;
    use anyhow::{Context, Result, bail};
    use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
    use ashpd::desktop::{PersistMode, Session};
    use oraculo_proto::ScreenStart;
    use std::fs;
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};
    use tokio::io::AsyncReadExt;
    use tokio::process::{Child, Command};
    use tokio::sync::{Mutex, oneshot};
    use tracing::{debug, info, warn};

    const GST_STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
    const CHILD_PW_FD: i32 = 3;
    const CHILD_OUT_FD: i32 = 4;

    pub(super) fn is_drm_connector_active(status: &str, enabled: Option<&str>) -> bool {
        if status.trim() != "connected" {
            return false;
        }
        match enabled {
            Some(v) => v.trim() != "disabled",
            None => true,
        }
    }

    pub(super) fn detect_connected_monitors() -> u8 {
        let Ok(entries) = fs::read_dir("/sys/class/drm") else {
            return 0;
        };
        let mut count = 0u8;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_lossy = name.to_string_lossy();
            if !name_lossy.starts_with("card") || !name_lossy.contains('-') {
                continue;
            }
            let path = entry.path();
            let Ok(status) = fs::read_to_string(path.join("status")) else {
                continue;
            };
            let enabled = fs::read_to_string(path.join("enabled")).ok();
            if is_drm_connector_active(&status, enabled.as_deref()) {
                count = count.saturating_add(1);
            }
        }
        count
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) struct PortalMonitor {
        pub(super) node_id: u32,
        pub(super) position: (i32, i32),
        pub(super) pw_source_arg: String,
        pub(super) source_size: Option<(u16, u16)>,
    }

    pub struct PortalScreencast {
        proxy: Screencast,
        session: Session<Screencast>,
        monitors: Vec<PortalMonitor>,
        active_monitor: usize,
        available_monitors: u8,
    }

    pub type PendingReauth = PortalScreencast;

    struct ReauthFlagGuard(Arc<AtomicBool>);

    impl Drop for ReauthFlagGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }

    impl PortalScreencast {
        async fn open(token_file: &Path, use_saved_token: bool) -> Result<Self> {
            let proxy = Screencast::new()
                .await
                .context("Portal ScreenCast indisponível na sessão gráfica")?;
            let session = proxy
                .create_session(Default::default())
                .await
                .context("Falha ao criar sessão ScreenCast")?;

            let saved_token = if use_saved_token {
                fs::read_to_string(token_file)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            };

            let options = SelectSourcesOptions::default()
                .set_sources(SourceType::MONITOR)
                .set_multiple(true)
                .set_cursor_mode(CursorMode::EMBEDDED)
                .set_persist_mode(PersistMode::ExplicitlyRevoked)
                .set_restore_token(saved_token.as_deref());

            proxy
                .select_sources(&session, options)
                .await
                .context("Falha ao selecionar monitor no portal ScreenCast")?;

            let response = proxy
                .start(&session, None, Default::default())
                .await
                .context("Falha ao iniciar permissão de compartilhamento de tela")?
                .response()
                .context("Compartilhamento de tela cancelado ou negado no PC")?;

            if let Some(token) = response.restore_token() {
                if let Err(err) = write_private_file(token_file, token.as_bytes()) {
                    warn!("Não foi possível salvar screencast_token: {err:#}");
                }
            }

            let mut monitors: Vec<PortalMonitor> = response
                .streams()
                .iter()
                .map(|stream| {
                    let pw_source_arg = if let Some(serial) = stream.pipewire_serial() {
                        format!("target-object={serial}")
                    } else {
                        format!("path={}", stream.pipe_wire_node_id())
                    };
                    let source_size = stream
                        .size()
                        .and_then(|(w, h)| Some((u16::try_from(w).ok()?, u16::try_from(h).ok()?)))
                        .filter(|(w, h)| *w >= 320 && *h >= 240);
                    PortalMonitor {
                        node_id: stream.pipe_wire_node_id(),
                        position: stream.position().unwrap_or((i32::MAX, i32::MAX)),
                        pw_source_arg,
                        source_size,
                    }
                })
                .collect();

            if monitors.is_empty() {
                bail!("Nenhum monitor foi selecionado para compartilhamento");
            }
            monitors.sort_by_key(|m| (m.position.0, m.position.1, m.node_id));

            let mon_count_u8 = u8::try_from(monitors.len().min(u8::MAX as usize)).unwrap_or(1);
            let available_monitors = detect_connected_monitors().max(mon_count_u8).max(1);

            if monitors.len() == 1 && available_monitors > 1 {
                warn!(
                    available_monitors,
                    "Há {available_monitors} monitores ativos no PC, mas apenas 1 foi autorizado no GNOME. Para alternar pelo celular sem abrir janela no PC, rode `oraculo-host monitors` e selecione TODOS os monitores antes de clicar em Compartilhar."
                );
            }

            info!(
                monitor_count = monitors.len(),
                available_monitors,
                first_node_id = monitors[0].node_id,
                first_source = %monitors[0].pw_source_arg,
                first_size = ?monitors[0].source_size,
                "Portal ScreenCast autorizado pelo sistema"
            );

            Ok(Self {
                proxy,
                session,
                monitors,
                active_monitor: 0,
                available_monitors,
            })
        }

        fn selected_monitor(&self) -> (u8, u8, u8, &PortalMonitor) {
            let count = self.monitors.len();
            let idx = self.active_monitor.min(count.saturating_sub(1));
            let monitor = &self.monitors[idx];
            let count_u8 = u8::try_from(count.min(u8::MAX as usize)).unwrap_or(1);
            let avail_u8 = self.available_monitors.max(count_u8).max(1);
            (
                u8::try_from(idx).unwrap_or(u8::MAX),
                count_u8,
                avail_u8,
                monitor,
            )
        }

        async fn open_pipewire_fd(&self) -> Result<OwnedFd> {
            self.proxy
                .open_pipe_wire_remote(&self.session, Default::default())
                .await
                .context("Falha ao abrir socket PipeWire do portal ScreenCast")
        }
    }

    pub async fn authorize_monitors_interactive(token_file: &Path) -> Result<()> {
        let detected = detect_connected_monitors();
        println!("Abrindo seletor de monitores do sistema (Wayland Portal)...");
        if detected > 1 {
            println!("Detectados {detected} monitores ativos neste computador.");
        }
        println!("No diálogo do GNOME:");
        println!("  1. Clique para selecionar TODOS os monitores que deseja usar no celular");
        println!("  2. Deixe marcada a opção \"Lembrar esta seleção\"");
        println!("  3. Clique em \"Compartilhar\"");
        println!();

        let portal = PortalScreencast::open(token_file, false).await?;
        let count = portal.monitors.len();
        let avail = portal.available_monitors as usize;
        if count > 1 {
            println!(
                "✓ {count} monitores autorizados e salvos! Agora você pode alternar direto pelo celular (botão M1/{count}) sem abrir janelas no PC."
            );
        } else if avail > 1 {
            println!(
                "⚠ Apenas 1 de {avail} monitores foi selecionado. Rode `oraculo-host monitors` novamente e marque todos os monitores antes de clicar em Compartilhar se quiser alternar direto pelo celular."
            );
        } else {
            println!("✓ 1 monitor autorizado e salvo.");
        }
        Ok(())
    }

    pub(super) fn pick_reselected_monitor_index(
        prev_node_id: Option<u32>,
        monitors: &[PortalMonitor],
        advance: bool,
    ) -> usize {
        if monitors.len() <= 1 {
            return 0;
        }
        if let Some(prev_id) = prev_node_id {
            if let Some(pos) = monitors.iter().position(|m| m.node_id == prev_id) {
                return if advance {
                    (pos + 1) % monitors.len()
                } else {
                    pos
                };
            }
        }
        0
    }

    pub struct WaylandScreenBackend {
        token_file: PathBuf,
        portal: Mutex<Option<PortalScreencast>>,
        reauth_in_flight: Arc<AtomicBool>,
    }

    pub struct CaptureSession {
        pub info: NegotiatedStreamInfo,
        pub queue: Arc<ScreenFrameQueue>,
        failure: Arc<StdMutex<Option<String>>>,
        child: Option<Child>,
        reader_task: Option<tokio::task::JoinHandle<()>>,
        stderr_task: Option<tokio::task::JoinHandle<()>>,
    }

    impl CaptureSession {
        pub fn take_failure(&self) -> Option<String> {
            self.failure
                .lock()
                .ok()
                .and_then(|mut g| g.take())
        }

        pub async fn stop(&mut self) {
            self.queue.close();
            if let Some(mut child) = self.child.take() {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            }
            if let Some(task) = self.reader_task.take() {
                let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
            }
            if let Some(task) = self.stderr_task.take() {
                let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
            }
        }
    }

    impl Drop for CaptureSession {
        fn drop(&mut self) {
            self.queue.close();
            if let Some(mut child) = self.child.take() {
                let _ = child.start_kill();
            }
        }
    }

    impl WaylandScreenBackend {
        pub fn new(token_file: PathBuf) -> Self {
            Self {
                token_file,
                portal: Mutex::new(None),
                reauth_in_flight: Arc::new(AtomicBool::new(false)),
            }
        }

        pub async fn start_capture(&self, cfg: ScreenStart) -> Result<CaptureSession> {
            let cfg = cfg.sanitized();
            let candidates = available_encoder_candidates().await;
            if candidates.is_empty() {
                bail!(
                    "Nenhum encoder H.264 do GStreamer disponível (`nvh264enc` ou `openh264enc`) nem `gst-launch-1.0`"
                );
            }

            let mut portal_guard = self.portal.lock().await;
            if portal_guard.is_none() {
                *portal_guard = Some(PortalScreencast::open(&self.token_file, true).await?);
            }

            self.spawn_from_locked_portal(&mut portal_guard, &candidates, cfg)
                .await
        }

        pub async fn switch_monitor(
            &self,
            cfg: ScreenStart,
            active_capture: &mut CaptureSession,
        ) -> Result<bool> {
            let cfg = cfg.sanitized();
            let candidates = available_encoder_candidates().await;
            if candidates.is_empty() {
                bail!(
                    "Nenhum encoder H.264 do GStreamer disponível (`nvh264enc` ou `openh264enc`) nem `gst-launch-1.0`"
                );
            }

            let mut portal_guard = self.portal.lock().await;
            let Some(portal) = portal_guard.as_mut() else {
                return Ok(false);
            };
            if portal.monitors.len() <= 1 {
                // Nunca abre diálogo do sistema às cegas a partir do botão de alternância no celular.
                return Ok(false);
            }

            portal.active_monitor = (portal.active_monitor + 1) % portal.monitors.len();
            info!(
                monitor_index = portal.active_monitor + 1,
                monitor_count = portal.monitors.len(),
                "Alternando imediatamente para próximo monitor autorizado"
            );
            active_capture.stop().await;
            let new_session = self
                .spawn_from_locked_portal(&mut portal_guard, &candidates, cfg)
                .await?;
            *active_capture = new_session;
            Ok(true)
        }

        pub fn begin_reauthorize(&self) -> Option<oneshot::Receiver<Result<PendingReauth>>> {
            if self.reauth_in_flight.swap(true, Ordering::AcqRel) {
                return None;
            }
            let (tx, rx) = oneshot::channel();
            let token_file = self.token_file.clone();
            let flag = Arc::clone(&self.reauth_in_flight);
            tokio::spawn(async move {
                let _guard = ReauthFlagGuard(flag);
                let res = PortalScreencast::open(&token_file, false).await;
                let _ = tx.send(res);
            });
            Some(rx)
        }

        pub async fn apply_reauthorized(
            &self,
            cfg: ScreenStart,
            mut new_portal: PendingReauth,
            active_capture: &mut CaptureSession,
        ) -> Result<Option<String>> {
            let cfg = cfg.sanitized();
            let candidates = available_encoder_candidates().await;
            if candidates.is_empty() {
                bail!(
                    "Nenhum encoder H.264 do GStreamer disponível (`nvh264enc` ou `openh264enc`) nem `gst-launch-1.0`"
                );
            }

            let mut portal_guard = self.portal.lock().await;
            let prev_node_id = portal_guard
                .as_ref()
                .and_then(|p| p.monitors.get(p.active_monitor))
                .map(|m| m.node_id);
            new_portal.active_monitor =
                pick_reselected_monitor_index(prev_node_id, &new_portal.monitors, true);

            let authorized_count = new_portal.monitors.len();
            let available_count = new_portal.available_monitors;

            active_capture.stop().await;
            *portal_guard = Some(new_portal);
            let new_session = self
                .spawn_from_locked_portal(&mut portal_guard, &candidates, cfg)
                .await?;
            *active_capture = new_session;

            let notice = if authorized_count > 1 {
                Some(format!(
                    "{authorized_count} monitores autorizados para troca direta no celular."
                ))
            } else if available_count > 1 {
                Some(format!(
                    "Só 1 dos {available_count} monitores foi marcado no GNOME. Selecione todos os monitores antes de clicar em Compartilhar."
                ))
            } else {
                None
            };
            Ok(notice)
        }

        async fn spawn_from_locked_portal(
            &self,
            portal_guard: &mut Option<PortalScreencast>,
            candidates: &[GstEncoderKind],
            cfg: ScreenStart,
        ) -> Result<CaptureSession> {
            let mut last_err = None;
            for (attempt_idx, encoder) in candidates.iter().copied().enumerate() {
                let pw_fd = match portal_guard.as_ref().unwrap().open_pipewire_fd().await {
                    Ok(fd) => fd,
                    Err(first_err) => {
                        warn!("Sessão ScreenCast expirada ou inválida ({first_err:#}); reabrindo");
                        let prev_node_id = portal_guard
                            .as_ref()
                            .and_then(|p| p.monitors.get(p.active_monitor))
                            .map(|m| m.node_id);
                        let mut reopened = PortalScreencast::open(&self.token_file, true).await?;
                        reopened.active_monitor =
                            pick_reselected_monitor_index(prev_node_id, &reopened.monitors, false);
                        *portal_guard = Some(reopened);
                        portal_guard.as_ref().unwrap().open_pipewire_fd().await?
                    }
                };
                let portal_ref = portal_guard.as_ref().unwrap();
                let (monitor_index, monitor_count, available_monitors, mon) =
                    portal_ref.selected_monitor();
                let pw_source_arg = mon.pw_source_arg.clone();
                let source_size = mon.source_size;

                match spawn_gst_pipeline(
                    cfg,
                    &pw_source_arg,
                    source_size,
                    monitor_index,
                    monitor_count,
                    available_monitors,
                    pw_fd,
                    encoder,
                )
                .await
                {
                    Ok(session) => return Ok(session),
                    Err(err) => {
                        warn!(
                            encoder = encoder.element_name(),
                            attempt = attempt_idx + 1,
                            "Falha ao iniciar pipeline GStreamer: {err:#}"
                        );
                        last_err = Some(err);
                    }
                }
            }

            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Falha ao iniciar pipeline de captura")))
        }
    }

    async fn available_encoder_candidates() -> Vec<GstEncoderKind> {
        static CACHED: tokio::sync::Mutex<Option<Vec<GstEncoderKind>>> =
            tokio::sync::Mutex::const_new(None);
        let mut guard = CACHED.lock().await;
        if let Some(cached) = guard.as_ref() {
            return cached.clone();
        }

        let has_gst_launch = tokio::process::Command::new("gst-launch-1.0")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success());
        if !has_gst_launch {
            return Vec::new();
        }

        let mut list = Vec::new();
        for kind in [GstEncoderKind::NvEnc, GstEncoderKind::OpenH264] {
            let ok = tokio::process::Command::new("gst-inspect-1.0")
                .arg(kind.element_name())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .is_ok_and(|s| s.success());
            if ok {
                list.push(kind);
            }
        }
        if !list.is_empty() {
            *guard = Some(list.clone());
        }
        list
    }

    async fn spawn_gst_pipeline(
        cfg: ScreenStart,
        pw_source_arg: &str,
        source_size: Option<(u16, u16)>,
        monitor_index: u8,
        monitor_count: u8,
        available_monitors: u8,
        pw_fd: OwnedFd,
        encoder: GstEncoderKind,
    ) -> Result<CaptureSession> {
        let (mut pipe_read, pipe_write) = create_cloexec_pipe()?;
        let args = build_gst_pipeline_args(
            cfg,
            pw_source_arg,
            source_size,
            CHILD_PW_FD,
            CHILD_OUT_FD,
            encoder,
        );

        let pw_raw = pw_fd.as_raw_fd();
        let out_raw = pipe_write.as_raw_fd();

        let mut cmd = Command::new("gst-launch-1.0");
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        // Remapeia os FDs no processo filho sem `FD_CLOEXEC` antes do `exec`.
        unsafe {
            cmd.pre_exec(move || remap_child_fds(pw_raw, out_raw));
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Falha ao executar gst-launch-1.0 ({})", encoder.element_name()))?;

        // Fecha cópias do pai imediatamente para detectar EOF real caso o GStreamer encerre.
        drop(pw_fd);
        drop(pipe_write);

        let stderr_pipe = child.stderr.take();
        let stderr_log = Arc::new(StdMutex::new(String::new()));
        let stderr_log_clone = stderr_log.clone();
        let stderr_task = tokio::spawn(async move {
            let Some(mut stderr) = stderr_pipe else {
                return;
            };
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]);
                        debug!("gst-launch-1.0 ({}) stderr: {}", encoder.element_name(), chunk.trim());
                        if let Ok(mut guard) = stderr_log_clone.lock() {
                            if guard.len() < 4096 {
                                guard.push_str(&chunk);
                            }
                        }
                    }
                }
            }
        });

        let queue = Arc::new(ScreenFrameQueue::new());
        let failure = Arc::new(StdMutex::new(None));
        let (ready_tx, ready_rx) = oneshot::channel::<Result<NegotiatedStreamInfo>>();

        let queue_clone = queue.clone();
        let failure_clone = failure.clone();
        let stderr_for_reader = stderr_log.clone();
        let encoder_name = encoder.element_name().to_string();

        let reader_task = tokio::task::spawn_blocking(move || {
            let mut ready_tx = Some(ready_tx);
            let mut reader = GdpPacketReader::new();
            let mut sps_cache: Option<Vec<u8>> = None;
            let mut pps_cache: Option<Vec<u8>> = None;
            let mut negotiated_dim: Option<(u16, u16)> = None;
            let mut base_pts_ns: Option<u64> = None;
            let started_at = Instant::now();
            let mut buf = vec![0u8; 64 * 1024];

            let end_reason: String = loop {
                match pipe_read.read(&mut buf) {
                    Ok(0) => {
                        let stderr_tail = stderr_for_reader
                            .lock()
                            .map(|s| s.trim().to_string())
                            .unwrap_or_default();
                        break if stderr_tail.is_empty() {
                            format!("Pipeline {} encerrado sem saída", encoder_name)
                        } else {
                            format!("Pipeline {} falhou: {}", encoder_name, stderr_tail)
                        };
                    }
                    Ok(n) => {
                        let packets = match reader.push(&buf[..n]) {
                            Ok(pkts) => pkts,
                            Err(err) => break format!("Erro no fluxo GDP ({encoder_name}): {err:#}"),
                        };
                        for pkt in packets {
                            match pkt {
                                GdpPacket::Caps(caps) => {
                                    if let Some(dim) = parse_gdp_caps_dimensions(&caps) {
                                        negotiated_dim = Some(dim);
                                    }
                                }
                                GdpPacket::Buffer { pts_ns, payload } => {
                                    let Some(au) =
                                        normalize_annex_b_au(payload, &mut sps_cache, &mut pps_cache)
                                    else {
                                        continue;
                                    };
                                    if negotiated_dim.is_none() {
                                        negotiated_dim = au.sps_dimensions;
                                    }
                                    let pts_ms = compute_pts_ms(pts_ns, &mut base_pts_ns, started_at);
                                    let should_signal_ready = ready_tx.is_some();

                                    // Aguarda o primeiro keyframe antes de sinalizar Ready para o cliente.
                                    if should_signal_ready && !au.keyframe {
                                        continue;
                                    }
                                    if let Some(tx) = ready_tx.take() {
                                        let (width, height) =
                                            negotiated_dim.unwrap_or((cfg.max_width, cfg.max_height));
                                        let _ = tx.send(Ok(NegotiatedStreamInfo {
                                            width,
                                            height,
                                            fps: cfg.fps,
                                            encoder: encoder_name.clone(),
                                            monitor_index,
                                            monitor_count,
                                            available_monitors,
                                        }));
                                    }
                                    queue_clone.push(pts_ms, au.keyframe, au.data);
                                }
                                GdpPacket::Other => {}
                            }
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(err) => break format!("Erro lendo pipe GStreamer: {err}"),
                }
            };

            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(Err(anyhow::anyhow!("{end_reason}")));
            }
            if let Ok(mut g) = failure_clone.lock() {
                *g = Some(end_reason);
            }
            queue_clone.close();
        });

        match tokio::time::timeout(GST_STARTUP_TIMEOUT, ready_rx).await {
            Ok(Ok(Ok(info))) => {
                info!(
                    encoder = %info.encoder,
                    width = info.width,
                    height = info.height,
                    fps = info.fps,
                    "Pipeline de streaming de tela ativo"
                );
                Ok(CaptureSession {
                    info,
                    queue,
                    failure,
                    child: Some(child),
                    reader_task: Some(reader_task),
                    stderr_task: Some(stderr_task),
                })
            }
            Ok(Ok(Err(err))) => {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
                let _ = tokio::time::timeout(Duration::from_secs(1), reader_task).await;
                let _ = tokio::time::timeout(Duration::from_secs(1), stderr_task).await;
                Err(err)
            }
            Ok(Err(_)) => {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
                let _ = tokio::time::timeout(Duration::from_secs(1), stderr_task).await;
                bail!("Canal de inicialização do GStreamer encerrado")
            }
            Err(_) => {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
                let _ = tokio::time::timeout(Duration::from_secs(1), reader_task).await;
                let _ = tokio::time::timeout(Duration::from_secs(1), stderr_task).await;
                bail!(
                    "Timeout aguardando primeiro keyframe de {} ({}s)",
                    encoder.element_name(),
                    GST_STARTUP_TIMEOUT.as_secs()
                )
            }
        }
    }

    fn compute_pts_ms(pts_ns: u64, base_pts_ns: &mut Option<u64>, started_at: Instant) -> u64 {
        if pts_ns == u64::MAX {
            return started_at.elapsed().as_millis() as u64;
        }
        let base = *base_pts_ns.get_or_insert(pts_ns);
        pts_ns.saturating_sub(base) / 1_000_000
    }

    fn create_cloexec_pipe() -> Result<(std::fs::File, OwnedFd)> {
        let mut fds: [libc::c_int; 2] = [0; 2];
        let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error()).context("Falha ao criar pipe POSIX");
        }
        let read_file = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        let write_fd = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        Ok((read_file, write_fd))
    }

    unsafe fn remap_child_fds(pw_raw: libc::c_int, mut out_raw: libc::c_int) -> std::io::Result<()> {
        let mut temp_out: Option<libc::c_int> = None;
        if out_raw == CHILD_PW_FD {
            let moved = unsafe { libc::fcntl(out_raw, libc::F_DUPFD_CLOEXEC, CHILD_OUT_FD + 1) };
            if moved < 0 {
                return Err(std::io::Error::last_os_error());
            }
            out_raw = moved;
            temp_out = Some(moved);
        }

        unsafe {
            dup_without_cloexec(pw_raw, CHILD_PW_FD)?;
            dup_without_cloexec(out_raw, CHILD_OUT_FD)?;
            if let Some(fd) = temp_out {
                libc::close(fd);
            }
        }
        Ok(())
    }

    unsafe fn dup_without_cloexec(src: libc::c_int, dst: libc::c_int) -> std::io::Result<()> {
        if src == dst {
            if unsafe { libc::fcntl(dst, libc::F_SETFD, 0) } < 0 {
                return Err(std::io::Error::last_os_error());
            }
        } else if unsafe { libc::dup2(src, dst) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod stub {
    use super::{NegotiatedStreamInfo, ScreenFrameQueue};
    use anyhow::{Result, bail};
    use oraculo_proto::ScreenStart;
    use std::path::PathBuf;
    use std::sync::Arc;

    pub struct StubScreenBackend;

    pub struct CaptureSession {
        pub info: NegotiatedStreamInfo,
        pub queue: Arc<ScreenFrameQueue>,
    }

    impl CaptureSession {
        pub fn take_failure(&self) -> Option<String> {
            None
        }

        pub async fn stop(&mut self) {}
    }

    pub struct PendingReauth;

    pub async fn authorize_monitors_interactive(_token_file: &std::path::Path) -> Result<()> {
        bail!("Pré-seleção de monitores disponível apenas para Linux/Wayland")
    }

    impl StubScreenBackend {
        pub fn new(_token_file: PathBuf) -> Self {
            Self
        }

        pub async fn start_capture(&self, _cfg: ScreenStart) -> Result<CaptureSession> {
            bail!(
                "Captura de tela ainda implementada apenas para Linux/Wayland (backend Windows DXGI planejado)"
            )
        }

        pub async fn switch_monitor(
            &self,
            _cfg: ScreenStart,
            _active_capture: &mut CaptureSession,
        ) -> Result<bool> {
            bail!("Alternância de monitor disponível apenas para Linux/Wayland")
        }

        pub fn begin_reauthorize(
            &self,
        ) -> Option<tokio::sync::oneshot::Receiver<Result<PendingReauth>>> {
            None
        }

        pub async fn apply_reauthorized(
            &self,
            _cfg: ScreenStart,
            _new_portal: PendingReauth,
            _active_capture: &mut CaptureSession,
        ) -> Result<Option<String>> {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oraculo_proto::ScreenPreset;

    fn make_gdp_packet(ptype: u16, pts_ns: u64, payload: &[u8]) -> Vec<u8> {
        let mut pkt = vec![0u8; 62 + payload.len()];
        pkt[0] = 1;
        pkt[1] = 0;
        pkt[2..4].copy_from_slice(&1u16.to_be_bytes());
        pkt[4..6].copy_from_slice(&ptype.to_be_bytes());
        pkt[6..10].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        pkt[10..18].copy_from_slice(&pts_ns.to_be_bytes());
        pkt[62..].copy_from_slice(payload);
        pkt
    }

    #[test]
    fn gdp_reader_handles_fragmented_caps_and_buffers() {
        let caps_pkt = make_gdp_packet(
            2,
            0,
            b"video/x-h264, width=(int)1152, height=(int)720, framerate=(fraction)30/1\0",
        );
        let buf_pkt = make_gdp_packet(1, 33_000_000, &[0, 0, 0, 1, 0x65, 0x88]);
        let mut wire = Vec::new();
        wire.extend_from_slice(&caps_pkt);
        wire.extend_from_slice(&buf_pkt);

        let mut reader = GdpPacketReader::new();
        let first = reader.push(&wire[..70]).unwrap();
        assert!(first.is_empty());

        let second = reader.push(&wire[70..]).unwrap();
        assert_eq!(second.len(), 2);

        match &second[0] {
            GdpPacket::Caps(s) => assert_eq!(parse_gdp_caps_dimensions(s), Some((1152, 720))),
            other => panic!("Esperava Caps, obteve {other:?}"),
        }
        assert_eq!(
            second[1],
            GdpPacket::Buffer {
                pts_ns: 33_000_000,
                payload: vec![0, 0, 0, 1, 0x65, 0x88],
            }
        );
    }

    #[test]
    fn parses_real_sps_dimensions_including_1080p_crop() {
        // Real SPS captured from nvh264enc 1152x720
        let nv_1152 = [
            0x67, 0x42, 0xc0, 0x1f, 0x95, 0x90, 0x04, 0x80, 0x5b, 0xb0, 0x16, 0xa0, 0x20, 0x20,
            0x28, 0x00, 0x00, 0x1f, 0x40, 0x00, 0x07, 0x53, 0x04, 0x20,
        ];
        assert_eq!(parse_sps_dimensions(&nv_1152), Some((1152, 720)));

        // Real SPS captured from openh264enc 1920x1080 (with 1088 -> 1080 bottom crop)
        let o264_1080 = [
            0x67, 0x42, 0xc0, 0x2a, 0x8c, 0x68, 0x07, 0x80, 0x22, 0x7e, 0x58, 0x07, 0x84, 0x42,
            0x35,
        ];
        assert_eq!(parse_sps_dimensions(&o264_1080), Some((1920, 1080)));
    }

    #[test]
    fn normalize_au_prepends_cached_sps_pps_to_idr_when_missing() {
        let mut sps_cache = None;
        let mut pps_cache = None;

        let init_au = vec![
            0, 0, 0, 1, 0x09, 0x10, // AUD
            0, 0, 0, 1, 0x67, 0x42, 0xc0, 0x1f, // SPS
            0, 0, 0, 1, 0x68, 0xcb, // PPS
            0, 0, 0, 1, 0x65, 0xb8, // IDR
        ];
        let parsed0 = normalize_annex_b_au(init_au.clone(), &mut sps_cache, &mut pps_cache).unwrap();
        assert!(parsed0.keyframe);
        assert_eq!(parsed0.data, init_au);

        // Delta frame
        let delta_au = vec![0, 0, 0, 1, 0x61, 0xe0];
        let parsed1 = normalize_annex_b_au(delta_au.clone(), &mut sps_cache, &mut pps_cache).unwrap();
        assert!(!parsed1.keyframe);
        assert_eq!(parsed1.data, delta_au);

        // IDR arriving alone must recover cached SPS + PPS in front
        let bare_idr = vec![0, 0, 0, 1, 0x65, 0x99];
        let parsed2 = normalize_annex_b_au(bare_idr, &mut sps_cache, &mut pps_cache).unwrap();
        assert!(parsed2.keyframe);
        assert_eq!(
            parsed2.data,
            vec![0, 0, 0, 1, 0x67, 0x42, 0xc0, 0x1f, 0, 0, 0, 1, 0x68, 0xcb, 0, 0, 0, 1, 0x65, 0x99]
        );
    }

    #[tokio::test]
    async fn frame_queue_drops_stale_backlog_on_saturation_or_new_keyframe() {
        let queue = ScreenFrameQueue::new();

        // Deltas before any keyframe are dropped cleanly
        queue.push(10, false, vec![1]);
        assert_eq!(queue.dropped_frames(), 1);

        // First keyframe unblocks stream and resets seq
        queue.push(20, true, vec![10]);
        for i in 1..MAX_PENDING_DELTAS {
            queue.push(20 + i as u64, false, vec![i as u8]);
        }

        // Next delta saturates queue -> clears backlog and waits for next IDR
        queue.push(99, false, vec![99]);
        assert!(queue.dropped_frames() > 1);

        // Orphan delta during wait is ignored
        queue.push(100, false, vec![100]);

        // New keyframe clears any queued frames and emits immediately
        queue.push(120, true, vec![42]);
        let item = queue.pop_wait().await.unwrap();
        assert!(item.keyframe);
        assert_eq!(item.pts_ms, 120);
        assert_eq!(item.data, vec![42]);
    }

    #[test]
    fn compute_target_dimensions_preserves_aspect_and_avoids_range_collapse() {
        let p720 = ScreenStart::from_preset(ScreenPreset::Balanced720p30);
        let p1080_30 = ScreenStart::from_preset(ScreenPreset::Crisp1080p30);
        let p1080 = ScreenStart::from_preset(ScreenPreset::Smooth1080p60);

        assert_eq!(compute_target_dimensions(Some((1920, 1080)), p720), (1280, 720));
        assert_eq!(compute_target_dimensions(Some((1920, 1080)), p1080_30), (1920, 1080));
        assert_eq!(compute_target_dimensions(Some((1920, 1080)), p1080), (1920, 1080));
        assert_eq!(compute_target_dimensions(Some((1920, 1200)), p720), (1152, 720));
        assert_eq!(compute_target_dimensions(Some((2560, 1080)), p720), (1280, 540));
        assert_eq!(compute_target_dimensions(Some((800, 600)), p720), (800, 600));
        assert_eq!(compute_target_dimensions(Some((1080, 1920)), p720), (404, 720));
        assert_eq!(compute_target_dimensions(None, p720), (1280, 720));
    }

    #[test]
    fn gst_pipeline_args_cover_nvenc_and_openh264_units() {
        let cfg = ScreenStart::from_preset(ScreenPreset::Balanced720p30);
        let nv_scaled = build_gst_pipeline_args(
            cfg,
            "target-object=55",
            Some((1920, 1080)),
            3,
            4,
            GstEncoderKind::NvEnc,
        );
        assert!(nv_scaled.contains(&"video/x-raw,width=1280,height=720,pixel-aspect-ratio=1/1".to_string()));
        assert!(!nv_scaled.iter().any(|a| a.contains("[320,")));
        assert!(nv_scaled.contains(&"videoscale".to_string()));
        assert!(nv_scaled.contains(&"n-threads=0".to_string()));
        assert!(nv_scaled.contains(&"method=lanczos".to_string()));
        assert!(nv_scaled.contains(&"leaky=downstream".to_string()));
        assert!(nv_scaled.contains(&"preset=p2".to_string()));
        assert!(nv_scaled.contains(&"tune=ultra-low-latency".to_string()));
        assert!(nv_scaled.contains(&"spatial-aq=true".to_string()));
        assert!(nv_scaled.contains(&"aud=false".to_string()));
        assert!(nv_scaled.contains(&"bitrate=2000".to_string()));
        assert!(nv_scaled.contains(&"max-bitrate=3000".to_string()));
        assert!(nv_scaled.contains(&"vbv-buffer-size=666".to_string()));
        assert!(nv_scaled.contains(&"gop-size=60".to_string()));
        assert!(nv_scaled.contains(&"video/x-h264,profile=high,stream-format=byte-stream,alignment=au".to_string()));
        assert!(!nv_scaled.contains(&"h264parse".to_string()));
        assert!(nv_scaled.contains(&"fd=3".to_string()));
        assert!(nv_scaled.contains(&"fd=4".to_string()));
        assert!(nv_scaled.contains(&"gdppay".to_string()));

        // Em 1080p nativo com fonte 1920x1080, o pipeline evita `videoscale` em CPU.
        let cfg_1080 = ScreenStart::from_preset(ScreenPreset::Crisp1080p30);
        let nv_native = build_gst_pipeline_args(
            cfg_1080,
            "target-object=55",
            Some((1920, 1080)),
            3,
            4,
            GstEncoderKind::NvEnc,
        );
        assert!(!nv_native.contains(&"videoscale".to_string()));
        assert!(nv_native.contains(&"video/x-raw,width=1920,height=1080,pixel-aspect-ratio=1/1".to_string()));
        assert!(nv_native.contains(&"bitrate=5500".to_string()));
        assert!(nv_native.contains(&"max-bitrate=8250".to_string()));
        assert!(nv_native.contains(&"vbv-buffer-size=1833".to_string()));

        let sw = build_gst_pipeline_args(cfg, "path=77", None, 3, 4, GstEncoderKind::OpenH264);
        assert!(sw.contains(&"video/x-raw,width=1280,height=720,pixel-aspect-ratio=1/1".to_string()));
        assert!(sw.contains(&"videoscale".to_string()));
        assert!(sw.contains(&"h264parse".to_string()));
        assert!(sw.contains(&"bitrate=2000000".to_string()));
        assert!(sw.contains(&"usage-type=screen".to_string()));
        assert!(sw.contains(&"path=77".to_string()));
    }

    #[tokio::test]
    async fn claim_generation_updates_from_zero_receivers_and_detects_supersede() {
        let controller = ScreenController::new(std::env::temp_dir().join("oraculo_test_unused_tok"));

        let (gen1, mut rx1) = controller.claim_generation();
        assert_eq!(gen1, 1);
        assert_eq!(*rx1.borrow(), 1);

        let (gen2, rx2) = controller.claim_generation();
        assert_eq!(gen2, 2);
        assert_eq!(*rx2.borrow(), 2);

        rx1.changed().await.unwrap();
        assert_eq!(*rx1.borrow(), 2);
    }

    #[tokio::test]
    async fn iroh_screen_error_frame_arrives_before_finish() {
        use iroh::{Endpoint, EndpointAddr, endpoint::presets};
        use oraculo_proto::{StreamKind, read_stream_kind, write_stream_kind};
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};

        let ep_srv = Endpoint::builder(presets::N0)
            .alpns(vec![oraculo_proto::ALPN.to_vec()])
            .bind()
            .await
            .unwrap();
        let ep_cli = Endpoint::builder(presets::N0).bind().await.unwrap();
        let port = ep_srv
            .bound_sockets()
            .into_iter()
            .find(|a| a.is_ipv4())
            .expect("socket IPv4 local")
            .port();
        let srv_addr = EndpointAddr::new(ep_srv.id())
            .with_ip_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));

        let srv_task = tokio::spawn(async move {
            let inc = ep_srv.accept().await.unwrap();
            let conn = inc.await.unwrap();
            let (mut send, mut recv) = conn.accept_bi().await.unwrap();
            assert_eq!(read_stream_kind(&mut recv).await.unwrap(), StreamKind::Screen);
            let init: Option<ScreenStart> = read_frame(&mut recv).await.unwrap();
            assert!(init.is_some());
            write_frame(&mut send, &ScreenHostMsg::Error("falha simulada".into()))
                .await
                .unwrap();
            let _ = send.finish();
            (ep_srv, conn)
        });

        let cli_conn = ep_cli.connect(srv_addr, oraculo_proto::ALPN).await.unwrap();
        let (mut cli_send, mut cli_recv) = cli_conn.open_bi().await.unwrap();
        write_stream_kind(&mut cli_send, StreamKind::Screen).await.unwrap();
        write_frame(&mut cli_send, &ScreenStart::from_preset(ScreenPreset::Balanced720p30))
            .await
            .unwrap();

        let resp: Option<ScreenHostMsg> = read_frame(&mut cli_recv).await.unwrap();
        assert_eq!(resp, Some(ScreenHostMsg::Error("falha simulada".into())));

        drop(cli_send);
        drop(cli_recv);
        drop(cli_conn);
        let (ep_srv, srv_conn) = srv_task.await.unwrap();
        drop(srv_conn);
        ep_srv.close().await;
        ep_cli.close().await;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn multi_monitor_selection_advances_to_other_monitor_and_preserves_index_on_reopen() {
        let monitors = vec![
            linux::PortalMonitor {
                node_id: 42,
                position: (0, 0),
                pw_source_arg: "path=42".into(),
                source_size: Some((1920, 1080)),
            },
            linux::PortalMonitor {
                node_id: 88,
                position: (1920, 0),
                pw_source_arg: "path=88".into(),
                source_size: Some((2560, 1440)),
            },
        ];

        assert_eq!(linux::pick_reselected_monitor_index(Some(42), &monitors, true), 1);
        assert_eq!(linux::pick_reselected_monitor_index(Some(88), &monitors, true), 0);
        assert_eq!(linux::pick_reselected_monitor_index(Some(88), &monitors, false), 1);
        assert_eq!(linux::pick_reselected_monitor_index(Some(999), &monitors, true), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn drm_connector_active_checks_status_and_enabled_flags() {
        assert!(linux::is_drm_connector_active("connected\n", Some("enabled\n")));
        assert!(linux::is_drm_connector_active("connected", None));
        assert!(!linux::is_drm_connector_active("connected\n", Some("disabled\n")));
        assert!(!linux::is_drm_connector_active("disconnected\n", Some("enabled\n")));
    }
}
