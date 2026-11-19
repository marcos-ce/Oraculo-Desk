# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![Licença](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](licenses/LICENSE)

*Read this in [English](README.md).*

Acesso remoto do celular Android ao desktop Linux via QUIC (Iroh) com criptografia ponta a ponta. Sem contas e sem precisar abrir portas no roteador.

## O que faz

- Transmite a tela do computador para o celular (H.264, decodificação por hardware) com troca multimonitor
- Controla mouse e teclado remotamente (touchpad e toque direto)
- Abre sessões de terminal persistentes em múltiplas abas que continuam vivas mesmo ao reconectar
- Executa e gerencia comandos rápidos pré-configurados com um toque
- Navega, baixa e envia arquivos (confinado ao diretório home)
- Destrava telas de bloqueio e login do GDM usando túnel RDP local criptografado (`127.0.0.1:3389`)
- Liga o computador remotamente via Wake-on-LAN na rede local ou por Pontes de rede (`oraculo-bridge`, outro Android ou Cardputer/ESP32), detalhado no [Guia Completo de Wake-on-LAN](docs/wake-on-lan.pt-BR.md)

O aplicativo desktop (`oraculo-desktop`) oferece interface gráfica nativa (GTK4/libadwaita + WebKitGTK no Linux e Edge WebView2 no Windows) para pareamento, permissões em tempo real, autorização de monitores, comandos rápidos, Wake-on-LAN, inicialização com o sistema e firewall do login remoto.

O app Android tem interface Material 3 com troca PT/EN, temas claro/escuro, **Bloqueio por biometria ou PIN** opcional, zoom por pinça e quatro abas: Touchpad, Terminal, Comandos e Arquivos.

## Download

