# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![Licença](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

*Read this in [English](README.md).*

O **Oráculo** é um conjunto de aplicativos (Desktop Linux + Android) para controlar seu computador pelo celular sobre QUIC criptografado ponta a ponta (`iroh`) — sem contas, sem login em nuvem e sem abrir portas no roteador.

## Recursos

- **Espelhamento de tela e controle remoto:** vídeo H.264 decodificado por hardware, troca multimonitor, modo touchpad, toque direto e zoom por pinça.
- **Terminal persistente em abas:** sessões de shell que continuam vivas mesmo após reconectar.
- **Comandos rápidos:** atalhos sincronizados entre computador e celular, executados com um toque.
- **Arquivos confinados à pasta pessoal:** navegue, visualize, baixe e envie arquivos restritos à pasta `~`.
- **Desbloqueio de login remoto:** destrave telas do GDM / bloqueio do sistema via túnel RDP local criptografado (`127.0.0.1:3389`), com verificação de firewall integrada.
- **Wake-on-LAN:** ligue o computador na rede local ou fora de casa usando uma ponte (`oraculo-bridge`, um celular Android sobrando ou placa ESP32 / M5Stack Cardputer).
- **Apps nativos em Material Design 3:** interface GTK4/libadwaita + WebKitGTK no Linux e Jetpack Compose no Android, ambos com temas Claro/Escuro e idioma PT-BR/EN.

## Download e instalação

