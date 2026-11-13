use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use iroh::SecretKey;
use oraculo_proto::QuickCommandInfo;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, watch};

#[derive(Debug, Clone)]
pub struct HostPaths {
    pub secret_key_file: PathBuf,
    pub trusted_peers_file: PathBuf,
    pub commands_file: PathBuf,
    pub settings_file: PathBuf,
    pub remote_desktop_token_file: PathBuf,
    pub screencast_token_file: PathBuf,
}

impl HostPaths {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "oraculo", "oraculo")
            .context("Não foi possível determinar diretório de configuração do sistema")?;

        let config_dir = dirs.config_dir();
        let data_dir = dirs.data_local_dir();

        fs::create_dir_all(config_dir)?;
        fs::create_dir_all(data_dir)?;

        Ok(Self {
            secret_key_file: data_dir.join("host.key"),
            trusted_peers_file: data_dir.join("trusted_peers.json"),
            commands_file: config_dir.join("commands.toml"),
            settings_file: config_dir.join("settings.toml"),
            remote_desktop_token_file: data_dir.join("remote_desktop_token"),
            screencast_token_file: data_dir.join("screencast_token"),
        })
    }
}

pub fn local_hostname() -> String {
    if let Ok(name) = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")) {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(content) = fs::read_to_string("/etc/hostname") {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "oraculo-host".to_string()
}

pub fn load_or_create_secret_key(path: &Path) -> Result<SecretKey> {
    if path.exists() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Falha ao ler chave do host em {}", path.display()))?;
        let bytes = hex::decode(raw.trim()).context("Arquivo host.key inválido (hex)")?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Arquivo host.key deve conter exatamente 32 bytes"))?;
        return Ok(SecretKey::from_bytes(&arr));
    }

    let key = SecretKey::generate();
    let encoded = hex::encode(key.to_bytes());
    write_private_file(path, encoded.as_bytes())?;
    Ok(key)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedPeer {
    pub endpoint_id: String,
    pub name: String,
    pub paired_at_unix: u64,
}

#[derive(Debug, Clone)]
pub struct TrustedPeersStore {
    path: PathBuf,
    peers: Arc<RwLock<HashMap<String, TrustedPeer>>>,
    last_stamp: Arc<RwLock<Option<(u64, SystemTime)>>>,
}

impl TrustedPeersStore {
    pub fn load(path: PathBuf) -> Result<Self> {
        let peers = load_peers_map(&path)?;
        let stamp = file_stamp(&path);
        Ok(Self {
            path,
            peers: Arc::new(RwLock::new(peers)),
            last_stamp: Arc::new(RwLock::new(stamp)),
        })
    }

    pub async fn is_trusted(&self, endpoint_id: &str) -> bool {
        self.peers.read().await.contains_key(endpoint_id)
    }

    pub async fn count(&self) -> usize {
        self.peers.read().await.len()
    }

    pub async fn list(&self) -> Vec<TrustedPeer> {
        let mut items: Vec<_> = self.peers.read().await.values().cloned().collect();
        items.sort_by(|a, b| a.paired_at_unix.cmp(&b.paired_at_unix));
        items
    }

