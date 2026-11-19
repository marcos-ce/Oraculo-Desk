use crate::config::{HostPaths, write_private_file};
use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

pub const PORTAL_APP_ID: &str = "com.oraculo.Desktop";

const PORTAL_VAULT_VERSION: u32 = 1;
const LOCK_POLICY_CACHE_TTL: Duration = Duration::from_secs(15);

#[cfg(target_os = "linux")]
const INTERACTIVE_PORTAL_TIMEOUT: Duration = Duration::from_secs(90);
#[cfg(target_os = "linux")]
const LOCK_CHECK_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalMonitor {
    pub node_id: u32,
    pub pw_source_arg: String,
    pub source_size: Option<(u16, u16)>,
    pub width: u16,
    pub height: u16,
    pub origin_x: i32,
    pub origin_y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayTopology {
    pub id: String,
    pub label: String,
    pub active_outputs: u8,
    pub connected_outputs: u8,
}

#[derive(Debug, Clone)]
pub struct PortalVaultPaths {
    pub portal_tokens_file: PathBuf,
    pub legacy_remote_desktop_token_file: PathBuf,
    pub legacy_screencast_token_file: PathBuf,
}

impl From<&HostPaths> for PortalVaultPaths {
    fn from(paths: &HostPaths) -> Self {
        Self {
            portal_tokens_file: paths.portal_tokens_file.clone(),
            legacy_remote_desktop_token_file: paths.remote_desktop_token_file.clone(),
            legacy_screencast_token_file: paths.screencast_token_file.clone(),
        }
    }
}

impl PortalVaultPaths {
    pub fn standalone_legacy(screencast_token_file: &Path) -> Self {
        let base_dir = screencast_token_file
            .parent()
            .unwrap_or_else(|| Path::new("."));
        Self {
            portal_tokens_file: base_dir.join("portal_tokens.json"),
            legacy_remote_desktop_token_file: base_dir.join("remote_desktop_token"),
            legacy_screencast_token_file: screencast_token_file.to_path_buf(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PortalTokenVault {
    pub version: u32,
    /// Token independente de topologia para entrada remota (teclado/mouse).
    pub input_token: Option<String>,
    /// Tokens unificados (RemoteDesktop + ScreenCast) indexados por topologia de monitores.
    pub combined_tokens: BTreeMap<String, String>,
    /// Tokens somente ScreenCast indexados por topologia de monitores.
    pub screencast_tokens: BTreeMap<String, String>,
}

impl PortalTokenVault {
    pub fn load_or_migrate(paths: &PortalVaultPaths, current_topology_id: &str) -> Result<Self> {
        if let Some(vault) = read_vault_file(&paths.portal_tokens_file) {
            return Ok(vault);
        }

        let mut vault = Self {
            version: PORTAL_VAULT_VERSION,
            ..Self::default()
        };
        let mut migrated = false;

        if let Some(tok) = read_trimmed_token(&paths.legacy_remote_desktop_token_file) {
            vault.input_token = Some(tok);
            migrated = true;
        }
        if let Some(tok) = read_trimmed_token(&paths.legacy_screencast_token_file) {
            vault
                .screencast_tokens
                .insert(current_topology_id.to_string(), tok);
            migrated = true;
        }

        if migrated {
            let _ = vault.save(&paths.portal_tokens_file);
        }
        Ok(vault)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let mut to_save = self.clone();
        to_save.version = PORTAL_VAULT_VERSION;
        let payload = serde_json::to_vec_pretty(&to_save)
            .context("Falha ao serializar cofre de permissões do portal")?;
        write_private_file(path, &payload)
    }

    pub fn combined_token(&self, topology_id: &str) -> Option<String> {
        self.combined_tokens.get(topology_id).cloned()
    }

    pub fn screencast_token(&self, topology_id: &str) -> Option<String> {
        self.screencast_tokens.get(topology_id).cloned()
    }

    pub fn input_token(&self) -> Option<String> {
        self.input_token.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalAuthorizeOutcome {
    pub readiness: PortalReadiness,
    pub authorized_monitors: u8,
    pub available_monitors: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalReadiness {
    pub ready_for_unattended: bool,
    pub input_authorized: bool,
    pub screen_authorized: bool,
    pub combined_authorized: bool,
    pub topology_id: String,
    pub topology_label: String,
    pub connected_monitors: u8,
    pub saved_topology_profiles: usize,
    pub screen_locked: bool,
    pub gnome_lock_enabled: bool,
    pub gnome_idle_delay_secs: u32,
    pub desktop_entry_present: bool,
}

#[cfg(target_os = "linux")]
pub struct ActiveRemoteDesktop {
    pub proxy: ashpd::desktop::remote_desktop::RemoteDesktop,
    pub session: ashpd::desktop::Session<ashpd::desktop::remote_desktop::RemoteDesktop>,
    pub monitors: Vec<PortalMonitor>,
    pub topology_id: String,
    pub has_screencast: bool,
}

#[cfg(target_os = "linux")]
pub struct ActiveScreencastOnly {
    pub proxy: ashpd::desktop::screencast::Screencast,
    pub session: ashpd::desktop::Session<ashpd::desktop::screencast::Screencast>,
    pub monitors: Vec<PortalMonitor>,
    pub topology_id: String,
}

#[cfg(target_os = "linux")]
pub struct PortalScreenGrant {
    pub fd: std::os::fd::OwnedFd,
    pub monitors: Vec<PortalMonitor>,
    pub topology: DisplayTopology,
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
enum ActivePortalSession {
    RemoteDesktop(Arc<ActiveRemoteDesktop>),
    ScreencastOnly(Arc<ActiveScreencastOnly>),
}

#[cfg(target_os = "linux")]
impl ActivePortalSession {
    async fn close(self) {
        match self {
            Self::RemoteDesktop(rd) => {
                let _ = rd.session.close().await;
            }
            Self::ScreencastOnly(sc) => {
                let _ = sc.session.close().await;
            }
        }
    }
}

#[cfg(target_os = "linux")]
struct PendingSessionGuard<T: ashpd::desktop::SessionPortal + Send + Sync + 'static> {
    session: Option<ashpd::desktop::Session<T>>,
}

#[cfg(target_os = "linux")]
impl<T: ashpd::desktop::SessionPortal + Send + Sync + 'static> PendingSessionGuard<T> {
    fn new(session: ashpd::desktop::Session<T>) -> Self {
        Self {
            session: Some(session),
        }
    }

    fn disarm(mut self) -> ashpd::desktop::Session<T> {
        self.session.take().expect("portal session ainda presente")
    }
}

#[cfg(target_os = "linux")]
impl<T: ashpd::desktop::SessionPortal + Send + Sync + 'static> Drop for PendingSessionGuard<T> {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _ = session.close().await;
                });
            }
        }
    }
}

pub struct PortalCoordinator {
    paths: PortalVaultPaths,
    vault: StdMutex<PortalTokenVault>,
    lock_policy_cache: StdMutex<Option<(Instant, (bool, u32))>>,
    #[cfg(target_os = "linux")]
    open_lock: tokio::sync::Mutex<()>,
    #[cfg(target_os = "linux")]
    active: tokio::sync::RwLock<Option<ActivePortalSession>>,
}

impl PortalCoordinator {
    pub fn new(paths: PortalVaultPaths) -> Self {
        let topology = detect_display_topology();
        let vault = PortalTokenVault::load_or_migrate(&paths, &topology.id).unwrap_or_default();
        Self {
            paths,
            vault: StdMutex::new(vault),
            lock_policy_cache: StdMutex::new(None),
            #[cfg(target_os = "linux")]
            open_lock: tokio::sync::Mutex::new(()),
            #[cfg(target_os = "linux")]
            active: tokio::sync::RwLock::new(None),
        }
    }

    pub fn from_host_paths(paths: &HostPaths) -> Self {
        Self::new(PortalVaultPaths::from(paths))
    }

    pub async fn readiness(&self) -> PortalReadiness {
        let topology = detect_display_topology();
        let screen_locked = is_gnome_screen_locked().await;
        let (gnome_lock_enabled, gnome_idle_delay_secs) =
            self.cached_gnome_lock_policy().await;
        let desktop_entry_present = has_desktop_entry();

        let (
            input_authorized,
            screen_authorized,
            combined_authorized,
            saved_topology_profiles,
        ) = {
            let guard = self.vault.lock().unwrap_or_else(|e| e.into_inner());
            let combined = guard.combined_tokens.contains_key(&topology.id);
            let screencast = guard.screencast_tokens.contains_key(&topology.id);
            let input = combined || guard.input_token.is_some();
            let screen = combined || screencast;
            let profiles = guard.combined_tokens.len().max(guard.screencast_tokens.len());
            (input, screen, combined, profiles)
        };

        PortalReadiness {
            ready_for_unattended: input_authorized && screen_authorized && !screen_locked,
            input_authorized,
            screen_authorized,
            combined_authorized,
            topology_id: topology.id,
            topology_label: topology.label,
            connected_monitors: topology.active_outputs,
            saved_topology_profiles,
            screen_locked,
            gnome_lock_enabled,
            gnome_idle_delay_secs,
            desktop_entry_present,
        }
    }

    async fn cached_gnome_lock_policy(&self) -> (bool, u32) {
        if let Ok(guard) = self.lock_policy_cache.lock() {
            if let Some((at, policy)) = *guard {
                if at.elapsed() < LOCK_POLICY_CACHE_TTL {
                    return policy;
                }
            }
        }
        let fresh = tokio::task::spawn_blocking(read_gnome_lock_policy)
            .await
            .unwrap_or((false, 0));
        if let Ok(mut guard) = self.lock_policy_cache.lock() {
            *guard = Some((Instant::now(), fresh));
        }
        fresh
    }

    pub fn topology(&self) -> DisplayTopology {
        detect_display_topology()
    }

    #[cfg(not(target_os = "linux"))]
    pub async fn authorize_interactive(
        &self,
        _allow_input: bool,
        _allow_screen: bool,
    ) -> Result<PortalAuthorizeOutcome> {
        let readiness = self.readiness().await;
        let available = readiness.connected_monitors.max(1);
        Ok(PortalAuthorizeOutcome {
            readiness,
            authorized_monitors: available,
            available_monitors: available,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub async fn release_active_session(&self) {}

    #[cfg(target_os = "linux")]
    pub async fn release_active_session(&self) {
        let prev = {
            let mut active = self.active.write().await;
            active.take()
        };
        if let Some(session) = prev {
            session.close().await;
        }
    }

    #[cfg(target_os = "linux")]
    pub async fn authorize_interactive(
        &self,
        allow_input: bool,
        allow_screen: bool,
    ) -> Result<PortalAuthorizeOutcome> {
        if !allow_input && !allow_screen {
            bail!("Ative captura de tela ou mouse/teclado em Acesso antes de autorizar.");
        }
        if is_gnome_screen_locked().await {
            bail!("Tela do computador bloqueada. Desbloqueie a sessão antes de autorizar.");
        }

        let _open_guard = self.open_lock.lock().await;
        self.clear_active_session().await;

        let topology = detect_display_topology();
        let authorized_monitors = if allow_input && allow_screen {
            let rd = self.open_combined_locked(&topology, false).await?;
            rd.monitors.len().min(u8::MAX as usize) as u8
        } else if allow_screen {
            let sc = self.open_screencast_only_locked(&topology, false).await?;
            sc.monitors.len().min(u8::MAX as usize) as u8
        } else {
            self.open_input_only_locked(false).await?;
            0
        };

        // Fecha a sessão interativa imediatamente após obter e gravar o restore_token:
        // evita manter o indicador laranja de compartilhamento ativo no GNOME sem cliente conectado.
        self.clear_active_session().await;
        let readiness = self.readiness().await;
        let available_monitors = readiness
            .connected_monitors
            .max(authorized_monitors)
            .max(1);
        Ok(PortalAuthorizeOutcome {
            readiness,
            authorized_monitors,
            available_monitors,
        })
    }

    #[cfg(target_os = "linux")]
    pub async fn ensure_remote_desktop(&self, allow_screen: bool) -> Result<Arc<ActiveRemoteDesktop>> {
        if let Some(ActivePortalSession::RemoteDesktop(rd)) = self.active.read().await.clone() {
            return Ok(rd);
        }

        let _open_guard = self.open_lock.lock().await;
        if let Some(ActivePortalSession::RemoteDesktop(rd)) = self.active.read().await.clone() {
            return Ok(rd);
        }

        self.open_remote_desktop_for_input_locked(allow_screen).await
    }

    #[cfg(target_os = "linux")]
    pub async fn reopen_remote_desktop(
        &self,
        allow_screen: bool,
        failed: &Arc<ActiveRemoteDesktop>,
    ) -> Result<Arc<ActiveRemoteDesktop>> {
        let _open_guard = self.open_lock.lock().await;
        if let Some(ActivePortalSession::RemoteDesktop(current)) = self.active.read().await.clone() {
            if !Arc::ptr_eq(&current, failed) {
                return Ok(current);
            }
        }
        self.clear_active_session().await;
        self.open_remote_desktop_for_input_locked(allow_screen).await
    }

    #[cfg(target_os = "linux")]
    pub async fn open_pipewire_for_screen(
        &self,
        allow_input: bool,
        force_reauth: bool,
    ) -> Result<PortalScreenGrant> {
        let topology = detect_display_topology();

        if !force_reauth {
            if let Some(grant) = self.try_open_from_active(&topology, allow_input).await {
                return Ok(grant);
            }
        }

        let _open_guard = self.open_lock.lock().await;
        if !force_reauth {
            if let Some(grant) = self.try_open_from_active(&topology, allow_input).await {
                return Ok(grant);
            }
        }

        if is_gnome_screen_locked().await {
            bail!(
                "Tela do computador bloqueada. Desbloqueie a sessão para compartilhar a tela."
            );
        }

        self.clear_active_session().await;
        let use_saved_token = !force_reauth;

        if allow_input {
            let rd = self.open_combined_locked(&topology, use_saved_token).await?;
            let fd = open_pipewire_remote_rd(&rd.session).await?;
            Ok(PortalScreenGrant {
                fd,
                monitors: rd.monitors.clone(),
                topology,
            })
        } else {
            let sc = self
                .open_screencast_only_locked(&topology, use_saved_token)
                .await?;
            let fd = open_pipewire_remote_sc(&sc.proxy, &sc.session).await?;
            Ok(PortalScreenGrant {
                fd,
                monitors: sc.monitors.clone(),
                topology,
            })
        }
    }

    #[cfg(target_os = "linux")]
    async fn try_open_from_active(
        &self,
        topology: &DisplayTopology,
        allow_input: bool,
    ) -> Option<PortalScreenGrant> {
        let active = self.active.read().await.clone()?;
        match active {
            ActivePortalSession::RemoteDesktop(rd)
                if rd.has_screencast && rd.topology_id == topology.id && !rd.monitors.is_empty() =>
            {
                let fd = open_pipewire_remote_rd(&rd.session).await.ok()?;
                Some(PortalScreenGrant {
                    fd,
                    monitors: rd.monitors.clone(),
                    topology: topology.clone(),
                })
            }
            ActivePortalSession::ScreencastOnly(sc)
                if !allow_input && sc.topology_id == topology.id && !sc.monitors.is_empty() =>
            {
                let fd = open_pipewire_remote_sc(&sc.proxy, &sc.session).await.ok()?;
                Some(PortalScreenGrant {
                    fd,
                    monitors: sc.monitors.clone(),
                    topology: topology.clone(),
                })
            }
            _ => None,
        }
    }

    #[cfg(target_os = "linux")]
    async fn clear_active_session(&self) {
        let prev = {
            let mut active = self.active.write().await;
            active.take()
        };
        if let Some(session) = prev {
            session.close().await;
        }
    }

    #[cfg(target_os = "linux")]
    async fn open_remote_desktop_for_input_locked(
        &self,
        allow_screen: bool,
    ) -> Result<Arc<ActiveRemoteDesktop>> {
        if is_gnome_screen_locked().await {
            bail!(
                "Tela do computador bloqueada. Desbloqueie a sessão para usar mouse e teclado remotos."
            );
        }

        self.clear_active_session().await;
        let topology = detect_display_topology();
        let (has_combined, has_input) = {
            let guard = self.vault.lock().unwrap_or_else(|e| e.into_inner());
            (
                guard.combined_tokens.contains_key(&topology.id),
                guard.input_token.is_some(),
            )
        };

        // Se já existe token combinado para esta topologia e tela estiver habilitada,
        // abre unificado sem prompt e já prepara a tela também.
        // Se só existe token de input, mantém Touchpad silencioso com InputOnly.
        // Se não há nenhum token ainda e tela estiver habilitada, solicita permissão unificada em 1 passo.
        if allow_screen && (has_combined || !has_input) {
            self.open_combined_locked(&topology, has_combined).await
        } else {
            self.open_input_only_locked(has_input).await
        }
    }

    #[cfg(target_os = "linux")]
    async fn open_combined_locked(
        &self,
        topology: &DisplayTopology,
        use_saved_token: bool,
    ) -> Result<Arc<ActiveRemoteDesktop>> {
        use ashpd::desktop::remote_desktop::{
            DeviceType, RemoteDesktop, SelectDevicesOptions,
        };
        use ashpd::desktop::screencast::{
            CursorMode, Screencast, SelectSourcesOptions, SourceType,
        };
        use ashpd::desktop::PersistMode;

        ensure_portal_app_registered().await;

        let rd = RemoteDesktop::new()
            .await
            .context("Portal RemoteDesktop indisponível no compositor Wayland")?;
        let sc = Screencast::new()
            .await
            .context("Portal ScreenCast indisponível no compositor Wayland")?;

        let session = rd
            .create_session(Default::default())
            .await
            .context("Falha ao criar sessão unificada RemoteDesktop/ScreenCast")?;
        let guard = PendingSessionGuard::new(session);
        let session_ref = guard.session.as_ref().expect("sessão recém-criada");

        let saved_token = if use_saved_token {
            let g = self.vault.lock().unwrap_or_else(|e| e.into_inner());
            g.combined_token(&topology.id)
        } else {
            None
        };

        let mut dev_opts = SelectDevicesOptions::default()
            .set_devices(DeviceType::KEYBOARD | DeviceType::POINTER)
            .set_persist_mode(PersistMode::ExplicitlyRevoked);
        if let Some(tok) = saved_token.as_deref() {
            dev_opts = dev_opts.set_restore_token(tok);
        }

        rd.select_devices(session_ref, dev_opts)
            .await
            .context("Falha ao configurar dispositivos no portal RemoteDesktop")?;

        let src_opts = SelectSourcesOptions::default()
            .set_sources(SourceType::MONITOR)
            .set_multiple(true)
            .set_cursor_mode(CursorMode::EMBEDDED);
        sc.select_sources(session_ref, src_opts)
            .await
            .context("Falha ao configurar seleção de telas na sessão unificada")?;

        let start_req = Self::await_portal_start(
            rd.start(session_ref, None, Default::default()),
            use_saved_token && saved_token.is_some(),
            "permissão de tela e controle remoto",
            || self.clear_combined_token(&topology.id),
        )
        .await?;

        let selected = match start_req.response() {
            Ok(resp) => resp,
            Err(err) => {
                let locked = is_gnome_screen_locked().await;
                if use_saved_token && saved_token.is_some() && !locked {
                    self.clear_combined_token(&topology.id);
                }
                return Err(format_portal_error(
                    err,
                    locked,
                    "permissão de tela e controle remoto",
                ));
            }
        };

        let monitors = parse_portal_streams(selected.streams());
        if monitors.is_empty() {
            return Err(anyhow::anyhow!(
                "Nenhum monitor foi selecionado para compartilhamento."
            ));
        }

        if let Some(new_token) = selected.restore_token() {
            self.store_combined_token(&topology.id, Some(new_token));
        } else if use_saved_token && saved_token.is_some() {
            self.store_combined_token(&topology.id, None);
        }

        let session = guard.disarm();
        let active_rd = Arc::new(ActiveRemoteDesktop {
            proxy: rd,
            session,
            monitors,
            topology_id: topology.id.clone(),
            has_screencast: true,
        });
        *self.active.write().await = Some(ActivePortalSession::RemoteDesktop(active_rd.clone()));
        Ok(active_rd)
    }

    #[cfg(target_os = "linux")]
    async fn open_input_only_locked(
        &self,
        use_saved_token: bool,
    ) -> Result<Arc<ActiveRemoteDesktop>> {
        use ashpd::desktop::remote_desktop::{DeviceType, RemoteDesktop, SelectDevicesOptions};
        use ashpd::desktop::PersistMode;

        ensure_portal_app_registered().await;

        let rd = RemoteDesktop::new()
            .await
            .context("Portal RemoteDesktop indisponível no compositor Wayland")?;
        let session = rd
            .create_session(Default::default())
            .await
            .context("Falha ao criar sessão RemoteDesktop")?;
        let guard = PendingSessionGuard::new(session);
        let session_ref = guard.session.as_ref().expect("sessão recém-criada");

        let saved_token = if use_saved_token {
            let g = self.vault.lock().unwrap_or_else(|e| e.into_inner());
            g.input_token()
        } else {
            None
        };

        let mut dev_opts = SelectDevicesOptions::default()
            .set_devices(DeviceType::KEYBOARD | DeviceType::POINTER)
            .set_persist_mode(PersistMode::ExplicitlyRevoked);
        if let Some(tok) = saved_token.as_deref() {
            dev_opts = dev_opts.set_restore_token(tok);
        }

        rd.select_devices(session_ref, dev_opts)
            .await
            .context("Falha ao selecionar dispositivos no portal RemoteDesktop")?;

        let start_req = Self::await_portal_start(
            rd.start(session_ref, None, Default::default()),
            use_saved_token && saved_token.is_some(),
            "permissão de mouse e teclado",
            || self.store_input_token(None),
        )
        .await?;

        let selected = match start_req.response() {
            Ok(resp) => resp,
            Err(err) => {
                let locked = is_gnome_screen_locked().await;
                if use_saved_token && saved_token.is_some() && !locked {
                    self.store_input_token(None);
                }
                return Err(format_portal_error(
                    err,
                    locked,
                    "permissão de mouse e teclado",
                ));
            }
        };

        if let Some(new_token) = selected.restore_token() {
            self.store_input_token(Some(new_token));
        } else if use_saved_token && saved_token.is_some() {
            self.store_input_token(None);
        }

        let session = guard.disarm();
        let active_rd = Arc::new(ActiveRemoteDesktop {
            proxy: rd,
            session,
            monitors: Vec::new(),
            topology_id: String::new(),
            has_screencast: false,
        });
        *self.active.write().await = Some(ActivePortalSession::RemoteDesktop(active_rd.clone()));
        Ok(active_rd)
    }

    #[cfg(target_os = "linux")]
    async fn open_screencast_only_locked(
        &self,
        topology: &DisplayTopology,
        use_saved_token: bool,
    ) -> Result<Arc<ActiveScreencastOnly>> {
        use ashpd::desktop::screencast::{
            CursorMode, Screencast, SelectSourcesOptions, SourceType,
        };
        use ashpd::desktop::PersistMode;

        ensure_portal_app_registered().await;

        let sc = Screencast::new()
            .await
            .context("Portal ScreenCast indisponível no compositor Wayland")?;
        let session = sc
            .create_session(Default::default())
            .await
            .context("Falha ao criar sessão ScreenCast")?;
        let guard = PendingSessionGuard::new(session);
        let session_ref = guard.session.as_ref().expect("sessão recém-criada");

        let saved_token = if use_saved_token {
            let g = self.vault.lock().unwrap_or_else(|e| e.into_inner());
            g.screencast_token(&topology.id)
        } else {
            None
        };

        let mut opts = SelectSourcesOptions::default()
            .set_sources(SourceType::MONITOR)
            .set_multiple(true)
            .set_cursor_mode(CursorMode::EMBEDDED)
            .set_persist_mode(PersistMode::ExplicitlyRevoked);
        if let Some(tok) = saved_token.as_deref() {
            opts = opts.set_restore_token(tok);
        }

        sc.select_sources(session_ref, opts)
            .await
            .context("Falha ao selecionar fontes no portal ScreenCast")?;

        let start_req = Self::await_portal_start(
            sc.start(session_ref, None, Default::default()),
            use_saved_token && saved_token.is_some(),
            "permissão de captura de tela",
            || self.clear_screencast_token(&topology.id),
        )
        .await?;

        let streams = match start_req.response() {
            Ok(resp) => resp,
            Err(err) => {
                let locked = is_gnome_screen_locked().await;
                if use_saved_token && saved_token.is_some() && !locked {
                    self.clear_screencast_token(&topology.id);
                }
                return Err(format_portal_error(
                    err,
                    locked,
                    "permissão de captura de tela",
                ));
            }
        };

        let monitors = parse_portal_streams(streams.streams());
        if monitors.is_empty() {
            return Err(anyhow::anyhow!(
                "Nenhum monitor selecionado no diálogo ScreenCast."
            ));
        }

        if let Some(new_token) = streams.restore_token() {
            self.store_screencast_token(&topology.id, Some(new_token));
        } else if use_saved_token && saved_token.is_some() {
            self.store_screencast_token(&topology.id, None);
        }

        let session = guard.disarm();
        let active_sc = Arc::new(ActiveScreencastOnly {
            proxy: sc,
            session,
            monitors,
            topology_id: topology.id.clone(),
        });
        *self.active.write().await = Some(ActivePortalSession::ScreencastOnly(active_sc.clone()));
        Ok(active_sc)
    }

    #[cfg(target_os = "linux")]
    async fn await_portal_start<R, Fut, F>(
        fut: Fut,
        clear_saved_on_err: bool,
        action: &str,
        clear_token: F,
    ) -> Result<R>
    where
        Fut: std::future::Future<Output = std::result::Result<R, ashpd::Error>>,
        F: FnOnce(),
    {
        match tokio::time::timeout(INTERACTIVE_PORTAL_TIMEOUT, fut).await {
            Ok(Ok(req)) => Ok(req),
            Ok(Err(err)) => {
                let locked = is_gnome_screen_locked().await;
                if clear_saved_on_err && !locked {
                    clear_token();
                }
                Err(format_portal_error(err, locked, action))
            }
            Err(_) => Err(anyhow::anyhow!(
                "Tempo esgotado aguardando {action} no computador."
            )),
        }
    }

    fn store_combined_token(&self, topology_id: &str, token: Option<&str>) {
        let mut guard = self.vault.lock().unwrap_or_else(|e| e.into_inner());
        match token.map(str::trim).filter(|s| !s.is_empty()) {
            Some(tok) => {
                guard
                    .combined_tokens
                    .insert(topology_id.to_string(), tok.to_string());
            }
            None => {
                guard.combined_tokens.remove(topology_id);
            }
        }
        if let Err(err) = guard.save(&self.paths.portal_tokens_file) {
            tracing::warn!(error = %err, "Falha ao persistir portal_tokens.json");
        }
    }

    fn clear_combined_token(&self, topology_id: &str) {
        self.store_combined_token(topology_id, None);
    }

    fn store_screencast_token(&self, topology_id: &str, token: Option<&str>) {
        let mut guard = self.vault.lock().unwrap_or_else(|e| e.into_inner());
        match token.map(str::trim).filter(|s| !s.is_empty()) {
            Some(tok) => {
                guard
                    .screencast_tokens
                    .insert(topology_id.to_string(), tok.to_string());
            }
            None => {
                guard.screencast_tokens.remove(topology_id);
            }
        }
        if let Err(err) = guard.save(&self.paths.portal_tokens_file) {
            tracing::warn!(error = %err, "Falha ao persistir portal_tokens.json");
        }
    }

    fn clear_screencast_token(&self, topology_id: &str) {
        self.store_screencast_token(topology_id, None);
    }

    fn store_input_token(&self, token: Option<&str>) {
        let mut guard = self.vault.lock().unwrap_or_else(|e| e.into_inner());
        guard.input_token = token
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        if let Err(err) = guard.save(&self.paths.portal_tokens_file) {
            tracing::warn!(error = %err, "Falha ao persistir portal_tokens.json");
        }
    }
}

#[cfg(target_os = "linux")]
async fn open_pipewire_remote_rd(
    session: &ashpd::desktop::Session<ashpd::desktop::remote_desktop::RemoteDesktop>,
) -> Result<std::os::fd::OwnedFd> {
    let sc = ashpd::desktop::screencast::Screencast::new()
        .await
        .context("Portal ScreenCast indisponível")?;
    sc.open_pipe_wire_remote(session, Default::default())
        .await
        .context("Falha ao abrir FD PipeWire da sessão unificada")
}

#[cfg(target_os = "linux")]
async fn open_pipewire_remote_sc(
    proxy: &ashpd::desktop::screencast::Screencast,
    session: &ashpd::desktop::Session<ashpd::desktop::screencast::Screencast>,
) -> Result<std::os::fd::OwnedFd> {
    proxy
        .open_pipe_wire_remote(session, Default::default())
        .await
        .context("Falha ao abrir FD PipeWire da sessão ScreenCast")
}

#[cfg(target_os = "linux")]
fn parse_portal_streams(streams: &[ashpd::desktop::screencast::Stream]) -> Vec<PortalMonitor> {
    let mut monitors: Vec<PortalMonitor> = streams
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
            let (width, height) = source_size.unwrap_or((1920, 1080));
            let (origin_x, origin_y) = stream.position().unwrap_or((i32::MAX, i32::MAX));
            PortalMonitor {
                node_id: stream.pipe_wire_node_id(),
                pw_source_arg,
                source_size,
                width,
                height,
                origin_x,
                origin_y,
            }
        })
        .collect();
    monitors.sort_by_key(|m| (m.origin_x, m.origin_y, m.node_id));
    monitors
}

#[cfg(target_os = "linux")]
fn format_portal_error(err: ashpd::Error, locked: bool, action: &str) -> anyhow::Error {
    if locked {
        return anyhow::anyhow!(
            "Tela do computador bloqueada. Desbloqueie a sessão para liberar {action}."
        );
    }
    if matches!(
        err,
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
    ) {
        return anyhow::anyhow!("Autorização ({action}) cancelada no computador.");
    }
    anyhow::anyhow!("Falha ao obter {action} no portal Wayland: {err}")
}

#[cfg(target_os = "linux")]
pub async fn ensure_portal_app_registered() {
    static REGISTERED: tokio::sync::Mutex<bool> = tokio::sync::Mutex::const_new(false);

    if ashpd::is_sandboxed() {
        return;
    }

    let mut guard = REGISTERED.lock().await;
    if *guard {
        return;
    }

    let Ok(app_id) = PORTAL_APP_ID.parse::<ashpd::AppID>() else {
        return;
    };
    match ashpd::register_host_app(app_id).await {
        Ok(()) => {
            *guard = true;
        }
        Err(err) => {
            let msg = err.to_string();
            // Se já estiver registrado na mesma conexão, trata como concluído.
            if msg.contains("already") {
                *guard = true;
            } else {
                tracing::debug!(
                    app_id = PORTAL_APP_ID,
                    error = %msg,
                    "org.freedesktop.host.portal.Registry indisponível ou não aceitou registro"
                );
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn ensure_portal_app_registered() {}

#[cfg(target_os = "linux")]
pub async fn is_gnome_screen_locked() -> bool {
    static BUS: tokio::sync::OnceCell<ashpd::zbus::Connection> =
        tokio::sync::OnceCell::const_new();

    let Ok(conn) = BUS
        .get_or_try_init(|| ashpd::zbus::Connection::session())
        .await
    else {
        return false;
    };

    let fut = conn.call_method(
        Some("org.gnome.ScreenSaver"),
        "/org/gnome/ScreenSaver",
        Some("org.gnome.ScreenSaver"),
        "GetActive",
        &(),
    );

    match tokio::time::timeout(LOCK_CHECK_TIMEOUT, fut).await {
        Ok(Ok(reply)) => reply.body().deserialize::<bool>().unwrap_or(false),
        _ => false,
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn is_gnome_screen_locked() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn read_gnome_lock_policy() -> (bool, u32) {
    let lock_out = crate::config::host_std_command("gsettings")
        .args(["get", "org.gnome.desktop.screensaver", "lock-enabled"])
        .output();
    let lock_enabled = match lock_out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim() == "true",
        _ => false,
    };

    let delay_out = crate::config::host_std_command("gsettings")
        .args(["get", "org.gnome.desktop.session", "idle-delay"])
        .output();
    let idle_delay = match delay_out {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.split_whitespace()
                .last()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0)
        }
        _ => 0,
    };

    (lock_enabled, idle_delay)
}

#[cfg(not(target_os = "linux"))]
fn read_gnome_lock_policy() -> (bool, u32) {
    (false, 0)
}

fn has_desktop_entry() -> bool {
    let file_name = format!("{PORTAL_APP_ID}.desktop");
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        if !xdg_data.trim().is_empty()
            && Path::new(&xdg_data)
                .join("applications")
                .join(&file_name)
                .exists()
        {
            return true;
        }
    }
    if let Some(dirs) = BaseDirs::new() {
        if dirs.data_local_dir().join("applications").join(&file_name).exists() {
            return true;
        }
    }
    Path::new("/usr/share/applications")
        .join(&file_name)
        .exists()
        || Path::new("/usr/local/share/applications")
            .join(&file_name)
            .exists()
}

pub fn normalize_connector_name(raw: &str) -> &str {
    if let Some(rest) = raw.strip_prefix("card") {
        if let Some(idx) = rest.find('-') {
            if !rest[..idx].is_empty() && rest[..idx].bytes().all(|b| b.is_ascii_digit()) {
                return &rest[idx + 1..];
            }
        }
    }
    raw
}

pub fn parse_edid_fingerprint(edid: &[u8]) -> String {
    if edid.len() < 16 {
        return "noedid".to_string();
    }
    let mfg = u16::from_be_bytes([edid[8], edid[9]]);
    let c1 = edid_char(mfg >> 10);
    let c2 = edid_char(mfg >> 5);
    let c3 = edid_char(mfg);
    let product = u16::from_le_bytes([edid[10], edid[11]]);
    let serial = u32::from_le_bytes([edid[12], edid[13], edid[14], edid[15]]);
    format!("{c1}{c2}{c3}:{product:04x}:{serial:08x}")
}

fn edid_char(bits: u16) -> char {
    let idx = (bits & 0x1f) as u8;
    if (1..=26).contains(&idx) {
        (b'A' + idx - 1) as char
    } else {
        '?'
    }
}

pub fn build_display_topology(connectors: &[(String, bool, bool, Vec<u8>)]) -> DisplayTopology {
    let mut connected_count: u8 = 0;
    let mut active_entries: Vec<(String, String)> = Vec::new();
    let mut fallback_entries: Vec<(String, String)> = Vec::new();

    for (raw_name, is_connected, is_enabled, edid) in connectors {
        if !*is_connected {
            continue;
        }
        connected_count = connected_count.saturating_add(1);
        let conn = normalize_connector_name(raw_name).to_string();
        let fp = parse_edid_fingerprint(edid);
        fallback_entries.push((conn.clone(), fp.clone()));
        if *is_enabled {
            active_entries.push((conn, fp));
        }
    }

    let mut chosen = if !active_entries.is_empty() {
        active_entries
    } else {
        fallback_entries
    };
    chosen.sort();
    chosen.dedup();

    if chosen.is_empty() {
        return DisplayTopology {
            id: "headless".to_string(),
            label: "headless".to_string(),
            active_outputs: 0,
            connected_outputs: connected_count,
        };
    }

    let active_outputs = chosen.len().min(u8::MAX as usize) as u8;
    let id = chosen
        .iter()
        .map(|(conn, fp)| format!("{conn}={fp}"))
        .collect::<Vec<_>>()
        .join("|");
    let label = chosen
        .iter()
        .map(|(conn, _)| conn.as_str())
        .collect::<Vec<_>>()
        .join(" + ");

    DisplayTopology {
        id,
        label,
        active_outputs,
        connected_outputs: connected_count,
    }
}

#[cfg(target_os = "linux")]
pub fn detect_display_topology() -> DisplayTopology {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return build_display_topology(&[]);
    };

    let mut connectors = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("card") || !name_str.contains('-') {
            continue;
        }
        let dir = entry.path();
        let Ok(status) = fs::read_to_string(dir.join("status")) else {
            continue;
        };
        let is_connected = status.trim() == "connected";
        let is_enabled = fs::read_to_string(dir.join("enabled"))
            .map(|s| s.trim() == "enabled")
            .unwrap_or(true);
        let edid = if is_connected {
            fs::read(dir.join("edid")).unwrap_or_default()
        } else {
            Vec::new()
        };
        connectors.push((name_str.into_owned(), is_connected, is_enabled, edid));
    }

    build_display_topology(&connectors)
}

#[cfg(not(target_os = "linux"))]
pub fn detect_display_topology() -> DisplayTopology {
    build_display_topology(&[])
}

fn read_vault_file(path: &Path) -> Option<PortalTokenVault> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<PortalTokenVault>(&raw).ok()
}

fn read_trimmed_token(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fake_edid(mfg_a: u8, mfg_b: u8, product: u16, serial: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 128];
        bytes[8] = mfg_a;
        bytes[9] = mfg_b;
        let p = product.to_le_bytes();
        bytes[10] = p[0];
        bytes[11] = p[1];
        let s = serial.to_le_bytes();
        bytes[12..16].copy_from_slice(&s);
        bytes
    }

    fn temp_vault_paths(tag: &str) -> PortalVaultPaths {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("oraculo-portal-test-{tag}-{ts}"));
        fs::create_dir_all(&dir).unwrap();
        PortalVaultPaths {
            portal_tokens_file: dir.join("portal_tokens.json"),
            legacy_remote_desktop_token_file: dir.join("remote_desktop_token"),
            legacy_screencast_token_file: dir.join("screencast_token"),
        }
    }

    #[test]
    fn topology_key_distinguishes_docked_vs_undocked_and_ignores_card_index() {
        let edid_edp = fake_edid(0x0d, 0xae, 0x1521, 0);
        let edid_sam = fake_edid(0x4c, 0x2d, 0x7706, 0);

        let undocked = build_display_topology(&[
            ("card1-eDP-1".into(), true, true, edid_edp.clone()),
            ("card1-HDMI-A-1".into(), false, false, Vec::new()),
        ]);
        assert_eq!(undocked.id, "eDP-1=CMN:1521:00000000");
        assert_eq!(undocked.label, "eDP-1");
        assert_eq!(undocked.active_outputs, 1);

        let undocked_card0 = build_display_topology(&[
            ("card0-eDP-1".into(), true, true, edid_edp.clone()),
        ]);
        assert_eq!(undocked.id, undocked_card0.id);

        let docked = build_display_topology(&[
            ("card1-HDMI-A-1".into(), true, true, edid_sam),
            ("card1-eDP-1".into(), true, true, edid_edp),
        ]);
        assert_ne!(docked.id, undocked.id);
        assert_eq!(
            docked.id,
            "HDMI-A-1=SAM:7706:00000000|eDP-1=CMN:1521:00000000"
        );
        assert_eq!(docked.label, "HDMI-A-1 + eDP-1");
        assert_eq!(docked.active_outputs, 2);
    }

    #[test]
    fn vault_migrates_legacy_tokens_and_preserves_distinct_monitor_profiles() {
        let paths = temp_vault_paths("migrate");
        fs::write(&paths.legacy_remote_desktop_token_file, "rd_tok_1\n").unwrap();
        fs::write(&paths.legacy_screencast_token_file, "sc_tok_1\n").unwrap();

        let topo_laptop = "eDP-1=CMN:1521:00000000";
        let topo_docked = "HDMI-A-1=SAM:7706:00000000|eDP-1=CMN:1521:00000000";

        let mut vault = PortalTokenVault::load_or_migrate(&paths, topo_laptop).unwrap();
        assert_eq!(vault.input_token(), Some("rd_tok_1".to_string()));
        assert_eq!(
            vault.screencast_token(topo_laptop),
            Some("sc_tok_1".to_string())
        );

        vault
            .combined_tokens
            .insert(topo_laptop.to_string(), "comb_laptop".to_string());
        vault
            .combined_tokens
            .insert(topo_docked.to_string(), "comb_docked".to_string());
        vault.save(&paths.portal_tokens_file).unwrap();

        let reloaded = PortalTokenVault::load_or_migrate(&paths, topo_docked).unwrap();
        assert_eq!(
            reloaded.combined_token(topo_laptop),
            Some("comb_laptop".to_string())
        );
        assert_eq!(
            reloaded.combined_token(topo_docked),
            Some("comb_docked".to_string())
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&paths.portal_tokens_file)
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = fs::remove_dir_all(paths.portal_tokens_file.parent().unwrap());
    }
}
