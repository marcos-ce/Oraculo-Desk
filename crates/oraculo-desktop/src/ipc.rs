use anyhow::{Context, Result};
#[cfg(not(target_os = "linux"))]
use anyhow::bail;
use directories::BaseDirs;
use iroh::SecretKey;
use oraculo_host::config::HostSettings;
use oraculo_host::control_ipc::{
    DeleteCommandDto, HostBackend, HttpHeadError, SaveCommandDto, SettingsPatchDto, read_http_head,
};
use oraculo_host::portal::{PORTAL_APP_ID, PortalReadiness, ensure_portal_app_registered};
use oraculo_host::tunnel::{RdpStatus, effective_rdp_port, harden_rdp_firewall};
use oraculo_host::wol::{discover_nics, enable_nm_magic_packet, inspect_nm_wol_profiles};
use oraculo_proto::QuickCommandInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const UI_TEMPLATE: &str = include_str!("ui.html");
const FONT_GOOGLE_SANS_LATIN: &[u8] = include_bytes!("../assets/google-sans-latin.woff2");
const FONT_GOOGLE_SANS_LATIN_EXT: &[u8] = include_bytes!("../assets/google-sans-latin-ext.woff2");
const FONT_GOOGLE_SANS_CODE_LATIN: &[u8] =
    include_bytes!("../assets/google-sans-code-latin.woff2");
const MAX_HTTP_BODY_BYTES: usize = 64 * 1024;

struct CachedWolDiag {
    captured_at: Instant,
    nics: Vec<NicDto>,
    nm_profiles: Vec<NmProfileDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UiPrefs {
    theme: String,
    lang: String,
    #[serde(default)]
    autostart_default_applied: bool,
    #[serde(default)]
    bg_notice_shown: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            lang: oraculo_proto::i18n::UiLang::from_system_env()
                .as_code()
                .to_string(),
            autostart_default_applied: false,
            bg_notice_shown: false,
        }
    }
}

pub fn persist_bg_notice_shown(paths: &oraculo_host::config::HostPaths) {
    let path = ui_prefs_path(&paths.settings_file);
    let mut prefs = load_ui_prefs(&path);
    if !prefs.bg_notice_shown {
        prefs.bg_notice_shown = true;
        let _ = save_ui_prefs(&path, &prefs);
    }
}

fn ui_prefs_path(settings_file: &Path) -> PathBuf {
    settings_file.with_file_name("ui_prefs.json")
}

fn sanitize_theme(raw: &str) -> String {
    match raw {
        "light" => "light".to_string(),
        "auto" => "auto".to_string(),
        _ => "dark".to_string(),
    }
}

fn sanitize_lang(raw: &str) -> String {
    if raw == "en" {
        "en".to_string()
    } else {
        "pt".to_string()
    }
}

fn load_ui_prefs(path: &Path) -> UiPrefs {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return UiPrefs::default();
    };
    match serde_json::from_str::<UiPrefs>(&raw) {
        Ok(p) => UiPrefs {
            theme: sanitize_theme(&p.theme),
            lang: sanitize_lang(&p.lang),
            autostart_default_applied: p.autostart_default_applied,
            bg_notice_shown: p.bg_notice_shown,
        },
        Err(_) => UiPrefs::default(),
    }
}

struct CachedSystemStatus {
    captured_at: Instant,
    autostart_enabled: bool,
    linger_enabled: bool,
}

fn save_ui_prefs(path: &Path, prefs: &UiPrefs) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(prefs)?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn ensure_default_autostart_with<F>(
    prefs: &mut UiPrefs,
    prefs_file: &Path,
    already_enabled: bool,
    enable_fn: F,
) -> bool
where
    F: FnOnce() -> Result<()>,
{
    if prefs.autostart_default_applied {
        return false;
    }
    let ready = if already_enabled {
        true
    } else {
        enable_fn().is_ok()
    };
    if ready {
        prefs.autostart_default_applied = true;
        let _ = save_ui_prefs(prefs_file, prefs);
    }
    ready
}

struct IpcContext {
    port: u16,
    token: String,
    host: HostBackend,
    wol_cache: Mutex<Option<CachedWolDiag>>,
    sys_cache: Mutex<Option<CachedSystemStatus>>,
    ui_prefs: Mutex<UiPrefs>,
    ui_prefs_file: PathBuf,
}