    pub async fn add_or_update(&self, endpoint_id: String, name: String) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut guard = self.peers.write().await;
        guard.insert(
            endpoint_id.clone(),
            TrustedPeer {
                endpoint_id,
                name,
                paired_at_unix: now,
            },
        );
        self.persist_locked(&guard).await
    }

    pub async fn revoke(&self, endpoint_id: &str) -> Result<bool> {
        let mut guard = self.peers.write().await;
        let removed = guard.remove(endpoint_id).is_some();
        if removed {
            self.persist_locked(&guard).await?;
        }
        Ok(removed)
    }

    /// Recarrega `trusted_peers.json` se alterado externamente (ex.: CLI `revoke`) e devolve IDs removidos.
    pub async fn sync_from_disk(&self) -> Result<Vec<String>> {
        let disk_stamp = file_stamp(&self.path);
        if *self.last_stamp.read().await == disk_stamp {
            return Ok(Vec::new());
        }

        let next = load_peers_map(&self.path)?;
        let mut guard = self.peers.write().await;
        let current_ids: HashSet<String> = guard.keys().cloned().collect();
        let removed: Vec<String> = current_ids
            .into_iter()
            .filter(|id| !next.contains_key(id))
            .collect();
        *guard = next;
        *self.last_stamp.write().await = file_stamp(&self.path);
        Ok(removed)
    }

    async fn persist_locked(&self, map: &HashMap<String, TrustedPeer>) -> Result<()> {
        let mut list: Vec<_> = map.values().cloned().collect();
        list.sort_by(|a, b| a.paired_at_unix.cmp(&b.paired_at_unix));
        let json = serde_json::to_string_pretty(&list)?;
        write_private_file(&self.path, json.as_bytes())?;
        *self.last_stamp.write().await = file_stamp(&self.path);
        Ok(())
    }
}

fn load_peers_map(path: &Path) -> Result<HashMap<String, TrustedPeer>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)?;
    let list: Vec<TrustedPeer> = serde_json::from_str(&content)
        .with_context(|| format!("JSON inválido em {}", path.display()))?;
    Ok(list.into_iter().map(|p| (p.endpoint_id.clone(), p)).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HostSettings {
    pub allow_terminal: bool,
    pub allow_files_read: bool,
    pub allow_files_write: bool,
    pub allow_input: bool,
    pub allow_screen: bool,
    pub allow_commands_run: bool,
    pub allow_commands_edit: bool,
    pub wol_enabled: bool,
    pub wol_mac: Option<String>,
    pub wol_broadcast: Option<String>,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            allow_terminal: true,
            allow_files_read: true,
            allow_files_write: true,
            allow_input: true,
            allow_screen: true,
            allow_commands_run: true,
            allow_commands_edit: true,
            wol_enabled: false,
            wol_mac: None,
            wol_broadcast: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    inner: Arc<RwLock<HostSettings>>,
    last_stamp: Arc<RwLock<Option<(u64, SystemTime)>>>,
    tx: watch::Sender<HostSettings>,
}

impl SettingsStore {
    pub fn load_or_create(path: PathBuf) -> Result<Self> {
        let created = !path.exists();
        let settings = load_settings_file(&path)?;
        if created {
            persist_settings_file(&path, &settings)?;
        }
        let stamp = file_stamp(&path);
        let (tx, _rx) = watch::channel(settings.clone());
        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(settings)),
            last_stamp: Arc::new(RwLock::new(stamp)),
            tx,
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<HostSettings> {
        self.tx.subscribe()
    }

    pub async fn current(&self) -> HostSettings {
        self.inner.read().await.clone()
    }

    pub async fn update<F>(&self, f: F) -> Result<HostSettings>
    where
        F: FnOnce(&mut HostSettings) -> Result<()>,
    {
        let mut guard = self.inner.write().await;
        let mut draft = guard.clone();
        f(&mut draft)?;
        if let Some(mac) = draft.wol_mac.take() {
            let trimmed = mac.trim();
            if !trimmed.is_empty() {
                draft.wol_mac = Some(normalize_wol_mac(trimmed)?);
            }
        }
        persist_settings_file(&self.path, &draft)?;
        *guard = draft.clone();
        *self.last_stamp.write().await = file_stamp(&self.path);
        let _ = self.tx.send(draft.clone());
        Ok(draft)
    }

    pub async fn sync_from_disk(&self) -> Result<bool> {
        let disk_stamp = file_stamp(&self.path);
        if *self.last_stamp.read().await == disk_stamp {
            return Ok(false);
        }
        let next = load_settings_file(&self.path)?;
        let mut guard = self.inner.write().await;
        let changed = *guard != next;
        *guard = next.clone();
        *self.last_stamp.write().await = file_stamp(&self.path);
        if changed {
            let _ = self.tx.send(next);
        }
        Ok(changed)
    }
}

pub fn normalize_wol_mac(raw: &str) -> Result<String> {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.'))
        .collect();
    if clean.len() != 12 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("Endereço MAC WoL inválido (use formato AA:BB:CC:DD:EE:FF)");
    }
    let upper = clean.to_ascii_uppercase();
    let mut parts = Vec::with_capacity(6);
    for i in (0..12).step_by(2) {
        parts.push(&upper[i..i + 2]);
    }
    Ok(parts.join(":"))
}

