pub mod pairing;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Identificador ALPN único do protocolo Oráculo sobre Iroh/QUIC.
pub const ALPN: &[u8] = b"oraculo/1";

/// Limite máximo por mensagem serializada (4 MiB) para proteger contra frames malformados.
pub const MAX_FRAME_SIZE: usize = 4 * 1024 * 1024;

/// Tamanho de bloco padrão para transferência streaming de arquivos (128 KiB).
pub const FILE_CHUNK_SIZE: usize = 128 * 1024;

/// Primeiro byte de todo stream bidirecional aberto sobre o ALPN do Oráculo.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Pair = 1,
    Control = 2,
    Terminal = 3,
    Input = 4,
    Files = 5,
    Screen = 6,
}

impl TryFrom<u8> for StreamKind {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Pair),
            2 => Ok(Self::Control),
            3 => Ok(Self::Terminal),
            4 => Ok(Self::Input),
            5 => Ok(Self::Files),
            6 => Ok(Self::Screen),
            other => bail!("Tipo de stream desconhecido: {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairRequest {
    pub secret: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairResponse {
    pub accepted: bool,
    pub host_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickCommandInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    #[serde(default)]
    pub command_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntryInfo {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_unix: u64,
    pub mime_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryListing {
    pub current_path: String,
    pub home_label: String,
    pub parent_path: Option<String>,
    pub quick_paths: Vec<FileEntryInfo>,
    pub entries: Vec<FileEntryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlRequest {
    Ping,
    ListCommands,
    RunCommand { id: String },
    SaveCommand {
        id: Option<String>,
        label: String,
        description: String,
        command_line: String,
    },
    DeleteCommand { id: String },
    ListFiles {
        rel_path: String,
        show_hidden: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlResponse {
    Pong { host_name: String },
    Commands(Vec<QuickCommandInfo>),
    CommandResult(CommandOutput),
    DirectoryListing(DirectoryListing),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTransferRequest {
    Download {
        rel_path: String,
    },
    Upload {
        dir_rel_path: String,
        file_name: String,
        total_size: u64,
        overwrite: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileHostMsg {
    DownloadStart {
        file_name: String,
        total_size: u64,
        mime_hint: String,
    },
    UploadReady {
        resolved_name: String,
    },
    Chunk(Vec<u8>),
    Done {
        bytes_transferred: u64,
        saved_rel_path: String,
    },
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileClientMsg {
    Chunk(Vec<u8>),
    Finish,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInit {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalClientMsg {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalHostMsg {
    Output(Vec<u8>),
    Exit(i32),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteKey {
    Backspace,
    Enter,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    Super,
    Control,
    Alt,
    Shift,
    F4,
    F5,
    F11,
    VolumeMute,
    VolumeDown,
    VolumeUp,
    PlayPause,
    MediaNext,
    MediaPrev,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputInitAck {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    Move { dx: f64, dy: f64 },
    Button { button: MouseButton, pressed: bool },
    Click { button: MouseButton },
    Scroll { dx: f64, dy: f64 },
    Key(RemoteKey),
    Text(String),
    KeyState { key: RemoteKey, pressed: bool },
}

/// Perfis de qualidade predefinidos para streaming de tela em H.264 Annex-B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenPreset {
    /// Conservador para dispositivos mais antigos/redes instáveis: até 1280x720 @ 30fps (~2 Mbps).
    Balanced720p30,
    /// Alta fluidez para LAN / hardware recente: até 1920x1080 @ 60fps (~8,5 Mbps VBR).
    Smooth1080p60,
    /// Padrão nítido nativo para monitores Full HD: até 1920x1080 @ 30fps (~5,5 Mbps VBR).
    Crisp1080p30,
}

impl ScreenPreset {
    pub fn defaults(self) -> (u16, u16, u8, u32) {
        match self {
            Self::Balanced720p30 => (1280, 720, 30, 2000),
            Self::Smooth1080p60 => (1920, 1080, 60, 8500),
            Self::Crisp1080p30 => (1920, 1080, 30, 5500),
        }
    }
}

/// Parâmetros iniciais (ou de reconfiguração) enviados pelo cliente ao abrir `StreamKind::Screen`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenStart {
    pub max_width: u16,
    pub max_height: u16,
    pub fps: u8,
    pub bitrate_kbps: u32,
}

impl ScreenStart {
    pub fn from_preset(preset: ScreenPreset) -> Self {
        let (max_width, max_height, fps, bitrate_kbps) = preset.defaults();
        Self {
            max_width,
            max_height,
            fps,
            bitrate_kbps,
        }
    }

    /// Ajusta dimensões para múltiplos de 2 e limites seguros de H.264.
    pub fn sanitized(&self) -> Self {
        Self {
            max_width: even_dim(self.max_width.clamp(320, 3840)),
            max_height: even_dim(self.max_height.clamp(240, 2160)),
            fps: self.fps.clamp(10, 60),
            bitrate_kbps: self.bitrate_kbps.clamp(400, 20_000),
        }
    }
}

fn even_dim(v: u16) -> u16 {
    v & !1
}

/// Comandos de controle enviados pelo cliente durante o streaming de tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenClientMsg {
    RequestKeyframe,
    Reconfigure(ScreenStart),
    SwitchMonitor,
    /// Solicitação explícita para reabrir o seletor de monitores no desktop (apenas quando confirmado pelo usuário).
    ReauthorizeMonitors,
    Stop,
}

/// Mensagens de vídeo e estado enviadas pelo host durante `StreamKind::Screen`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenHostMsg {
    Ready {
        width: u16,
        height: u16,
        fps: u8,
        encoder: String,
        monitor_index: u8,
        monitor_count: u8,
        available_monitors: u8,
    },
    /// Access Unit completo em H.264 Annex-B (`00 00 00 01 ...`).
    /// Quando `keyframe == true`, inclui SPS + PPS antes do IDR para sincronização imediata no decodificador.
    Frame {
        seq: u64,
        pts_ms: u64,
        keyframe: bool,
        data: Vec<u8>,
    },
    /// Aviso não-fatal exibido na UI sem interromper o streaming em curso.
    Notice(String),
    Error(String),
}

/// Escreve o cabeçalho de 1 byte identificando o tipo de stream.
pub async fn write_stream_kind<W: AsyncWrite + Unpin>(writer: &mut W, kind: StreamKind) -> Result<()> {
    writer.write_all(&[kind as u8]).await?;
    Ok(())
}

/// Lê o cabeçalho de 1 byte identificando o tipo de stream.
pub async fn read_stream_kind<R: AsyncRead + Unpin>(reader: &mut R) -> Result<StreamKind> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf).await?;
    StreamKind::try_from(buf[0])
}

/// Serializa uma mensagem com `postcard` e envia precedida pelo tamanho (u32 big-endian) em uma única escrita.
pub async fn write_frame<W, T>(writer: &mut W, msg: &T) -> Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let mut framed = postcard::to_extend(msg, vec![0u8; 4])?;
    let payload_len = framed.len() - 4;
    if payload_len > MAX_FRAME_SIZE {
        bail!("Frame excede limite máximo de {MAX_FRAME_SIZE} bytes");
    }
    let len = u32::try_from(payload_len)?;
    framed[..4].copy_from_slice(&len.to_be_bytes());
    writer.write_all(&framed).await?;
    Ok(())
}

/// Lê um frame prefixado por tamanho (u32 big-endian) e desserializa com `postcard`.
/// Retorna `Ok(None)` em EOF limpo antes do cabeçalho de tamanho.
pub async fn read_frame<R, T>(reader: &mut R) -> Result<Option<T>>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        bail!("Frame recebido ({len} bytes) excede limite de {MAX_FRAME_SIZE}");
    }

    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    let msg = postcard::from_bytes(&payload).map_err(|err| {
        anyhow::anyhow!(
            "Protocolo incompatível ou frame inválido ({err}). Se atualizou o app, reinicie o oraculo-host no PC."
        )
    })?;
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_frames() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        write_stream_kind(&mut tx, StreamKind::Control).await.unwrap();
        write_frame(&mut tx, &ControlRequest::RunCommand { id: "uptime".into() })
            .await
            .unwrap();
        drop(tx);

        assert_eq!(read_stream_kind(&mut rx).await.unwrap(), StreamKind::Control);
        let msg: ControlRequest = read_frame(&mut rx).await.unwrap().unwrap();
        assert!(matches!(msg, ControlRequest::RunCommand { id } if id == "uptime"));
        assert!(read_frame::<_, ControlRequest>(&mut rx).await.unwrap().is_none());

        let (mut stx, mut srx) = tokio::io::duplex(2048);
        write_frame(
            &mut stx,
            &ControlRequest::SaveCommand {
                id: None,
                label: "Teste".into(),
                description: "Roda teste".into(),
                command_line: "echo ok".into(),
            },
        )
        .await
        .unwrap();
        write_frame(&mut stx, &ControlRequest::DeleteCommand { id: "teste".into() })
            .await
            .unwrap();
        drop(stx);
        assert!(matches!(
            read_frame::<_, ControlRequest>(&mut srx).await.unwrap().unwrap(),
            ControlRequest::SaveCommand { label, command_line, .. }
                if label == "Teste" && command_line == "echo ok"
        ));
        assert!(matches!(
            read_frame::<_, ControlRequest>(&mut srx).await.unwrap().unwrap(),
            ControlRequest::DeleteCommand { id } if id == "teste"
        ));

        let (mut itx, mut irx) = tokio::io::duplex(1024);
        write_stream_kind(&mut itx, StreamKind::Input).await.unwrap();
        write_frame(&mut itx, &InputEvent::Move { dx: 12.5, dy: -4.0 })
            .await
            .unwrap();
        write_frame(
            &mut itx,
            &InputEvent::KeyState {
                key: RemoteKey::Super,
                pressed: true,
            },
        )
        .await
        .unwrap();
        drop(itx);
        assert_eq!(read_stream_kind(&mut irx).await.unwrap(), StreamKind::Input);
        let ev: InputEvent = read_frame(&mut irx).await.unwrap().unwrap();
        assert!(matches!(ev, InputEvent::Move { dx, dy } if dx == 12.5 && dy == -4.0));
        let ev2: InputEvent = read_frame(&mut irx).await.unwrap().unwrap();
        assert!(matches!(
            ev2,
            InputEvent::KeyState {
                key: RemoteKey::Super,
                pressed: true
            }
        ));

        let (mut ftx, mut frx) = tokio::io::duplex(2048);
        write_stream_kind(&mut ftx, StreamKind::Files).await.unwrap();
        write_frame(
            &mut ftx,
            &FileTransferRequest::Upload {
                dir_rel_path: "Downloads".into(),
                file_name: "foto.png".into(),
                total_size: 42,
                overwrite: false,
            },
        )
        .await
        .unwrap();
        write_frame(&mut ftx, &FileClientMsg::Chunk(vec![1, 2, 3]))
            .await
            .unwrap();
        write_frame(&mut ftx, &FileClientMsg::Finish).await.unwrap();
        drop(ftx);

        assert_eq!(read_stream_kind(&mut frx).await.unwrap(), StreamKind::Files);
        assert!(matches!(
            read_frame::<_, FileTransferRequest>(&mut frx).await.unwrap().unwrap(),
            FileTransferRequest::Upload { dir_rel_path, file_name, total_size: 42, overwrite: false }
                if dir_rel_path == "Downloads" && file_name == "foto.png"
        ));
        assert!(matches!(
            read_frame::<_, FileClientMsg>(&mut frx).await.unwrap().unwrap(),
            FileClientMsg::Chunk(data) if data == vec![1, 2, 3]
        ));
        assert!(matches!(
            read_frame::<_, FileClientMsg>(&mut frx).await.unwrap().unwrap(),
            FileClientMsg::Finish
        ));

        let raw_screen = ScreenStart {
            max_width: 1281,
            max_height: 120,
            fps: 90,
            bitrate_kbps: 100,
        };
        assert_eq!(
            raw_screen.sanitized(),
            ScreenStart {
                max_width: 1280,
                max_height: 240,
                fps: 60,
                bitrate_kbps: 400,
            }
        );

        let (mut vtx, mut vrx) = tokio::io::duplex(4096);
        let init = ScreenStart::from_preset(ScreenPreset::Crisp1080p30);
        assert_eq!(
            init,
            ScreenStart {
                max_width: 1920,
                max_height: 1080,
                fps: 30,
                bitrate_kbps: 5500,
            }
        );
        let p720 = ScreenStart::from_preset(ScreenPreset::Balanced720p30);
        write_stream_kind(&mut vtx, StreamKind::Screen).await.unwrap();
        write_frame(&mut vtx, &p720).await.unwrap();
        write_frame(&mut vtx, &ScreenClientMsg::RequestKeyframe).await.unwrap();
        write_frame(
            &mut vtx,
            &ScreenClientMsg::Reconfigure(ScreenStart::from_preset(ScreenPreset::Smooth1080p60)),
        )
        .await
        .unwrap();
        write_frame(&mut vtx, &ScreenClientMsg::SwitchMonitor).await.unwrap();
        write_frame(&mut vtx, &ScreenClientMsg::ReauthorizeMonitors).await.unwrap();
        write_frame(&mut vtx, &ScreenClientMsg::Stop).await.unwrap();
        drop(vtx);

        assert_eq!(read_stream_kind(&mut vrx).await.unwrap(), StreamKind::Screen);
        assert_eq!(
            read_frame::<_, ScreenStart>(&mut vrx).await.unwrap().unwrap(),
            ScreenStart {
                max_width: 1280,
                max_height: 720,
                fps: 30,
                bitrate_kbps: 2000,
            }
        );
        assert_eq!(
            read_frame::<_, ScreenClientMsg>(&mut vrx).await.unwrap().unwrap(),
            ScreenClientMsg::RequestKeyframe
        );
        assert_eq!(
            read_frame::<_, ScreenClientMsg>(&mut vrx).await.unwrap().unwrap(),
            ScreenClientMsg::Reconfigure(ScreenStart {
                max_width: 1920,
                max_height: 1080,
                fps: 60,
                bitrate_kbps: 8500,
            })
        );
        assert_eq!(
            read_frame::<_, ScreenClientMsg>(&mut vrx).await.unwrap().unwrap(),
            ScreenClientMsg::SwitchMonitor
        );
        assert_eq!(
            read_frame::<_, ScreenClientMsg>(&mut vrx).await.unwrap().unwrap(),
            ScreenClientMsg::ReauthorizeMonitors
        );
        assert_eq!(
            read_frame::<_, ScreenClientMsg>(&mut vrx).await.unwrap().unwrap(),
            ScreenClientMsg::Stop
        );

        let (mut htx, mut hrx) = tokio::io::duplex(4096);
        write_frame(
            &mut htx,
            &ScreenHostMsg::Ready {
                width: 1152,
                height: 720,
                fps: 30,
                encoder: "nvh264enc".into(),
                monitor_index: 1,
                monitor_count: 2,
                available_monitors: 2,
            },
        )
        .await
        .unwrap();
        write_frame(
            &mut htx,
            &ScreenHostMsg::Frame {
                seq: 7,
                pts_ms: 233,
                keyframe: true,
                data: vec![0, 0, 0, 1, 0x67, 0x42],
            },
        )
        .await
        .unwrap();
        write_frame(&mut htx, &ScreenHostMsg::Notice("ok".into()))
            .await
            .unwrap();
        drop(htx);

        assert_eq!(
            read_frame::<_, ScreenHostMsg>(&mut hrx).await.unwrap().unwrap(),
            ScreenHostMsg::Ready {
                width: 1152,
                height: 720,
                fps: 30,
                encoder: "nvh264enc".into(),
                monitor_index: 1,
                monitor_count: 2,
                available_monitors: 2,
            }
        );
        assert_eq!(
            read_frame::<_, ScreenHostMsg>(&mut hrx).await.unwrap().unwrap(),
            ScreenHostMsg::Frame {
                seq: 7,
                pts_ms: 233,
                keyframe: true,
                data: vec![0, 0, 0, 1, 0x67, 0x42],
            }
        );
        assert_eq!(
            read_frame::<_, ScreenHostMsg>(&mut hrx).await.unwrap().unwrap(),
            ScreenHostMsg::Notice("ok".into())
        );
    }
}
