use anyhow::Result;
use clap::{Parser, Subcommand};
use oraculo_host::config::{
    HostPaths, SettingsStore, TrustedPeersStore, load_or_create_commands,
    load_or_create_secret_key, normalize_wol_mac,
};
use oraculo_host::{screen, server, wol};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "oraculo-host", about = "Daemon P2P (Iroh) do Oráculo para Linux e Windows")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Inicia o servidor P2P do Oráculo (padrão)
    Serve {
        /// Exibe imediatamente um QR Code de pareamento ao iniciar
        #[arg(long)]
        pair: bool,
    },
    /// Lista dispositivos pareados
    Peers,
    /// Remove o acesso de um dispositivo pareado pelo EndpointId
    Revoke { endpoint_id: String },
    /// Consulta ou altera permissões e Wake-on-LAN em settings.toml
    Permissions {
        #[arg(long)]
        terminal: Option<bool>,
        #[arg(long)]
        files_read: Option<bool>,
        #[arg(long)]
        files_write: Option<bool>,
        #[arg(long)]
        input: Option<bool>,
        #[arg(long)]
        screen: Option<bool>,
        #[arg(long)]
        commands_run: Option<bool>,
        #[arg(long)]
        commands_edit: Option<bool>,
        #[arg(long)]
        rdp_tunnel: Option<bool>,
        #[arg(long)]
        rdp_port: Option<u16>,
        #[arg(long)]
        wol_enabled: Option<bool>,
        #[arg(long)]
        wol_mac: Option<String>,
        #[arg(long)]
        wol_broadcast: Option<String>,
    },
    /// Mostra os caminhos de configuração e identidade do host
    Info,
    /// Abre o seletor do sistema para autorizar todos os monitores para troca remota pelo celular
    Monitors,
    /// Inspeciona ou ativa o Wake-on-LAN (placas físicas e NetworkManager)
    Wol {
        /// Ativa Wake-on-LAN em settings.toml e habilita Magic Packet no NetworkManager (Linux)
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        /// Desativa o anúncio de dados de Wake-on-LAN para clientes pareados
        #[arg(long)]
        disable: bool,
        /// Fixa um endereço MAC específico (vazio = autodetectar placas físicas)
        #[arg(long)]
        mac: Option<String>,
        /// Fixa um endereço de broadcast customizado (vazio = autodetectar sub-rede)
        #[arg(long)]
        broadcast: Option<String>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    let paths = HostPaths::discover()?;
    let secret_key = load_or_create_secret_key(&paths.secret_key_file)?;
    let trusted_peers = TrustedPeersStore::load(paths.trusted_peers_file.clone())?;
    let settings = SettingsStore::load_or_create(paths.settings_file.clone())?;

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        match cli.command.unwrap_or(Command::Serve { pair: false }) {
            Command::Serve { pair } => {
                let _ = load_or_create_commands(&paths.commands_file)?;
                server::run_server(secret_key, paths, trusted_peers, settings, pair).await?;
            }
            Command::Peers => {
                let peers = trusted_peers.list().await;
                if peers.is_empty() {
                    println!("Nenhum dispositivo pareado ainda.");
                } else {
                    for peer in peers {
                        println!("{}  ({})", peer.endpoint_id, peer.name);
                    }
                }
            }
            Command::Revoke { endpoint_id } => {
                if trusted_peers.revoke(&endpoint_id).await? {
                    println!("Acesso revogado para {endpoint_id}");
                } else {
                    println!("Dispositivo não encontrado: {endpoint_id}");
                }
            }
            Command::Permissions {
                terminal,
                files_read,
                files_write,
                input,
                screen,
                commands_run,
                commands_edit,
                rdp_tunnel,
                rdp_port,
                wol_enabled,
                wol_mac,
                wol_broadcast,
            } => {
                let has_changes = terminal.is_some()
                    || files_read.is_some()
                    || files_write.is_some()
                    || input.is_some()
                    || screen.is_some()
                    || commands_run.is_some()
                    || commands_edit.is_some()
                    || rdp_tunnel.is_some()
                    || rdp_port.is_some()
                    || wol_enabled.is_some()
                    || wol_mac.is_some()
                    || wol_broadcast.is_some();

                let effective = if has_changes {
                    settings
                        .update(|s| {
                            if let Some(v) = terminal {
                                s.allow_terminal = v;
                            }
                            if let Some(v) = files_read {
                                s.allow_files_read = v;
                            }
                            if let Some(v) = files_write {
                                s.allow_files_write = v;
                            }
                            if let Some(v) = input {
                                s.allow_input = v;
                            }
                            if let Some(v) = screen {
                                s.allow_screen = v;
                            }
                            if let Some(v) = commands_run {
                                s.allow_commands_run = v;
                            }
                            if let Some(v) = commands_edit {
                                s.allow_commands_edit = v;
                            }
                            if let Some(v) = rdp_tunnel {
                                s.allow_rdp_tunnel = v;
                            }
                            if let Some(port) = rdp_port {
                                if port == 0 {
                                    anyhow::bail!("Porta RDP deve ser entre 1 e 65535");
                                }
                                s.rdp_port = port;
                            }
                            if let Some(v) = wol_enabled {
                                s.wol_enabled = v;
                            }
                            if let Some(mac) = wol_mac {
                                let trimmed = mac.trim();
                                s.wol_mac = if trimmed.is_empty() {
                                    None
                                } else {
                                    Some(normalize_wol_mac(trimmed)?)
                                };
                            }
                            if let Some(bc) = wol_broadcast {
                                let trimmed = bc.trim();
                                s.wol_broadcast = if trimmed.is_empty() {
                                    None
                                } else {
                                    let safe = oraculo_proto::wol::parse_safe_lan_broadcast_v4(trimmed)
                                        .ok_or_else(|| {
                                            anyhow::anyhow!("Broadcast IPv4 inválido: {trimmed}")
                                        })?;
                                    Some(safe.to_string())
                                };
                            }
                            Ok(())
                        })
                        .await?
                } else {
                    settings.current().await
                };

                println!("Arquivo:        {}", paths.settings_file.display());
                println!("terminal:       {}", effective.allow_terminal);
                println!("files_read:     {}", effective.allow_files_read);
                println!("files_write:    {}", effective.allow_files_write);
                println!("input:          {}", effective.allow_input);
                println!("screen:         {}", effective.allow_screen);
                println!("commands_run:   {}", effective.allow_commands_run);
                println!("commands_edit:  {}", effective.allow_commands_edit);
                println!("rdp_tunnel:     {}", effective.allow_rdp_tunnel);
                println!("rdp_port:       {}", effective.rdp_port);
                println!("wol_enabled:    {}", effective.wol_enabled);
                println!(
                    "wol_mac:        {}",
                    effective.wol_mac.as_deref().unwrap_or("(não configurado)")
                );
                println!(
                    "wol_broadcast:  {}",
                    effective
                        .wol_broadcast
                        .as_deref()
                        .unwrap_or("(automático)")
                );
            }
            Command::Info => {
                println!("EndpointId:     {}", secret_key.public());
                println!("Chave do host:  {}", paths.secret_key_file.display());
                println!("Peers salvos:   {}", paths.trusted_peers_file.display());
                println!("Comandos:       {}", paths.commands_file.display());
                println!("Permissões:     {}", paths.settings_file.display());
            }
            Command::Monitors => {
                let cur = settings.current().await;
                let portal = oraculo_host::portal::PortalCoordinator::from_host_paths(&paths);
                screen::authorize_monitors_with_portal(&portal, cur.allow_input, cur.allow_screen)
                    .await?;
            }
            Command::Wol {
                enable,
                disable,
                mac,
                broadcast,
            } => {
                let changed = enable || disable || mac.is_some() || broadcast.is_some();
                let effective = if changed {
                    settings
                        .update(|s| {
                            if enable {
                                s.wol_enabled = true;
                            } else if disable {
                                s.wol_enabled = false;
                            }
                            if let Some(raw_mac) = mac {
                                let trimmed = raw_mac.trim();
                                s.wol_mac = if trimmed.is_empty() {
                                    None
                                } else {
                                    Some(normalize_wol_mac(trimmed)?)
                                };
                            }
                            if let Some(raw_bc) = broadcast {
                                let trimmed = raw_bc.trim();
                                s.wol_broadcast = if trimmed.is_empty() {
                                    None
                                } else {
                                    let safe = oraculo_proto::wol::parse_safe_lan_broadcast_v4(trimmed)
                                        .ok_or_else(|| {
                                            anyhow::anyhow!("Broadcast IPv4 inválido: {trimmed}")
                                        })?;
                                    Some(safe.to_string())
                                };
                            }
                            Ok(())
                        })
                        .await?
                } else {
                    settings.current().await
                };

                if enable {
                    for res in wol::enable_nm_magic_packet() {
                        match res {
                            Ok(msg) => println!("[NetworkManager] Ativado: {msg}"),
                            Err(err) => eprintln!("[NetworkManager] Aviso: {err}"),
                        }
                    }
                }

                println!("Wake-on-LAN (settings.toml): {}", if effective.wol_enabled { "ativo" } else { "desativado" });
                let nics = wol::discover_nics();
                if nics.is_empty() {
                    println!("Placas físicas detectadas:   (nenhuma)");
                } else {
                    println!("Placas físicas detectadas:");
                    for nic in &nics {
                        let kind = if nic.is_wireless { "Wi-Fi" } else { "Ethernet" };
                        let state_str = if nic.is_up { "conectada" } else { "desconectada" };
                        let route_str = if nic.is_default_route { " · rota padrão" } else { "" };
                        let bc_str = nic
                            .broadcast_v4
                            .as_deref()
                            .map(|b| format!(" · broadcast {b}"))
                            .unwrap_or_default();
                        let perm_str = nic
                            .perm_mac
                            .as_deref()
                            .map(|p| format!(" (físico: {p})"))
                            .unwrap_or_default();
                        println!(
                            "  - {} ({kind}, {state_str}{route_str}): {}{perm_str}{bc_str}",
                            nic.name, nic.mac
                        );
                    }
                }

                if let Some(resolved) = wol::resolve_host_wol_info(&effective) {
                    println!("MACs anunciados aos peers:   {}", resolved.macs.join(", "));
                    println!(
                        "Broadcast anunciado:         {}",
                        resolved.broadcast.as_deref().unwrap_or("255.255.255.255")
                    );
                }

                let nm_profiles = wol::inspect_nm_wol_profiles();
                if !nm_profiles.is_empty() {
                    println!("Perfis NetworkManager:");
                    for p in nm_profiles {
                        let dev = p.device.as_deref().unwrap_or("inativo");
                        let status = if p.magic_enabled {
                            "Magic Packet ATIVO"
                        } else {
                            "Magic Packet desativado (rode 'oraculo-host wol --enable')"
                        };
                        println!("  - {} [{} · {dev}]: {status}", p.name, p.conn_type);
                    }
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    // Evita bloquear no drop do runtime caso threads de stdin/PTY bloqueantes ainda estejam pendentes.
    rt.shutdown_timeout(Duration::from_millis(800));
    result
}
