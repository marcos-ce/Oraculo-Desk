use crate::config::HostSettings;
use crate::portal::{PortalCoordinator, PortalVaultPaths};
use anyhow::Result;
use iroh::endpoint::{RecvStream, SendStream};
use oraculo_proto::{
    InputEvent, InputInitAck, MouseButton, RemoteKey, read_frame, write_frame,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

pub struct InputController {
    #[cfg(target_os = "linux")]
    backend: linux::WaylandPortalBackend,
}

impl InputController {
    pub fn with_portal(portal: Arc<PortalCoordinator>) -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                backend: linux::WaylandPortalBackend::new(portal),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = portal;
            Self {}
        }
    }

    pub fn new(token_file: PathBuf) -> Self {
        Self::with_portal(Arc::new(PortalCoordinator::new(
            PortalVaultPaths::standalone_legacy(&token_file),
        )))
    }

    pub async fn ensure_ready(&self, allow_screen: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.backend.ensure_ready(allow_screen).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = allow_screen;
            anyhow::bail!("Controle remoto de mouse/teclado implementado apenas para Linux/Wayland")
        }
    }

    pub async fn dispatch(&self, event: InputEvent, allow_screen: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.backend.dispatch(event, allow_screen).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (event, allow_screen);
            anyhow::bail!("Plataforma sem suporte de entrada remota")
        }
    }
}

