//! Localização centralizada (PT-BR / EN) de mensagens de erro, avisos de stream
//! e rótulos operacionais enviados pelo protocolo/backend do Oráculo.

/// Idioma de exibição usado nos clientes (Android / Desktop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiLang {
    #[default]
    Pt,
    En,
}

impl UiLang {
    /// Converte código de idioma (`"en"`, `"en-US"`, `"pt"`, `"pt-BR"`, `"auto"`).
    pub fn from_code(raw: &str) -> Self {
        let lower = raw.trim().to_ascii_lowercase();
        if lower.starts_with("en") {
            Self::En
        } else if lower.starts_with("pt") {
            Self::Pt
        } else {
            Self::from_system_env()
        }
    }

    /// Detecta o idioma padrão a partir das variáveis POSIX (`LC_ALL`, `LC_MESSAGES`, `LANG`).
    pub fn from_system_env() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(val) = std::env::var(key) {
                let trimmed = val.trim();
                if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("c") && !trimmed.eq_ignore_ascii_case("posix") {
                    if trimmed.to_ascii_lowercase().starts_with("pt") {
                        return Self::Pt;
                    }
                    return Self::En;
                }
            }
        }
        Self::En
    }

    pub fn as_code(self) -> &'static str {
        match self {
            Self::Pt => "pt",
            Self::En => "en",
        }
    }
}

/// Traduz mensagens operacionais e erros do protocolo/backend quando o cliente usa Inglês.
/// No modo Português (`UiLang::Pt`), retorna o texto original intacto.
pub fn localize_text(raw: &str, lang: UiLang) -> String {
    if lang == UiLang::Pt || raw.is_empty() {
        return raw.to_string();
    }
    translate_pt_to_en(raw)
}