#[derive(Serialize)]
struct StateResponse {
    host_name: String,
    endpoint_id: String,
    relay_url: Option<String>,
    peers: Vec<PeerDto>,
    pairing: Option<PairingDto>,
    settings: HostSettings,
    wol: WolDiagDto,
    commands: Vec<QuickCommandInfo>,
    portal: PortalReadiness,
    rdp: RdpStatus,
    autostart_enabled: bool,
    linger_enabled: bool,
    daemon_mode: &'static str,
    paths: PathsDto,
}

#[derive(Serialize)]
struct PeerDto {
    endpoint_id: String,
    name: String,
    paired_at_unix: u64,
    connected: bool,
}

#[derive(Serialize)]
struct PairingDto {
    short_code: String,
    uri: String,
    qr_svg: String,
    remaining_secs: u64,
}

#[derive(Clone, Serialize)]
struct NicDto {
    name: String,
    mac: String,
    perm_mac: Option<String>,
    broadcast_v4: Option<String>,
    is_wireless: bool,
    is_up: bool,
}

#[derive(Clone, Serialize)]
struct NmProfileDto {
    name: String,
    conn_type: String,
    device: Option<String>,
    magic_enabled: bool,
}

#[derive(Serialize)]
struct WolDiagDto {
    nics: Vec<NicDto>,
    nm_profiles: Vec<NmProfileDto>,
    all_magic_ready: bool,
}

#[derive(Serialize)]
struct PathsDto {
    settings_file: String,
    commands_file: String,
    peers_file: String,
}

#[derive(Deserialize)]
struct RevokeReq {
    endpoint_id: String,
}

#[derive(Deserialize)]
struct AutostartReq {
    enabled: bool,
}

#[derive(Deserialize)]
struct ThemeSyncReq {
    mode: Option<String>,
    lang: Option<String>,
}

/// Inicia o servidor IPC local autenticado (`127.0.0.1:0`) e retorna a URL inicial da janela nativa.
pub async fn start_ipc_server(host: HostBackend, start_hidden: bool) -> Result<String> {
    let prefs_file = ui_prefs_path(&host.paths().settings_file);
    let mut initial_prefs = load_ui_prefs(&prefs_file);
    crate::webview::set_ui_lang(&initial_prefs.lang);
    crate::webview::set_bg_notice_shown(initial_prefs.bg_notice_shown);
    let theme_for_boot = initial_prefs.theme.clone();
    let _ = tokio::task::spawn_blocking(move || crate::webview::sync_window_theme(&theme_for_boot))
        .await;

    let prefs_file_bg = prefs_file.clone();
    let prefs_after_boot = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        sync_desktop_integration_on_startup();

        ensure_default_autostart_with(
            &mut initial_prefs,
            &prefs_file_bg,
            is_autostart_enabled(),
            || set_autostart_enabled(true),
        );
        initial_prefs
    })
    .await
    .unwrap_or_else(|_| load_ui_prefs(&prefs_file));

    #[cfg(target_os = "linux")]
    ensure_portal_app_registered().await;

    if !start_hidden {
        if let Ok(snap) = host.snapshot().await {
            if snap.trusted_peers.is_empty() && snap.pairing.is_none() {
                let _ = host.issue_pairing().await;
            }
        }
    }

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("Falha ao abrir canal IPC local em 127.0.0.1")?;
    let port = listener.local_addr()?.port();
    let token = hex::encode(SecretKey::generate().to_bytes());

    let ctx = Arc::new(IpcContext {
        port,
        token: token.clone(),
        host,
        wol_cache: Mutex::new(None),
        sys_cache: Mutex::new(None),
        ui_prefs: Mutex::new(prefs_after_boot),
        ui_prefs_file: prefs_file,
    });

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let ctx_clone = Arc::clone(&ctx);
                    tokio::spawn(async move {
                        let _ = handle_http_conn(stream, ctx_clone).await;
                    });
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    });

    Ok(format!("http://127.0.0.1:{port}/?t={token}"))
}

