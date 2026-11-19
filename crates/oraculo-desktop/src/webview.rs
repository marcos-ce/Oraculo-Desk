use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering as StdOrdering};

static ATTACHED_TO_DAEMON: AtomicBool = AtomicBool::new(false);
static FORCE_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static UI_LANG_EN: AtomicBool = AtomicBool::new(false);
static BG_NOTICE_SHOWN: AtomicBool = AtomicBool::new(false);

pub fn set_attached_to_daemon(attached: bool) {
    ATTACHED_TO_DAEMON.store(attached, StdOrdering::Release);
}

pub fn is_attached_to_daemon() -> bool {
    ATTACHED_TO_DAEMON.load(StdOrdering::Acquire)
}

pub fn was_force_quit_requested() -> bool {
    FORCE_QUIT_REQUESTED.load(StdOrdering::Acquire)
}

pub fn set_ui_lang(lang: &str) {
    UI_LANG_EN.store(lang.trim().eq_ignore_ascii_case("en"), StdOrdering::Release);
}

pub fn is_ui_lang_en() -> bool {
    UI_LANG_EN.load(StdOrdering::Acquire)
}

pub fn set_bg_notice_shown(shown: bool) {
    BG_NOTICE_SHOWN.store(shown, StdOrdering::Release);
}

pub fn was_bg_notice_shown() -> bool {
    BG_NOTICE_SHOWN.load(StdOrdering::Acquire)
}

/// Resultado da verificação de instância única antes de subir o servidor P2P.
pub enum InstanceDisposition {
    /// Esta é a instância primária; prossiga subindo o servidor e chame [`WindowRuntime::run`].
    Primary(WindowRuntime),
    /// Outra instância do Oráculo Desktop já estava ativa e recebeu o comando (focar janela ou encerrar).
    ForwardedToExisting,
}

pub struct WindowRuntime {
    #[cfg(target_os = "linux")]
    inner: linux::PreparedApp,
    #[cfg(not(target_os = "linux"))]
    start_hidden: bool,
}

/// Prepara a instância gráfica nativa antes de adquirir `host.lock`.
/// No Linux (Wayland/GNOME), registra `com.oraculo.Desktop` no barramento D-Bus da sessão:
/// se o Oráculo já estiver rodando em segundo plano, apenas envia `Activate` (reabrindo a janela)
/// ou `quit` (se `--quit` foi passado) e encerra imediatamente sem conflito de porta ou lock.
pub fn prepare_instance(start_hidden: bool, quit_existing: bool) -> Result<InstanceDisposition> {
    #[cfg(target_os = "linux")]
    {
        linux::prepare_gtk_instance(start_hidden, quit_existing)
    }
    #[cfg(not(target_os = "linux"))]
    {
        if quit_existing {
            return Ok(InstanceDisposition::ForwardedToExisting);
        }
        Ok(InstanceDisposition::Primary(WindowRuntime { start_hidden }))
    }
}

impl WindowRuntime {
    /// Executa o loop principal da interface na thread principal do sistema operacional.
    pub fn run(self, url: &str, title: &str) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            linux::run_prepared_app(self.inner, url, title)
        }
        #[cfg(target_os = "windows")]
        {
            windows::run_windows_session(url, self.start_hidden)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = title;
            if !self.start_hidden {
                let _ = std::process::Command::new("open").arg(url).status();
            }
            anyhow::bail!("Feche com Ctrl+C quando terminar")
        }
    }
}

/// Sincroniza o tema (`"auto"`, `"light"`, `"dark"`) com a decoração nativa da janela (HeaderBar / libadwaita)
/// e retorna `Some(true/false)` quando o backend conhece o esquema efetivo, ou `None` para delegar ao `matchMedia` do WebView.
pub fn sync_window_theme(mode: &str) -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        linux::sync_theme(mode)
    }
    #[cfg(not(target_os = "linux"))]
    {
        match mode {
            "light" => Some(false),
            "dark" => Some(true),
            _ => None,
        }
    }
}

/// Solicita o encerramento completo do aplicativo (incluindo o servidor em segundo plano).
pub fn request_quit() {
    FORCE_QUIT_REQUESTED.store(true, StdOrdering::Release);
    #[cfg(target_os = "linux")]
    {
        linux::request_quit();
    }
    #[cfg(target_os = "windows")]
    {
        windows::request_quit();
    }
}

