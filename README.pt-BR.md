# Oráculo Desk

Idioma: [English](README.md) | **Português (Brasil)**

O Oráculo Desk é uma suíte peer-to-peer auto-hospedada de área de trabalho remota e administração de sistemas escrita em Rust e Kotlin. Ele permite controlar um computador Linux (Wayland ou X11) ou Windows a partir de um celular Android ou de outro PC por conexões QUIC diretas (`iroh`), atravessando NATs e firewalls sem redirecionamento de portas no roteador, sem DNS dinâmico e sem contas em servidores de terceiros.

## Visão geral

Grande parte das ferramentas de acesso remoto apresenta limitações em sessões Linux Wayland, exige cadastro em servidores centrais ou reduz toda a interação no celular a uma tela difícil de enxergar. O Oráculo Desk separa as tarefas do dia a dia em módulos dedicados de baixo consumo que operam sobre uma única conexão P2P criptografada:

- Área de trabalho remota H.264 de baixa latência com codificação acelerada por hardware (NVIDIA NVENC, VA-API, OpenH264 ou x264), modo de toque direto, modo trackpad relativo, rolagem com dois dedos, zoom por pinça de até 4x, troca de monitor em tempo real pelo celular, modo tela cheia horizontal para uso com periféricos e barra de teclas especiais (`Ctrl`, `Alt`, `Super`, `Esc`, `Tab`, setas, `F1`-`F12`).
- Suporte nativo a Wayland e tela de login via `xdg-desktop-portal` (`ScreenCast` e `RemoteDesktop`) com token de restauração persistente para não pedir permissão a cada reinicialização. Quando não há sessão gráfica ativa (como na tela de login ou em um TTY físico), pode recorrer à captura direta de framebuffer DRM/KMS (`/dev/dri/card*`) e dispositivos virtuais `/dev/uinput`.
- Terminal PTY interativo com acesso completo ao shell (`bash`, `zsh`, `fish` ou PowerShell) processado nativamente em Rust por uma máquina de estados `vt100` com suporte a cores ANSI/256/truecolor e histórico de rolagem.
- Comandos rápidos com um toque para executar scripts e comandos pré-autorizados (`commands.toml`) ou criar novos atalhos pelo aplicativo, com timeout de segurança e limite de memória por saída.
- Gerenciador de arquivos em sandbox restrito à pasta pessoal (`~`) do host, com prévia de imagens e textos no próprio app, download para a pasta `Downloads` do Android e upload de arquivos com numeração automática de conflitos e proteção contra escape por symlinks.
- Telemetria em tempo real e controle de energia com leitura ao vivo de CPU, memória RAM, swap, uptime, tráfego de rede por interface e processos mais pesados, além de bloqueio de tela, suspensão, reinicialização, desligamento e pacote mágico Wake-on-LAN (WoL) na rede local.

## Plataformas suportadas

| Componente | Plataformas |
| :--- | :--- |
| Daemon do host (`oraculo-host`) | Linux (Wayland via PipeWire/`xdg-desktop-portal`, DRM/KMS ou X11) e Windows |
| Cliente mobile (`Oráculo`) | Android 8.0+ (API 26+, `arm64-v8a`), decodificação H.264 por hardware via `MediaCodec` |
| Cliente de linha de comando (`oraculo-cli`) | Linux e Windows (para shell remoto entre computadores, ping, telemetria e WoL) |

## Guia rápido

### 1. Instalar dependências de vídeo no host Linux

Para habilitar a captura via PipeWire e a codificação H.264 por hardware no Linux, instale os plugins do GStreamer da sua distribuição:

**Debian / Ubuntu:**
```bash
sudo apt update
sudo apt install gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-pipewire
```

**Fedora:**
```bash
sudo dnf install gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good \
  gstreamer1-plugins-bad-free gstreamer1-plugin-openh264 pipewire-gstreamer
```

**Arch Linux:**
```bash
sudo pacman -S gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad \
  gst-plugin-pipewire openh264
```

### 2. Iniciar o serviço no host

Após compilar ou baixar os binários `oraculo-host` e `oraculo-cli`, instale-os em `~/.local/bin` e ative o serviço de usuário do systemd:

```bash
install -Dm755 ./dist/oraculo-host ~/.local/bin/oraculo-host
install -Dm755 ./dist/oraculo-cli ~/.local/bin/oraculo-cli
oraculo-host install-service
```

### 3. Parear o celular Android ou outro computador

Gere um QR code de pareamento de uso único no terminal do host:

```bash
oraculo-host pair
```

Abra o aplicativo Oráculo no Android, toque em **Novo pareamento** e escaneie o QR code exibido no terminal (ou cole a URI `oraculo://pair?...` manualmente). O código vale por 5 minutos e é invalidado após o primeiro uso ou após 5 tentativas incorretas.

### 4. Opcional: entrada virtual `/dev/uinput` e captura na tela de bloqueio no Linux

