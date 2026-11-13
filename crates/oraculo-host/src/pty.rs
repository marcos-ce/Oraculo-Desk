use crate::config::HostSettings;
use anyhow::{Context, Result};
use iroh::endpoint::{RecvStream, SendStream};
use oraculo_proto::{
    TerminalClientMsg, TerminalHostMsg, TerminalInit, read_frame, write_frame,
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use tokio::sync::{mpsc, watch};

/// Atende uma sessão interativa de terminal PTY (Linux PTY / Windows ConPTY).
pub async fn handle_terminal_stream(
    mut send: SendStream,
    mut recv: RecvStream,
    mut settings_rx: watch::Receiver<HostSettings>,
) -> Result<()> {
    if !settings_rx.borrow().allow_terminal {
        let _ = write_frame(
            &mut send,
            &TerminalHostMsg::Error("Terminal remoto desativado pelo host".to_string()),
        )
        .await;
        let _ = send.finish();
        return Ok(());
    }

    let init: TerminalInit = match read_frame(&mut recv).await? {
        Some(init) => init,
        None => return Ok(()),
    };

    if !settings_rx.borrow().allow_terminal {
        let _ = write_frame(
            &mut send,
            &TerminalHostMsg::Error("Terminal remoto desativado pelo host".to_string()),
        )
        .await;
        let _ = send.finish();
        return Ok(());
    }

    let size = PtySize {
        rows: init.rows.max(1),
        cols: init.cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(size).context("Falha ao abrir PTY")?;

    let mut cmd = default_shell_command();
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .context("Falha ao iniciar shell no PTY")?;
    let mut killer = child.clone_killer();

    // Fecha o lado slave no processo pai para garantir detecção limpa de EOF ao encerrar o shell.
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (out_tx, mut out_rx) = mpsc::channel::<TerminalHostMsg>(64);
    let (in_tx, mut in_rx) = mpsc::channel::<Vec<u8>>(64);

    // Leitura bloqueante do PTY -> canal async
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

    // Escrita bloqueante no PTY (mantém ordem dos bytes digitados)
    let writer_task = tokio::task::spawn_blocking(move || {
        while let Some(bytes) = in_rx.blocking_recv() {
            if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });

    // Envia saída do PTY para o cliente via QUIC
    let send_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let is_terminal = matches!(msg, TerminalHostMsg::Exit(_) | TerminalHostMsg::Error(_));
            if write_frame(&mut send, &msg).await.is_err() {
                break;
            }
            if is_terminal {
                break;
            }
        }
        let _ = send.finish();
    });

    // Lê teclas e redimensionamentos vindos do cliente
    let master = pair.master;
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
            frame = read_frame::<_, TerminalClientMsg>(&mut recv) => frame?,
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

    drop(out_tx);
    drop(in_tx);
    drop(master);
    let _ = tokio::task::spawn_blocking(move || killer.kill()).await;
    let _ = tokio::join!(
        tokio::time::timeout(std::time::Duration::from_secs(2), reader_task),
        writer_task,
        send_task
    );
    Ok(())
}

fn default_shell_command() -> CommandBuilder {
    if cfg!(windows) {
        let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string());
        CommandBuilder::new(shell)
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        CommandBuilder::new(shell)
    }
}