async fn wait_until_input_disabled(settings_rx: &mut watch::Receiver<HostSettings>) {
    loop {
        if !settings_rx.borrow().allow_input {
            return;
        }
        if settings_rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

fn coalesce_input_event(current: &mut InputEvent, next: InputEvent) -> Option<InputEvent> {
    match (current, next) {
        (InputEvent::Move { dx, dy }, InputEvent::Move { dx: nx, dy: ny })
        | (InputEvent::Scroll { dx, dy }, InputEvent::Scroll { dx: nx, dy: ny }) => {
            *dx += nx;
            *dy += ny;
            None
        }
        (_, other) => Some(other),
    }
}

pub async fn handle_input_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    controller: Arc<InputController>,
    mut settings_rx: watch::Receiver<HostSettings>,
) -> Result<()> {
    let _ = send.set_priority(100);
    if !settings_rx.borrow().allow_input {
        let _ = write_frame(
            &mut send,
            &InputInitAck {
                ok: false,
                message: "Controle de teclado e mouse desativado pelo host".to_string(),
            },
        )
        .await;
        let _ = send.finish();
        tokio::time::sleep(Duration::from_millis(60)).await;
        return Ok(());
    }

    let allow_screen = settings_rx.borrow().allow_screen;
    let ready_outcome = tokio::select! {
        biased;
        _ = wait_until_input_disabled(&mut settings_rx) => {
            Err(anyhow::anyhow!("Controle de teclado e mouse desativado pelo host"))
        }
        _ = send.stopped() => {
            return Ok(());
        }
        res = controller.ensure_ready(allow_screen) => res,
    };

    match ready_outcome {
        Ok(()) => {
            write_frame(
                &mut send,
                &InputInitAck {
                    ok: true,
                    message: "Touchpad e teclado ativos".to_string(),
                },
            )
            .await?;
            // Mantemos `send` aberto até o fim do loop para o cliente distinguir sessão ativa de encerramento no host.
        }
        Err(err) => {
            let _ = write_frame(
                &mut send,
                &InputInitAck {
                    ok: false,
                    message: format!("Falha ao iniciar controle remoto no PC: {err:#}"),
                },
            )
            .await;
            let _ = send.finish();
            tokio::time::sleep(Duration::from_millis(60)).await;
            return Err(err);
        }
    }

    info!("Sessão de entrada remota iniciada");
    let mut held_keys: HashSet<RemoteKey> = HashSet::new();
    let mut held_buttons: HashSet<MouseButton> = HashSet::new();

    // Desacopla leitura QUIC da injeção D-Bus: em rajadas após oscilações de rede fraca,
    // múltiplos `Move`/`Scroll` consecutivos são somados em uma única chamada ao portal.
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<InputEvent>();
    let reader_task = tokio::spawn(async move {
        loop {
            match read_frame::<_, InputEvent>(&mut recv).await {
                Ok(Some(ev)) => {
                    if ev_tx.send(ev).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    warn!("Leitura do canal de entrada encerrada: {err:#}");
                    break;
                }
            }
        }
    });

    let mut pending: Option<InputEvent> = None;

    loop {
        let mut event = match pending.take() {
            Some(ev) => ev,
            None => tokio::select! {
                biased;
                changed = settings_rx.changed() => {
                    if changed.is_ok() && !settings_rx.borrow().allow_input {
                        warn!("Controle de entrada remota desativado pelo host; encerrando sessão");
                        break;
                    }
                    continue;
                }
                maybe = ev_rx.recv() => match maybe {
                    Some(ev) => ev,
                    None => break,
                }
            },
        };

        while let Ok(queued) = ev_rx.try_recv() {
            if let Some(barrier) = coalesce_input_event(&mut event, queued) {
                pending = Some(barrier);
                break;
            }
        }

        if matches!(event, InputEvent::Move { dx, dy } | InputEvent::Scroll { dx, dy } if dx == 0.0 && dy == 0.0) {
            continue;
        }

        match &event {
            InputEvent::KeyState { key, pressed } => {
                if *pressed {
                    held_keys.insert(*key);
                } else {
                    held_keys.remove(key);
                }
            }
            InputEvent::Button { button, pressed } => {
                if *pressed {
                    held_buttons.insert(*button);
                } else {
                    held_buttons.remove(button);
                }
            }
            _ => {}
        }

        let current_allow_screen = settings_rx.borrow().allow_screen;
        if let Err(err) = controller.dispatch(event, current_allow_screen).await {
            warn!("Erro ao injetar entrada remota: {err:#}");
        }
    }

    reader_task.abort();

    // Garante que nenhuma tecla modificadora ou botão do mouse fique preso se a conexão cair.
    let current_allow_screen = settings_rx.borrow().allow_screen;
    for key in held_keys {
        let _ = controller
            .dispatch(
                InputEvent::KeyState {
                    key,
                    pressed: false,
                },
                current_allow_screen,
            )
            .await;
    }
    for button in held_buttons {
        let _ = controller
            .dispatch(
                InputEvent::Button {
                    button,
                    pressed: false,
                },
                current_allow_screen,
            )
            .await;
    }

    let _ = send.finish();
    info!("Sessão de entrada remota encerrada");
    Ok(())
}

fn mouse_button_code(button: MouseButton) -> i32 {
    match button {
        MouseButton::Left => 272,   // BTN_LEFT
        MouseButton::Right => 273,  // BTN_RIGHT
        MouseButton::Middle => 274, // BTN_MIDDLE
    }
}

fn remote_key_to_keysym(key: RemoteKey) -> i32 {
    match key {
        RemoteKey::Backspace => 0xff08,
        RemoteKey::Tab => 0xff09,
        RemoteKey::Enter => 0xff0d,
        RemoteKey::Escape => 0xff1b,
        RemoteKey::Home => 0xff50,
        RemoteKey::Left => 0xff51,
        RemoteKey::Up => 0xff52,
        RemoteKey::Right => 0xff53,
        RemoteKey::Down => 0xff54,
        RemoteKey::End => 0xff57,
        RemoteKey::F4 => 0xffc1,
        RemoteKey::F5 => 0xffc2,
        RemoteKey::F11 => 0xffc8,
        RemoteKey::Shift => 0xffe1,
        RemoteKey::Control => 0xffe3,
        RemoteKey::Alt => 0xffe9,
        RemoteKey::Super => 0xffeb,
        RemoteKey::Delete => 0xffff,
        RemoteKey::VolumeDown => 0x1008_ff11,
        RemoteKey::VolumeMute => 0x1008_ff12,
        RemoteKey::VolumeUp => 0x1008_ff13,
        RemoteKey::PlayPause => 0x1008_ff14,
        RemoteKey::MediaPrev => 0x1008_ff16,
        RemoteKey::MediaNext => 0x1008_ff17,
    }
}

fn char_to_direct_keysym(ch: char) -> Option<i32> {
    match ch {
        '\n' | '\r' => Some(0xff0d),
        '\t' => Some(0xff09),
        '\x08' => Some(0xff08),
        c if (' '..='~').contains(&c) => Some(c as i32),
        _ => None,
    }
}

fn dead_key_pair(ch: char) -> Option<(i32, char)> {
    const DEAD_GRAVE: i32 = 0xfe50;
    const DEAD_ACUTE: i32 = 0xfe51;
    const DEAD_CIRCUMFLEX: i32 = 0xfe52;
    const DEAD_TILDE: i32 = 0xfe53;
    const DEAD_DIAERESIS: i32 = 0xfe57;

    let pair = match ch {
        'á' => (DEAD_ACUTE, 'a'),
        'Á' => (DEAD_ACUTE, 'A'),
        'é' => (DEAD_ACUTE, 'e'),
        'É' => (DEAD_ACUTE, 'E'),
        'í' => (DEAD_ACUTE, 'i'),
        'Í' => (DEAD_ACUTE, 'I'),
        'ó' => (DEAD_ACUTE, 'o'),
        'Ó' => (DEAD_ACUTE, 'O'),
        'ú' => (DEAD_ACUTE, 'u'),
        'Ú' => (DEAD_ACUTE, 'U'),
        'ý' => (DEAD_ACUTE, 'y'),
        'Ý' => (DEAD_ACUTE, 'Y'),

        'à' => (DEAD_GRAVE, 'a'),
        'À' => (DEAD_GRAVE, 'A'),
        'è' => (DEAD_GRAVE, 'e'),
        'È' => (DEAD_GRAVE, 'E'),
        'ì' => (DEAD_GRAVE, 'i'),
        'Ì' => (DEAD_GRAVE, 'I'),
        'ò' => (DEAD_GRAVE, 'o'),
        'Ò' => (DEAD_GRAVE, 'O'),
        'ù' => (DEAD_GRAVE, 'u'),
        'Ù' => (DEAD_GRAVE, 'U'),

        'â' => (DEAD_CIRCUMFLEX, 'a'),
        'Â' => (DEAD_CIRCUMFLEX, 'A'),
        'ê' => (DEAD_CIRCUMFLEX, 'e'),
        'Ê' => (DEAD_CIRCUMFLEX, 'E'),
        'î' => (DEAD_CIRCUMFLEX, 'i'),
        'Î' => (DEAD_CIRCUMFLEX, 'I'),
        'ô' => (DEAD_CIRCUMFLEX, 'o'),
        'Ô' => (DEAD_CIRCUMFLEX, 'O'),
        'û' => (DEAD_CIRCUMFLEX, 'u'),
        'Û' => (DEAD_CIRCUMFLEX, 'U'),

        'ã' => (DEAD_TILDE, 'a'),
        'Ã' => (DEAD_TILDE, 'A'),
        'õ' => (DEAD_TILDE, 'o'),
        'Õ' => (DEAD_TILDE, 'O'),
        'ñ' => (DEAD_TILDE, 'n'),
        'Ñ' => (DEAD_TILDE, 'N'),

        'ä' => (DEAD_DIAERESIS, 'a'),
        'Ä' => (DEAD_DIAERESIS, 'A'),
        'ë' => (DEAD_DIAERESIS, 'e'),
        'Ë' => (DEAD_DIAERESIS, 'E'),
        'ï' => (DEAD_DIAERESIS, 'i'),
        'Ï' => (DEAD_DIAERESIS, 'I'),
        'ö' => (DEAD_DIAERESIS, 'o'),
        'Ö' => (DEAD_DIAERESIS, 'O'),
        'ü' => (DEAD_DIAERESIS, 'u'),
        'Ü' => (DEAD_DIAERESIS, 'U'),
        'ÿ' => (DEAD_DIAERESIS, 'y'),

        _ => return None,
    };
    Some(pair)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutCapabilities {
    has_dead_keys: bool,
    has_ccedilla: bool,
}

impl Default for LayoutCapabilities {
    fn default() -> Self {
        Self {
            has_dead_keys: false,
            has_ccedilla: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharInputPlan {
    Single(i32),
    Sequence(i32, i32),
    UnicodeHex(u32),
    Ignore,
}

fn plan_char_input(ch: char, caps: LayoutCapabilities) -> CharInputPlan {
    if let Some(sym) = char_to_direct_keysym(ch) {
        return CharInputPlan::Single(sym);
    }
    if caps.has_ccedilla
        && matches!(
            ch,
            'ç' | 'Ç' | 'ª' | 'º' | '°' | '§' | '¹' | '²' | '³' | '¢' | '£' | '¬' | 'µ'
        )
    {
        return CharInputPlan::Single(ch as i32);
    }
    if caps.has_dead_keys {
        if let Some((dead, base)) = dead_key_pair(ch) {
            return CharInputPlan::Sequence(dead, base as i32);
        }
    }
    if !ch.is_control() {
        return CharInputPlan::UnicodeHex(ch as u32);
    }
    CharInputPlan::Ignore
}

#[cfg(any(target_os = "linux", test))]
fn parse_gnome_layout_capabilities(raw: &str) -> LayoutCapabilities {
    let mut parts = raw.split("'xkb'");
    let _ = parts.next();
    let Some(after_xkb) = parts.next() else {
        return LayoutCapabilities::default();
    };
    let mut quotes = after_xkb.split('\'');
    let _ = quotes.next();
    let Some(layout) = quotes.next() else {
        return LayoutCapabilities::default();
    };

    if layout.contains("nodeadkeys") {
        return LayoutCapabilities::default();
    }

    let base = layout.split('+').next().unwrap_or(layout);
    let has_ccedilla = matches!(base, "br" | "pt" | "fr");
    let has_dead_keys =
        has_ccedilla || matches!(base, "es" | "de" | "it") || layout.contains("intl");

    LayoutCapabilities {
        has_dead_keys,
        has_ccedilla,
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{
        CharInputPlan, LayoutCapabilities, mouse_button_code, parse_gnome_layout_capabilities,
        plan_char_input, remote_key_to_keysym,
    };
    use crate::portal::{ActiveRemoteDesktop, PortalCoordinator};
    use anyhow::Result;
    use ashpd::desktop::remote_desktop::{KeyState, NotifyPointerAxisOptions};
    use oraculo_proto::InputEvent;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex;

    const CONTROL_L: i32 = 0xffe3;
    const SHIFT_L: i32 = 0xffe1;
    const RETURN_KEY: i32 = 0xff0d;
    const LAYOUT_CACHE_TTL: Duration = Duration::from_secs(2);

    pub struct WaylandPortalBackend {
        portal: Arc<PortalCoordinator>,
        layout_cache: Mutex<Option<(Instant, LayoutCapabilities)>>,
    }

    impl WaylandPortalBackend {
        pub fn new(portal: Arc<PortalCoordinator>) -> Self {
            Self {
                portal,
                layout_cache: Mutex::new(None),
            }
        }

        pub async fn ensure_ready(&self, allow_screen: bool) -> Result<()> {
            self.portal.ensure_remote_desktop(allow_screen).await.map(|_| ())
        }

        pub async fn dispatch(&self, event: InputEvent, allow_screen: bool) -> Result<()> {
            let rd = self.portal.ensure_remote_desktop(allow_screen).await?;
            if Self::send_event(&rd, &self.layout_cache, &event).await.is_ok() {
                return Ok(());
            }

            // Sessão pode ter expirado (ex.: bloqueio de tela); reabre uma vez e reenvia.
            let reopened = self.portal.reopen_remote_desktop(allow_screen, &rd).await?;
            Self::send_event(&reopened, &self.layout_cache, &event).await
        }

        async fn set_key(active: &ActiveRemoteDesktop, sym: i32, state: KeyState) -> Result<()> {
            active
                .proxy
                .notify_keyboard_keysym(&active.session, sym, state, Default::default())
                .await?;
            Ok(())
        }

        async fn tap_keysym(active: &ActiveRemoteDesktop, sym: i32) -> Result<()> {
            Self::set_key(active, sym, KeyState::Pressed).await?;
            Self::set_key(active, sym, KeyState::Released).await
        }

        async fn type_unicode_hex(active: &ActiveRemoteDesktop, codepoint: u32) -> Result<()> {
            Self::set_key(active, CONTROL_L, KeyState::Pressed).await?;
            Self::set_key(active, SHIFT_L, KeyState::Pressed).await?;
            Self::tap_keysym(active, b'u' as i32).await?;
            Self::set_key(active, SHIFT_L, KeyState::Released).await?;
            Self::set_key(active, CONTROL_L, KeyState::Released).await?;

            for byte in format!("{codepoint:x}").into_bytes() {
                Self::tap_keysym(active, byte as i32).await?;
            }
            Self::tap_keysym(active, RETURN_KEY).await
        }

        async fn active_layout_capabilities(
            layout_cache: &Mutex<Option<(Instant, LayoutCapabilities)>>,
        ) -> LayoutCapabilities {
            {
                let guard = layout_cache.lock().await;
                if let Some((cached_at, caps)) = *guard {
                    if cached_at.elapsed() < LAYOUT_CACHE_TTL {
                        return caps;
                    }
                }
            }

            let caps = detect_gnome_layout_capabilities().await;
            *layout_cache.lock().await = Some((Instant::now(), caps));
            caps
        }

        async fn send_event(
            active: &ActiveRemoteDesktop,
            layout_cache: &Mutex<Option<(Instant, LayoutCapabilities)>>,
            event: &InputEvent,
        ) -> Result<()> {
            match event {
                InputEvent::Move { dx, dy } => {
                    active
                        .proxy
                        .notify_pointer_motion(&active.session, *dx, *dy, Default::default())
                        .await?;
                }
                InputEvent::Button { button, pressed } => {
                    let state = if *pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    active
                        .proxy
                        .notify_pointer_button(
                            &active.session,
                            mouse_button_code(*button),
                            state,
                            Default::default(),
                        )
                        .await?;
                }
                InputEvent::Click { button } => {
                    let code = mouse_button_code(*button);
                    active
                        .proxy
                        .notify_pointer_button(
                            &active.session,
                            code,
                            KeyState::Pressed,
                            Default::default(),
                        )
                        .await?;
                    active
                        .proxy
                        .notify_pointer_button(
                            &active.session,
                            code,
                            KeyState::Released,
                            Default::default(),
                        )
                        .await?;
                }
                InputEvent::Scroll { dx, dy } => {
                    active
                        .proxy
                        .notify_pointer_axis(
                            &active.session,
                            *dx,
                            *dy,
                            NotifyPointerAxisOptions::default().set_finish(false),
                        )
                        .await?;
                }
                InputEvent::Key(key) => {
                    let sym = remote_key_to_keysym(*key);
                    Self::tap_keysym(active, sym).await?;
                }
                InputEvent::KeyState { key, pressed } => {
                    let sym = remote_key_to_keysym(*key);
                    let state = if *pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    Self::set_key(active, sym, state).await?;
                }
                InputEvent::Text(text) => {
                    let caps = if text.bytes().any(|b| b >= 0x80) {
                        Self::active_layout_capabilities(layout_cache).await
                    } else {
                        LayoutCapabilities::default()
                    };

                    for ch in text.chars() {
                        match plan_char_input(ch, caps) {
                            CharInputPlan::Single(sym) => Self::tap_keysym(active, sym).await?,
                            CharInputPlan::Sequence(dead, base) => {
                                Self::tap_keysym(active, dead).await?;
                                Self::tap_keysym(active, base).await?;
                            }
                            CharInputPlan::UnicodeHex(cp) => {
                                Self::type_unicode_hex(active, cp).await?;
                            }
                            CharInputPlan::Ignore => {}
                        }
                    }
                }
            }
            Ok(())
        }
    }

    async fn detect_gnome_layout_capabilities() -> LayoutCapabilities {
        for key in ["mru-sources", "sources"] {
            if let Ok(out) = tokio::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.input-sources", key])
                .output()
                .await
            {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    if stdout.contains("'xkb'") {
                        return parse_gnome_layout_capabilities(&stdout);
                    }
                }
            }
        }
        LayoutCapabilities::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_mouse_and_keysyms() {
        assert_eq!(mouse_button_code(MouseButton::Left), 272);
        assert_eq!(mouse_button_code(MouseButton::Right), 273);
        assert_eq!(mouse_button_code(MouseButton::Middle), 274);
        assert_eq!(remote_key_to_keysym(RemoteKey::Backspace), 0xff08);
        assert_eq!(remote_key_to_keysym(RemoteKey::Control), 0xffe3);
        assert_eq!(remote_key_to_keysym(RemoteKey::Alt), 0xffe9);
        assert_eq!(remote_key_to_keysym(RemoteKey::Shift), 0xffe1);
        assert_eq!(remote_key_to_keysym(RemoteKey::Super), 0xffeb);
        assert_eq!(remote_key_to_keysym(RemoteKey::PlayPause), 0x1008_ff14);

        let br = parse_gnome_layout_capabilities("[('xkb', 'br'), ('xkb', 'us')]");
        assert!(br.has_dead_keys);
        assert!(br.has_ccedilla);
        assert_eq!(plan_char_input('a', br), CharInputPlan::Single('a' as i32));
        assert_eq!(plan_char_input('\n', br), CharInputPlan::Single(0xff0d));
        assert_eq!(plan_char_input('ç', br), CharInputPlan::Single(0x00e7));
        assert_eq!(
            plan_char_input('ã', br),
            CharInputPlan::Sequence(0xfe53, 'a' as i32)
        );
        assert_eq!(
            plan_char_input('é', br),
            CharInputPlan::Sequence(0xfe51, 'e' as i32)
        );

        let us = parse_gnome_layout_capabilities("[('xkb', 'us'), ('xkb', 'br')]");
        assert!(!us.has_dead_keys);
        assert!(!us.has_ccedilla);
        assert_eq!(plan_char_input('ã', us), CharInputPlan::UnicodeHex(0x00e3));
        assert_eq!(plan_char_input('ç', us), CharInputPlan::UnicodeHex(0x00e7));
    }

    #[test]
    fn coalesces_consecutive_move_and_scroll_without_reordering_clicks() {
        let mut cur = InputEvent::Move { dx: 3.5, dy: -1.0 };
        assert!(coalesce_input_event(&mut cur, InputEvent::Move { dx: 2.5, dy: 4.0 }).is_none());
        assert!(matches!(cur, InputEvent::Move { dx, dy } if (dx - 6.0).abs() < 1e-9 && (dy - 3.0).abs() < 1e-9));

        let barrier = coalesce_input_event(
            &mut cur,
            InputEvent::Click {
                button: MouseButton::Left,
            },
        );
        assert!(matches!(
            barrier,
            Some(InputEvent::Click {
                button: MouseButton::Left
            })
        ));
        assert!(matches!(cur, InputEvent::Move { dx, dy } if (dx - 6.0).abs() < 1e-9 && (dy - 3.0).abs() < 1e-9));

        let mut scroll = InputEvent::Scroll { dx: 0.0, dy: 1.5 };
        assert!(coalesce_input_event(&mut scroll, InputEvent::Scroll { dx: -0.5, dy: 2.5 }).is_none());
        assert!(matches!(scroll, InputEvent::Scroll { dx, dy } if (dx - (-0.5)).abs() < 1e-9 && (dy - 4.0).abs() < 1e-9));
    }
}