/// Classifica se um erro remoto é permanente (permissão desativada, tela bloqueada,
/// autorização cancelada ou recurso inexistente), independente do idioma (PT ou EN).
pub fn is_permanent_remote_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    [
        // Português
        "bloqueada",
        "desativad",
        "cancelad",
        "negad",
        "nenhum monitor",
        "não encontrada",
        "nao encontrada",
        "não pareado",
        "nao pareado",
        "revogad",
        "acesso bloqueado",
        "upload bloqueado",
        // Inglês
        "locked",
        "disabled",
        "canceled",
        "cancelled",
        "denied",
        "no monitor",
        "not found",
        "not paired",
        "unpaired",
        "revoked",
        "access blocked",
        "upload blocked",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn translate_action_phrase(action: &str) -> &str {
    match action.trim() {
        "permissão de captura de tela" => "screen capture permission",
        "permissão de mouse e teclado" => "mouse and keyboard permission",
        "permissão de tela e controle remoto" => "screen and remote control permission",
        other => other,
    }
}

fn translate_exact(msg: &str) -> Option<&'static str> {
    Some(match msg {
        // Conexão / Pareamento / Sessão
        "Computador indisponível ou Oráculo fechado no PC" => {
            "Computer unavailable or Oráculo is closed on the PC"
        }
        "Computador salvo não encontrado" => "Saved computer not found",
        "Acesso recusado pelo host" => "Access denied by host",
        "Acesso removido neste computador. Faça o pareamento novamente." => {
            "Access removed on this computer. Pair again."
        }
        "Assinatura criptográfica do host inválida (conexão bloqueada)" => {
            "Invalid host cryptographic signature (connection blocked)"
        }
        "Conexão encerrada pelo computador." => "Connection closed by the computer.",
        "Canal de estado encerrado pelo computador." => "State channel closed by the computer.",
        "O Oráculo foi encerrado no computador." => "Oráculo was closed on the computer.",
        "Host Oráculo encerrado" => "Oráculo host shut down",
        "Sessão desconectada no computador." => "Session disconnected on the computer.",
        "Sessão encerrada pelo usuário" => "Session ended by user",
        "Dispositivo não pareado" => "Device not paired",
        "Dispositivo não pareado nesta ponte" => "Device not paired on this bridge",
        "Dispositivo revogado no host" => "Device revoked on host",
        "Código inválido ou expirado" => "Invalid or expired code",
        "Código de pareamento inválido ou expirado" => "Invalid or expired pairing code",
        "Código da ponte inválido ou expirado" => "Invalid or expired bridge code",
        "Token de pareamento inválido ou expirado" => "Invalid or expired pairing token",
        "Código de pareamento inválido: informe os 8 caracteres (ex.: K7P4-M9X2)" => {
            "Invalid pairing code: enter all 8 characters (e.g. K7P4-M9X2)"
        }
        "Código não encontrado ou expirado. Confira os 8 caracteres e gere um novo código com 'p' no PC." => {
            "Code not found or expired. Check the 8 characters and generate a new code on the computer."
        }
        "Pareado por código com sucesso" => "Paired with code successfully",
        "Ponte Wake-on-LAN pareada com sucesso" => "Wake-on-LAN bridge paired successfully",
        "Parâmetro 'id' ausente na URI de pareamento" => "Missing 'id' parameter in pairing URI",
        "Parâmetro 'secret' ausente na URI de pareamento" => {
            "Missing 'secret' parameter in pairing URI"
        }
        "Parâmetro 'id' ausente na URI MCU" => "Missing 'id' parameter in MCU URI",
        "Parâmetro 's' (segredo) ausente na URI MCU" => "Missing 's' (secret) parameter in MCU URI",
        "Segredo hexadecimal inválido" => "Invalid hexadecimal secret",
        "Chave da ponte com tamanho inválido" => "Invalid bridge key length",
        "Identificador da ponte MCU inválido (esperado 16 a 32 caracteres hexadecimais)" => {
            "Invalid MCU bridge ID (expected 16 to 32 hexadecimal characters)"
        }
        "Porta TLS MQTT inválida na URI MCU" => "Invalid MQTT TLS port in MCU URI",
        "Usuário MQTT inválido na URI MCU" => "Invalid MQTT username in MCU URI",
        "Senha MQTT inválida na URI MCU" => "Invalid MQTT password in MCU URI",
        "Caractere Base64url inválido" => "Invalid Base64url character",
        "Comprimento Base64url inválido" => "Invalid Base64url length",
        "Caractere hexadecimal inválido na URI" => "Invalid hexadecimal character in URI",
        "Sequência percent-encoded truncada" => "Truncated percent-encoded sequence",
        "Tempo esgotado tentando conectar ao host" => "Timed out trying to connect to computer",
        "Tempo esgotado ao abrir canal de comunicação" => "Timed out opening communication channel",
        "Tempo esgotado aguardando resposta do host" => "Timed out waiting for computer response",
        "Tempo esgotado aguardando ping do host" => "Timed out waiting for ping from computer",
        "Tempo esgotado aguardando confirmação do código" => {
            "Timed out waiting for code confirmation"
        }
        "Tempo esgotado aguardando permissão de controle remoto no PC" => {
            "Timed out waiting for remote control permission on the computer"
        }
        "Tempo esgotado verificando acesso RDP no computador" => {
            "Timed out checking remote login (RDP) on the computer"
        }
        "Tempo esgotado lendo requisição de túnel" => "Timed out reading tunnel request",
        "Host encerrou conexão antes de responder" => "Computer closed connection before responding",
        "Host encerrou conexão antes de confirmar o código" => {
            "Computer closed connection before confirming code"
        }
        "Host encerrou conexão antes do download" => "Computer closed connection before download",
        "Host encerrou conexão antes do envio" => "Computer closed connection before upload",
        "Host fechou stream de controle sem resposta" => {
            "Computer closed control stream without response"
        }
        "Host fechou stream de ping sem resposta" => "Computer closed ping stream without response",
        "Host não confirmou conclusão do envio" => "Computer did not confirm upload completion",
        "Servidor encerrou canal de entrada antes da confirmação" => {
            "Computer closed input channel before confirmation"
        }
        "Computador encerrou canal RDP antes da resposta" => {
            "Computer closed RDP channel before responding"
        }
        "Endpoint indisponível" => "Endpoint unavailable",
        "Endpoint de controle não encontrado" => "Control endpoint not found",
        "Daemon oraculo-host não está acessível" => "oraculo-host daemon is not reachable",
        "Sessão IPC não autorizada" => "Unauthorized IPC session",
        "Rota não encontrada" => "Route not found",
        "Resposta HTTP inválida do daemon" => "Invalid HTTP response from daemon",
        "JSON inválido retornado pelo daemon" => "Invalid JSON returned by daemon",
        "Timeout lendo corpo da requisição de controle" => "Timed out reading control request body",

        // Tela / Portal / Input
        "Espelhamento de tela desativado pelo host" => "Screen sharing disabled by host",
        "Controle de mouse e teclado desativado pelo host" => {
            "Mouse and keyboard control disabled by host"
        }
        "Captura de tela encerrada inesperadamente" => "Screen capture ended unexpectedly",
        "Host encerrou o streaming de tela" => "Computer closed screen stream",
        "Host encerrou o canal de entrada" => "Computer closed input channel",
        "Sessão de streaming de tela encerrada" => "Screen streaming session ended",
        "Sessão de touchpad encerrada" => "Touchpad session ended",
        "Conexão de entrada encerrada" => "Input connection closed",
        "Apenas 1 monitor ativo detectado neste PC." => {
            "Only 1 active monitor detected on this computer."
        }
        "A janela de seleção de monitores já está aberta no PC." => {
            "The monitor picker window is already open on the computer."
        }
        "A janela de seleção de monitores já está em uso no PC." => {
            "The monitor picker window is already in use on the computer."
        }
        "Seletor aberto no PC: marque TODOS os monitores e clique em Compartilhar." => {
            "Picker opened on the computer: select ALL monitors and click Share."
        }
        "Seleção no PC cancelada; mantendo o monitor atual." => {
            "Selection canceled on the computer; keeping current monitor."
        }
        "Seletor de monitores encerrado sem confirmação." => {
            "Monitor picker closed without confirmation."
        }
        "Nenhum monitor foi selecionado para compartilhamento." => {
            "No monitor was selected for sharing."
        }
        "Nenhum monitor selecionado no diálogo ScreenCast." => {
            "No monitor selected in ScreenCast dialog."
        }
        "Ative captura de tela ou mouse/teclado em Acesso antes de autorizar." => {
            "Enable screen sharing or mouse/keyboard in Access before authorizing."
        }
        "Tela do computador bloqueada. Desbloqueie a sessão antes de autorizar." => {
            "Computer screen is locked. Unlock the session before authorizing."
        }
        "Tela do computador bloqueada. Desbloqueie a sessão para compartilhar a tela." => {
            "Computer screen is locked. Unlock the session to share the screen."
        }
        "Tela do computador bloqueada. Desbloqueie a sessão para usar mouse e teclado remotos." => {
            "Computer screen is locked. Unlock the session to use remote mouse and keyboard."
        }
        "Portal RemoteDesktop indisponível no compositor Wayland" => {
            "RemoteDesktop portal unavailable in Wayland compositor"
        }
        "Portal ScreenCast indisponível" => "ScreenCast portal unavailable",
        "Portal ScreenCast indisponível no compositor Wayland" => {
            "ScreenCast portal unavailable in Wayland compositor"
        }
        "Falha ao criar sessão RemoteDesktop" => "Failed to create RemoteDesktop session",
        "Falha ao criar sessão ScreenCast" => "Failed to create ScreenCast session",
        "Falha ao criar sessão unificada RemoteDesktop/ScreenCast" => {
            "Failed to create unified RemoteDesktop/ScreenCast session"
        }
        "Falha ao selecionar dispositivos no portal RemoteDesktop" => {
            "Failed to select devices in RemoteDesktop portal"
        }
        "Falha ao selecionar fontes no portal ScreenCast" => {
            "Failed to select sources in ScreenCast portal"
        }
        "Falha ao configurar dispositivos no portal RemoteDesktop" => {
            "Failed to configure devices in RemoteDesktop portal"
        }
        "Falha ao configurar seleção de telas na sessão unificada" => {
            "Failed to configure screen selection in unified session"
        }
        "Falha ao abrir FD PipeWire da sessão ScreenCast" => {
            "Failed to open PipeWire FD from ScreenCast session"
        }
        "Falha ao abrir FD PipeWire da sessão unificada" => {
            "Failed to open PipeWire FD from unified session"
        }
        "Fila de vídeo saturada; descartando deltas até o próximo keyframe" => {
            "Video queue saturated; dropping delta frames until next keyframe"
        }

        // Terminal e Comandos
        "Terminal remoto desativado pelo host" => "Remote terminal disabled by host",
        "Sessão de terminal já encerrada" => "Terminal session already ended",
        "Falha ao abrir PTY" => "Failed to open PTY",
        "Falha ao iniciar shell no PTY" => "Failed to start shell in PTY",
        "Execução de comandos rápidos desativada pelo host" => {
            "Quick commands execution disabled by host"
        }
        "Edição de comandos rápidos desativada pelo host" => {
            "Quick commands editing disabled by host"
        }
        "ID do comando inválido" => "Invalid command ID",
        "Aspas não fechadas na linha do comando" => "Unclosed quotes in command line",

        // Arquivos
        "Acesso a arquivos desativado pelo host" => "File access disabled by host",
        "Acesso a arquivos desativado pelo host durante o download" => {
            "File access disabled by host during download"
        }
        "Envio de arquivos desativado pelo host" => "File upload disabled by host",
        "Permissão de upload desativada pelo host durante a transferência" => {
            "Upload permission disabled by host during transfer"
        }
        "Download interrompido antes da conclusão" => "Download interrupted before completion",
        "Conexão encerrada antes de concluir o envio do arquivo" => {
            "Connection closed before file upload completed"
        }
        "Acesso bloqueado: caminho de sistema ou credencial protegido pelo Oráculo" => {
            "Access blocked: system or credential path protected by Oráculo"
        }
        "Acesso bloqueado: o destino aponta para fora da pasta pessoal (~)" => {
            "Access blocked: destination points outside home folder (~)"
        }
        "Upload bloqueado: destino protegido pelo Oráculo" => {
            "Upload blocked: destination protected by Oráculo"
        }
        "Caminho absoluto não permitido; use caminho relativo à pasta pessoal (~)" => {
            "Absolute path not allowed; use a path relative to home folder (~)"
        }
        "Caminho fora da pasta pessoal (~)" => "Path outside home folder (~)",
        "Caminho contém caracteres inválidos" => "Path contains invalid characters",
        "Travessia '..' fora da pasta pessoal não é permitida" => {
            "'..' traversal outside home folder is not allowed"
        }
        "Diretório de destino alterado ou fora da pasta pessoal (~)" => {
            "Destination directory changed or outside home folder (~)"
        }
        "Destino do upload sem diretório pai" => "Upload destination has no parent directory",
        "Destino não pode ser sobrescrito: diretório ou symlink detectado" => {
            "Destination cannot be overwritten: directory or symlink detected"
        }
        "Não é possível sobrescrever um diretório ou symlink com upload" => {
            "Cannot overwrite a directory or symlink with an upload"
        }
        "Nome de arquivo inválido" => "Invalid file name",
        "O caminho informado não é um arquivo regular" => "The specified path is not a regular file",
        "O caminho informado não é uma pasta" => "The specified path is not a folder",
        "Início (~)" => "Home (~)",

        // Túnel RDP e Firewall
        "Túnel de login remoto (RDP) desativado nas permissões deste computador" => {
            "Remote login (RDP) tunnel disabled in this computer's permissions"
        }
        "Autorização de administrador cancelada." => "Administrator authorization canceled.",
        "Autorização de administrador cancelada ou recusada no Windows." => {
            "Administrator authorization canceled or denied on Windows."
        }
        "Bloqueio automático de firewall disponível apenas no Linux e Windows" => {
            "Automatic firewall hardening is only available on Linux and Windows"
        }
        "A regra de bloqueio RDP não foi aplicada no Firewall do Windows." => {
            "The RDP block rule was not applied in Windows Firewall."
        }
        "pkexec não está instalado neste sistema para aplicar regras no firewall" => {
            "pkexec is not installed on this system to apply firewall rules"
        }
        "Falha ao invocar pkexec para configurar firewalld" => {
            "Failed to invoke pkexec to configure firewalld"
        }
        "Falha ao solicitar elevação UAC para o Windows Firewall" => {
            "Failed to request UAC elevation for Windows Firewall"
        }

        // Wake-on-LAN / Pontes / MCU MQTT
        "Ponte Wake-on-LAN desativada neste host" => "Wake-on-LAN bridge disabled on this host",
        "Dados de Wake-on-LAN inválidos para retransmissão" => {
            "Invalid Wake-on-LAN data for relay"
        }
        "Nenhum endereço MAC configurado para Wake-on-LAN" => {
            "No MAC address configured for Wake-on-LAN"
        }
        "Nenhum endereço MAC configurado para Wake-on-LAN neste computador" => {
            "No MAC address configured for Wake-on-LAN on this computer"
        }
        "Nenhum endereço MAC válido para enviar à ponte MCU" => {
            "No valid MAC address to send to MCU bridge"
        }
        "Nenhum endereço de destino configurado para Wake-on-LAN" => {
            "No destination address configured for Wake-on-LAN"
        }
        "Endereço de destino Wake-on-LAN vazio" => "Empty Wake-on-LAN destination address",
        "Endereço MAC WoL inválido (use formato AA:BB:CC:DD:EE:FF)" => {
            "Invalid WoL MAC address (use format AA:BB:CC:DD:EE:FF)"
        }
        "Não foi possível transmitir nenhum pacote Wake-on-LAN" => {
            "Could not transmit any Wake-on-LAN packet"
        }
        "Este aparelho atua como Ponte Wake-on-LAN e não pode ser ligado por si mesmo" => {
            "This device acts as a Wake-on-LAN Bridge and cannot wake itself"
        }
        "Este dispositivo atua apenas como Ponte Wake-on-LAN" => {
            "This device acts only as a Wake-on-LAN Bridge"
        }
        "Este serviço atua apenas como Ponte Wake-on-LAN" => {
            "This service acts only as a Wake-on-LAN Bridge"
        }
        "Ponte MCU salva não encontrada" => "Saved MCU bridge not found",
        "Pacote WoL inválido recebido" => "Invalid WoL packet received",
        "Pacote WoL inválido recebido pela ponte" => "Invalid WoL packet received by bridge",
        "Falha ao abrir socket UDP IPv4 para Wake-on-LAN" => {
            "Failed to open IPv4 UDP socket for Wake-on-LAN"
        }
        "Endereço do broker MQTT inválido" => "Invalid MQTT broker address",
        "Hostname do broker MQTT contém caracteres inválidos" => {
            "MQTT broker hostname contains invalid characters"
        }
        "Broker MQTT apontando para localhost é bloqueado por segurança" => {
            "MQTT broker pointing to localhost is blocked for security"
        }
        "Broker MQTT encerrou a conexão" => "MQTT broker closed the connection",
        "Broker MQTT negou inscrição no tópico da ponte MCU" => {
            "MQTT broker denied subscription to MCU bridge topic"
        }
        "SUBACK inválido do broker MQTT" => "Invalid SUBACK from MQTT broker",
        "A ponte MCU recusou ou não conseguiu enviar o Magic Packet" => {
            "The MCU bridge rejected or failed to send the Magic Packet"
        }
        "Tempo esgotado tentando alcançar a ponte MCU" => {
            "Timed out trying to reach the MCU bridge"
        }
        "Tempo esgotado aguardando resposta da ponte MCU no broker MQTT" => {
            "Timed out waiting for MCU bridge response on MQTT broker"
        }
        "Tempo esgotado aguardando confirmação (ACK) da ponte MCU" => {
            "Timed out waiting for MCU bridge acknowledgment (ACK)"
        }
        "Falha na verificação TLS do broker MQTT" => "MQTT broker TLS verification failed",
        "Nome de servidor TLS inválido" => "Invalid TLS server name",
        "Pacote MQTT excede limite da especificação" => {
            "MQTT packet exceeds specification limit"
        }
        "Pacote MQTT excede limite máximo de segurança" => {
            "MQTT packet exceeds maximum safety limit"
        }
        "Comprimento variável MQTT malformado" => "Malformed MQTT variable length",
        "Conexão MQTT encerrada durante leitura do cabeçalho" => {
            "MQTT connection closed while reading header"
        }
        "Conexão MQTT encerrada ao ler tamanho do pacote" => {
            "MQTT connection closed while reading packet size"
        }
        "Conexão MQTT encerrada ao drenar pacote excedente" => {
            "MQTT connection closed while draining oversized packet"
        }
        "Conexão MQTT encerrada antes do fim do payload" => {
            "MQTT connection closed before end of payload"
        }
        "Inicialização automática não implementada nesta plataforma" => {
            "Autostart is not implemented on this platform"
        }
        "Falha ao registrar inicialização automática no Windows" => {
            "Failed to register autostart on Windows"
        }
        "Caminho do executável indisponível" => "Executable path unavailable",
        "Caminho do executável oraculo-desktop indisponível" => {
            "oraculo-desktop executable path unavailable"
        }
        "Diretório ~/.config indisponível" => "~/.config directory unavailable",
        "Não foi possível determinar a pasta pessoal (~)" => {
            "Could not determine home directory (~)"
        }
        "Não foi possível determinar diretório de configuração do sistema" => {
            "Could not determine system configuration directory"
        }
        "Não foi possível determinar diretório de dados do oraculo-bridge" => {
            "Could not determine oraculo-bridge data directory"
        }
        _ => return None,
    })
}