Baixe os arquivos da versão mais recente na página de [Releases](https://github.com/marcos-ce/Oraculo-Desk/releases):

| Plataforma | Arquivo | Requisitos |
| :--- | :--- | :--- |
| **Linux (x86_64)** | `oraculo-v*-linux-x86_64.tar.gz` | Wayland + PipeWire, WebKitGTK (`webkitgtk6.0` ou `webkit2gtk-4.1`), GStreamer H.264 |
| **Android** | `oraculo-v*-android-arm64.apk` | Android 8.0+ (`arm64-v8a`) |
| **Checksums** | `SHA256SUMS-*.txt` | Verificação opcional de integridade (`sha256sum -c SHA256SUMS-*.txt`) |

### Computador (Linux)

Extraia o pacote e instale o aplicativo junto com o atalho do menu e o ícone:

```bash
tar xzf oraculo-v*-linux-x86_64.tar.gz
install -Dm 755 oraculo-desktop oraculo-host oraculo-bridge oraculo-cli -t ~/.local/bin/
install -Dm 644 com.oraculo.Desktop.desktop -t ~/.local/share/applications/
install -Dm 644 com.oraculo.Desktop.svg -t ~/.local/share/icons/hicolor/scalable/apps/
```

Para transmissão de tela no Fedora (Wayland + PipeWire):

```bash
sudo dnf install gstreamer1-plugins-base pipewire-gstreamer gstreamer1-plugin-openh264
```

Placas NVIDIA usam `nvh264enc` automaticamente quando disponível; nos demais casos, o Oráculo usa `openh264enc`.

### Celular (Android)

Transfira o `.apk` para o celular e instale (permita a instalação de fontes desconhecidas se o sistema solicitar).

## Primeiros passos

1. **Abra o Oráculo no computador:** inicie o **Oráculo** pelo menu de aplicativos (ou execute `oraculo-desktop`). Um QR Code de uso único e um código de 8 caracteres aparecem na aba **Início**.
2. **Pareie o celular:** abra o app **Oráculo** no Android, toque em **Parear dispositivo** e aponte a câmera para o QR Code — ou toque em **Digitar código de pareamento** para informar o código (ex.: `K7P4-M9X2`).
3. **Controle e configure direto pelos apps:**
   - **Abas do app Desktop:**
     - **Início:** parear novos aparelhos e ver, desconectar ou remover dispositivos autorizados.
     - **Acesso:** ativar/desativar permissões na hora (tela, mouse/teclado, terminal, login remoto RDP, arquivos, comandos), autorizar monitores Wayland uma única vez, isolar a porta RDP no firewall e ativar o Wake-on-LAN.
     - **Comandos:** criar, editar, duplicar ou remover atalhos de comandos rápidos.
     - **Sistema:** configurar inicialização automática, tema (`Auto` / `Claro` / `Escuro`), idioma (`Português` / `English`) ou encerrar o serviço em segundo plano.
   - **Abas do app Android:** alterne entre **Touchpad** (com espelhamento de tela), **Terminal**, **Comandos** e **Arquivos**, ou ative o **Bloqueio por biometria ou PIN** no menu `☰`.

Fechar a janela do app Desktop libera a memória da interface mantendo o serviço ativo em segundo plano; basta abrir o **Oráculo** novamente para reexibir a janela.

## Wake-on-LAN

No mesmo Wi-Fi ou cabo de rede, o app Android envia o Magic Packet diretamente. Para ligar o computador estando fora de casa, pareie uma das três opções de ponte na sua rede local:

- **Ponte dedicada para Linux / Raspberry Pi / Termux** (`oraculo-bridge`)
- **Celular Android sobrando** (**Modo Ponte Wake-on-LAN** no menu `☰` do app)
- **ESP32 / M5Stack Cardputer** ([`firmware/oraculo-wol-esp32`](firmware/oraculo-wol-esp32/README.md))

Veja o passo a passo completo no [Guia de Wake-on-LAN](docs/wake-on-lan.pt-BR.md).

## Segurança

- **Criptografia ponta a ponta:** todas as conexões usam QUIC (TLS 1.3) com autenticação mútua e chaves Ed25519 via `iroh`. A comunicação tenta caminho direto P2P primeiro; quando NAT ou firewalls bloqueiam a rota direta, o `iroh` encaminha pacotes cifrados pelos relays públicos `presets::N0`, que nunca têm acesso às chaves nem ao tráfego em claro. A ponte opcional ESP32 usa MQTT sobre TLS (`8883`) com nonces Challenge-Response de uso único e payload cifrado com `HMAC-SHA256`.
- **Pareamento temporário e revogação imediata:** códigos de pareamento são de uso único, expiram em 3 minutos e são descartados após 5 tentativas inválidas. Desativar qualquer permissão ou remover um aparelho encerra as sessões ativas na hora.
- **Bloqueio do aplicativo (Android):** proteção opcional por biometria ou PIN antes de abrir ou ligar um computador salvo e ao voltar para uma sessão ativa.
- **Sandbox de arquivos:** navegação e transferências ficam restritas à pasta pessoal (`~`) com proteção contra `..` e symlinks externos, bloqueando `.ssh`, `.gnupg`, keyrings, arquivos de inicialização do shell, `systemd/user`, `autostart`, `environment.d` e diretórios internos do Oráculo. Chaves privadas e tokens são gravados com permissão `0600`.
- **Isolamento do firewall RDP:** quando o Login Remoto do sistema está ativo para destravar a tela de login, o **Oráculo Desktop** avisa se a porta `3389` estiver acessível na rede local e oferece o botão **Bloquear** (usando [`packaging/linux/harden-rdp-firewall.sh`](packaging/linux/harden-rdp-firewall.sh)) para aceitar apenas conexões pelo túnel criptografado em `127.0.0.1`.

Para reportar vulnerabilidades de forma privada, consulte [`.github/SECURITY.md`](.github/SECURITY.md).

## Suporte de plataformas

| Recurso | Linux | Windows | macOS | Android | iOS |
| :--- | :---: | :---: | :---: | :---: | :---: |
| Aplicativo Desktop (`oraculo-desktop`) | ✅ | em desenvolvimento | — | — | — |
| Terminal, comandos, arquivos, Wake-on-LAN | ✅ | em desenvolvimento | experimental | ✅ (cliente) | planejado |
| Transmissão de tela e controle remoto | ✅ (Wayland) | próximas atualizações | planejado | ✅ (cliente) | planejado |

- **Ambientes gráficos no Linux:** a transmissão de tela e o controle de mouse/teclado usam os padrões **XDG Desktop Portal** (`ScreenCast` + `RemoteDesktop`) e **PipeWire**. O desenvolvimento e os testes principais foram feitos em **GNOME (Wayland / Fedora)** e **Android (`arm64-v8a`)**. Outros ambientes Wayland que implementam ambos os portais (como **KDE Plasma 6**) também tendem a funcionar; já em **sessões X11** (como Cinnamon ou XFCE em X11), o app desktop, terminal, comandos, arquivos e Wake-on-LAN funcionam, mas ainda não há captura de tela/entrada via X11.
- **Tela bloqueada / login inicial no Wayland (GDM):** dentro de uma sessão já desbloqueada, o espelhamento de tela e o controle rodam diretamente no app Android. Na tela de login do GDM ou com o sistema bloqueado por senha, o Wayland restringe capturas pelo portal; nesses momentos o desbloqueio acontece pelo túnel RDP criptografado do Oráculo com um cliente RDP no celular (`aFreeRDP` ou `Windows App`).
- **Nota de beta:** esta é uma versão beta em evolução ativa. Cenários fora do conjunto principal GNOME Wayland + Android (como KDE Plasma, outros compositores Wayland e o firmware ESP32) ainda tiveram cobertura limitada de testes em campo — relatos de compatibilidade e bugs em [Issues](https://github.com/marcos-ce/Oraculo-Desk/issues) são muito bem-vindos.

## Próximas atualizações

- **Suporte completo ao Windows (próxima prioridade):** captura de tela nativa, controle remoto de mouse e teclado, empacotamento e testes ponta a ponta no Windows.
- **Mais ambientes Linux:** validação dedicada no KDE Plasma, compositores wlroots/COSMIC e eventual suporte a sessões X11 (Cinnamon, XFCE, MATE).
- **Etapas futuras:** cliente RDP embutido no Android para destravar telas bloqueadas sem app externo e aplicativo iOS compartilhando o mesmo núcleo em Rust.

<details>
<summary><strong>Uso avançado: modo servidor (headless), cliente CLI e ponte dedicada</strong></summary>

Todos os recursos também podem ser operados via terminal em máquinas sem interface gráfica ou em automações.

### Opções do app Desktop
```bash
oraculo-desktop              # abre a interface (embute o host ou conecta ao serviço ativo)
oraculo-desktop --background # inicia em segundo plano sem abrir janela
oraculo-desktop --quit       # encerra a interface e o serviço em segundo plano
```

### Daemon em modo servidor (`oraculo-host`)
```bash
oraculo-host serve --pair             # inicia o daemon exibindo QR + código de 8 caracteres (tecle 'p' para renovar)
oraculo-host peers                    # listar dispositivos pareados
oraculo-host revoke <ENDPOINT_ID>     # revogar um dispositivo imediatamente
oraculo-host permissions              # consultar permissões atuais em settings.toml
oraculo-host permissions \
  --screen true --input true --terminal true \
  --files-read true --files-write false \
  --commands-run true --commands-edit false \
  --rdp-tunnel true --rdp-port 3389 \
  --wol-enabled true --wol-mac "" --wol-broadcast ""
oraculo-host info                     # exibir EndpointId e diretórios de configuração
oraculo-host monitors                 # escolher monitores para transmissão multimonitor
oraculo-host wol --enable             # ativar Wake-on-LAN / Magic Packet nas placas físicas
```

### Cliente de terminal (`oraculo-cli`)
```bash
oraculo-cli pair "<CODIGO_OU_URI>" --name "notebook"
oraculo-cli ping                      # aceita --host <ENDPOINT_ID>
oraculo-cli commands                  # aceita --host <ENDPOINT_ID>
oraculo-cli run <ID_DO_COMANDO>       # aceita --host <ENDPOINT_ID>
oraculo-cli shell                     # aceita --host <ENDPOINT_ID>
oraculo-cli wake                      # ligar o computador padrão na rede local
oraculo-cli wake --bridge <BRIDGE_ID> # ligar via uma ponte WoL pareada
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9
```

### Ponte Wake-on-LAN dedicada (`oraculo-bridge`)
```bash
oraculo-bridge                                    # primeira execução exibe QR Code e código rápido
oraculo-bridge serve --pair --name "Raspberry"    # nome personalizado + novo código de pareamento
oraculo-bridge peers                              # listar celulares autorizados
oraculo-bridge revoke <ENDPOINT_ID>               # revogar um celular
oraculo-bridge info                               # exibir EndpointId e diretórios da ponte
```

</details>

## Compilar a partir do código-fonte

A compilação roda em contêineres reproduzíveis com [Podman](https://podman.io/) trazendo Rust, Android SDK 35 e NDK fixados:

```bash
git clone https://github.com/marcos-ce/Oraculo-Desk.git
cd Oraculo-Desk

./scripts/build.sh test          # executar testes do workspace
./scripts/build.sh release       # compilar host, desktop, bridge e cli → ./dist/
./scripts/build-android.sh debug # compilar APK Android → ./dist/
```

## Licença

Distribuído sob a [Apache License 2.0](LICENSE). Atribuições de terceiros e licenças de fontes embutidas estão em [`NOTICE`](NOTICE).