Dentro de uma sessão Wayland comum, a transmissão de tela e o controle de mouse/teclado funcionam via `xdg-desktop-portal` sem privilégios administrativos. Para habilitar também o dispositivo virtual `/dev/uinput` e a captura direta via DRM/KMS antes do login, adicione seu usuário ao grupo `input` e aplique `cap_sys_admin` ao binário:

```bash
sudo usermod -aG input "$USER"
sudo setcap cap_sys_admin+ep ~/.local/bin/oraculo-host
```

## Referência do CLI do host

```text
oraculo-host serve                          Executa o daemon em primeiro plano
oraculo-host pair [--ttl-secs 300]          Gera um QR code e URI de pareamento de uso único
oraculo-host id                             Exibe o EndpointId público do host
oraculo-host devices                        Lista todos os dispositivos pareados
oraculo-host revoke <ENDPOINT_ID>           Revoga o acesso de um dispositivo imediatamente
oraculo-host permissions                    Exibe as permissões atuais dos subsistemas (settings.toml)
oraculo-host set-permission <PERM> <on|off> Altera terminal, files, input, screen ou commands-edit
oraculo-host wol [--mac AA:BB:CC:DD:EE:FF]  Consulta ou define o MAC usado para Wake-on-LAN
oraculo-host commands                       Lista os comandos rápidos cadastrados (commands.toml)
oraculo-host install-service                Instala e inicia ~/.config/systemd/user/oraculo-host.service
oraculo-host uninstall-service              Para e remove o serviço systemd do usuário
oraculo-host service-status                 Exibe o status do serviço e das capacidades DRM/uinput
```

## Segurança

Todas as conexões utilizam QUIC com TLS 1.3 e autenticação mútua por chaves Ed25519 (`iroh`, ALPN `oraculo/p2p/1`). As chaves privadas (`host.key`, `client.key`), a lista de dispositivos confiáveis (`trusted_peers.json`) e o token de restauração do portal são criados com permissão `0600` em sistemas Unix.

Conexões de dispositivos não pareados que tentem abrir streams funcionais são encerradas imediatamente (`401 unpaired`). Revogar um dispositivo (`oraculo-host revoke <ID>`) ou desativar um subsistema (`oraculo-host set-permission <subsistema> off`) interrompe as sessões ativas em tempo real sem reiniciar o serviço.

O gerenciador de arquivos opera restrito ao diretório pessoal canonizado (`~`) do usuário. A resolução de caminhos bloqueia navegação `..`, symlinks externos e acesso a arquivos sensíveis dentro de `~`, incluindo `.config/oraculo`, `.local/share/oraculo`, `.ssh`, `.gnupg`, `.local/share/keyrings`, `.config/autostart` e arquivos de inicialização de shell (`.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.zprofile`).

## Arquitetura do projeto

O repositório é composto por um workspace Rust integrado ao aplicativo Android nativo:

| Componente | Caminho | Descrição |
| :--- | :--- | :--- |
| `oraculo-proto` | `crates/oraculo-proto` | Mensagens do protocolo binário, framing `postcard` prefixado por tamanho de 4 bytes (máx. 4 MiB por frame), cabeçalhos de multiplexação `StreamKind` e parser de URIs `oraculo://pair`. |
| `oraculo-host` | `crates/oraculo-host` | Daemon desktop (`server.rs`), captura de tela PipeWire/GStreamer/DRM (`screen.rs`), controle de entrada Portal/`uinput`/`enigo` (`input.rs`), terminal `portable-pty` (`pty.rs`) e sandbox de arquivos (`files.rs`). |
| `oraculo-cli` | `crates/oraculo-cli` | Cliente de linha de comando para pareamento, medição de RTT, leitura de telemetria, shell remoto e execução de comandos. |
| `oraculo-mobile-core` | `crates/oraculo-mobile-core` | Biblioteca Rust compilada via `cargo-ndk` e exposta ao Kotlin via `uniffi`. Implementa o pool de conexões QUIC, emulador `vt100`, fila de quadros H.264 Annex-B e envio de Wake-on-LAN. |
| `android` | `android/` | Aplicativo Android em Jetpack Compose (Material 3) com `SurfaceView` + decodificação H.264 via `MediaCodec` e serviço de notificação de sessão em primeiro plano (`OraculoSessionService`). |

## Compilando a partir do código-fonte

O projeto inclui scripts Podman que executam o compilador Rust, cabeçalhos do GStreamer, Android SDK e Android NDK em containers reproduzíveis, armazenando caches de compilação em volumes dedicados.

```bash
# Executar todos os testes unitários do workspace
./scripts/build.sh test

# Compilar os binários desktop otimizados em ./dist/oraculo-host e ./dist/oraculo-cli
./scripts/build.sh release

# Compilar liboraculo_mobile_core.so (arm64-v8a), gerar bindings Kotlin UniFFI e montar ./dist/oraculo-debug.apk
./scripts/build-android.sh debug
```

Para compilar os binários desktop diretamente no sistema sem Podman, execute `cargo build --release --workspace` com Rust 1.85+ (`edition = "2024"`).

## Licença

Distribuído sob a licença [Apache License, Version 2.0](licenses/LICENSE) ([Avisos de Terceiros](licenses/NOTICE)).
