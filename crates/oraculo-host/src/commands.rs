use crate::config::QuickCommandSpec;
use oraculo_proto::CommandOutput;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_COMMAND_OUTPUT_BYTES: usize = 256 * 1024;

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(mut reader: R, limit: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut limited = (&mut reader).take((limit + 1) as u64);
    let _ = limited.read_to_end(&mut buf).await;
    if buf.len() > limit {
        buf.truncate(limit);
        // Drena o restante sem alocar em memória para não bloquear o processo filho em SIGPIPE.
        let mut sink = tokio::io::sink();
        let _ = tokio::io::copy(&mut reader, &mut sink).await;
        buf.extend_from_slice(
            b"\n[... sa\xc3\xadda truncada pelo limite de seguran\xc3\xa7a de 256 KB ...]",
        );
    }
    buf
}

/// Executa um comando rápido pré-autorizado em `commands.toml` com timeout seguro e limite de memória.
pub async fn run_quick_command(spec: &QuickCommandSpec) -> CommandOutput {
    #[cfg(target_os = "linux")]
    let mut cmd = if std::path::Path::new("/.flatpak-info").exists() {
        let mut c = Command::new("flatpak-spawn");
        c.arg("--host").arg(&spec.program).args(&spec.args);
        c
    } else {
        let mut c = Command::new(&spec.program);
        c.args(&spec.args);
        c
    };
    #[cfg(not(target_os = "linux"))]
    let mut cmd = {
        let mut c = Command::new(&spec.program);
        c.args(&spec.args);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return CommandOutput {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Falha ao iniciar '{}': {err}", spec.program),
            };
        }
    };

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let run_fut = async {
        let out_fut = async {
            match stdout_pipe {
                Some(pipe) => read_bounded(pipe, MAX_COMMAND_OUTPUT_BYTES).await,
                None => Vec::new(),
            }
        };
        let err_fut = async {
            match stderr_pipe {
                Some(pipe) => read_bounded(pipe, MAX_COMMAND_OUTPUT_BYTES).await,
                None => Vec::new(),
            }
        };
        let (stdout_bytes, stderr_bytes, status_res) =
            tokio::join!(out_fut, err_fut, child.wait());
        (stdout_bytes, stderr_bytes, status_res)
    };

    match tokio::time::timeout(COMMAND_TIMEOUT, run_fut).await {
        Ok((stdout_bytes, stderr_bytes, Ok(status))) => CommandOutput {
            exit_code: status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        },
        Ok((_, _, Err(err))) => CommandOutput {
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("Falha ao aguardar '{}': {err}", spec.program),
        },
        Err(_) => {
            let _ = child.kill().await;
            CommandOutput {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Tempo limite de {}s excedido", COMMAND_TIMEOUT.as_secs()),
            }
        }
    }
}