async fn handle_http_conn(stream: TcpStream, ctx: Arc<IpcContext>) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let head = match read_http_head(&mut reader, "x-oraculo-token").await {
        Ok(h) => h,
        Err(HttpHeadError::Empty) => return Ok(()),
        Err(HttpHeadError::HeadersTooLarge) => {
            return write_http_response(
                reader.into_inner(),
                431,
                "text/plain; charset=utf-8",
                b"Header fields too large",
            )
            .await;
        }
        Err(HttpHeadError::BadRequest) => {
            return write_http_response(
                reader.into_inner(),
                400,
                "text/plain; charset=utf-8",
                b"Bad request",
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

    // Bloqueia ataques de DNS rebinding exigindo Host literal de loopback na porta exata.
    let expected_host = format!("127.0.0.1:{}", ctx.port);
    if host_hdr != expected_host {
        return write_http_response(
            reader.into_inner(),
            403,
            "text/plain; charset=utf-8",
            b"Forbidden host",
        )
        .await;
    }

    if content_len > MAX_HTTP_BODY_BYTES {
        return write_http_response(
            reader.into_inner(),
            413,
            "text/plain; charset=utf-8",
            b"Payload too large",
        )
        .await;
    }

    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        tokio::time::timeout(Duration::from_secs(8), reader.read_exact(&mut body))
            .await
            .context("Timeout lendo corpo HTTP")??;
    }
    let stream = reader.into_inner();

    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    if method == "GET" && (path == "/" || path.starts_with("/assets/")) {
        let query_tok = query
            .split('&')
            .find_map(|kv| kv.strip_prefix("t="))
            .unwrap_or_default();
        if !constant_time_eq(query_tok.as_bytes(), ctx.token.as_bytes()) {
            return write_http_response(stream, 403, "text/plain; charset=utf-8", b"Unauthorized")
                .await;
        }
        if path != "/" {
            let font_bytes = match path {
                "/assets/google-sans-latin.woff2" => FONT_GOOGLE_SANS_LATIN,
                "/assets/google-sans-latin-ext.woff2" => FONT_GOOGLE_SANS_LATIN_EXT,
                "/assets/google-sans-code-latin.woff2" => FONT_GOOGLE_SANS_CODE_LATIN,
                _ => {
                    return write_http_response(stream, 404, "text/plain; charset=utf-8", b"Not found")
                        .await;
                }
            };
            return write_http_response(stream, 200, "font/woff2", font_bytes).await;
        }
        if let Ok(snap) = ctx.host.snapshot().await {
            if snap.trusted_peers.is_empty() && snap.pairing.is_none() {
                let _ = ctx.host.issue_pairing().await;
            }
        }
        let prefs_snapshot = ctx.ui_prefs.lock().await.clone();
        let theme_for_html = prefs_snapshot.theme.clone();
        let (initial_dark_opt, win_controls) = tokio::task::spawn_blocking(move || {
            (
                crate::webview::sync_window_theme(&theme_for_html),
                crate::webview::overlay_right_controls_count(),
            )
        })
        .await
        .unwrap_or((None, 1));
        let initial_effective = if prefs_snapshot.theme == "auto" {
            match initial_dark_opt {
                Some(true) => "dark",
                Some(false) => "light",
                None => "auto",
            }
        } else {
            prefs_snapshot.theme.as_str()
        };
        let chrome_mode = if crate::webview::uses_overlay_controls() {
            "overlay"
        } else {
            "windowed"
        };
        let rendered = UI_TEMPLATE
            .replace("__ORACULO_SESSION_TOKEN__", &ctx.token)
            .replace("__ORACULO_INIT_THEME__", &prefs_snapshot.theme)
            .replace("__ORACULO_INIT_EFFECTIVE_THEME__", initial_effective)
            .replace("__ORACULO_INIT_LANG__", &prefs_snapshot.lang)
            .replace("__ORACULO_CHROME__", chrome_mode)
            .replace("__ORACULO_WIN_CONTROLS__", &win_controls.to_string());
        return write_http_response(
            stream,
            200,
            "text/html; charset=utf-8",
            rendered.as_bytes(),
        )
        .await;
    }

    let ui_lang = {
        let prefs = ctx.ui_prefs.lock().await;
        oraculo_proto::i18n::UiLang::from_code(&prefs.lang)
    };
    let loc_err = |msg: &str| oraculo_proto::i18n::localize_text(msg, ui_lang);

    if !constant_time_eq(token_hdr.as_bytes(), ctx.token.as_bytes()) {
        return write_json_error(stream, 403, &loc_err("Sessão IPC não autorizada")).await;
    }

    let snap_or_err = |res: Result<StateResponse>| match res {
        Ok(snap) => Ok(snap),
        Err(err) => Err((503u16, loc_err(&format!("{err:#}")))),
    };

    match (method, path) {
        ("GET", "/api/state") => match snap_or_err(build_state_snapshot(&ctx, false).await) {
            Ok(snap) => write_json_ok(stream, &snap).await,
            Err((code, msg)) => write_json_error(stream, code, &msg).await,
        },
        ("POST", "/api/pair/issue") => {
            let issued = match ctx.host.issue_pairing().await {
                Ok(v) => v,
                Err(err) => return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await,
            };
            let dto = PairingDto {
                short_code: issued.short_code,
                uri: issued.uri,
                qr_svg: issued.qr_svg,
                remaining_secs: issued.remaining_secs,
            };
            write_json_ok(stream, &dto).await
        }
        ("POST", "/api/pair/cancel") => {
            let _ = ctx.host.cancel_pairing().await;
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/peers/disconnect") => {
            let req: RevokeReq = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            let _ = ctx.host.disconnect_peer(&req.endpoint_id).await;
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/peers/revoke") => {
            let req: RevokeReq = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            if let Err(err) = ctx.host.revoke_peer(&req.endpoint_id).await {
                return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await;
            }
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/settings") => {
            let patch: SettingsPatchDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            if let Err(err) = ctx.host.patch_settings(patch).await {
                return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await;
            }
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/wol/enable") => {
            if let Err(err) = ctx.host.enable_wol_setting().await {
                return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await;
            }
            let nm_results = tokio::task::spawn_blocking(enable_nm_magic_packet)
                .await
                .unwrap_or_default();
            let nm_errors: Vec<String> = nm_results
                .iter()
                .filter_map(|r| r.as_ref().err().cloned())
                .collect();
            let nm_ok_count = nm_results.iter().filter(|r| r.is_ok()).count();
            if !nm_errors.is_empty() && nm_ok_count == 0 {
                return write_json_error(stream, 400, &loc_err(&nm_errors.join("; "))).await;
            }
            match snap_or_err(build_state_snapshot(&ctx, true).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/portal/authorize") | ("POST", "/api/monitors/authorize") => {
            match ctx.host.authorize_portals().await {
                Ok(_) => match snap_or_err(build_state_snapshot(&ctx, false).await) {
                    Ok(snap) => write_json_ok(stream, &snap).await,
                    Err((code, msg)) => write_json_error(stream, code, &msg).await,
                },
                Err(err) => write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await,
            }
        }
        ("POST", "/api/rdp/harden-firewall") => {
            let port = ctx
                .host
                .snapshot()
                .await
                .map(|s| effective_rdp_port(&s.settings))
                .unwrap_or(oraculo_host::tunnel::DEFAULT_RDP_PORT);
            match tokio::task::spawn_blocking(move || harden_rdp_firewall(port)).await {
                Ok(Ok(())) => {
                    ctx.host.invalidate_rdp_cache().await;
                    match snap_or_err(build_state_snapshot(&ctx, false).await) {
                        Ok(snap) => write_json_ok(stream, &snap).await,
                        Err((code, msg)) => write_json_error(stream, code, &msg).await,
                    }
                }
                Ok(Err(err)) => write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await,
                Err(err) => write_json_error(stream, 500, &format!("{err}")).await,
            }
        }
        ("POST", "/api/commands/save") => {
            let req: SaveCommandDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            if let Err(err) = ctx.host.save_command(req).await {
                return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await;
            }
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/commands/delete") => {
            let req: DeleteCommandDto = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            if let Err(err) = ctx.host.delete_command(&req.id).await {
                return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await;
            }
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/autostart") => {
            let req: AutostartReq = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            let enabled = req.enabled;
            let res = match tokio::task::spawn_blocking(move || set_autostart_enabled(enabled)).await {
                Ok(r) => r,
                Err(err) => return write_json_error(stream, 500, &format!("{err}")).await,
            };
            if let Err(err) = res {
                return write_json_error(stream, 400, &loc_err(&format!("{err:#}"))).await;
            }
            *ctx.sys_cache.lock().await = None;
            {
                let mut prefs = ctx.ui_prefs.lock().await;
                if !prefs.autostart_default_applied {
                    prefs.autostart_default_applied = true;
                    let snapshot = prefs.clone();
                    let path = ctx.ui_prefs_file.clone();
                    drop(prefs);
                    let _ = tokio::task::spawn_blocking(move || save_ui_prefs(&path, &snapshot)).await;
                }
            }
            match snap_or_err(build_state_snapshot(&ctx, false).await) {
                Ok(snap) => write_json_ok(stream, &snap).await,
                Err((code, msg)) => write_json_error(stream, code, &msg).await,
            }
        }
        ("POST", "/api/ui/theme") | ("POST", "/api/ui/prefs") => {
            let req: ThemeSyncReq = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => return write_json_error(stream, 400, &loc_err(&format!("JSON inválido: {e}"))).await,
            };
            let (theme_after, snapshot_to_save) = {
                let mut prefs = ctx.ui_prefs.lock().await;
                let mut changed = false;
                if let Some(m) = req.mode.as_deref() {
                    let next_theme = sanitize_theme(m);
                    if prefs.theme != next_theme {
                        prefs.theme = next_theme;
                        changed = true;
                    }
                }
                if let Some(l) = req.lang.as_deref() {
                    let next_lang = sanitize_lang(l);
                    if prefs.lang != next_lang {
                        prefs.lang = next_lang;
                        changed = true;
                    }
                }
                crate::webview::set_ui_lang(&prefs.lang);
                (prefs.theme.clone(), changed.then(|| prefs.clone()))
            };
            let theme_for_sync = theme_after.clone();
            let dark =
                tokio::task::spawn_blocking(move || crate::webview::sync_window_theme(&theme_for_sync))
                    .await
                    .unwrap_or(None);
            if let Some(snapshot) = snapshot_to_save {
                let path = ctx.ui_prefs_file.clone();
                let _ = tokio::task::spawn_blocking(move || save_ui_prefs(&path, &snapshot)).await;
            }
            write_json_ok(stream, &serde_json::json!({ "dark": dark })).await
        }
        ("POST", "/api/app/quit") => {
            let res = write_json_ok(stream, &serde_json::json!({ "ok": true })).await;
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(60)).await;
                crate::webview::request_quit();
            });
            res
        }
        _ => write_json_error(stream, 404, &loc_err("Rota não encontrada")).await,
    }
}