fn load_settings_file(path: &Path) -> Result<HostSettings> {
    if !path.exists() {
        return Ok(HostSettings::default());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Falha ao ler {}", path.display()))?;
    let mut parsed: HostSettings =
        toml::from_str(&raw).with_context(|| format!("TOML inválido em {}", path.display()))?;
    if let Some(mac) = parsed.wol_mac.take() {
        let trimmed = mac.trim();
        if !trimmed.is_empty() {
            parsed.wol_mac = Some(normalize_wol_mac(trimmed)?);
        }
    }
    Ok(parsed)
}

fn persist_settings_file(path: &Path, settings: &HostSettings) -> Result<()> {
    let rendered = toml::to_string_pretty(settings).context("Falha ao serializar settings.toml")?;
    write_private_file(path, rendered.as_bytes())
}

fn file_stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    Some((meta.len(), modified))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickCommandSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl QuickCommandSpec {
    pub fn info(&self) -> QuickCommandInfo {
        QuickCommandInfo {
            id: self.id.clone(),
            label: self.label.clone(),
            description: self.description.clone(),
            command_line: format_command_line(&self.program, &self.args),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsFile {
    #[serde(default)]
    pub command: Vec<QuickCommandSpec>,
}

pub fn load_or_create_commands(path: &Path) -> Result<Vec<QuickCommandSpec>> {
    if !path.exists() {
        let defaults = default_commands_toml();
        write_private_file(path, defaults.as_bytes())?;
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("Falha ao ler {}", path.display()))?;
    let parsed: CommandsFile = toml::from_str(&raw)
        .with_context(|| format!("TOML inválido em {}", path.display()))?;

    for cmd in &parsed.command {
        if cmd.id.trim().is_empty() || cmd.program.trim().is_empty() {
            bail!("Comando inválido em {}: id e program são obrigatórios", path.display());
        }
    }

    Ok(parsed.command)
}

pub fn save_command(
    path: &Path,
    id: Option<&str>,
    label: &str,
    description: &str,
    command_line: &str,
) -> Result<Vec<QuickCommandSpec>> {
    let label = label.trim();
    if label.is_empty() {
        bail!("Informe um nome para o comando");
    }
    let description = description.trim().to_string();
    let (program, args) = parse_command_line(command_line)?;

    let mut commands = load_or_create_commands(path)?;
    let requested_id = id.map(str::trim).filter(|s| !s.is_empty());

    if let Some(target_id) = requested_id {
        if let Some(existing) = commands.iter_mut().find(|c| c.id == target_id) {
            existing.label = label.to_string();
            existing.description = description;
            existing.program = program;
            existing.args = args;
        } else {
            commands.push(QuickCommandSpec {
                id: target_id.to_string(),
                label: label.to_string(),
                description,
                program,
                args,
            });
        }
    } else {
        let new_id = slugify_command_id(label, &commands);
        commands.push(QuickCommandSpec {
            id: new_id,
            label: label.to_string(),
            description,
            program,
            args,
        });
    }

    persist_commands(path, &commands)?;
    Ok(commands)
}

pub fn delete_command(path: &Path, id: &str) -> Result<Vec<QuickCommandSpec>> {
    let target_id = id.trim();
    if target_id.is_empty() {
        bail!("ID do comando inválido");
    }
    let mut commands = load_or_create_commands(path)?;
    let before = commands.len();
    commands.retain(|c| c.id != target_id);
    if commands.len() == before {
        bail!("Comando '{target_id}' não encontrado");
    }
    persist_commands(path, &commands)?;
    Ok(commands)
}

fn persist_commands(path: &Path, commands: &[QuickCommandSpec]) -> Result<()> {
    let file = CommandsFile {
        command: commands.to_vec(),
    };
    let rendered = toml::to_string_pretty(&file).context("Falha ao serializar commands.toml")?;
    write_private_file(path, rendered.as_bytes())
}

pub fn parse_command_line(raw: &str) -> Result<(String, Vec<String>)> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut has_token = false;
    let mut chars = raw.trim().chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                has_token = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                has_token = true;
            }
            '\\' if !in_single => {
                has_token = true;
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                } else {
                    current.push('\\');
                }
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                has_token = true;
                current.push(c);
            }
        }
    }

    if in_single || in_double {
        bail!("Aspas não fechadas na linha do comando");
    }
    if has_token {
        tokens.push(current);
    }

    let mut iter = tokens.into_iter();
    let Some(program) = iter.next().filter(|p| !p.trim().is_empty()) else {
        bail!("Informe o comando a ser executado");
    };
    Ok((program, iter.collect()))
}