fn translate_template(msg: &str) -> Option<String> {
    // 1. Mensagens de portal parametrizadas por ação
    if let Some(rest) = msg
        .strip_prefix("Tela do computador bloqueada. Desbloqueie a sessão para liberar ")
        .and_then(|s| s.strip_suffix('.'))
    {
        return Some(format!(
            "Computer screen is locked. Unlock the session to grant {}.",
            translate_action_phrase(rest)
        ));
    }
    if let Some(rest) = msg
        .strip_prefix("Autorização (")
        .and_then(|s| s.strip_suffix(") cancelada no computador."))
    {
        return Some(format!(
            "Authorization ({}) canceled on the computer.",
            translate_action_phrase(rest)
        ));
    }
    if let Some(rest) = msg
        .strip_prefix("Tempo esgotado aguardando ")
        .and_then(|s| s.strip_suffix(" no computador."))
    {
        return Some(format!(
            "Timed out waiting for {} on the computer.",
            translate_action_phrase(rest)
        ));
    }

    // 2. Mensagens de monitores / ScreenCast
    if let Some(rest) = msg
        .strip_prefix("O GNOME só liberou 1 dos ")
        .and_then(|s| {
            s.strip_suffix(
                " monitores. Reautorize em Acesso no PC e marque todos os monitores.",
            )
        })
    {
        return Some(format!(
            "GNOME only shared 1 of {rest} monitors. Reauthorize in Access on the computer and select all monitors."
        ));
    }

    // 3. RDP / Túnel
    if let Some(rest) = msg
        .strip_prefix("Nenhum servidor RDP ativo em 127.0.0.1:")
        .and_then(|s| {
            s.strip_suffix(
                ". Ative o Login Remoto nas configurações do sistema para usar o login remoto.",
            )
            .or_else(|| {
                s.strip_suffix(
                    ". Ative o Acesso Remoto nas configurações do sistema para usar o login remoto.",
                )
            })
        })
    {
        return Some(format!(
            "No active RDP server on 127.0.0.1:{rest}. Turn on Remote Login in system settings to use remote login."
        ));
    }
    if let Some(port) = msg.strip_prefix("Servidor RDP acessível na porta ") {
        return Some(format!("RDP server reachable on port {port}"));
    }
    if let Some(port) = msg
        .strip_prefix("Túnel RDP ativo (127.0.0.1:")
        .and_then(|s| s.strip_suffix(')'))
    {
        return Some(format!("RDP tunnel active (127.0.0.1:{port})"));
    }
    if let Some(rest) = msg.strip_prefix("Porta solicitada (") {
        if let Some((req, allowed)) = rest
            .split_once(") diferente da porta RDP configurada (")
            .and_then(|(a, b)| b.strip_suffix(')').map(|b2| (a, b2)))
        {
            return Some(format!(
                "Requested port ({req}) differs from configured RDP port ({allowed})"
            ));
        }
    }

    // 4. Terminal / Sessões / Comandos / Arquivos
    if let Some(rest) = msg.strip_prefix("Limite de ").and_then(|s| {
        s.strip_suffix(
            " sessões persistentes ativas atingido. Encerre uma sessão antes de abrir outra.",
        )
    }) {
        return Some(format!(
            "Limit of {rest} active persistent sessions reached. Close a session before opening another."
        ));
    }
    if let Some(id) = msg
        .strip_prefix("Sessão '")
        .and_then(|s| s.strip_suffix("' não encontrada"))
    {
        return Some(format!("Session '{id}' not found"));
    }
    if let Some(id) = msg
        .strip_prefix("Sessão persistente '")
        .and_then(|s| s.strip_suffix("' não encontrada ou já encerrada"))
    {
        return Some(format!(
            "Persistent session '{id}' not found or already closed"
        ));
    }
    if let Some(id) = msg
        .strip_prefix("Comando '")
        .and_then(|s| s.strip_suffix("' não encontrado"))
    {
        return Some(format!("Command '{id}' not found"));
    }
    if let Some(rel) = msg
        .strip_prefix("Caminho não encontrado ou inacessível: '")
        .and_then(|s| s.strip_suffix('\''))
    {
        return Some(format!("Path not found or inaccessible: '{rel}'"));
    }
    if let Some(name) = msg
        .strip_prefix("Nome de arquivo inválido: '")
        .and_then(|s| s.strip_suffix('\''))
    {
        return Some(format!("Invalid file name: '{name}'"));
    }
    if let Some(mib) = msg
        .strip_prefix("Arquivo excede o limite máximo de envio (")
        .and_then(|s| s.strip_suffix(" MiB)"))
    {
        return Some(format!("File exceeds maximum upload limit ({mib} MiB)"));
    }
    if let Some(secs) = msg
        .strip_prefix("Tempo limite de ")
        .and_then(|s| s.strip_suffix("s excedido"))
    {
        return Some(format!("Timed out after {secs}s"));
    }
    if let Some(path) = msg.strip_prefix("Sem permissão para listar ") {
        return Some(format!("No permission to list {path}"));
    }

    // 5. MQTT / WoL / Pareamento
    if let Some(code) = msg
        .strip_prefix("Broker MQTT recusou conexão (código CONNACK ")
        .and_then(|s| s.strip_suffix(')'))
    {
        return Some(format!(
            "MQTT broker rejected connection (CONNACK code {code})"
        ));
    }
    if let Some(ip) = msg
        .strip_prefix("IP do broker MQTT bloqueado por segurança (")
        .and_then(|s| s.strip_suffix(')'))
    {
        return Some(format!("MQTT broker IP blocked for security ({ip})"));
    }
    if let Some(ip) = msg
        .strip_prefix("Endereço IP de broker MQTT bloqueado por segurança (")
        .and_then(|s| s.strip_suffix(')'))
    {
        return Some(format!("MQTT broker IP address blocked for security ({ip})"));
    }
    if let Some(host) = msg
        .strip_prefix("DNS do broker '")
        .and_then(|s| s.strip_suffix("' não retornou nenhum IP seguro/roteável"))
    {
        return Some(format!(
            "Broker DNS '{host}' returned no safe routable IP"
        ));
    }
    if let Some(host) = msg
        .strip_prefix("Não foi possível resolver DNS do broker '")
        .and_then(|s| s.strip_suffix('\''))
    {
        return Some(format!("Could not resolve DNS for broker '{host}'"));
    }
    if let Some(mac) = msg
        .strip_prefix("Endereço MAC WoL reservado/inválido (")
        .and_then(|s| s.strip_suffix(')'))
    {
        return Some(format!("Reserved or invalid WoL MAC address ({mac})"));
    }
    if let Some(ch) = msg
        .strip_prefix("Caractere '")
        .and_then(|s| s.strip_suffix("' inválido no código de pareamento (não usamos 0, O, 1 ou I)"))
    {
        return Some(format!(
            "Invalid character '{ch}' in pairing code (0, O, 1, and I are not used)"
        ));
    }
    if let Some(rest) = msg
        .strip_prefix("Protocolo incompatível ou frame inválido (")
        .and_then(|s| s.strip_suffix("). Se atualizou o app, reinicie o oraculo-host no PC."))
    {
        return Some(format!(
            "Incompatible protocol or invalid frame ({rest}). If you updated the app, restart Oráculo on your computer."
        ));
    }
    if let Some(status) = msg
        .strip_prefix("Erro do daemon (")
        .and_then(|s| s.strip_suffix(')'))
    {
        return Some(format!("Daemon error ({status})"));
    }

    // 6. Prefixos compostos "Prefixo: detalhe"
    let prefixes: &[(&str, &str)] = &[
        ("Não foi possível conectar ao host: ", "Could not connect to computer: "),
        ("Falha na conexão de pareamento: ", "Pairing connection failed: "),
        ("Falha ao conectar à ponte: ", "Failed to connect to bridge: "),
        ("Falha ao disparar Magic Packet: ", "Failed to send Magic Packet: "),
        ("Falha ao disparar WoL pela ponte: ", "Failed to trigger Wake-on-LAN via bridge: "),
        ("Falha ao retransmitir Wake-on-LAN na LAN local: ", "Failed to relay Wake-on-LAN on local LAN: "),
        ("Falha ao enviar pacote Wake-on-LAN na rede: ", "Failed to send Wake-on-LAN packet on network: "),
        ("Falha ao enviar entrada: ", "Failed to send input: "),
        ("Falha no canal de entrada: ", "Input channel error: "),
        ("Falha ao listar pasta: ", "Failed to list folder: "),
        ("Falha ao abrir arquivo para envio: ", "Failed to open file for upload: "),
        ("Falha ao criar arquivo local: ", "Failed to create local file: "),
        ("Falha ao preparar pasta local: ", "Failed to prepare local folder: "),
        ("Falha ao abrir stream de pareamento: ", "Failed to open pairing stream: "),
        ("Falha ao abrir canal de código: ", "Failed to open code channel: "),
        ("Falha ao abrir porta local de túnel: ", "Failed to open local tunnel port: "),
        ("Falha ao abrir sala de pareamento da ponte: ", "Failed to open bridge pairing room: "),
        ("Falha ao iniciar Iroh Endpoint: ", "Failed to start network endpoint: "),
        ("Falha ao iniciar runtime async: ", "Failed to start async runtime: "),
        ("Falha ao aplicar nova seleção de monitores: ", "Failed to apply new monitor selection: "),
        ("Não foi possível aplicar a troca de monitores: ", "Could not apply monitor switch: "),
        ("Não foi possível atualizar o firewall: ", "Could not update firewall: "),
        ("Protocolo de terminal incompatível ou corrompido (reinicie/atualize o Oráculo no PC): ", "Incompatible or corrupted terminal protocol (restart/update Oráculo on the computer): "),
        ("Stream de vídeo encerrado: ", "Video stream ended: "),
        ("Erro no terminal: ", "Terminal error: "),
        ("Erro ao ler comandos: ", "Failed to read commands: "),
        ("Erro ao abrir stream: ", "Failed to open stream: "),
        ("Erro ao abrir canal na ponte: ", "Failed to open bridge channel: "),
        ("Erro ao abrir stream de ping: ", "Failed to open ping stream: "),
        ("Erro ao gravar download: ", "Failed to write download: "),
        ("Erro ao salvar download: ", "Failed to save download: "),
        ("Erro ao ler arquivo local: ", "Failed to read local file: "),
        ("Conexão perdida com o computador: ", "Connection lost with computer: "),
        ("Conexão encerrada: ", "Connection closed: "),
        ("EndpointId inválido: ", "Invalid EndpointId: "),
        ("EndpointId não encontrado: ", "EndpointId not found: "),
        ("Broadcast IPv4 inválido: ", "Invalid IPv4 broadcast: "),
        ("Requisição de túnel inválida: ", "Invalid tunnel request: "),
        ("URL de relay inválida: ", "Invalid relay URL: "),
        ("JSON inválido: ", "Invalid JSON: "),
    ];
    for (pt_prefix, en_prefix) in prefixes {
        if let Some(rest) = msg.strip_prefix(pt_prefix) {
            let translated_rest = translate_pt_to_en(rest);
            return Some(format!("{en_prefix}{translated_rest}"));
        }
    }

    None
}