/// Retorna `true` quando os controles nativos da janela flutuam sobre o conteúdo (`AdwApplicationWindow` + `GtkOverlay`).
pub fn uses_overlay_controls() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::uses_overlay_controls()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Conta quantos botões nativos (`minimize`, `maximize`, `close`) ficam no canto superior direito
/// quando os controles flutuam em `GtkOverlay`, evitando espaço sobrando no GNOME padrão (`:close`).
pub fn overlay_right_controls_count() -> u8 {
    #[cfg(target_os = "linux")]
    {
        linux::overlay_right_controls_count()
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_right_controls_count(layout: &str) -> u8 {
    let trimmed = layout.trim().trim_matches('\'').trim_matches('"');
    let right = match trimmed.split_once(':') {
        Some((_, r)) => r,
        None => return 1,
    };
    let count = right
        .split(',')
        .map(str::trim)
        .filter(|tok| matches!(*tok, "close" | "minimize" | "maximize"))
        .count();
    count.min(3) as u8
}

#[cfg(test)]
mod overlay_tests {
    use super::parse_right_controls_count;

    #[test]
    fn parses_gnome_button_layouts() {
        assert_eq!(parse_right_controls_count("'appmenu:close'\n"), 1);
        assert_eq!(parse_right_controls_count("':minimize,maximize,close'"), 3);
        assert_eq!(parse_right_controls_count("'appmenu:minimize,close'"), 2);
        assert_eq!(parse_right_controls_count("'close:appmenu'"), 0);
        assert_eq!(parse_right_controls_count("invalid"), 1);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{InstanceDisposition, WindowRuntime};
    use anyhow::{Result, bail};
    use std::ffi::{CString, c_char, c_int, c_ulong, c_void};
    use std::ptr;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    use std::sync::{Mutex, OnceLock};

    const G_APPLICATION_DEFAULT_FLAGS: c_int = 0;
    const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: u32 = 600;
    const GTK_ALIGN_FILL: c_int = 0;
    const GTK_ALIGN_START: c_int = 1;
    const GTK_ALIGN_END: c_int = 2;
    const GTK_PACK_END: c_int = 1;

    // AdwColorScheme (libadwaita-1)
    const ADW_COLOR_SCHEME_DEFAULT: c_int = 0;
    const ADW_COLOR_SCHEME_FORCE_LIGHT: c_int = 1;
    const ADW_COLOR_SCHEME_FORCE_DARK: c_int = 4;

    const GTK4_DARK_CSS: &str = "\
window, window.background { background-color: #131314; color: #E3E3E3; }\n\
headerbar {\n\
  background-color: #131314;\n\
  background-image: none;\n\
  color: #E3E3E3;\n\
  border-bottom: none;\n\
  box-shadow: none;\n\
  min-height: 36px;\n\
  padding-top: 0;\n\
  padding-bottom: 0;\n\
}\n\
headerbar:backdrop {\n\
  background-color: #131314;\n\
  background-image: none;\n\
  color: #9AA0A6;\n\
}\n";

    const GTK4_LIGHT_CSS: &str = "\
window, window.background { background-color: #F8F9FA; color: #1F1F1F; }\n\
headerbar {\n\
  background-color: #F8F9FA;\n\
  background-image: none;\n\
  color: #1F1F1F;\n\
  border-bottom: none;\n\
  box-shadow: none;\n\
  min-height: 36px;\n\
  padding-top: 0;\n\
  padding-bottom: 0;\n\
}\n\
headerbar:backdrop {\n\
  background-color: #F8F9FA;\n\
  background-image: none;\n\
  color: #5F6368;\n\
}\n";

    #[repr(C)]
    struct GdkRgbaF32 {
        red: f32,
        green: f32,
        blue: f32,
        alpha: f32,
    }

    #[repr(C)]
    struct GdkRgbaF64 {
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
    }

    pub enum PreparedApp {
        Gtk4 { start_hidden: bool },
        Gtk3 { start_hidden: bool },
    }

    struct AdwApi {
        adw_application_new: unsafe extern "C" fn(*const c_char, c_int) -> *mut c_void,
        adw_application_window_new: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        adw_application_window_set_content: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        adw_style_manager_get_default: unsafe extern "C" fn() -> *mut c_void,
        adw_style_manager_set_color_scheme: unsafe extern "C" fn(*mut c_void, c_int),
        adw_style_manager_get_dark: unsafe extern "C" fn(*mut c_void) -> c_int,
    }

    struct Gtk4Api {
        adw: Option<AdwApi>,
        gtk_application_new: unsafe extern "C" fn(*const c_char, c_int) -> *mut c_void,
        g_application_register:
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> c_int,
        g_application_get_is_remote: unsafe extern "C" fn(*mut c_void) -> c_int,
        g_application_hold: unsafe extern "C" fn(*mut c_void),
        g_application_quit: unsafe extern "C" fn(*mut c_void),
        g_application_run: unsafe extern "C" fn(*mut c_void, c_int, *const *const c_char) -> c_int,
        g_simple_action_new: unsafe extern "C" fn(*const c_char, *const c_void) -> *mut c_void,
        g_action_map_add_action: unsafe extern "C" fn(*mut c_void, *mut c_void),
        g_action_group_activate_action:
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void),
        g_application_get_dbus_connection: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        g_dbus_connection_flush_sync:
            Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> c_int>,
        g_idle_add: unsafe extern "C" fn(unsafe extern "C" fn(*mut c_void) -> c_int, *mut c_void) -> u32,
        g_error_free: Option<unsafe extern "C" fn(*mut c_void)>,
        g_signal_connect_data: unsafe extern "C" fn(
            *mut c_void,
            *const c_char,
            *const c_void,
            *mut c_void,
            *const c_void,
            c_int,
        ) -> c_ulong,
        g_object_unref: unsafe extern "C" fn(*mut c_void),
        gtk_application_window_new: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        gtk_window_set_title: unsafe extern "C" fn(*mut c_void, *const c_char),
        gtk_window_set_default_size: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        gtk_widget_set_size_request: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        gtk_header_bar_new: unsafe extern "C" fn() -> *mut c_void,
        gtk_header_bar_set_show_title_buttons: unsafe extern "C" fn(*mut c_void, c_int),
        gtk_header_bar_set_title_widget: unsafe extern "C" fn(*mut c_void, *mut c_void),
        gtk_box_new: unsafe extern "C" fn(c_int, c_int) -> *mut c_void,
        gtk_window_set_titlebar: unsafe extern "C" fn(*mut c_void, *mut c_void),
        gtk_window_set_child: unsafe extern "C" fn(*mut c_void, *mut c_void),
        gtk_window_present: unsafe extern "C" fn(*mut c_void),
        gtk_overlay_new: Option<unsafe extern "C" fn() -> *mut c_void>,
        gtk_overlay_set_child: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        gtk_overlay_add_overlay: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        gtk_window_handle_new: Option<unsafe extern "C" fn() -> *mut c_void>,
        gtk_window_handle_set_child: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        gtk_window_controls_new: Option<unsafe extern "C" fn(c_int) -> *mut c_void>,
        gtk_widget_set_halign: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        gtk_widget_set_valign: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        gtk_widget_set_hexpand: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        gtk_widget_set_margin_top: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        gtk_widget_set_margin_start: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        gtk_widget_set_margin_end: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        gdk_display_get_default: unsafe extern "C" fn() -> *mut c_void,
        gtk_css_provider_new: unsafe extern "C" fn() -> *mut c_void,
        gtk_css_provider_load_from_string:
            Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
        gtk_css_provider_load_from_data:
            Option<unsafe extern "C" fn(*mut c_void, *const c_char, isize)>,
        gtk_style_context_add_provider_for_display:
            unsafe extern "C" fn(*mut c_void, *mut c_void, u32),
        webkit_web_view_new: unsafe extern "C" fn() -> *mut c_void,
        webkit_web_view_set_background_color:
            unsafe extern "C" fn(*mut c_void, *const GdkRgbaF32),
        webkit_web_view_load_uri: unsafe extern "C" fn(*mut c_void, *const c_char),
        webkit_web_view_get_settings: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        webkit_settings_set_enable_back_forward_navigation_gestures:
            Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        webkit_settings_set_enable_developer_extras:
            Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        webkit_settings_set_enable_page_cache: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        webkit_hit_test_result_context_is_editable:
            Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    }

    struct Gtk4State {
        api: Gtk4Api,
        app: AtomicPtr<c_void>,
        active_win: AtomicPtr<c_void>,
        active_webview: AtomicPtr<c_void>,
        css_provider: AtomicPtr<c_void>,
        url: OnceLock<CString>,
        title: OnceLock<CString>,
        skip_first_activate: AtomicBool,
        notified_bg_once: AtomicBool,
        theme_mode: Mutex<String>,
        effective_dark: AtomicBool,
    }

    // Safety: ponteiros de funções carregados via `dlopen` permanecem válidos durante a vida do processo,
    // e widgets GTK/WebKit são manipulados apenas na thread principal via callbacks do loop GLib (`g_idle_add`).
    unsafe impl Send for Gtk4State {}
    unsafe impl Sync for Gtk4State {}

    static GTK4_STATE: OnceLock<Gtk4State> = OnceLock::new();

    pub fn prepare_gtk_instance(
        start_hidden: bool,
        quit_existing: bool,
    ) -> Result<InstanceDisposition> {
        if let Ok(disp) = try_prepare_gtk4(start_hidden, quit_existing) {
            return Ok(disp);
        }
        if let Ok(disp) = try_prepare_gtk3(start_hidden, quit_existing) {
            return Ok(disp);
        }
        bail!(
            "Bibliotecas gráficas WebKitGTK não encontradas (instale webkitgtk6.0 no Fedora ou libwebkit2gtk-4.1-0 no Debian/Ubuntu)"
        )
    }

    pub fn run_prepared_app(prepared: PreparedApp, url: &str, title: &str) -> Result<()> {
        match prepared {
            PreparedApp::Gtk4 { start_hidden } => run_gtk4_loop(start_hidden, url, title),
            PreparedApp::Gtk3 { start_hidden } => run_gtk3_loop(start_hidden, url, title),
        }
    }

    pub fn sync_theme(mode: &str) -> Option<bool> {
        let normalized = match mode {
            "light" => "light",
            "auto" => "auto",
            _ => "dark",
        };
        if let Some(st) = GTK4_STATE.get() {
            if let Ok(mut guard) = st.theme_mode.lock() {
                *guard = normalized.to_string();
            }
            let resolved = match normalized {
                "light" => Some(false),
                "dark" => Some(true),
                _ => system_prefers_dark_gsettings(),
            };
            if let Some(is_dark) = resolved {
                st.effective_dark.store(is_dark, Ordering::Relaxed);
            }
            unsafe {
                (st.api.g_idle_add)(on_idle_apply_theme_gtk4, ptr::null_mut());
            }
            return resolved;
        }
        if let Some(st3) = GTK3_STATE.get() {
            let resolved = match normalized {
                "light" => Some(false),
                "dark" => Some(true),
                _ => system_prefers_dark_gsettings(),
            };
            if let Some(is_dark) = resolved {
                st3.effective_dark.store(is_dark, Ordering::Relaxed);
            }
            return resolved;
        }
        match normalized {
            "light" => Some(false),
            "dark" => Some(true),
            _ => system_prefers_dark_gsettings(),
        }
    }

    pub fn request_quit() {
        if let Some(st) = GTK4_STATE.get() {
            unsafe {
                (st.api.g_idle_add)(on_idle_quit_gtk4, ptr::null_mut());
            }
            return;
        }
        if let Some(st3) = GTK3_STATE.get() {
            unsafe {
                (st3.api.g_idle_add)(on_idle_quit_gtk3, ptr::null_mut());
            }
        }
    }

    pub fn uses_overlay_controls() -> bool {
        let Some(st) = GTK4_STATE.get() else {
            return false;
        };
        st.api
            .adw
            .as_ref()
            .and_then(|a| a.adw_application_window_new)
            .is_some()
            && st.api.gtk_overlay_new.is_some()
            && st.api.gtk_window_controls_new.is_some()
    }

    pub fn overlay_right_controls_count() -> u8 {
        if !uses_overlay_controls() {
            return 0;
        }
        let Ok(out) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
            .output()
        else {
            return 1;
        };
        if !out.status.success() {
            return 1;
        }
        let raw = String::from_utf8_lossy(&out.stdout);
        super::parse_right_controls_count(&raw)
    }

    fn system_prefers_dark_gsettings() -> Option<bool> {
        let out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        if s.contains("prefer-dark") {
            Some(true)
        } else if s.contains("prefer-light") {
            Some(false)
        } else {
            // `'default'` ocorre em KDE/Hyprland/Sway ou GNOME sem preferência explícita;
            // devolve `None` para o WebView (`matchMedia`) decidir.
            None
        }
    }

    unsafe fn dlopen_global(soname: &str) -> Option<*mut c_void> {
        let c_name = CString::new(soname).ok()?;
        let handle = unsafe { libc::dlopen(c_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
        (!handle.is_null()).then_some(handle)
    }

    unsafe fn load_sym<T: Copy>(handle: *mut c_void, sym: &str) -> Option<T> {
        let c_sym = CString::new(sym).ok()?;
        let raw = unsafe { libc::dlsym(handle, c_sym.as_ptr()) };
        if raw.is_null() {
            return None;
        }
        Some(unsafe { std::mem::transmute_copy(&raw) })
    }

    fn rgba_f32_for_dark(is_dark: bool) -> GdkRgbaF32 {
        if is_dark {
            // #131314 (OraculoDarkPalette.Background)
            GdkRgbaF32 {
                red: 19.0 / 255.0,
                green: 19.0 / 255.0,
                blue: 20.0 / 255.0,
                alpha: 1.0,
            }
        } else {
            // #F8F9FA (OraculoLightPalette.Background)
            GdkRgbaF32 {
                red: 248.0 / 255.0,
                green: 249.0 / 255.0,
                blue: 250.0 / 255.0,
                alpha: 1.0,
            }
        }
    }

    unsafe fn apply_gtk4_theme_on_main_thread(st: &Gtk4State) {
        let mode = st
            .theme_mode
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "dark".to_string());

        let is_dark = if let Some(adw) = &st.api.adw {
            let mgr = unsafe { (adw.adw_style_manager_get_default)() };
            if !mgr.is_null() {
                let scheme = match mode.as_str() {
                    "light" => ADW_COLOR_SCHEME_FORCE_LIGHT,
                    "dark" => ADW_COLOR_SCHEME_FORCE_DARK,
                    _ => ADW_COLOR_SCHEME_DEFAULT,
                };
                unsafe {
                    (adw.adw_style_manager_set_color_scheme)(mgr, scheme);
                }
                match mode.as_str() {
                    "light" => false,
                    "dark" => true,
                    _ => unsafe { (adw.adw_style_manager_get_dark)(mgr) != 0 },
                }
            } else {
                mode != "light"
            }
        } else {
            match mode.as_str() {
                "light" => false,
                "dark" => true,
                _ => system_prefers_dark_gsettings().unwrap_or(true),
            }
        };

        st.effective_dark.store(is_dark, Ordering::Relaxed);

        let mut provider = st.css_provider.load(Ordering::Acquire);
        if provider.is_null() {
            let disp = unsafe { (st.api.gdk_display_get_default)() };
            if !disp.is_null() {
                let created = unsafe { (st.api.gtk_css_provider_new)() };
                if !created.is_null() {
                    unsafe {
                        (st.api.gtk_style_context_add_provider_for_display)(
                            disp,
                            created,
                            GTK_STYLE_PROVIDER_PRIORITY_APPLICATION,
                        );
                    }
                    st.css_provider.store(created, Ordering::Release);
                    provider = created;
                }
            }
        }

        if !provider.is_null() {
            let css_str = if is_dark { GTK4_DARK_CSS } else { GTK4_LIGHT_CSS };
            if let Ok(c_css) = CString::new(css_str) {
                if let Some(load_str) = st.api.gtk_css_provider_load_from_string {
                    unsafe { load_str(provider, c_css.as_ptr()) };
                } else if let Some(load_data) = st.api.gtk_css_provider_load_from_data {
                    unsafe { load_data(provider, c_css.as_ptr(), css_str.len() as isize) };
                }
            }
        }

        let wv = st.active_webview.load(Ordering::Acquire);
        if !wv.is_null() {
            let bg = rgba_f32_for_dark(is_dark);
            unsafe {
                (st.api.webkit_web_view_set_background_color)(wv, &bg);
            }
        }
    }

    unsafe extern "C" fn on_idle_apply_theme_gtk4(_data: *mut c_void) -> c_int {
        if let Some(st) = GTK4_STATE.get() {
            unsafe { apply_gtk4_theme_on_main_thread(st) };
        }
        0
    }

    unsafe extern "C" fn on_idle_quit_gtk4(_data: *mut c_void) -> c_int {
        if let Some(st) = GTK4_STATE.get() {
            let app = st.app.load(Ordering::Acquire);
            if !app.is_null() {
                unsafe { (st.api.g_application_quit)(app) };
            }
        }
        0
    }

    unsafe extern "C" fn on_quit_action_gtk4(
        _action: *mut c_void,
        _param: *mut c_void,
        _user_data: *mut c_void,
    ) {
        crate::webview::FORCE_QUIT_REQUESTED.store(true, Ordering::Release);
        if let Some(st) = GTK4_STATE.get() {
            let app = st.app.load(Ordering::Acquire);
            if !app.is_null() {
                unsafe { (st.api.g_application_quit)(app) };
            }
        }
    }

    unsafe extern "C" fn on_close_request_gtk4(_win: *mut c_void, _user_data: *mut c_void) -> c_int {
        if let Some(st) = GTK4_STATE.get() {
            st.active_win.store(ptr::null_mut(), Ordering::Release);
            st.active_webview.store(ptr::null_mut(), Ordering::Release);

            if !crate::webview::BG_NOTICE_SHOWN.swap(true, Ordering::AcqRel)
                && !st.notified_bg_once.swap(true, Ordering::Relaxed)
            {
                let is_en = crate::webview::is_ui_lang_en();
                let (title, body) = if is_en {
                    (
                        "Oráculo keeps running in the background",
                        "Your devices can still connect. Reopen from the app icon or choose Quit Oráculo to shut down.",
                    )
                } else {
                    (
                        "Oráculo continua ativo em segundo plano",
                        "Seus dispositivos continuam podendo conectar. Reabra pelo ícone ou use Encerrar Oráculo para desligar.",
                    )
                };
                if crate::webview::is_attached_to_daemon() {
                    let _ = std::process::Command::new("notify-send")
                        .args(["-a", "Oráculo", title, body])
                        .status();
                } else {
                    std::thread::spawn(move || {
                        let _ = std::process::Command::new("notify-send")
                            .args(["-a", "Oráculo", title, body])
                            .status();
                    });
                }
            }

            if crate::webview::is_attached_to_daemon() {
                let app = st.app.load(Ordering::Acquire);
                if !app.is_null() {
                    // Quando o daemon externo (`oraculo-host`) já está ativo, encerramos só o processo GUI
                    // para liberar 100% da memória do WebKitGTK/GTK mantendo o serviço P2P em ~26 MiB.
                    unsafe { (st.api.g_application_quit)(app) };
                }
            }
        }
        // Retorna FALSE (0) para permitir que o GtkWindow destrua a janela e o WebKitWebView (liberando RAM),
        // enquanto `g_application_hold` mantém o daemon P2P embutido vivo quando não há serviço externo.
        0
    }

    unsafe extern "C" fn on_context_menu_wk6(
        _webview: *mut c_void,
        _context_menu: *mut c_void,
        hit_test_result: *mut c_void,
        _user_data: *mut c_void,
    ) -> c_int {
        if let Some(st) = GTK4_STATE.get() {
            if let Some(is_editable) = st.api.webkit_hit_test_result_context_is_editable {
                if !hit_test_result.is_null() && unsafe { is_editable(hit_test_result) } != 0 {
                    // Permite menu nativo GTK4 (Recortar / Copiar / Colar) apenas em campos de texto editáveis.
                    return 0;
                }
            }
        }
        // Bloqueia menu de navegador (Back / Forward / Reload) no restante do aplicativo.
        1
    }

    unsafe extern "C" fn on_activate_gtk4(app: *mut c_void, _user_data: *mut c_void) {
        let Some(st) = GTK4_STATE.get() else {
            return;
        };
        if st.skip_first_activate.swap(false, Ordering::SeqCst) {
            return;
        }

        let existing = st.active_win.load(Ordering::Acquire);
        if !existing.is_null() {
            unsafe { (st.api.gtk_window_present)(existing) };
            return;
        }

        let Some(url) = st.url.get() else {
            return;
        };
        let Some(title) = st.title.get() else {
            return;
        };

        unsafe {
            apply_gtk4_theme_on_main_thread(st);

            let webview = (st.api.webkit_web_view_new)();
            if let Some(get_settings) = st.api.webkit_web_view_get_settings {
                let settings = get_settings(webview);
                if !settings.is_null() {
                    if let Some(set_gestures) =
                        st.api.webkit_settings_set_enable_back_forward_navigation_gestures
                    {
                        set_gestures(settings, 0);
                    }
                    if let Some(set_dev) = st.api.webkit_settings_set_enable_developer_extras {
                        set_dev(settings, 0);
                    }
                    if let Some(set_cache) = st.api.webkit_settings_set_enable_page_cache {
                        set_cache(settings, 0);
                    }
                }
            }

            if let Ok(sig_ctx) = CString::new("context-menu") {
                (st.api.g_signal_connect_data)(
                    webview,
                    sig_ctx.as_ptr(),
                    on_context_menu_wk6 as *const c_void,
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                );
            }

            let is_dark = st.effective_dark.load(Ordering::Relaxed);
            let bg = rgba_f32_for_dark(is_dark);
            (st.api.webkit_web_view_set_background_color)(webview, &bg);
            (st.api.webkit_web_view_load_uri)(webview, url.as_ptr());

            let adw_win_new = st.api.adw.as_ref().and_then(|a| a.adw_application_window_new);
            let adw_set_content = st.api.adw.as_ref().and_then(|a| a.adw_application_window_set_content);

            let win = if let (
                Some(new_adw_win),
                Some(set_adw_content),
                Some(overlay_new),
                Some(overlay_set_child),
                Some(overlay_add_overlay),
                Some(handle_new),
                Some(handle_set_child),
                Some(controls_new),
                Some(set_halign),
                Some(set_valign),
                Some(set_hexpand),
                Some(set_margin_top),
                Some(set_margin_start),
                Some(set_margin_end),
            ) = (
                adw_win_new,
                adw_set_content,
                st.api.gtk_overlay_new,
                st.api.gtk_overlay_set_child,
                st.api.gtk_overlay_add_overlay,
                st.api.gtk_window_handle_new,
                st.api.gtk_window_handle_set_child,
                st.api.gtk_window_controls_new,
                st.api.gtk_widget_set_halign,
                st.api.gtk_widget_set_valign,
                st.api.gtk_widget_set_hexpand,
                st.api.gtk_widget_set_margin_top,
                st.api.gtk_widget_set_margin_start,
                st.api.gtk_widget_set_margin_end,
            ) {
                let win = new_adw_win(app);
                (st.api.gtk_window_set_title)(win, title.as_ptr());
                (st.api.gtk_window_set_default_size)(win, 1020, 690);
                (st.api.gtk_widget_set_size_request)(win, 760, 540);

                // Superfície unificada sem barra dupla: faixa arrastável no topo + controles nativos flutuando à direita.
                let overlay = overlay_new();
                overlay_set_child(overlay, webview);

                let drag_handle = handle_new();
                let drag_spacer = (st.api.gtk_box_new)(0, 0);
                (st.api.gtk_widget_set_size_request)(drag_spacer, -1, 58);
                handle_set_child(drag_handle, drag_spacer);
                set_valign(drag_handle, GTK_ALIGN_START);
                set_halign(drag_handle, GTK_ALIGN_FILL);
                set_hexpand(drag_handle, 1);
                set_margin_start(drag_handle, 80);
                set_margin_end(drag_handle, 370);
                overlay_add_overlay(overlay, drag_handle);

                let win_controls = controls_new(GTK_PACK_END);
                set_valign(win_controls, GTK_ALIGN_START);
                set_halign(win_controls, GTK_ALIGN_END);
                set_margin_top(win_controls, 16);
                set_margin_end(win_controls, 14);
                overlay_add_overlay(overlay, win_controls);

                set_adw_content(win, overlay);
                win
            } else {
                let win = (st.api.gtk_application_window_new)(app);
                (st.api.gtk_window_set_title)(win, title.as_ptr());
                (st.api.gtk_window_set_default_size)(win, 1020, 690);
                (st.api.gtk_widget_set_size_request)(win, 760, 540);

                let hbar = (st.api.gtk_header_bar_new)();
                (st.api.gtk_header_bar_set_show_title_buttons)(hbar, 1);
                let empty_title = (st.api.gtk_box_new)(0, 0);
                (st.api.gtk_header_bar_set_title_widget)(hbar, empty_title);
                (st.api.gtk_window_set_titlebar)(win, hbar);
                (st.api.gtk_window_set_child)(win, webview);
                win
            };

            if let Ok(sig_close) = CString::new("close-request") {
                (st.api.g_signal_connect_data)(
                    win,
                    sig_close.as_ptr(),
                    on_close_request_gtk4 as *const c_void,
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                );
            }

            st.active_webview.store(webview, Ordering::Release);
            st.active_win.store(win, Ordering::Release);

            (st.api.gtk_window_present)(win);
        }
    }

    fn try_prepare_gtk4(start_hidden: bool, quit_existing: bool) -> Result<InstanceDisposition> {
        unsafe {
            let Some(gobj) = dlopen_global("libgobject-2.0.so.0") else {
                bail!("libgobject ausente");
            };
            let Some(gio) = dlopen_global("libgio-2.0.so.0") else {
                bail!("libgio ausente");
            };
            let Some(glib) = dlopen_global("libglib-2.0.so.0") else {
                bail!("libglib ausente");
            };
            // Carrega `libwebkitgtk-6.0.so.4` ANTES de `libgtk-4.so.1` para não poluir símbolos globais
            // caso a distro só possua WebKit2GTK 4.1 e precise cair no fallback GTK3.
            let Some(wk) = dlopen_global("libwebkitgtk-6.0.so.4") else {
                bail!("libwebkitgtk-6.0 ausente");
            };
            let Some(gtk) = dlopen_global("libgtk-4.so.1") else {
                bail!("libgtk-4 ausente");
            };

            let adw = dlopen_global("libadwaita-1.so.0").and_then(|h| {
                Some(AdwApi {
                    adw_application_new: load_sym(h, "adw_application_new")?,
                    adw_application_window_new: load_sym(h, "adw_application_window_new"),
                    adw_application_window_set_content: load_sym(
                        h,
                        "adw_application_window_set_content",
                    ),
                    adw_style_manager_get_default: load_sym(h, "adw_style_manager_get_default")?,
                    adw_style_manager_set_color_scheme: load_sym(
                        h,
                        "adw_style_manager_set_color_scheme",
                    )?,
                    adw_style_manager_get_dark: load_sym(h, "adw_style_manager_get_dark")?,
                })
            });

            let api = Gtk4Api {
                adw,
                gtk_application_new: load_sym(gtk, "gtk_application_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_application_new"))?,
                g_application_register: load_sym(gio, "g_application_register")
                    .ok_or_else(|| anyhow::anyhow!("g_application_register"))?,
                g_application_get_is_remote: load_sym(gio, "g_application_get_is_remote")
                    .ok_or_else(|| anyhow::anyhow!("g_application_get_is_remote"))?,
                g_application_hold: load_sym(gio, "g_application_hold")
                    .ok_or_else(|| anyhow::anyhow!("g_application_hold"))?,
                g_application_quit: load_sym(gio, "g_application_quit")
                    .ok_or_else(|| anyhow::anyhow!("g_application_quit"))?,
                g_application_run: load_sym(gio, "g_application_run")
                    .ok_or_else(|| anyhow::anyhow!("g_application_run"))?,
                g_simple_action_new: load_sym(gio, "g_simple_action_new")
                    .ok_or_else(|| anyhow::anyhow!("g_simple_action_new"))?,
                g_action_map_add_action: load_sym(gio, "g_action_map_add_action")
                    .ok_or_else(|| anyhow::anyhow!("g_action_map_add_action"))?,
                g_action_group_activate_action: load_sym(gio, "g_action_group_activate_action")
                    .ok_or_else(|| anyhow::anyhow!("g_action_group_activate_action"))?,
                g_application_get_dbus_connection: load_sym(
                    gio,
                    "g_application_get_dbus_connection",
                ),
                g_dbus_connection_flush_sync: load_sym(gio, "g_dbus_connection_flush_sync"),
                g_idle_add: load_sym(glib, "g_idle_add")
                    .ok_or_else(|| anyhow::anyhow!("g_idle_add"))?,
                g_error_free: load_sym(glib, "g_error_free"),
                g_signal_connect_data: load_sym(gobj, "g_signal_connect_data")
                    .ok_or_else(|| anyhow::anyhow!("g_signal_connect_data"))?,
                g_object_unref: load_sym(gobj, "g_object_unref")
                    .ok_or_else(|| anyhow::anyhow!("g_object_unref"))?,
                gtk_application_window_new: load_sym(gtk, "gtk_application_window_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_application_window_new"))?,
                gtk_window_set_title: load_sym(gtk, "gtk_window_set_title")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_set_title"))?,
                gtk_window_set_default_size: load_sym(gtk, "gtk_window_set_default_size")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_set_default_size"))?,
                gtk_widget_set_size_request: load_sym(gtk, "gtk_widget_set_size_request")
                    .ok_or_else(|| anyhow::anyhow!("gtk_widget_set_size_request"))?,
                gtk_header_bar_new: load_sym(gtk, "gtk_header_bar_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_header_bar_new"))?,
                gtk_header_bar_set_show_title_buttons: load_sym(
                    gtk,
                    "gtk_header_bar_set_show_title_buttons",
                )
                .ok_or_else(|| anyhow::anyhow!("gtk_header_bar_set_show_title_buttons"))?,
                gtk_header_bar_set_title_widget: load_sym(gtk, "gtk_header_bar_set_title_widget")
                    .ok_or_else(|| anyhow::anyhow!("gtk_header_bar_set_title_widget"))?,
                gtk_box_new: load_sym(gtk, "gtk_box_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_box_new"))?,
                gtk_window_set_titlebar: load_sym(gtk, "gtk_window_set_titlebar")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_set_titlebar"))?,
                gtk_window_set_child: load_sym(gtk, "gtk_window_set_child")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_set_child"))?,
                gtk_window_present: load_sym(gtk, "gtk_window_present")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_present"))?,
                gtk_overlay_new: load_sym(gtk, "gtk_overlay_new"),
                gtk_overlay_set_child: load_sym(gtk, "gtk_overlay_set_child"),
                gtk_overlay_add_overlay: load_sym(gtk, "gtk_overlay_add_overlay"),
                gtk_window_handle_new: load_sym(gtk, "gtk_window_handle_new"),
                gtk_window_handle_set_child: load_sym(gtk, "gtk_window_handle_set_child"),
                gtk_window_controls_new: load_sym(gtk, "gtk_window_controls_new"),
                gtk_widget_set_halign: load_sym(gtk, "gtk_widget_set_halign"),
                gtk_widget_set_valign: load_sym(gtk, "gtk_widget_set_valign"),
                gtk_widget_set_hexpand: load_sym(gtk, "gtk_widget_set_hexpand"),
                gtk_widget_set_margin_top: load_sym(gtk, "gtk_widget_set_margin_top"),
                gtk_widget_set_margin_start: load_sym(gtk, "gtk_widget_set_margin_start"),
                gtk_widget_set_margin_end: load_sym(gtk, "gtk_widget_set_margin_end"),
                gdk_display_get_default: load_sym(gtk, "gdk_display_get_default")
                    .ok_or_else(|| anyhow::anyhow!("gdk_display_get_default"))?,
                gtk_css_provider_new: load_sym(gtk, "gtk_css_provider_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_css_provider_new"))?,
                gtk_css_provider_load_from_string: load_sym(
                    gtk,
                    "gtk_css_provider_load_from_string",
                ),
                gtk_css_provider_load_from_data: load_sym(gtk, "gtk_css_provider_load_from_data"),
                gtk_style_context_add_provider_for_display: load_sym(
                    gtk,
                    "gtk_style_context_add_provider_for_display",
                )
                .ok_or_else(|| anyhow::anyhow!("gtk_style_context_add_provider_for_display"))?,
                webkit_web_view_new: load_sym(wk, "webkit_web_view_new")
                    .ok_or_else(|| anyhow::anyhow!("webkit_web_view_new"))?,
                webkit_web_view_set_background_color: load_sym(
                    wk,
                    "webkit_web_view_set_background_color",
                )
                .ok_or_else(|| anyhow::anyhow!("webkit_web_view_set_background_color"))?,
                webkit_web_view_load_uri: load_sym(wk, "webkit_web_view_load_uri")
                    .ok_or_else(|| anyhow::anyhow!("webkit_web_view_load_uri"))?,
                webkit_web_view_get_settings: load_sym(wk, "webkit_web_view_get_settings"),
                webkit_settings_set_enable_back_forward_navigation_gestures: load_sym(
                    wk,
                    "webkit_settings_set_enable_back_forward_navigation_gestures",
                ),
                webkit_settings_set_enable_developer_extras: load_sym(
                    wk,
                    "webkit_settings_set_enable_developer_extras",
                ),
                webkit_settings_set_enable_page_cache: load_sym(
                    wk,
                    "webkit_settings_set_enable_page_cache",
                ),
                webkit_hit_test_result_context_is_editable: load_sym(
                    wk,
                    "webkit_hit_test_result_context_is_editable",
                ),
            };

            let app_id = CString::new("com.oraculo.Desktop")?;
            let app = if let Some(adw_api) = &api.adw {
                (adw_api.adw_application_new)(app_id.as_ptr(), G_APPLICATION_DEFAULT_FLAGS)
            } else {
                (api.gtk_application_new)(app_id.as_ptr(), G_APPLICATION_DEFAULT_FLAGS)
            };
            if app.is_null() {
                bail!("Falha ao instanciar GtkApplication/AdwApplication");
            }

            let quit_name = CString::new("quit")?;
            let sig_act = CString::new("activate")?;
            let quit_action = (api.g_simple_action_new)(quit_name.as_ptr(), ptr::null());
            if !quit_action.is_null() {
                (api.g_signal_connect_data)(
                    quit_action,
                    sig_act.as_ptr(),
                    on_quit_action_gtk4 as *const c_void,
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                );
                (api.g_action_map_add_action)(app, quit_action);
                (api.g_object_unref)(quit_action);
            }

            let mut gerr: *mut c_void = ptr::null_mut();
            let registered = (api.g_application_register)(app, ptr::null_mut(), &mut gerr) != 0;
            if !gerr.is_null() {
                if let Some(free_err) = api.g_error_free {
                    free_err(gerr);
                }
            }
            if registered {
                if (api.g_application_get_is_remote)(app) != 0 {
                    if quit_existing {
                        (api.g_action_group_activate_action)(
                            app,
                            quit_name.as_ptr(),
                            ptr::null_mut(),
                        );
                        if let (Some(get_conn), Some(flush_sync)) = (
                            api.g_application_get_dbus_connection,
                            api.g_dbus_connection_flush_sync,
                        ) {
                            let conn = get_conn(app);
                            if !conn.is_null() {
                                let mut ferr: *mut c_void = ptr::null_mut();
                                let _ = flush_sync(conn, ptr::null_mut(), &mut ferr);
                                if !ferr.is_null() {
                                    if let Some(free_err) = api.g_error_free {
                                        free_err(ferr);
                                    }
                                }
                            }
                        }
                        (api.g_object_unref)(app);
                        return Ok(InstanceDisposition::ForwardedToExisting);
                    } else if !start_hidden {
                        (api.g_application_run)(app, 0, ptr::null());
                        (api.g_object_unref)(app);
                        return Ok(InstanceDisposition::ForwardedToExisting);
                    }
                    (api.g_object_unref)(app);
                    return Ok(InstanceDisposition::ForwardedToExisting);
                }
            }

            if quit_existing {
                return Ok(InstanceDisposition::ForwardedToExisting);
            }

            let _ = GTK4_STATE.set(Gtk4State {
                api,
                app: AtomicPtr::new(app),
                active_win: AtomicPtr::new(ptr::null_mut()),
                active_webview: AtomicPtr::new(ptr::null_mut()),
                css_provider: AtomicPtr::new(ptr::null_mut()),
                url: OnceLock::new(),
                title: OnceLock::new(),
                skip_first_activate: AtomicBool::new(start_hidden),
                notified_bg_once: AtomicBool::new(false),
                theme_mode: Mutex::new("dark".to_string()),
                effective_dark: AtomicBool::new(true),
            });

            Ok(InstanceDisposition::Primary(WindowRuntime {
                inner: PreparedApp::Gtk4 { start_hidden },
            }))
        }
    }

    fn run_gtk4_loop(_start_hidden: bool, url: &str, title: &str) -> Result<()> {
        let st = GTK4_STATE
            .get()
            .ok_or_else(|| anyhow::anyhow!("Estado GTK4 não inicializado"))?;
        let _ = st.url.set(CString::new(url)?);
        let _ = st.title.set(CString::new(title)?);

        let app = st.app.load(Ordering::Acquire);
        let sig_name = CString::new("activate")?;

        unsafe {
            // Mantém o loop vivo em segundo plano mesmo após destruir a janela no X (`close-request`).
            (st.api.g_application_hold)(app);
            (st.api.g_signal_connect_data)(
                app,
                sig_name.as_ptr(),
                on_activate_gtk4 as *const c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
            );
            (st.api.g_application_run)(app, 0, ptr::null());
            st.app.store(ptr::null_mut(), Ordering::Release);
            (st.api.g_object_unref)(app);
        }
        Ok(())
    }

    // --- Fallback GTK3 + WebKit2GTK 4.1 (distros legadas sem GTK4/WebKit6) ---

    struct Gtk3Api {
        gtk_application_new: unsafe extern "C" fn(*const c_char, c_int) -> *mut c_void,
        g_application_register:
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> c_int,
        g_application_get_is_remote: unsafe extern "C" fn(*mut c_void) -> c_int,
        g_application_hold: unsafe extern "C" fn(*mut c_void),
        g_application_quit: unsafe extern "C" fn(*mut c_void),
        g_application_run: unsafe extern "C" fn(*mut c_void, c_int, *const *const c_char) -> c_int,
        g_simple_action_new: Option<unsafe extern "C" fn(*const c_char, *const c_void) -> *mut c_void>,
        g_action_map_add_action: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        g_action_group_activate_action:
            Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>,
        g_application_get_dbus_connection: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        g_dbus_connection_flush_sync:
            Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> c_int>,
        g_idle_add: unsafe extern "C" fn(unsafe extern "C" fn(*mut c_void) -> c_int, *mut c_void) -> u32,
        g_error_free: Option<unsafe extern "C" fn(*mut c_void)>,
        g_signal_connect_data: unsafe extern "C" fn(
            *mut c_void,
            *const c_char,
            *const c_void,
            *mut c_void,
            *const c_void,
            c_int,
        ) -> c_ulong,
        g_object_unref: unsafe extern "C" fn(*mut c_void),
        gtk_application_window_new: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        gtk_window_set_title: unsafe extern "C" fn(*mut c_void, *const c_char),
        gtk_window_set_default_size: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        gtk_widget_set_size_request: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int)>,
        gtk_container_add: unsafe extern "C" fn(*mut c_void, *mut c_void),
        gtk_widget_show_all: unsafe extern "C" fn(*mut c_void),
        gtk_window_present: unsafe extern "C" fn(*mut c_void),
        webkit_web_view_new: unsafe extern "C" fn() -> *mut c_void,
        webkit_web_view_set_background_color:
            unsafe extern "C" fn(*mut c_void, *const GdkRgbaF64),
        webkit_web_view_load_uri: unsafe extern "C" fn(*mut c_void, *const c_char),
    }

    struct Gtk3State {
        api: Gtk3Api,
        app: AtomicPtr<c_void>,
        active_win: AtomicPtr<c_void>,
        url: OnceLock<CString>,
        title: OnceLock<CString>,
        skip_first_activate: AtomicBool,
        effective_dark: AtomicBool,
    }

    unsafe impl Send for Gtk3State {}
    unsafe impl Sync for Gtk3State {}

    static GTK3_STATE: OnceLock<Gtk3State> = OnceLock::new();

    unsafe extern "C" fn on_quit_action_gtk3(
        _action: *mut c_void,
        _param: *mut c_void,
        _user_data: *mut c_void,
    ) {
        crate::webview::FORCE_QUIT_REQUESTED.store(true, Ordering::Release);
        if let Some(st) = GTK3_STATE.get() {
            let app = st.app.load(Ordering::Acquire);
            if !app.is_null() {
                unsafe { (st.api.g_application_quit)(app) };
            }
        }
    }

    unsafe extern "C" fn on_idle_quit_gtk3(_data: *mut c_void) -> c_int {
        if let Some(st) = GTK3_STATE.get() {
            let app = st.app.load(Ordering::Acquire);
            if !app.is_null() {
                unsafe { (st.api.g_application_quit)(app) };
            }
        }
        0
    }

    unsafe extern "C" fn on_destroy_gtk3(_win: *mut c_void, _user_data: *mut c_void) {
        if let Some(st) = GTK3_STATE.get() {
            st.active_win.store(ptr::null_mut(), Ordering::Release);
            if crate::webview::is_attached_to_daemon() {
                let app = st.app.load(Ordering::Acquire);
                if !app.is_null() {
                    unsafe { (st.api.g_application_quit)(app) };
                }
            }
        }
    }

    unsafe extern "C" fn on_activate_gtk3(app: *mut c_void, _user_data: *mut c_void) {
        let Some(st) = GTK3_STATE.get() else {
            return;
        };
        if st.skip_first_activate.swap(false, Ordering::SeqCst) {
            return;
        }
        let existing = st.active_win.load(Ordering::Acquire);
        if !existing.is_null() {
            unsafe { (st.api.gtk_window_present)(existing) };
            return;
        }
        let (Some(url), Some(title)) = (st.url.get(), st.title.get()) else {
            return;
        };
        unsafe {
            let win = (st.api.gtk_application_window_new)(app);
            (st.api.gtk_window_set_title)(win, title.as_ptr());
            (st.api.gtk_window_set_default_size)(win, 1020, 690);
            if let Some(set_min) = st.api.gtk_widget_set_size_request {
                set_min(win, 760, 540);
            }

            let webview = (st.api.webkit_web_view_new)();
            let is_dark = st.effective_dark.load(Ordering::Relaxed);
            let bg = if is_dark {
                GdkRgbaF64 {
                    red: 19.0 / 255.0,
                    green: 19.0 / 255.0,
                    blue: 20.0 / 255.0,
                    alpha: 1.0,
                }
            } else {
                GdkRgbaF64 {
                    red: 248.0 / 255.0,
                    green: 249.0 / 255.0,
                    blue: 250.0 / 255.0,
                    alpha: 1.0,
                }
            };
            (st.api.webkit_web_view_set_background_color)(webview, &bg);
            (st.api.webkit_web_view_load_uri)(webview, url.as_ptr());

            if let Ok(sig_destroy) = CString::new("destroy") {
                (st.api.g_signal_connect_data)(
                    win,
                    sig_destroy.as_ptr(),
                    on_destroy_gtk3 as *const c_void,
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                );
            }
            st.active_win.store(win, Ordering::Release);
            (st.api.gtk_container_add)(win, webview);
            (st.api.gtk_widget_show_all)(win);
        }
    }

    fn try_prepare_gtk3(start_hidden: bool, quit_existing: bool) -> Result<InstanceDisposition> {
        unsafe {
            let Some(gobj) = dlopen_global("libgobject-2.0.so.0") else {
                bail!("libgobject ausente");
            };
            let Some(gio) = dlopen_global("libgio-2.0.so.0") else {
                bail!("libgio ausente");
            };
            let Some(glib) = dlopen_global("libglib-2.0.so.0") else {
                bail!("libglib ausente");
            };
            let Some(wk) = dlopen_global("libwebkit2gtk-4.1.so.0")
                .or_else(|| dlopen_global("libwebkit2gtk-4.0.so.37"))
            else {
                bail!("libwebkit2gtk ausente");
            };
            let Some(gtk) = dlopen_global("libgtk-3.so.0") else {
                bail!("libgtk-3 ausente");
            };

            let api = Gtk3Api {
                gtk_application_new: load_sym(gtk, "gtk_application_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_application_new"))?,
                g_application_register: load_sym(gio, "g_application_register")
                    .ok_or_else(|| anyhow::anyhow!("g_application_register"))?,
                g_application_get_is_remote: load_sym(gio, "g_application_get_is_remote")
                    .ok_or_else(|| anyhow::anyhow!("g_application_get_is_remote"))?,
                g_application_hold: load_sym(gio, "g_application_hold")
                    .ok_or_else(|| anyhow::anyhow!("g_application_hold"))?,
                g_application_quit: load_sym(gio, "g_application_quit")
                    .ok_or_else(|| anyhow::anyhow!("g_application_quit"))?,
                g_application_run: load_sym(gio, "g_application_run")
                    .ok_or_else(|| anyhow::anyhow!("g_application_run"))?,
                g_simple_action_new: load_sym(gio, "g_simple_action_new"),
                g_action_map_add_action: load_sym(gio, "g_action_map_add_action"),
                g_action_group_activate_action: load_sym(gio, "g_action_group_activate_action"),
                g_application_get_dbus_connection: load_sym(
                    gio,
                    "g_application_get_dbus_connection",
                ),
                g_dbus_connection_flush_sync: load_sym(gio, "g_dbus_connection_flush_sync"),
                g_idle_add: load_sym(glib, "g_idle_add")
                    .ok_or_else(|| anyhow::anyhow!("g_idle_add"))?,
                g_error_free: load_sym(glib, "g_error_free"),
                g_signal_connect_data: load_sym(gobj, "g_signal_connect_data")
                    .ok_or_else(|| anyhow::anyhow!("g_signal_connect_data"))?,
                g_object_unref: load_sym(gobj, "g_object_unref")
                    .ok_or_else(|| anyhow::anyhow!("g_object_unref"))?,
                gtk_application_window_new: load_sym(gtk, "gtk_application_window_new")
                    .ok_or_else(|| anyhow::anyhow!("gtk_application_window_new"))?,
                gtk_window_set_title: load_sym(gtk, "gtk_window_set_title")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_set_title"))?,
                gtk_window_set_default_size: load_sym(gtk, "gtk_window_set_default_size")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_set_default_size"))?,
                gtk_widget_set_size_request: load_sym(gtk, "gtk_widget_set_size_request"),
                gtk_container_add: load_sym(gtk, "gtk_container_add")
                    .ok_or_else(|| anyhow::anyhow!("gtk_container_add"))?,
                gtk_widget_show_all: load_sym(gtk, "gtk_widget_show_all")
                    .ok_or_else(|| anyhow::anyhow!("gtk_widget_show_all"))?,
                gtk_window_present: load_sym(gtk, "gtk_window_present")
                    .ok_or_else(|| anyhow::anyhow!("gtk_window_present"))?,
                webkit_web_view_new: load_sym(wk, "webkit_web_view_new")
                    .ok_or_else(|| anyhow::anyhow!("webkit_web_view_new"))?,
                webkit_web_view_set_background_color: load_sym(
                    wk,
                    "webkit_web_view_set_background_color",
                )
                .ok_or_else(|| anyhow::anyhow!("webkit_web_view_set_background_color"))?,
                webkit_web_view_load_uri: load_sym(wk, "webkit_web_view_load_uri")
                    .ok_or_else(|| anyhow::anyhow!("webkit_web_view_load_uri"))?,
            };

            let app_id = CString::new("com.oraculo.Desktop")?;
            let app = (api.gtk_application_new)(app_id.as_ptr(), G_APPLICATION_DEFAULT_FLAGS);
            if app.is_null() {
                bail!("Falha ao instanciar GtkApplication GTK3");
            }

            let quit_name = CString::new("quit")?;
            let sig_act = CString::new("activate")?;
            if let (Some(new_action), Some(add_action)) =
                (api.g_simple_action_new, api.g_action_map_add_action)
            {
                let quit_action = new_action(quit_name.as_ptr(), ptr::null());
                if !quit_action.is_null() {
                    (api.g_signal_connect_data)(
                        quit_action,
                        sig_act.as_ptr(),
                        on_quit_action_gtk3 as *const c_void,
                        ptr::null_mut(),
                        ptr::null(),
                        0,
                    );
                    add_action(app, quit_action);
                    (api.g_object_unref)(quit_action);
                }
            }

            let mut gerr: *mut c_void = ptr::null_mut();
            let registered = (api.g_application_register)(app, ptr::null_mut(), &mut gerr) != 0;
            if !gerr.is_null() {
                if let Some(free_err) = api.g_error_free {
                    free_err(gerr);
                }
            }
            if registered {
                if (api.g_application_get_is_remote)(app) != 0 {
                    if quit_existing {
                        if let Some(activate_act) = api.g_action_group_activate_action {
                            activate_act(app, quit_name.as_ptr(), ptr::null_mut());
                        }
                        if let (Some(get_conn), Some(flush_sync)) = (
                            api.g_application_get_dbus_connection,
                            api.g_dbus_connection_flush_sync,
                        ) {
                            let conn = get_conn(app);
                            if !conn.is_null() {
                                let mut ferr: *mut c_void = ptr::null_mut();
                                let _ = flush_sync(conn, ptr::null_mut(), &mut ferr);
                                if !ferr.is_null() {
                                    if let Some(free_err) = api.g_error_free {
                                        free_err(ferr);
                                    }
                                }
                            }
                        }
                        (api.g_object_unref)(app);
                        return Ok(InstanceDisposition::ForwardedToExisting);
                    } else if !start_hidden {
                        (api.g_application_run)(app, 0, ptr::null());
                        (api.g_object_unref)(app);
                        return Ok(InstanceDisposition::ForwardedToExisting);
                    }
                    (api.g_object_unref)(app);
                    return Ok(InstanceDisposition::ForwardedToExisting);
                }
            }
            if quit_existing {
                (api.g_object_unref)(app);
                return Ok(InstanceDisposition::ForwardedToExisting);
            }

            let _ = GTK3_STATE.set(Gtk3State {
                api,
                app: AtomicPtr::new(app),
                active_win: AtomicPtr::new(ptr::null_mut()),
                url: OnceLock::new(),
                title: OnceLock::new(),
                skip_first_activate: AtomicBool::new(start_hidden),
                effective_dark: AtomicBool::new(true),
            });

            Ok(InstanceDisposition::Primary(WindowRuntime {
                inner: PreparedApp::Gtk3 { start_hidden },
            }))
        }
    }

    fn run_gtk3_loop(_start_hidden: bool, url: &str, title: &str) -> Result<()> {
        let st = GTK3_STATE
            .get()
            .ok_or_else(|| anyhow::anyhow!("Estado GTK3 não inicializado"))?;
        let _ = st.url.set(CString::new(url)?);
        let _ = st.title.set(CString::new(title)?);

        let app = st.app.load(Ordering::Acquire);
        let sig_name = CString::new("activate")?;
        unsafe {
            (st.api.g_application_hold)(app);
            (st.api.g_signal_connect_data)(
                app,
                sig_name.as_ptr(),
                on_activate_gtk3 as *const c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
            );
            (st.api.g_application_run)(app, 0, ptr::null());
            st.app.store(ptr::null_mut(), Ordering::Release);
            (st.api.g_object_unref)(app);
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use anyhow::{Context, Result};
    use directories::ProjectDirs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    static QUIT_NOTIFY: OnceLock<Mutex<Option<u32>>> = OnceLock::new();

    pub fn run_windows_session(url: &str, start_hidden: bool) -> Result<()> {
        let quit_lock = QUIT_NOTIFY.get_or_init(|| Mutex::new(None));
        if start_hidden {
            // No modo oculto de login, mantém o servidor P2P ativo até receber request_quit().
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if quit_lock.lock().ok().and_then(|g| *g) == Some(0) {
                    break;
                }
            }
            return Ok(());
        }

        let edge_bin =
            find_edge_binary().context("Microsoft Edge (WebView2) não encontrado no Windows")?;
        let profile_dir = ProjectDirs::from("com", "oraculo", "oraculo")
            .map(|d| d.data_local_dir().join("desktop-webview"))
            .unwrap_or_else(|| PathBuf::from("."));
        let _ = std::fs::create_dir_all(&profile_dir);

        let mut child = Command::new(edge_bin)
            .arg(format!("--app={url}"))
            .arg("--window-size=1020,690")
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .spawn()
            .context("Falha ao abrir janela do Oráculo Desktop")?;

        if let Ok(mut guard) = quit_lock.lock() {
            *guard = Some(child.id());
        }
        let _ = child.wait();
        Ok(())
    }

    pub fn request_quit() {
        if let Some(lock) = QUIT_NOTIFY.get() {
            if let Ok(mut guard) = lock.lock() {
                if let Some(pid) = guard.take() {
                    if pid != 0 {
                        let _ = Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/T", "/F"])
                            .status();
                    }
                }
                *guard = Some(0);
            }
        }
    }

    fn find_edge_binary() -> Option<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(pf86).join(r"Microsoft\Edge\Application\msedge.exe"));
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            candidates.push(PathBuf::from(pf).join(r"Microsoft\Edge\Application\msedge.exe"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join(r"Microsoft\Edge\Application\msedge.exe"));
        }
        candidates.into_iter().find(|p| p.exists())
    }
}
