#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod ipc;
mod webview;

use anyhow::Result;
use oraculo_host::config::{
    HostPaths, SettingsStore, TrustedPeersStore, load_or_create_commands,
    load_or_create_secret_key,
};
use oraculo_host::control_ipc::{DaemonControlClient, HostBackend};
use oraculo_host::server::start_embedded_server;
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use webview::InstanceDisposition;

fn main() -> Result<()> {
    let mut start_hidden = false;
    let mut quit_existing = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--background" | "--minimized" => start_hidden = true,
            "--quit" => quit_existing = true,
            "-h" | "--help" => {
                println!(
                    "Uso: oraculo-desktop [--background] [--quit]\n\
                     \n\
                     Opções:\n\
                       --background   Inicia o servidor P2P silenciosamente em segundo plano sem abrir janela\n\
                       --quit         Encerra a instância em execução do Oráculo Desktop"
                );
                return Ok(());
            }
            _ => {}
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    if quit_existing {
        let forwarded = matches!(
            webview::prepare_instance(start_hidden, true),
            Ok(InstanceDisposition::ForwardedToExisting)
        );
        if forwarded {
            std::thread::sleep(Duration::from_millis(120));
        }
        let paths = HostPaths::discover()?;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            if let Some(client) = DaemonControlClient::try_connect(&paths).await {
                let backend = HostBackend::DaemonClient(client);
                backend.shutdown().await;
            }
        });
        rt.shutdown_timeout(Duration::from_millis(500));
        return Ok(());
    }

    // Verifica instância única D-Bus antes de tocar em `host.lock` ou abrir sockets QUIC.
    let window_rt = match webview::prepare_instance(start_hidden, false) {
        Ok(InstanceDisposition::Primary(rt)) => rt,
        Ok(InstanceDisposition::ForwardedToExisting) => return Ok(()),
        Err(err) => {
            notify_startup_error(&format!("{err:#}"));
            return Err(err);
        }
    };

    let paths = HostPaths::discover()?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let backend = match rt.block_on(async {
        if let Some(client) = DaemonControlClient::try_connect(&paths).await {
            webview::set_attached_to_daemon(true);
            return Ok::<_, anyhow::Error>(HostBackend::DaemonClient(client));
        }

        let secret_key = load_or_create_secret_key(&paths.secret_key_file)?;
        let trusted_peers = TrustedPeersStore::load(paths.trusted_peers_file.clone())?;
        let settings = SettingsStore::load_or_create(paths.settings_file.clone())?;
        let _ = load_or_create_commands(&paths.commands_file)?;

        let handle = start_embedded_server(secret_key, paths, trusted_peers, settings).await?;
        webview::set_attached_to_daemon(false);
        Ok(HostBackend::Embedded(handle))
    }) {
        Ok(b) => b,
        Err(err) => {
            notify_startup_error(&format!("{err:#}"));
            return Err(err);
        }
    };

    let url = match rt.block_on(ipc::start_ipc_server(backend.clone(), start_hidden)) {
        Ok(u) => u,
        Err(err) => {
            notify_startup_error(&format!("{err:#}"));
            return Err(err);
        }
    };

    // AdwApplication / GtkApplication / WebView roda na thread principal do sistema operacional.
    let ui_res = window_rt.run(&url, "Oráculo");

    if webview::was_bg_notice_shown() {
        ipc::persist_bg_notice_shown(backend.paths());
    }

    if !backend.is_external_daemon() || webview::was_force_quit_requested() {
        rt.block_on(async {
            backend.shutdown().await;
        });
    }
    rt.shutdown_timeout(Duration::from_millis(800));

    ui_res
}

fn notify_startup_error(msg: &str) {
    eprintln!("Erro ao iniciar Oráculo Desktop: {msg}");
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("notify-send")
            .args(["-a", "Oráculo", "Oráculo", msg])
            .status();
    }
}