fn translate_pt_to_en(msg: &str) -> String {
    let trimmed = msg.trim();
    if let Some(exact) = translate_exact(trimmed) {
        return exact.to_string();
    }
    if let Some(templated) = translate_template(trimmed) {
        return templated;
    }
    // Cadeias anyhow no formato "contexto: causa" onde o contexto exato tem tradução conhecida.
    if let Some((head, tail)) = trimmed.split_once(": ") {
        if let Some(en_head) = translate_exact(head) {
            let en_tail = translate_pt_to_en(tail);
            return format!("{en_head}: {en_tail}");
        }
    }
    msg.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localizes_static_templated_and_chained_messages() {
        assert_eq!(
            localize_text("Computador indisponível ou Oráculo fechado no PC", UiLang::Pt),
            "Computador indisponível ou Oráculo fechado no PC"
        );
        assert_eq!(
            localize_text("Computador indisponível ou Oráculo fechado no PC", UiLang::En),
            "Computer unavailable or Oráculo is closed on the PC"
        );
        assert_eq!(
            localize_text(
                "Tela do computador bloqueada. Desbloqueie a sessão para liberar permissão de captura de tela.",
                UiLang::En
            ),
            "Computer screen is locked. Unlock the session to grant screen capture permission."
        );
        assert_eq!(
            localize_text(
                "O GNOME só liberou 1 dos 2 monitores. Reautorize em Acesso no PC e marque todos os monitores.",
                UiLang::En
            ),
            "GNOME only shared 1 of 2 monitors. Reauthorize in Access on the computer and select all monitors."
        );
        assert_eq!(
            localize_text(
                "Não foi possível conectar ao host: Tempo esgotado tentando conectar ao host",
                UiLang::En
            ),
            "Could not connect to computer: Timed out trying to connect to computer"
        );
        assert_eq!(
            localize_text(
                "Nenhum servidor RDP ativo em 127.0.0.1:3389. Ative o Login Remoto nas configurações do sistema para usar o login remoto.",
                UiLang::En
            ),
            "No active RDP server on 127.0.0.1:3389. Turn on Remote Login in system settings to use remote login."
        );
    }

    #[test]
    fn detects_permanent_remote_errors_in_both_languages() {
        assert!(is_permanent_remote_error(
            "Tela do computador bloqueada. Desbloqueie a sessão."
        ));
        assert!(is_permanent_remote_error(
            "Computer screen is locked. Unlock the session."
        ));
        assert!(is_permanent_remote_error(
            "No monitor was selected for sharing."
        ));
        assert!(is_permanent_remote_error("Screen sharing disabled by host"));
        assert!(!is_permanent_remote_error("Timed out trying to connect to computer"));
    }
}