async fn build_state_snapshot(ctx: &IpcContext, force_wol_refresh: bool) -> Result<StateResponse> {
    let daemon = ctx.host.snapshot().await?;
    let active_ids: HashSet<String> = daemon.active_peer_ids.into_iter().collect();
    let peers = daemon
        .trusted_peers
        .into_iter()
        .map(|p| PeerDto {
            connected: active_ids.contains(&p.endpoint_id),
            endpoint_id: p.endpoint_id,
            name: p.name,
            paired_at_unix: p.paired_at_unix,
        })
        .collect();

    let pairing = daemon.pairing.map(|p| PairingDto {
        short_code: p.short_code,
        uri: p.uri,
        qr_svg: p.qr_svg,
        remaining_secs: p.remaining_secs,
    });

    let (nics, nm_profiles) = get_cached_wol_diag(ctx, force_wol_refresh).await;
    let all_magic_ready = if nics.is_empty() {
        true
    } else if nm_profiles.is_empty() {
        true
    } else {
        nm_profiles.iter().all(|p| p.magic_enabled)
    };
    let (autostart_enabled, linger_enabled) = get_cached_system_status(ctx).await;

    let paths = ctx.host.paths();
    Ok(StateResponse {
        host_name: daemon.host_name,
        endpoint_id: daemon.endpoint_id,
        relay_url: daemon.relay_url,
        peers,
        pairing,
        settings: daemon.settings,
        wol: WolDiagDto {
            nics,
            nm_profiles,
            all_magic_ready,
        },
        commands: daemon.commands,
        portal: daemon.portal,
        rdp: daemon.rdp,
        autostart_enabled,
        linger_enabled,
        daemon_mode: if ctx.host.is_external_daemon() {
            "service"
        } else {
            "embedded"
        },
        paths: PathsDto {
            settings_file: paths.settings_file.display().to_string(),
            commands_file: paths.commands_file.display().to_string(),
            peers_file: paths.trusted_peers_file.display().to_string(),
        },
    })
}

