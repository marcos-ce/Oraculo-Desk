use crate::config::HostSettings;
use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use oraculo_proto::{
    DirectoryListing, FILE_CHUNK_SIZE, FileClientMsg, FileEntryInfo, FileHostMsg,
    FileTransferRequest, read_frame, write_frame,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;

/// Limite rígido por upload para evitar exaustão de disco no host (2 GiB).
pub const MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
static UPLOAD_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn discover_home_root() -> Result<PathBuf> {
    let raw = BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .context("Não foi possível determinar a pasta pessoal (~)")?;

    fs::canonicalize(&raw).with_context(|| {
        format!(
            "Falha ao resolver caminho canônico da pasta pessoal {}",
            raw.display()
        )
    })
}

pub fn list_directory(
    home_root: &Path,
    rel_path: &str,
    show_hidden: bool,
) -> Result<DirectoryListing> {
    let canonical_home = fs::canonicalize(home_root)
        .with_context(|| format!("Home inválida: {}", home_root.display()))?;
    let current_dir = resolve_existing_in_home(&canonical_home, rel_path, true)?;
    let current_rel = path_to_rel_string(&canonical_home, &current_dir)?;
    let parent_path = parent_rel_path(&current_rel);
    let quick_paths = collect_quick_paths(&canonical_home);

    let mut entries = Vec::new();
    let dir_iter = fs::read_dir(&current_dir)
        .with_context(|| format!("Sem permissão para listar {}", current_dir.display()))?;

    for item in dir_iter {
        let Ok(entry) = item else {
            continue;
        };
        let file_name_os = entry.file_name();
        let Some(name) = file_name_os.to_str() else {
            continue;
        };
        if name.starts_with(".oraculo-upload-") {
            continue;
        }
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        let entry_path = entry.path();
        let Ok(sym_meta) = fs::symlink_metadata(&entry_path) else {
            continue;
        };

        // Bloqueia symlinks quebrados ou que apontem para fora de ~.
        let (target_meta, entry_rel) = if sym_meta.file_type().is_symlink() {
            let Ok(resolved) = fs::canonicalize(&entry_path) else {
                continue;
            };
            if !resolved.starts_with(&canonical_home)
                || is_protected_in_home(&canonical_home, &resolved)
            {
                continue;
            }
            let Ok(meta) = fs::metadata(&resolved) else {
                continue;
            };
            let rel = if current_rel.is_empty() {
                name.to_string()
            } else {
                format!("{current_rel}/{name}")
            };
            (meta, rel)
        } else {
            let rel = if current_rel.is_empty() {
                name.to_string()
            } else {
                format!("{current_rel}/{name}")
            };
            (sym_meta, rel)
        };
        if is_protected_rel_str(&entry_rel) {
            continue;
        }

        let is_dir = target_meta.is_dir();
        if !is_dir && !target_meta.is_file() {
            continue;
        }

        let size = if is_dir { 0 } else { target_meta.len() };
        let modified_unix = target_meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mime_hint = if is_dir {
            "inode/directory".to_string()
        } else {
            infer_mime_hint(name).to_string()
        };

        entries.push(FileEntryInfo {
            name: name.to_string(),
            rel_path: entry_rel,
            is_dir,
            size,
            modified_unix,
            mime_hint,
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(DirectoryListing {
        current_path: current_rel,
        home_label: canonical_home.to_string_lossy().into_owned(),
        parent_path,
        quick_paths,
        entries,
    })
}

pub async fn handle_files_stream(
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    home_root: PathBuf,
    mut settings: watch::Receiver<HostSettings>,
) -> Result<()> {
    let Some(req): Option<FileTransferRequest> = read_frame(&mut recv).await? else {
        return Ok(());
    };

    match req {
        FileTransferRequest::Download { rel_path } => {
            if !settings.borrow().allow_files_read {
                let _ = write_frame(
                    &mut send,
                    &FileHostMsg::Error("Acesso a arquivos desativado pelo host".to_string()),
                )
                .await;
                let _ = send.finish();
                return Ok(());
            }
            if let Err(err) =
                handle_download(&mut send, &home_root, &rel_path, &mut settings).await
            {
                let _ = write_frame(&mut send, &FileHostMsg::Error(format!("{err:#}"))).await;
            }
            let _ = send.finish();
        }
        FileTransferRequest::Upload {
            dir_rel_path,
            file_name,
            total_size,
            overwrite,
        } => {
            let perms = settings.borrow().clone();
            if !perms.allow_files_read || !perms.allow_files_write {
                let _ = write_frame(
                    &mut send,
                    &FileHostMsg::Error("Upload de arquivos desativado pelo host".to_string()),
                )
                .await;
                let _ = send.finish();
                return Ok(());
            }
            if let Err(err) = handle_upload(
                &mut send,
                &mut recv,
                &home_root,
                &dir_rel_path,
                &file_name,
                total_size,
                overwrite,
                &mut settings,
            )
            .await
            {
                let _ = write_frame(&mut send, &FileHostMsg::Error(format!("{err:#}"))).await;
            }
            let _ = send.finish();
        }
    }

    Ok(())
}

async fn handle_download(
    send: &mut iroh::endpoint::SendStream,
    home_root: &Path,
    rel_path: &str,
    settings: &mut watch::Receiver<HostSettings>,
) -> Result<()> {
    let canonical_home = fs::canonicalize(home_root)?;
    let target = resolve_existing_in_home(&canonical_home, rel_path, false)?;
    let saved_rel_path = path_to_rel_string(&canonical_home, &target)?;

    let file_name = target
        .file_name()
        .and_then(|s| s.to_str())
        .context("Nome de arquivo inválido")?
        .to_string();

    let mut file = tokio::fs::File::open(&target)
        .await
        .with_context(|| format!("Não foi possível abrir '{saved_rel_path}'"))?;
    let meta = file.metadata().await?;
    let total_size = meta.len();
    let mime_hint = infer_mime_hint(&file_name).to_string();

    write_frame(
        send,
        &FileHostMsg::DownloadStart {
            file_name,
            total_size,
            mime_hint,
        },
    )
    .await?;

    let mut buf = vec![0u8; FILE_CHUNK_SIZE];
    let mut transferred: u64 = 0;
    loop {
        let read_res = tokio::select! {
            biased;
            changed = settings.changed() => {
                if changed.is_ok() && !settings.borrow().allow_files_read {
                    bail!("Acesso a arquivos desativado pelo host durante o download");
                }
                continue;
            }
            res = file.read(&mut buf) => res?,
        };
        if read_res == 0 {
            break;
        }
        write_frame(send, &FileHostMsg::Chunk(buf[..read_res].to_vec())).await?;
        transferred += read_res as u64;
    }

    write_frame(
        send,
        &FileHostMsg::Done {
            bytes_transferred: transferred,
            saved_rel_path,
        },
    )
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_upload(
    send: &mut iroh::endpoint::SendStream,
    recv: &mut iroh::endpoint::RecvStream,
    home_root: &Path,
    dir_rel_path: &str,
    raw_file_name: &str,
    declared_size: u64,
    overwrite: bool,
    settings: &mut watch::Receiver<HostSettings>,
) -> Result<()> {
    validate_upload_size(declared_size)?;

    let canonical_home = fs::canonicalize(home_root)?;
    let (target_dir, resolved_name, final_target) =
        resolve_upload_target(&canonical_home, dir_rel_path, raw_file_name, overwrite)?;

    let unique_nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = UPLOAD_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path = target_dir.join(format!(
        ".oraculo-upload-{}-{unique_nonce}-{seq}.tmp",
        std::process::id(),
    ));

    let mut tmp_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
        .await
        .with_context(|| format!("Falha ao criar arquivo temporário em {}", target_dir.display()))?;

    let mut written: u64 = 0;
    let stream_res: Result<()> = async {
        write_frame(
            send,
            &FileHostMsg::UploadReady {
                resolved_name: resolved_name.clone(),
            },
        )
        .await?;

        loop {
            let msg_opt = tokio::select! {
                biased;
                changed = settings.changed() => {
                    if changed.is_ok() {
                        let perms = settings.borrow().clone();
                        if !perms.allow_files_read || !perms.allow_files_write {
                            bail!("Permissão de upload desativada pelo host durante a transferência");
                        }
                    }
                    continue;
                }
                frame = read_frame::<_, FileClientMsg>(recv) => frame?,
            };
            let Some(msg) = msg_opt else {
                break;
            };
            match msg {
                FileClientMsg::Chunk(bytes) => {
                    let next_written = written
                        .checked_add(bytes.len() as u64)
                        .context("Tamanho do upload excedeu limite suportado")?;
                    validate_upload_size(next_written)?;
                    tmp_file.write_all(&bytes).await?;
                    written = next_written;
                }
                FileClientMsg::Finish => {
                    tmp_file.flush().await?;
                    tmp_file.sync_all().await?;
                    drop(tmp_file);
                    verify_upload_destination_before_rename(
                        &canonical_home,
                        &target_dir,
                        &final_target,
                        overwrite,
                    )?;
                    tokio::fs::rename(&tmp_path, &final_target).await?;
                    let saved_rel_path = path_to_rel_string(&canonical_home, &final_target)?;
                    write_frame(
                        send,
                        &FileHostMsg::Done {
                            bytes_transferred: written,
                            saved_rel_path,
                        },
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
        bail!("Conexão encerrada antes de concluir o envio do arquivo");
    }
    .await;

    if let Err(err) = stream_res {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(err);
    }

    Ok(())
}

fn validate_upload_size(size: u64) -> Result<()> {
    if size > MAX_UPLOAD_BYTES {
        bail!(
            "Arquivo excede o limite máximo de envio ({} MiB)",
            MAX_UPLOAD_BYTES / (1024 * 1024)
        );
    }
    Ok(())
}

fn verify_upload_destination_before_rename(
    canonical_home: &Path,
    expected_target_dir: &Path,
    final_target: &Path,
    overwrite: bool,
) -> Result<()> {
    let parent = final_target
        .parent()
        .context("Destino do upload sem diretório pai")?;
    let canon_parent = fs::canonicalize(parent)
        .with_context(|| format!("Diretório pai inacessível: {}", parent.display()))?;
    if !canon_parent.starts_with(canonical_home) || canon_parent != expected_target_dir {
        bail!("Diretório de destino alterado ou fora da pasta pessoal (~)");
    }

    if let Ok(existing) = fs::symlink_metadata(final_target) {
        if existing.file_type().is_symlink() || existing.is_dir() {
            bail!("Destino não pode ser sobrescrito: diretório ou symlink detectado");
        }
        if !overwrite {
            bail!("Conflito detectado: o arquivo de destino foi criado durante o upload");
        }
    }
    Ok(())
}

fn validate_rel_components(rel_path: &str) -> Result<Vec<&str>> {
    let trimmed = rel_path.trim();
    if trimmed.contains('\0') || trimmed.contains('\\') {
        bail!("Caminho contém caracteres inválidos");
    }
    if trimmed.starts_with('/') || trimmed.starts_with('~') {
        bail!("Caminho absoluto não permitido; use caminho relativo à pasta pessoal (~)");
    }

    let mut parts = Vec::new();
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            bail!("Travessia '..' fora da pasta pessoal não é permitida");
        }
        parts.push(segment);
    }
    Ok(parts)
}

fn validate_single_file_name(raw_name: &str) -> Result<String> {
    let trimmed = raw_name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || trimmed.starts_with(".oraculo-upload-")
    {
        bail!("Nome de arquivo inválido: '{raw_name}'");
    }
    Ok(trimmed.to_string())
}

fn is_protected_rel_str(rel_path: &str) -> bool {
    let trimmed = rel_path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    const PROTECTED_PREFIXES: &[&str] = &[
        ".config/oraculo",
        ".config/oraculo-bridge",
        ".config/oraculo-cli",
        ".local/share/oraculo",
        ".local/share/oraculo-bridge",
        ".local/share/oraculo-cli",
        "appdata/roaming/oraculo",
        "appdata/local/oraculo",
        ".ssh",
        ".gnupg",
        ".local/share/keyrings",
        ".config/autostart",
        ".config/systemd/user",
        ".config/environment.d",
        ".config/fish",
    ];
    for prefix in PROTECTED_PREFIXES {
        if lower == *prefix || lower.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }
    matches!(
        lower.as_str(),
        ".bashrc"
            | ".bash_profile"
            | ".bash_login"
            | ".bash_logout"
            | ".profile"
            | ".zshrc"
            | ".zprofile"
            | ".zshenv"
    )
}

fn is_protected_in_home(canonical_home: &Path, candidate: &Path) -> bool {
    let Ok(rel) = candidate.strip_prefix(canonical_home) else {
        return true;
    };
    let mut parts = Vec::new();
    for comp in rel.components() {
        if let Component::Normal(os) = comp {
            parts.push(os.to_string_lossy().into_owned());
        }
    }
    is_protected_rel_str(&parts.join("/"))
}

fn resolve_existing_in_home(
    canonical_home: &Path,
    rel_path: &str,
    expect_dir: bool,
) -> Result<PathBuf> {
    if is_protected_rel_str(rel_path) {
        bail!("Acesso bloqueado: caminho de sistema ou credencial protegido pelo Oráculo");
    }
    let parts = validate_rel_components(rel_path)?;
    let mut candidate = canonical_home.to_path_buf();
    for p in parts {
        candidate.push(p);
    }

    let resolved = fs::canonicalize(&candidate)
        .with_context(|| format!("Caminho não encontrado ou inacessível: '{rel_path}'"))?;

    if !resolved.starts_with(canonical_home) {
        bail!("Acesso bloqueado: o destino aponta para fora da pasta pessoal (~)");
    }
    if is_protected_in_home(canonical_home, &resolved) {
        bail!("Acesso bloqueado: caminho de sistema ou credencial protegido pelo Oráculo");
    }

    let meta = fs::metadata(&resolved)?;
    if expect_dir && !meta.is_dir() {
        bail!("O caminho informado não é uma pasta");
    }
    if !expect_dir && !meta.is_file() {
        bail!("O caminho informado não é um arquivo regular");
    }

    Ok(resolved)
}

fn resolve_upload_target(
    canonical_home: &Path,
    dir_rel_path: &str,
    raw_file_name: &str,
    overwrite: bool,
) -> Result<(PathBuf, String, PathBuf)> {
    let target_dir = resolve_existing_in_home(canonical_home, dir_rel_path, true)?;
    let requested_name = validate_single_file_name(raw_file_name)?;
    if is_protected_in_home(canonical_home, &target_dir.join(&requested_name)) {
        bail!("Upload bloqueado: destino protegido pelo Oráculo");
    }

    let (resolved_name, final_target) = if overwrite {
        let candidate = target_dir.join(&requested_name);
        if let Ok(existing) = fs::symlink_metadata(&candidate) {
            if existing.file_type().is_symlink() || existing.is_dir() {
                bail!("Não é possível sobrescrever um diretório ou symlink com upload");
            }
        }
        (requested_name, candidate)
    } else {
        let mut candidate_name = requested_name.clone();
        let mut candidate_path = target_dir.join(&candidate_name);
        if fs::symlink_metadata(&candidate_path).is_ok() {
            let path_obj = Path::new(&requested_name);
            let stem = path_obj
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("arquivo");
            let ext = path_obj.extension().and_then(|s| s.to_str());
            let mut idx = 1usize;
            loop {
                candidate_name = match ext {
                    Some(e) if !e.is_empty() => format!("{stem} ({idx}).{e}"),
                    _ => format!("{stem} ({idx})"),
                };
                candidate_path = target_dir.join(&candidate_name);
                if fs::symlink_metadata(&candidate_path).is_err() {
                    break;
                }
                idx += 1;
                if idx > 9999 {
                    bail!("Muitos arquivos com o mesmo nome na pasta");
                }
            }
        }
        (candidate_name, candidate_path)
    };

    Ok((target_dir, resolved_name, final_target))
}

fn path_to_rel_string(canonical_home: &Path, target: &Path) -> Result<String> {
    let stripped = target
        .strip_prefix(canonical_home)
        .context("Caminho fora da pasta pessoal (~)")?;
    let mut segments = Vec::new();
    for comp in stripped.components() {
        match comp {
            Component::Normal(os) => {
                segments.push(os.to_string_lossy().into_owned());
            }
            Component::CurDir => {}
            _ => bail!("Componente de caminho inesperado fora de ~"),
        }
    }
    Ok(segments.join("/"))
}

fn parent_rel_path(current_rel: &str) -> Option<String> {
    if current_rel.is_empty() {
        return None;
    }
    match current_rel.rsplit_once('/') {
        Some((parent, _)) => Some(parent.to_string()),
        None => Some(String::new()),
    }
}

fn collect_quick_paths(canonical_home: &Path) -> Vec<FileEntryInfo> {
    let candidates = [
        ("", "Início (~)"),
        ("Downloads", "Downloads"),
        ("Documentos", "Documentos"),
        ("Documents", "Documents"),
        ("Área de trabalho", "Área de trabalho"),
        ("Desktop", "Desktop"),
        ("Imagens", "Imagens"),
        ("Pictures", "Pictures"),
        ("Vídeos", "Vídeos"),
        ("Videos", "Videos"),
        ("Música", "Música"),
        ("Music", "Music"),
        ("Projetos", "Projetos"),
    ];

    let mut seen_paths = HashSet::new();
    let mut items = Vec::new();

    for (rel, label) in candidates {
        let Ok(resolved) = resolve_existing_in_home(canonical_home, rel, true) else {
            continue;
        };
        if !seen_paths.insert(resolved) {
            continue;
        }
        items.push(FileEntryInfo {
            name: label.to_string(),
            rel_path: rel.to_string(),
            is_dir: true,
            size: 0,
            modified_unix: 0,
            mime_hint: "inode/directory".to_string(),
        });
    }

    items
}

pub fn infer_mime_hint(file_name: &str) -> &'static str {
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "apk" => "application/vnd.android.package-archive",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "txt" | "log" | "conf" | "ini" | "toml" | "yaml" | "yml" | "sh" | "bash" | "zsh"
        | "rs" | "kt" | "kts" | "java" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "h"
        | "cpp" | "css" | "sql" | "env" => "text/plain",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("oraculo-files-test-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn blocks_traversal_and_external_symlinks() {
        let root = temp_test_dir("sandbox");
        let home = root.join("home");
        let outside = root.join("outside");
        fs::create_dir_all(home.join("Downloads")).unwrap();
        fs::create_dir_all(home.join(".config/oraculo")).unwrap();
        fs::create_dir_all(home.join(".ssh")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(home.join("Downloads/nota.txt"), b"ok").unwrap();
        fs::write(home.join(".config/oraculo/host.key"), b"chave").unwrap();
        fs::write(home.join(".ssh/id_ed25519"), b"ssh").unwrap();
        fs::write(home.join(".bashrc"), b"export PATH").unwrap();
        fs::write(outside.join("segredo.txt"), b"fora").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("segredo.txt"), home.join("link_segredo.txt"))
                .unwrap();
            std::os::unix::fs::symlink(&outside, home.join("link_fora")).unwrap();
            std::os::unix::fs::symlink(home.join("Downloads"), home.join("atalho_downloads"))
                .unwrap();
            std::os::unix::fs::symlink(home.join(".ssh"), home.join("atalho_ssh")).unwrap();
        }

        let canon_home = fs::canonicalize(&home).unwrap();
        assert!(resolve_existing_in_home(&canon_home, "../outside/segredo.txt", false).is_err());
        assert!(resolve_existing_in_home(&canon_home, "/etc/passwd", false).is_err());
        assert!(resolve_existing_in_home(&canon_home, "Downloads/nota.txt", false).is_ok());
        assert!(resolve_existing_in_home(&canon_home, ".config/oraculo/host.key", false).is_err());
        assert!(resolve_existing_in_home(&canon_home, ".ssh/id_ed25519", false).is_err());
        assert!(resolve_existing_in_home(&canon_home, ".bashrc", false).is_err());
        assert!(is_protected_rel_str(".local/share/oraculo-bridge/bridge.key"));
        assert!(is_protected_rel_str(".local/share/oraculo-cli/client.key"));
        assert!(is_protected_rel_str(".config/systemd/user/evil.service"));
        assert!(is_protected_rel_str(".config/environment.d/99-env.conf"));
        assert!(is_protected_rel_str(".config/fish/config.fish"));
        assert!(is_protected_rel_str(".zshenv"));
        assert!(is_protected_rel_str(".bash_login"));

        let listing = list_directory(&canon_home, "", true).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Downloads"));
        assert!(!names.contains(&".ssh"));
        assert!(!names.contains(&".bashrc"));
        #[cfg(unix)]
        {
            assert!(!names.contains(&"link_segredo.txt"));
            assert!(!names.contains(&"link_fora"));
            assert!(!names.contains(&"atalho_ssh"));
            assert!(names.contains(&"atalho_downloads"));
            assert!(resolve_existing_in_home(&canon_home, "link_segredo.txt", false).is_err());
            assert!(resolve_existing_in_home(&canon_home, "link_fora/segredo.txt", false).is_err());
            assert!(resolve_existing_in_home(&canon_home, "atalho_ssh/id_ed25519", false).is_err());
            assert!(
                resolve_existing_in_home(&canon_home, "atalho_downloads/nota.txt", false).is_ok()
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolves_unique_upload_names_and_rejects_bad_filenames() {
        let root = temp_test_dir("upload");
        let canon = fs::canonicalize(&root).unwrap();
        fs::write(canon.join("foto.jpg"), b"123").unwrap();

        let (_, first_free, _) =
            resolve_upload_target(&canon, "", "foto.jpg", false).unwrap();
        assert_eq!(first_free, "foto (1).jpg");

        let (_, exact, _) = resolve_upload_target(&canon, "", "foto.jpg", true).unwrap();
        assert_eq!(exact, "foto.jpg");

        assert!(resolve_upload_target(&canon, "", "../evil.txt", false).is_err());
        assert!(resolve_upload_target(&canon, "", "a/b.txt", false).is_err());
        assert!(resolve_upload_target(&canon, "", ".oraculo-upload-x.tmp", false).is_err());
        assert!(resolve_upload_target(&canon, "", ".bashrc", true).is_err());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn enforces_upload_size_limit_and_toctou_guards() {
        assert!(validate_upload_size(MAX_UPLOAD_BYTES).is_ok());
        assert!(validate_upload_size(MAX_UPLOAD_BYTES + 1).is_err());

        let root = temp_test_dir("toctou");
        let outside = root.join("outside");
        let home = root.join("home");
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir_all(&home).unwrap();
        let canon_home = fs::canonicalize(&home).unwrap();
        let target = canon_home.join("arquivo.bin");

        assert!(verify_upload_destination_before_rename(&canon_home, &canon_home, &target, false).is_ok());

        // Simula conflito criado durante o upload sem overwrite=true
        fs::write(&target, b"ja_existe").unwrap();
        assert!(verify_upload_destination_before_rename(&canon_home, &canon_home, &target, false).is_err());
        assert!(verify_upload_destination_before_rename(&canon_home, &canon_home, &target, true).is_ok());
        fs::remove_file(&target).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("alvo.bin"), &target).unwrap();
            assert!(
                verify_upload_destination_before_rename(&canon_home, &canon_home, &target, true)
                    .is_err()
            );
        }

        let _ = fs::remove_dir_all(&root);
    }
}