pub fn format_command_line(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(quote_shell_arg)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_shell_arg(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | '\\'))
    {
        arg.to_string()
    } else {
        let escaped = arg.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }
}

fn slugify_command_id(label: &str, existing: &[QuickCommandSpec]) -> String {
    let mut base = String::new();
    let mut prev_dash = false;
    for ch in label.chars() {
        let folded = match ch.to_ascii_lowercase() {
            'a'..='z' | '0'..='9' => Some(ch.to_ascii_lowercase()),
            'á' | 'à' | 'â' | 'ã' | 'ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' => Some('i'),
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' => Some('u'),
            'ç' => Some('c'),
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => Some('a'),
            'É' | 'È' | 'Ê' | 'Ë' => Some('e'),
            'Í' | 'Ì' | 'Î' | 'Ï' => Some('i'),
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => Some('o'),
            'Ú' | 'Ù' | 'Û' | 'Ü' => Some('u'),
            'Ç' => Some('c'),
            _ => None,
        };
        match folded {
            Some(c) => {
                base.push(c);
                prev_dash = false;
            }
            None if !base.is_empty() && !prev_dash => {
                base.push('-');
                prev_dash = true;
            }
            None => {}
        }
    }
    let base = base.trim_matches('-');
    let stem = if base.is_empty() { "cmd" } else { base };
    if existing.iter().all(|c| c.id != stem) {
        return stem.to_string();
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{stem}-{n}");
        if existing.iter().all(|c| c.id != candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn default_commands_toml() -> &'static str {
    if cfg!(windows) {
        r#"[[command]]
id = "whoami"
label = "Usuário Atual"
description = "Mostra usuário e domínio logados"
program = "whoami"
args = []

[[command]]
id = "lock"
label = "Bloquear Tela"
description = "Bloqueia a sessão do Windows imediatamente"
program = "rundll32.exe"
args = ["user32.dll,LockWorkStation"]
"#
    } else {
        r#"[[command]]
id = "uptime"
label = "Tempo de Uso"
description = "Tempo ligado e carga do sistema"
program = "uptime"
args = []

[[command]]
id = "disk"
label = "Espaço em Disco"
description = "Uso das partições montadas"
program = "df"
args = ["-h", "--output=source,size,used,avail,pcent,target"]

[[command]]
id = "lock"
label = "Bloquear Sessão"
description = "Bloqueia a tela atual via systemd-logind"
program = "loginctl"
args = ["lock-session"]
"#
    }
}

fn write_atomic_inner(path: &Path, data: &[u8], private_mode: bool) -> Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("oraculo");
    let tmp = path.with_file_name(format!(".{base_name}.{}.{nonce}.tmp", std::process::id()));

    let write_res: Result<()> = (|| {
        use std::io::Write;
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(if private_mode { 0o600 } else { 0o644 });
        }
        let mut file = opts.open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        if private_mode {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    })();

    if write_res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_res
}

pub fn write_private_file(path: &Path, data: &[u8]) -> Result<()> {
    write_atomic_inner(path, data, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_command_lines() {
        let (prog, args) = parse_command_line(r#"notify-send "Oi mundo" 'teste 2'"#).unwrap();
        assert_eq!(prog, "notify-send");
        assert_eq!(args, vec!["Oi mundo", "teste 2"]);
        assert_eq!(
            format_command_line(&prog, &args),
            r#"notify-send "Oi mundo" "teste 2""#
        );
        assert!(parse_command_line(r#"echo "unclosed"#).is_err());
    }

    #[test]
    fn saves_updates_and_deletes_commands() {
        let dir = std::env::temp_dir().join(format!(
            "oraculo_cfg_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("commands.toml");

        let list = save_command(
            &file,
            None,
            "Reiniciar Áudio",
            "Reinicia pipewire",
            "systemctl --user restart pipewire",
        )
        .unwrap();
        let created = list.iter().find(|c| c.id == "reiniciar-audio").unwrap();
        assert_eq!(created.program, "systemctl");
        assert_eq!(created.args, vec!["--user", "restart", "pipewire"]);

        let updated = save_command(
            &file,
            Some("reiniciar-audio"),
            "Áudio",
            "",
            "wpctl status",
        )
        .unwrap();
        assert_eq!(
            updated.iter().find(|c| c.id == "reiniciar-audio").unwrap().program,
            "wpctl"
        );

        let after_del = delete_command(&file, "reiniciar-audio").unwrap();
        assert!(after_del.iter().all(|c| c.id != "reiniciar-audio"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn settings_roundtrip_and_wol_mac_validation() {
        let dir = std::env::temp_dir().join(format!(
            "oraculo_settings_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.toml");

        let store = SettingsStore::load_or_create(file.clone()).unwrap();
        let initial = store.current().await;
        assert!(initial.allow_terminal);
        assert!(initial.allow_files_read);
        assert!(initial.allow_files_write);
        assert!(!initial.wol_enabled);

        assert!(normalize_wol_mac("invalid-mac").is_err());
        assert_eq!(normalize_wol_mac("aa-bb-cc-11-22-33").unwrap(), "AA:BB:CC:11:22:33");

        let updated = store
            .update(|s| {
                s.allow_terminal = false;
                s.allow_files_write = false;
                s.wol_enabled = true;
                s.wol_mac = Some("aa-bb-cc-dd-ee-ff".into());
                Ok(())
            })
            .await
            .unwrap();
        assert!(!updated.allow_terminal);
        assert!(!updated.allow_files_write);
        assert_eq!(updated.wol_mac.as_deref(), Some("AA:BB:CC:DD:EE:FF"));

        let reloaded = SettingsStore::load_or_create(file).unwrap().current().await;
        assert_eq!(reloaded, updated);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn trusted_peers_sync_detects_external_revoke() {
        let dir = std::env::temp_dir().join(format!(
            "oraculo_peers_sync_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("trusted_peers.json");

        let daemon_store = TrustedPeersStore::load(file.clone()).unwrap();
        daemon_store
            .add_or_update("peer-1".into(), "Celular".into())
            .await
            .unwrap();
        assert!(daemon_store.is_trusted("peer-1").await);

        let cli_store = TrustedPeersStore::load(file).unwrap();
        assert!(cli_store.revoke("peer-1").await.unwrap());

        let revoked = daemon_store.sync_from_disk().await.unwrap();
        assert_eq!(revoked, vec!["peer-1".to_string()]);
        assert!(!daemon_store.is_trusted("peer-1").await);

        let _ = fs::remove_dir_all(&dir);
    }
}
