# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![Licença](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](licenses/LICENSE)
[![Rust](https://img.shields.io/badge/rust-edition%202024-orange.svg)](Cargo.toml)
[![Android](https://img.shields.io/badge/android-API%2026%2B%20(arm64)-3ddc84.svg)](android/app/build.gradle.kts)

*Read this in [English](README.md).*

**Oráculo** é uma suíte peer-to-peer de acesso remoto com desktop, terminal interativo e navegador de arquivos confinado ao diretório pessoal, escrita em **Rust** e **Kotlin** (Jetpack Compose). Toda comunicação trafega diretamente por streams QUIC criptografados via [iroh](https://github.com/n0-computer/iroh) (`ALPN: oraculo/1`), usando identidades `Ed25519` sem necessidade de abertura de portas no roteador ou contas centrais.

---

## Principais Recursos

- **Transporte P2P Zero-Config (`iroh`):** Travessia de NAT, streams QUIC criptografados e verificação criptográfica `Ed25519` contra a lista local de confiança (`trusted_peers.json`).
- **Transmissão de Tela Wayland com Baixa Latência (Linux):** Captura via `xdg-desktop-portal` (`ScreenCast`) + PipeWire e codificação H.264 Annex-B via GStreamer (`nvh264enc` com fallback automático em CPU para `openh264enc`) com `videoscale method=lanczos`, decodificada em hardware (`MediaCodec`) no Android.
- **Entrada Remota Wayland (Linux):** Movimento do ponteiro, cliques, rolagem, keysyms X11, fallback Unicode e detecção de layout de teclado do GNOME via `xdg-desktop-portal` (`RemoteDesktop`).
- **Terminal PTY Interativo:** Sessões de shell multiplataforma com `portable-pty` no host e emulação `vt100` no aplicativo Android.
- **Comandos Rápidos:** Executa tarefas pré-configuradas em `commands.toml` (timeout de `30s` e limite de `256 KiB` por stream) e permite criar/remover comandos remotamente quando `commands_edit` está habilitado.
- **Arquivos Confinados à Home:** Navegação, visualização, download e upload de até `2 GiB`, restritos ao diretório pessoal do usuário (`~`) com proteção contra symlinks e traversal (`..`).
- **App Android Bilíngue (`arm64-v8a`):** Interface Material 3 com troca instantânea **PT / EN**, seletor de tema (Escuro/Claro/Sistema), 4 abas de controle (**Touchpad**, **Terminal**, **Comandos**, **Arquivos**), presets de qualidade (`1080p30`, `1080p60`, `720p30`), pinch-to-zoom, medidor de latência RTT e notificação persistente de sessão ativa.

---

## Suporte de Plataformas

| Componente | Alvo | Estado Atual |
| :--- | :--- | :--- |
| **Controle, Terminal PTY, Comandos, Arquivos** | Linux, macOS, Windows | Core Rust multiplataforma |
| **Captura de Tela & Entrada Remota (Host)** | Linux (Wayland + PipeWire + GStreamer) | Suportado via `xdg-desktop-portal` |
| **Captura de Tela & Entrada Remota (Host)** | Windows / outros sistemas | Stub (`backend Windows DXGI planejado`) |
| **Aplicativo Mobile (`Oráculo`)** | Android 8.0+ (API 26+, `arm64-v8a`) | Suportado |
| **CLI Cliente (`oraculo-cli`)** | Linux / macOS / Windows | Suportado |

> **Requisito de execução no host Linux:** A transmissão de tela invoca `gst-launch-1.0` com `gdppay`, plugins PipeWire do GStreamer e um encoder H.264 (`nvh264enc` ou `openh264enc`), além de `xdg-desktop-portal` e PipeWire ativos na sessão Wayland.

---

## Arquitetura do Workspace

| Componente | Caminho | Responsabilidade |
| :--- | :--- | :--- |
| **Protocolo Wire** | [`crates/oraculo-proto`](crates/oraculo-proto) | Framing `postcard` (frame máximo `4 MiB`, blocos de arquivo `128 KiB`) e streams (`Pair=1`, `Control=2`, `Terminal=3`, `Input=4`, `Files=5`, `Screen=6`). |
| **Daemon Host** | [`crates/oraculo-host`](crates/oraculo-host) | Servidor QUIC, pareamento QR de uso único (validade `3 min`), autorização de pares, portal tela/entrada, PTY, comandos e sandbox de arquivos. |
| **Core Mobile (FFI)** | [`crates/oraculo-mobile-core`](crates/oraculo-mobile-core) | Pool de conexões QUIC (`OraculoMobileClient`), emulador `vt100`, fila H.264, transferência de arquivos e bindings UniFFI. |
| **CLI Cliente** | [`crates/oraculo-cli`](crates/oraculo-cli) | Cliente de terminal para parear, medir ping, listar/executar comandos rápidos e abrir shell remoto. |
| **App Android** | [`android/`](android/) | Cliente Kotlin + Jetpack Compose com leitor QR, player `MediaCodec`/`SurfaceView`, touchpad, terminal, comandos e arquivos. |

---

## Início Rápido

### 1. Inicie o Host no Computador

Execute o daemon exibindo o QR Code de pareamento (válido por **3 minutos**):

```bash
oraculo-host serve --pair
```

Com o `oraculo-host` aberto em modo interativo, pressione `p` + `Enter` a qualquer momento para gerar um novo QR Code.

### 2. Pareie um Dispositivo

- **App Android:** Toque em **Parear novo computador** e aponte a câmera para o QR Code, ou use **Colar link manualmente** com a URI `oraculo://pair?...`.
- **CLI Desktop (`oraculo-cli`):**
  ```bash
  oraculo-cli pair "<COLE_A_URI_DE_PAREAMENTO>" --name "meu-notebook"
  oraculo-cli ping
  oraculo-cli shell
  ```

### 3. Gerenciamento e Permissões

```bash
# Listar dispositivos autorizados
oraculo-host peers

# Revogar acesso de um dispositivo (encerra streams ativos imediatamente)
oraculo-host revoke <ENDPOINT_ID>

# Consultar ou alterar permissões e Wake-on-LAN no settings.toml
oraculo-host permissions --screen true --input true --terminal true
oraculo-host permissions --files-read true --files-write false --commands-edit false

# Exibir EndpointId do host e caminhos de configuração/dados
oraculo-host info

# Autorizar monitores via seletor do portal para alternância multi-monitor
oraculo-host monitors
```

Todos os subcomandos do `oraculo-cli` (`pair`, `ping`, `commands`, `run`, `shell`) aceitam `--host <ENDPOINT_ID>`.

---

## Modelo de Segurança

- **Identidade Criptográfica Mútua:** Autenticação via chaves `Ed25519` do `iroh`; pares desconhecidos só podem abrir `StreamKind::Pair`.
- **Pareamento Protegido:** Tokens expiram em **180 segundos**, são de uso único, invalidam após **5 tentativas incorretas** e usam comparação em tempo constante.
- **Revogação e Bloqueio Instantâneos:** `oraculo-host revoke` ou desabilitar um subsistema via `oraculo-host permissions` derruba os streams ativos imediatamente.
- **Sandbox no Diretório Pessoal:** Toda operação de arquivo resolve o caminho canônico dentro de `~`, bloqueia `..` e escapes por symlink, e nega acesso a `.config/oraculo`, `.local/share/oraculo`, `appdata/roaming/oraculo`, `appdata/local/oraculo`, `.ssh`, `.gnupg`, `.local/share/keyrings`, `.config/autostart` e scripts de shell (`.bashrc`, `.bash_profile`, `.profile`, `.zshrc`, `.zprofile`).
- **Chaves Privadas:** `host.key`, `trusted_peers.json`, `remote_desktop_token`, `screencast_token` e `client.key` são gravados atomicamente com permissão Unix `0600`.

Consulte [`.github/SECURITY.md`](.github/SECURITY.md) para informações sobre reporte de vulnerabilidades.

---

## Compilação a partir do Código-Fonte

As compilações utilizam contêineres Podman (`Containerfile` e `Containerfile.android`) com volumes em cache para Rust (`edition = "2024"`), Android SDK 35 (`minSdk 26`), NDK e bindings UniFFI:

```bash
# Verificar compilação de todo o workspace Rust
./scripts/build.sh check

# Executar todos os testes unitários (oraculo-proto, oraculo-host, oraculo-mobile-core)
./scripts/build.sh test

# Compilar binários desktop otimizados em ./dist/oraculo-host e ./dist/oraculo-cli
./scripts/build.sh release

# Compilar liboraculo_mobile_core.so (arm64-v8a), gerar bindings UniFFI e gerar ./dist/oraculo-debug.apk
./scripts/build-android.sh debug
```

---

## Licença

Licenciado sob a **Apache License, Version 2.0**. Veja [`licenses/LICENSE`](licenses/LICENSE) e [`licenses/NOTICE`](licenses/NOTICE) para o texto completo e atribuições de terceiros.