async fn get_cached_system_status(ctx: &IpcContext) -> (bool, bool) {
    {
        let guard = ctx.sys_cache.lock().await;
        if let Some(cached) = guard.as_ref() {
            if cached.captured_at.elapsed() < Duration::from_secs(5) {
                return (cached.autostart_enabled, cached.linger_enabled);
            }
        }
    }
    let (autostart_enabled, linger_enabled) = tokio::task::spawn_blocking(|| {
        (is_autostart_enabled(), is_linger_enabled())
    })
    .await
    .unwrap_or((false, false));
    let mut guard = ctx.sys_cache.lock().await;
    *guard = Some(CachedSystemStatus {
        captured_at: Instant::now(),
        autostart_enabled,
        linger_enabled,
    });
    (autostart_enabled, linger_enabled)
}

async fn get_cached_wol_diag(
    ctx: &IpcContext,
    force_refresh: bool,
) -> (Vec<NicDto>, Vec<NmProfileDto>) {
    {
        let guard = ctx.wol_cache.lock().await;
        if !force_refresh {
            if let Some(cached) = guard.as_ref() {
                if cached.captured_at.elapsed() < Duration::from_secs(10) {
                    return (cached.nics.clone(), cached.nm_profiles.clone());
                }
            }
        }
    }

    let (nics, nm_profiles) = tokio::task::spawn_blocking(|| {
        let nics = discover_nics()
            .into_iter()
            .map(|n| NicDto {
                name: n.name,
                mac: n.mac,
                perm_mac: n.perm_mac,
                broadcast_v4: n.broadcast_v4,
                is_wireless: n.is_wireless,
                is_up: n.is_up,
            })
            .collect::<Vec<_>>();
        let nm = inspect_nm_wol_profiles()
            .into_iter()
            .map(|p| NmProfileDto {
                name: p.name,
                conn_type: p.conn_type,
                device: p.device,
                magic_enabled: p.magic_enabled,
            })
            .collect::<Vec<_>>();
        (nics, nm)
    })
    .await
    .unwrap_or_default();

    let mut guard = ctx.wol_cache.lock().await;
    *guard = Some(CachedWolDiag {
        captured_at: Instant::now(),
        nics: nics.clone(),
        nm_profiles: nm_profiles.clone(),
    });
    (nics, nm_profiles)
}