Acesse a página de [Releases](https://github.com/marcos-ce/Oraculo-Desk/releases/latest) e baixe os dois:

| | Arquivo | Para |
| :--- | :--- | :--- |
| 🖥️ | `oraculo-vX.Y.Z-linux-x86_64.tar.gz` | Seu computador Linux (`oraculo-desktop`, `oraculo-host`, `oraculo-bridge`, `oraculo-cli`) |
| 📱 | `oraculo-vX.Y.Z-android-arm64.apk` | Seu celular Android (8.0+) |

### Instalar no Linux

```bash
tar xzf oraculo-v*.tar.gz
sudo install -m 755 oraculo-host oraculo-desktop oraculo-bridge oraculo-cli /usr/local/bin/
```

A transmissão de tela precisa de uma sessão Wayland com PipeWire, GStreamer (com plugins PipeWire) e um encoder H.264. No Fedora:

```bash
sudo dnf install gstreamer1-plugins-base pipewire-gstreamer gstreamer1-plugin-openh264
```

Se você tem uma GPU NVIDIA, `nvh264enc` é usado automaticamente quando disponível (mais rápido, menos uso de CPU). Caso contrário, usa `openh264enc`.

### Instalar no Android

Transfira o `.apk` para o celular e instale. Pode ser necessário permitir "Instalar de fontes desconhecidas" nas configurações do aparelho.

## Como usar

### 1. Inicie o Oráculo no computador

Pelo aplicativo gráfico (recomendado):

```bash
oraculo-desktop                       # abre a interface (embute o host ou conecta ao daemon ativo)
oraculo-desktop --background          # inicia em segundo plano sem abrir janela
oraculo-desktop --quit                # encerra a instância em execução do Oráculo Desktop e o serviço
```

Fechar a janela do `oraculo-desktop` libera a memória da interface mantendo o serviço ativo em segundo plano; abrir o `oraculo-desktop` novamente reexibe a janela.

Ou rode o daemon diretamente no terminal:

```bash
oraculo-host serve --pair
```

Um QR code e um código rápido de 8 caracteres aparecem no terminal, válidos por 3 minutos. Pressione `p` + Enter para gerar um novo a qualquer momento.

### 2. Pareie o celular

Abra o app Oráculo, toque em **Parear dispositivo** e escaneie o QR code — ou toque em **Digitar código de pareamento** / **Código** (ou **Parear com código** no menu `☰`) para informar o código rápido de 8 caracteres (ex.: `K7P4-M9X2`).

Pronto. Seu celular está conectado. Use as abas para alternar entre Touchpad, Terminal, Comandos e Arquivos.

### 3. Gerenciar dispositivos, permissões e Wake-on-LAN

```bash
oraculo-host peers                    # listar dispositivos pareados
oraculo-host revoke <ENDPOINT_ID>     # remover um dispositivo
oraculo-host permissions              # consultar permissões atuais em settings.toml
oraculo-host permissions \
  --screen true --input true --terminal true \
  --files-read true --files-write false \
  --commands-run true --commands-edit false \
  --rdp-tunnel true --rdp-port 3389 \
  --wol-enabled true --wol-mac "" --wol-broadcast ""
oraculo-host info                     # mostrar EndpointId do host e caminhos de config
oraculo-host monitors                 # escolher monitores para transmissão multimonitor
oraculo-host wol --enable             # ativar Wake-on-LAN / Magic Packet na placa de rede
```

Passe `""` em `--wol-mac` ou `--wol-broadcast` para voltar a autodetectar placas e sub-rede.

## Cliente CLI (`oraculo-cli`)

Também existe um cliente de terminal para parear, executar comandos rápidos, acordar o computador e abrir shells remotos de outro computador:

```bash
oraculo-cli pair "<CODIGO_OU_URI>" --name "meu-notebook"
oraculo-cli ping
oraculo-cli commands
oraculo-cli run <ID_DO_COMANDO>
oraculo-cli shell
oraculo-cli wake                      # ligar o computador padrão salvo na rede local
oraculo-cli wake --bridge <BRIDGE_ID> # ligar via uma ponte WoL pareada
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9
```

- `ping`, `commands`, `run` e `shell` aceitam `--host <ENDPOINT_ID>` para direcionar a um host específico.
- `pair` aceita um código rápido de 8 caracteres ou uma URI `oraculo://pair?...` (além de `--name`).
- `wake` usa o MAC sincronizado do host padrão quando `--mac` é omitido e aceita `--mac`, `--target`, `--port` (padrão `9`) e `--bridge <BRIDGE_ENDPOINT_ID>`.

## Ponte Wake-on-LAN dedicada (`oraculo-bridge`)

Para ligar o computador fora da rede de casa, você pode rodar o `oraculo-bridge` em um servidor Linux, Raspberry Pi ou Termux. O binário compila apenas o encaminhamento Wake-on-LAN, sem código de tela, entrada, terminal ou arquivos:

```bash
oraculo-bridge                                    # primeira execução gera QR code e código rápido
oraculo-bridge serve --pair --name "Raspberry"    # nome personalizado + novo código de pareamento
oraculo-bridge peers                              # listar celulares autorizados
oraculo-bridge revoke <ENDPOINT_ID>               # revogar acesso de um celular
oraculo-bridge info                               # exibir EndpointId e diretórios da ponte
```

Você também pode usar um celular Android sobrando (**Modo Ponte Wake-on-LAN**) ou uma placa ESP32 / M5Stack Cardputer. Veja o [Guia Completo de Wake-on-LAN](docs/wake-on-lan.pt-BR.md).

## Segurança

- **Criptografia ponta a ponta e relays:** as conexões usam QUIC (TLS 1.3) com autenticação mútua e chaves Ed25519 via `iroh`. O tráfego tenta caminho direto P2P primeiro; quando NAT ou firewalls impedem conexão direta, o `iroh` utiliza relays públicos `presets::N0` para encaminhar pacotes já criptografados. Relays não possuem chaves e não conseguem ler conteúdo em claro nem se passar por um dispositivo pareado. Na ponte opcional ESP32 / Cardputer, o transporte usa MQTT sobre TLS (`8883`, padrão `broker.emqx.io` ou broker privado) com Challenge-Response de uso único e payload cifrado com `HMAC-SHA256`, impedindo que o broker veja endereços MAC ou comandos em claro.
- **Pareamento e revogação imediata:** tokens de pareamento são de uso único, expiram em 3 minutos e travam após 5 tentativas erradas. Revogar um dispositivo ou desativar qualquer permissão encerra os streams ativos na hora.
- **Bloqueio por biometria ou PIN (Android):** opção no menu `☰` Configurações que exige desbloqueio do aparelho antes de abrir ou ligar um computador salvo e ao retornar para uma sessão ativa.
- **Sandbox de arquivos:** o acesso fica confinado ao diretório home (`~`) com proteção contra symlinks e traversal. Caminhos sensíveis (`.ssh`, `.gnupg`, keyrings, configs do shell, `systemd/user`, `autostart`, `environment.d` e diretórios de dados/config do próprio Oráculo) são sempre bloqueados. Chaves privadas e tokens são gravados com permissão `0600`.
- **Proteção do firewall para Login Remoto (RDP):** ao ativar o Login Remoto do GNOME para destravar via túnel RDP do Oráculo, o `oraculo-desktop` detecta se a porta `3389` está visível na LAN e oferece o botão **Bloquear na LAN** — ou execute `sudo ./packaging/linux/harden-rdp-firewall.sh [PORTA]` para rejeitar conexões TCP diretas nas zonas ativas do `firewalld`, mantendo o acesso somente pelo túnel criptografado em `127.0.0.1`.

Veja [`.github/SECURITY.md`](.github/SECURITY.md) para reportar vulnerabilidades.

## Compilar do código-fonte

Se quiser compilar você mesmo em vez de usar os binários da release, precisa do [Podman](https://podman.io/). Os scripts de build cuidam do toolchain Rust, Android SDK e NDK dentro de contêineres.

```bash
git clone https://github.com/marcos-ce/Oraculo-Desk.git
cd Oraculo-Desk

./scripts/build.sh release            # host, desktop, bridge, cli → ./dist/
./scripts/build-android.sh debug      # APK Android → ./dist/
./scripts/build.sh test               # rodar testes do workspace
```

## Suporte de plataformas

| Componente | Linux | Windows | macOS | Android | iOS |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Terminal, comandos, arquivos, WoL | ✅ | ✅ (sem teste em Windows) | experimental | ✅ (cliente) | planejado |
| App desktop (`oraculo-desktop`) | ✅ (GTK4/GTK3 WebKit) | ✅ (Edge WebView2, sem teste) | — | — | — |
| Transmissão de tela + entrada remota | ✅ (Wayland) | stub (planejado) | stub | ✅ (cliente) | planejado |

### Limitações do Beta v0.1

- **Tela travada ou login inicial no Wayland (GDM):** dentro de uma sessão já desbloqueada, o streaming de tela e o controle de mouse e teclado rodam direto no app. Quando o computador acabou de ligar na tela de login do GDM ou acordou da suspensão pedindo senha, o Wayland bloqueia novas sessões pelo `xdg-desktop-portal`. Nesses casos, o desbloqueio usa o túnel criptografado do Oráculo na porta `3389` em conjunto com um cliente RDP externo no celular (`aFreeRDP` ou `Windows App`). Um cliente RDP embutido fica para a v0.2.
- **Windows:** terminal, comandos, arquivos, Wake-on-LAN, autostart, app desktop e regras de firewall já têm código para Windows no repositório, mas a captura de tela (`screen.rs`) e a entrada remota (`input.rs`) ainda são stubs. O porte completo e os testes nativos no Windows ficam para depois deste Beta.
- **iPhone (iOS):** não faz parte do Beta v0.1. O núcleo em Rust usa UniFFI e já pode gerar bindings Swift no futuro, mas a interface em SwiftUI e o build em macOS com Xcode ficam para uma etapa seguinte.

## Licença

[Apache License 2.0](licenses/LICENSE). Atribuições de terceiros em [`licenses/NOTICE`](licenses/NOTICE).