async fn write_json_ok<T: Serialize>(stream: TcpStream, data: &T) -> Result<()> {
    let body = serde_json::to_vec(data)?;
    write_http_response(stream, 200, "application/json; charset=utf-8", &body).await
}

async fn write_json_error(stream: TcpStream, status: u16, message: &str) -> Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({ "error": message }))?;
    write_http_response(stream, status, "application/json; charset=utf-8", &body).await
}

async fn write_http_response(
    mut stream: TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Content-Security-Policy: default-src 'self' 'unsafe-inline' data:\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
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

#[cfg(target_os = "linux")]
fn systemd_unit_path() -> Option<PathBuf> {
    BaseDirs::new().map(|b| b.config_dir().join("systemd/user/oraculo-desktop.service"))
}

#[cfg(target_os = "linux")]
fn host_unit_path() -> Option<PathBuf> {
    BaseDirs::new().map(|b| b.config_dir().join("systemd/user/oraculo-host.service"))
}

#[cfg(target_os = "linux")]
fn resolve_host_exe(desktop_exe: &Path) -> Option<PathBuf> {
    let sibling = desktop_exe.with_file_name("oraculo-host");
    if sibling.exists() {
        return Some(sibling);
    }
    None
}

#[cfg(target_os = "linux")]
fn is_systemd_unit_enabled(unit: &str, unit_file: Option<PathBuf>) -> bool {
    let Some(path) = unit_file else {
        return false;
    };
    if !path.exists() {
        return false;
    }
    match std::process::Command::new("systemctl")
        .args(["--user", "is-enabled", unit])
        .output()
    {
        Ok(out) => {
            let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
            out.status.success() || state == "enabled" || state == "static"
        }
        Err(_) => true,
    }
}

#[cfg(target_os = "linux")]
fn is_autostart_enabled() -> bool {
    is_systemd_unit_enabled("oraculo-host.service", host_unit_path())
        || is_systemd_unit_enabled("oraculo-desktop.service", systemd_unit_path())
}

#[cfg(target_os = "linux")]
fn is_linger_enabled() -> bool {
    // O modo "antes do login" só se aplica ao daemon `oraculo-host.service` (`WantedBy=default.target`).
    if !is_systemd_unit_enabled("oraculo-host.service", host_unit_path()) {
        return false;
    }
    let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")) else {
        return false;
    };
    let trimmed = user.trim();
    if trimmed.is_empty() {
        return false;
    }
    Path::new("/var/lib/systemd/linger").join(trimmed).exists()
}

#[cfg(not(target_os = "linux"))]
fn is_linger_enabled() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn build_host_autostart_unit_body(host_exe: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=Oraculo P2P Host Daemon\n\
         After=network-online.target\n\
         Wants=network-online.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart=\"{}\" serve\n\
         Restart=on-failure\n\
         RestartSec=3\n\n\
         [Install]\n\
         WantedBy=default.target\n",
        host_exe.display()
    )
}

#[cfg(target_os = "linux")]
fn build_desktop_fallback_unit_body(desktop_exe: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=Oraculo Desktop P2P Background Service\n\
         After=graphical-session.target xdg-desktop-portal.service network-online.target\n\
         PartOf=graphical-session.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart=\"{}\" --background\n\
         Restart=on-failure\n\
         RestartSec=3\n\n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        desktop_exe.display()
    )
}

#[cfg(target_os = "linux")]
fn ensure_user_desktop_entry(desktop_exe: &Path) {
    let Some(dirs) = BaseDirs::new() else {
        return;
    };
    let apps_dir = dirs.data_local_dir().join("applications");
    let desktop_file = apps_dir.join(format!("{PORTAL_APP_ID}.desktop"));
    let body = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Oráculo\n\
         Comment=Encrypted remote access, screen sharing, terminal and Wake-on-LAN\n\
         Comment[pt_BR]=Acesso remoto criptografado, espelhamento de tela, terminal e Wake-on-LAN\n\
         Exec=\"{0}\"\n\
         Icon={PORTAL_APP_ID}\n\
         Terminal=false\n\
         Categories=Utility;Network;RemoteAccess;\n\
         StartupWMClass={PORTAL_APP_ID}\n\
         X-GNOME-UsesNotifications=true\n\
         Actions=Quit;\n\n\
         [Desktop Action Quit]\n\
         Name=Quit Oráculo\n\
         Name[pt_BR]=Encerrar Oráculo\n\
         Exec=\"{0}\" --quit\n",
        desktop_exe.display()
    );
    if std::fs::read_to_string(&desktop_file).ok().as_deref() == Some(body.as_str()) {
        return;
    }
    if std::fs::create_dir_all(&apps_dir).is_err() {
        return;
    }
    let _ = std::fs::write(desktop_file, body);
}

#[cfg(target_os = "linux")]
fn enable_user_linger_best_effort() {
    if is_linger_enabled() {
        return;
    }
    if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")) {
        let trimmed = user.trim();
        if !trimmed.is_empty() {
            let _ = std::process::Command::new("loginctl")
                .args(["enable-linger", trimmed])
                .status();
        }
    }
}

#[cfg(target_os = "linux")]
fn install_linux_autostart_unit(desktop_exe: &Path) -> Result<()> {
    use std::process::Command;

    let desktop_unit = systemd_unit_path().context("Diretório ~/.config indisponível")?;
    let host_unit = host_unit_path().context("Diretório ~/.config indisponível")?;
    if let Some(parent) = host_unit.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if let Some(host_exe) = resolve_host_exe(desktop_exe) {
        if desktop_unit.exists() {
            let _ = Command::new("systemctl")
                .args(["--user", "disable", "oraculo-desktop.service"])
                .status();
            let _ = std::fs::remove_file(&desktop_unit);
        }
        let expected = build_host_autostart_unit_body(&host_exe);
        let current = std::fs::read_to_string(&host_unit).unwrap_or_default();
        if current != expected {
            std::fs::write(&host_unit, expected)?;
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let _ = Command::new("systemctl")
            .args(["--user", "reenable", "oraculo-host.service"])
            .status();
        enable_user_linger_best_effort();
    } else {
        let expected = build_desktop_fallback_unit_body(desktop_exe);
        let current = std::fs::read_to_string(&desktop_unit).unwrap_or_default();
        if current != expected {
            std::fs::write(&desktop_unit, expected)?;
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        let _ = Command::new("systemctl")
            .args(["--user", "reenable", "oraculo-desktop.service"])
            .status();
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn sync_desktop_integration_on_startup() {
    let Ok(desktop_exe) = std::env::current_exe() else {
        return;
    };
    ensure_user_desktop_entry(&desktop_exe);

    if is_autostart_enabled() {
        let _ = install_linux_autostart_unit(&desktop_exe);
    }
}

#[cfg(target_os = "linux")]
fn set_autostart_enabled(enabled: bool) -> Result<()> {
    use std::process::Command;

    if enabled {
        let desktop_exe =
            std::env::current_exe().context("Caminho do executável oraculo-desktop indisponível")?;
        ensure_user_desktop_entry(&desktop_exe);
        install_linux_autostart_unit(&desktop_exe)?;
    } else {
        for (unit, opt_path) in [
            ("oraculo-host.service", host_unit_path()),
            ("oraculo-desktop.service", systemd_unit_path()),
        ] {
            let _ = Command::new("systemctl")
                .args(["--user", "disable", unit])
                .status();
            if let Some(p) = opt_path {
                if p.exists() {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_autostart_enabled() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "OraculoHost",
        ])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(target_os = "windows")]
fn set_autostart_enabled(enabled: bool) -> Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    if enabled {
        let exe = std::env::current_exe().context("Caminho do executável indisponível")?;
        let host_exe = exe.with_file_name("oraculo-host.exe");
        let val = if host_exe.exists() {
            format!("\"{}\" serve", host_exe.display())
        } else {
            format!("\"{}\" --background", exe.display())
        };
        let status = Command::new("reg")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["add", key, "/v", "OraculoHost", "/t", "REG_SZ", "/d", &val, "/f"])
            .status()?;
        if !status.success() {
            bail!("Falha ao registrar inicialização automática no Windows");
        }
    } else {
        let _ = Command::new("reg")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["delete", key, "/v", "OraculoHost", "/f"])
            .status();
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn is_autostart_enabled() -> bool {
    false
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn set_autostart_enabled(_enabled: bool) -> Result<()> {
    bail!("Inicialização automática não implementada nesta plataforma")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_prefs_defaults_autostart_flag_and_enables_once_until_user_opts_out() {
        let dir = std::env::temp_dir().join(format!(
            "oraculo-ipc-prefs-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let prefs_path = dir.join("ui_prefs.json");

        // JSON legado sem o campo `autostart_default_applied`.
        std::fs::write(&prefs_path, r#"{"theme":"light","lang":"en"}"#).unwrap();
        let mut prefs = load_ui_prefs(&prefs_path);
        assert_eq!(prefs.theme, "light");
        assert_eq!(prefs.lang, "en");
        assert!(!prefs.autostart_default_applied);

        // Primeira execução aplica o padrão uma única vez.
        let mut called = 0usize;
        let applied = ensure_default_autostart_with(&mut prefs, &prefs_path, false, || {
            called += 1;
            Ok(())
        });
        assert!(applied);
        assert_eq!(called, 1);
        assert!(prefs.autostart_default_applied);

        // Em execuções futuras, se o usuário desativou, não reativa automaticamente.
        let mut reloaded = load_ui_prefs(&prefs_path);
        assert!(reloaded.autostart_default_applied);
        let applied_again =
            ensure_default_autostart_with(&mut reloaded, &prefs_path, false, || {
                called += 1;
                Ok(())
            });
        assert!(!applied_again);
        assert_eq!(called, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_autostart_does_not_mark_applied_when_enable_fails() {
        let dir = std::env::temp_dir().join(format!(
            "oraculo-ipc-fail-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let prefs_path = dir.join("ui_prefs.json");

        let mut prefs = UiPrefs::default();
        let applied = ensure_default_autostart_with(&mut prefs, &prefs_path, false, || {
            anyhow::bail!("erro simulado")
        });
        assert!(!applied);
        assert!(!prefs.autostart_default_applied);
        assert!(!prefs_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
