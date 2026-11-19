# Guia Completo de Wake-on-LAN (WoL) no Oráculo

*Read this in [English](wake-on-lan.md).*

O **Wake-on-LAN (WoL)** permite acordar um computador desligado ou suspenso diretamente pelo aplicativo Oráculo no celular ou pelo `oraculo-cli`.

Este guia explica como o recurso funciona por dentro, como configurar **computador, Linux/Windows, BIOS e roteador**, como funcionam as **Pontes de rede** para acordar o computador fora de casa, quais são os **riscos reais de segurança** e como diagnosticar falhas comuns.

---

## Sumário

1. [Como funciona no Oráculo](#1-como-funciona-no-oráculo)
2. [Os 4 modos de disparo (na LAN e fora de casa)](#2-os-4-modos-de-disparo-na-lan-e-fora-de-casa)
3. [Onde ver e ativar no computador](#3-onde-ver-e-ativar-no-computador)
4. [Preparar o sistema operacional, rede e BIOS](#4-preparar-o-sistema-operacional-rede-e-bios)
5. [Suspenso vs. Desligado e Cabo Ethernet vs. Wi-Fi (WoWLAN)](#5-suspenso-vs-desligado-e-cabo-ethernet-vs-wi-fi-wowlan)
6. [O que acontece após ligar (autostart e tela de login / GDM)](#6-o-que-acontece-após-ligar-autostart-e-tela-de-login--gdm)
7. [Segurança: é perigoso? Por que vem desativado?](#7-segurança-é-perigoso-por-que-vem-desativado)
8. [Diagnóstico e solução de problemas](#8-diagnóstico-e-solução-de-problemas)

---

## 1. Como funciona no Oráculo

Diferente de conexões QUIC P2P normais (que dependem do computador estar ligado), um computador desligado ou em suspensão profunda não roda nenhum sistema operacional — apenas o chip da placa de rede fica alimentado em standby ouvindo o **Magic Packet** IEEE/AMD.

- **Formato do pacote:** quadro UDP de `102 bytes` contendo `6` bytes `0xFF` seguidos do endereço MAC físico repetido `16` vezes.
- **Portas e rajada:** o Oráculo envia `3` rajadas espaçadas por `40 ms` nas portas UDP configurada (padrão `9` / Discard) e secundária (`7` / Echo), mitigando perdas comuns de multicast/broadcast em Wi-Fi (DTIM).
- **Descoberta automática de MAC e sub-rede:** assim que você conecta uma vez ao computador ligado, o host envia sua lista de até `6` endereços MAC físicos (priorizando interfaces cabeadas ativas e incluindo MACs permanentes `permaddr` quando há randomização no Wi-Fi) junto com o broadcast da sub-rede local (ex.: `192.168.18.255`).
- **Configuração manual opcional:** na caixa de diálogo de Wake-on-LAN no Android, é possível editar o **Endereço MAC** manualmente ou expandir **Opções avançadas (DDNS e porta)** para definir um IP/domínio externo e porta UDP customizada.

---

## 2. Os 4 modos de disparo (na LAN e fora de casa)

Quando você toca em **Ligar computador** no app Android, o cliente dispara os Magic Packets em paralelo por todos os canais disponíveis:

### A. Disparo direto no Wi-Fi de casa (sem ponte)
Se o celular estiver no mesmo Wi-Fi/roteador que o computador, o app envia o Magic Packet diretamente em broadcast UDP para a sub-rede local salva (`192.168.x.255`) e para `255.255.255.255`. Não precisa de nenhum equipamento extra.

### B. Ponte Android (outro celular ou tablet antigo na rede de casa)
Se você estiver na rua (4G/5G ou outro Wi-Fi), o broadcast UDP da operadora não atravessa roteadores domésticos nem CGNAT. Qualquer dispositivo Android antigo deixado conectado ao Wi-Fi de casa pode atuar como ponte usando o **mesmo aplicativo Oráculo**:
1. No celular secundário, abra o menu **`☰`** → ative **Modo Ponte Wake-on-LAN**.
2. Toque em **Código** no cartão de ponte ativa (ou em **Gerar código desta ponte** no menu **`☰`**) para gerar um **código de pareamento de 8 caracteres (`XXXX-XXXX`)**, válido por 3 minutos. *(A Ponte Android exibe apenas o código curto — não gera QR Code.)*
3. No celular principal, toque em **Código** / **Digitar código de pareamento** (ou **Parear com código** no menu **`☰`**) e digite os 8 caracteres. A ponte aparecerá automaticamente no grupo **Pontes de rede**.

### C. Ponte leve em PC / Raspberry Pi / Linux / Termux (`oraculo-bridge`)
Binário standalone ([`crates/oraculo-bridge/src/main.rs`](../crates/oraculo-bridge/src/main.rs)) feito exclusivamente para encaminhar Wake-on-LAN sem incluir código de captura de tela, terminal, arquivos nem controle de entrada:

```bash
# Primeira execução: imprime o QR Code UTF-8 no terminal e o código rápido
oraculo-bridge

# Ou forçar novo código / definir nome amigável
oraculo-bridge serve --pair --name "Raspberry-Casa"

# Gerenciar acessos e ver diretórios (~/.local/share/oraculo-bridge/)
oraculo-bridge peers
oraculo-bridge revoke <ENDPOINT_ID>
oraculo-bridge info
```

### D. Ponte embarcada ESP32 / M5Stack Cardputer (`oraculo_wol_esp32.ino`)
Firmware autocontido em [`firmware/oraculo-wol-esp32/oraculo_wol_esp32.ino`](../firmware/oraculo-wol-esp32/oraculo_wol_esp32.ino) (~0,3 W ligado direto na porta USB do roteador):
- **Compatibilidade:** M5Stack Cardputer (tela + QR Code + atalhos físicos `Q`, `Enter`, `P`, `R`) e placas ESP32 / ESP32-C3 / ESP32-S2 / ESP32-S3 genéricas (portal Wi-Fi `Oraculo-Ponte-XXXX` em `http://192.168.4.1`, protegido por senha WPA2 aleatória de 10 caracteres por sessão exibida na tela e no console Serial + CLI serial a `115200 baud`). Por segurança, o portal HTTP não expõe a URI/segredo de pareamento — após salvar o Wi-Fi, faça o pareamento pelo QR Code do Cardputer ou pelo comando `uri` no Serial.
- **Transporte:** comunica via MQTT obrigatoriamente sobre **TLS 8883** (`broker.emqx.io` padrão ou broker privado configurável), com validação de certificado X.509 após sincronização NTP.
- Veja o passo a passo completo de gravação e configuração em [`firmware/oraculo-wol-esp32/README.md`](../firmware/oraculo-wol-esp32/README.md#português-brasil).

> **Regra de organização:** pontes nunca são vinculadas a um único computador. Pareou uma ponte uma vez no celular principal, ela passa a aparecer em **Pontes de rede** e atende todos os computadores salvos daquela rede automaticamente.

---

## 3. Onde ver e ativar no computador

### No Oráculo Desktop (interface gráfica)
1. Abra o `oraculo-desktop` e clique na aba **Acesso**.
2. Role até a seção **Wake-on-LAN**:
   - O switch **Ativar Wake-on-LAN** controla `wol_enabled` em `settings.toml` (vem **ativado** por padrão para compartilhar os MACs com celulares já pareados).
   - Em **Placas de rede**, veja cada interface detectada (Ethernet e Wi-Fi) e o status dos perfis do NetworkManager.
   - Se algum perfil estiver sem suporte a Magic Packet, clique em **Ativar na placa de rede**.

### Pela linha de comando (`oraculo-host wol`)
```bash
# Inspecionar placas detectadas, MACs anunciados, broadcast e perfis NetworkManager
oraculo-host wol

# Ativar em settings.toml e configurar wake-on-lan = magic nos perfis NetworkManager
oraculo-host wol --enable

# Fixar um MAC ou broadcast manualmente (vazio "" volta a autodetectar)
oraculo-host wol --mac AA:BB:CC:DD:EE:FF --broadcast 192.168.1.255

# Desativar o anúncio de dados WoL para clientes pareados
oraculo-host wol --disable
```

### Pelo cliente de terminal (`oraculo-cli wake`)
```bash
# Disparar WoL para o computador padrão usando o MAC e broadcast salvos no pareamento/ping
oraculo-cli wake

# Disparar na LAN e também encaminhar por uma ponte P2P pareada
oraculo-cli wake --bridge <BRIDGE_ENDPOINT_ID>

# Informar MAC, endereço de destino/broadcast ou porta UDP (padrão: 9) explicitamente
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9

# Acordar um MAC específico usando uma ponte P2P pareada
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --bridge <BRIDGE_ENDPOINT_ID>
```

---

## 4. Preparar o sistema operacional, rede e BIOS

O Wake-on-LAN exige cooperação entre **4 camadas**: Oráculo → Sistema Operacional / Driver → BIOS/UEFI → Cabo/Roteador.

### Linux (Fedora / Ubuntu / Arch com NetworkManager)
1. **Ative no perfil NetworkManager** (ou clique em **Ativar na placa de rede** no Oráculo Desktop):
   ```bash
   nmcli connection show
   nmcli connection modify "Conexão cabeada 1" 802-3-ethernet.wake-on-lan magic
   nmcli device reapply eno1
   ```
   *(Substitua `"Conexão cabeada 1"` e `eno1` pelo nome do seu perfil e interface.)*
2. **Confira o estado real do chip Ethernet** (com o cabo conectado):
   ```bash
   nmcli -f 802-3-ethernet.wake-on-lan connection show "Conexão cabeada 1"
   sudo ethtool eno1 | grep -i wake-on
   ```
   - `Wake-on: g` significa ativo somente para Magic Packet.
   - `Wake-on: d` significa desativado no driver (mesmo se o NetworkManager pedir `magic`, alguns drivers precisam reaplicar após subir link ou atualizar firmware/kernel).
3. **Verifique se o kernel permite wakeup no barramento:**
   ```bash
   cat /sys/class/net/eno1/device/power/wakeup
   ```
   Deve retornar `enabled`.

### BIOS / UEFI (obrigatório em praticamente todo notebook e em muitos desktops)
1. Ao ligar o computador, entre na BIOS (`F2` ou `Del` na maioria das placas ASUS/desktops).
2. Procure em **Advanced** → **APM Configuration** / **Power Management**:
   - **`ErP Ready` / `ErP Support`:** coloque em **`Disabled`** (esta é a opção que mais bloqueia WoL em notebooks ASUS e placas-mãe modernas).
   - **`Power On By PCI-E` / `Wake on LAN` / `Resume by Onboard LAN`:** coloque em **`Enabled`**.
3. Em notebooks, mantenha o **carregador conectado na tomada** ao testar S5 (desligado total): quase todos os notebooks desligam a alimentação standby da porta RJ-45 quando estão apenas na bateria.
4. **Teste físico rápido:** com o computador completamente desligado, o carregador ligado e o cabo Ethernet conectado ao roteador, **observe os LEDs da porta RJ-45** (ou a luz da porta correspondente no roteador). Se ficarem 100% apagados, a BIOS/placa cortou energia em S5 e nenhum pacote de rede poderá acordá-lo daquele estado.

### Windows (quando usado como host WoL)
1. Abra **Gerenciador de Dispositivos** → **Adaptadores de rede** → clique com o botão direito na placa Ethernet → **Propriedades**:
   - Em **Gerenciamento de Energia**: marque *Permitir que este dispositivo acorde o computador* e *Somente permitir que um Magic Packet acorde o computador*.
   - Em **Avançado**: ative *Wake on Magic Packet* / *Despertar com pacote mágico* (e *Shutdown Wake-On-Lan*, se existir).
2. No **Painel de Controle → Opções de Energia → Escolher a função dos botões de energia**, desmarque **Ligar inicialização rápida** (*Fast Startup*), porque ela coloca o Windows em hibernação parcial (S4 híbrido) onde várias placas Realtek/Intel ignoram pacotes WoL.

---

## 5. Suspenso vs. Desligado e Cabo Ethernet vs. Wi-Fi (WoWLAN)

| Estado da máquina | Cabo Ethernet (`r8169`, Intel, etc.) | Wi-Fi (WoWLAN) |
| :--- | :--- | :--- |
| **Suspenso** (`systemctl suspend` / Sleep) | **Muito confiável.** Geralmente funciona direto sem mexer em ErP. | **Pode funcionar**, se a placa Wi-Fi e o roteador suportarem manter associação/DTIM em standby e `802-11-wireless.wake-on-wlan = magic` estiver ativo. |
| **Desligado total** (ACPI S5 / `poweroff`) | **Depende da BIOS + carregador.** Requer `ErP Ready = Disabled`, `Wake-on: g` mantido no shutdown e LED de link ativo em standby. | **Raramente suportado** na maioria dos notebooks e placas-mãe, pois o rádio Wi-Fi é desligado em S5. |

---

## 6. O que acontece após ligar (autostart e tela de login / GDM)

Acordar o hardware é metade do caminho; para você conseguir **usar** o computador logo em seguida:

1. **Daemon em segundo plano desde o boot (`oraculo-host.service`):**
   - Ao habilitar **Iniciar com o sistema** no Linux (ativado por padrão na primeira abertura do `oraculo-desktop`), o Oráculo instala `~/.config/systemd/user/oraculo-host.service` (`WantedBy=default.target`) e habilita **`loginctl enable-linger`**.
   - Isso garante que o `oraculo-host` suba **antes mesmo do login gráfico** — sem precisar abrir uma janela visível nem ter usuário logado no GDM.
   - A janela `oraculo-desktop` continua sob demanda (não abre sozinha a cada boot), enquanto o serviço fica rodando em segundo plano.
2. **Janela de espera automática no app Android:**
   - Depois de enviar os Magic Packets, o [`WakeOnLanDialog`](../android/app/src/main/java/com/oraculo/app/WakeOnLanDialog.kt) sonda o host a cada tentativa (timeout de `8s` por sondagem em janela de `55s`) e conecta automaticamente assim que o `oraculo-host` fica online.
3. **Destravar na tela de senha (GDM no Fedora / tela bloqueada):**
   - No GNOME Wayland antes do login (GDM), o `xdg-desktop-portal` do usuário ainda não existe, então **Abrir tela** e **Touchpad** nativos só funcionam depois de entrar na sessão.
   - Para digitar a senha do GDM remotamente, use **Destravar via login remoto (RDP)** (ou o ícone de cadeado **Login remoto (RDP)** na barra superior) no Oráculo, abrindo o túnel criptografado para `127.0.0.1:3389`. No Fedora/GNOME, isso exige ter ligado **Login Remoto** (serviço de sistema `gnome-remote-desktop.service`, diferente do *Compartilhamento de Área de Trabalho* que só sobe após o login) em **Configurações → Sistema → Área de Trabalho Remota**, e um cliente RDP no Android (`aFreeRDP` ou `Windows App`).
   - Para impedir acesso direto à porta `3389` na rede local mantendo o túnel funcionando, clique em **Bloquear na LAN** no `oraculo-desktop` ou execute `sudo ./packaging/linux/harden-rdp-firewall.sh 3389`.

---

## 7. Segurança: é perigoso? Por que vem desativado?

### É perigoso ativar?
- **WoL não é acesso remoto:** o Magic Packet apenas liga a fonte/placa-mãe. Ele **não** expõe arquivos, **não** abre shell e **não** ignora a senha do seu sistema ou criptografia de disco (LUKS/BitLocker).
- **Risco prático real:** qualquer dispositivo que já esteja na **mesma rede local** e saiba o MAC da placa pode ligar a máquina a qualquer momento (consumo de energia ou aquecimento caso seja um notebook fechado dentro de mochila/capa conectado na tomada).
- **Por que o Oráculo NÃO recomenda abrir porta UDP `9` no roteador:** expor porta `9` na internet permite que desconhecidos disparem broadcasts se descobrirem seu IP/MAC. Prefira sempre uma **Ponte de rede** autenticada.

### Proteções implementadas no Oráculo
- **Compartilhamento controlado de MACs:** somente dispositivos pareados recebem os endereços MAC do computador, e apenas quando `wol_enabled = true`.
- **Bloqueio de SSRF e DNS beacon ([`crates/oraculo-proto/src/wol.rs`](../crates/oraculo-proto/src/wol.rs)):** `sanitize_wol_info` e `parse_safe_lan_broadcast_v4` aceitam apenas literais IPv4 privados, CGNAT, link-local ou broadcast vindos da rede — IPs públicos e nomes de domínio remotos recebidos do host são rejeitados.
- **Ponte `oraculo-bridge` isolada:** não compila nem carrega código de tela, entrada, terminal ou arquivos; guarda chaves em `~/.local/share/oraculo-bridge/` com permissão `0600`.
- **Pontes MCU (`Cardputer / ESP32`) protegidas ponta a ponta:**
  - Conexão **MQTT exclusivamente sobre TLS (`8883`)**, nunca porta `1883` em texto puro.
  - Segredo único de `256 bits` gerado por TRNG (`esp_fill_random`) e gravado na NVS — zero segredo de fábrica.
  - **Portal Wi-Fi com WPA2 e zero exposição de segredos:** o ponto de acesso `Oraculo-Ponte-XXXX` gera uma senha WPA2 aleatória de 10 caracteres a cada sessão (exibida na tela e no Serial) e nunca exibe a URI de pareamento nem credenciais MQTT via HTTP.
  - Fluxo **Challenge-Response com `nonce` único (TTL `25s`, anti-replay)** e payload cifrado com fluxo derivado por **`HMAC-SHA256`** + verificação de assinatura antes do disparo UDP e retorno **`ACK`** autenticado. Nem o broker MQTT enxerga os MACs nem os pacotes enviados.

### Por que vem desativado de fábrica nos computadores/BIOS?
1. **Normas ambientais (ErP / EuP):** exigem consumo em standby desligado abaixo de `0,5 W`–`1 W`, levando fabricantes (sobretudo de notebooks) a ativar **`ErP Ready`** na BIOS e cortar energia da LAN quando desligado.
2. **Preservação de bateria:** em notebooks fora da tomada, manter a NIC ativa em standby reduziria autonomia.
3. **Evitar religamentos acidentais:** modos antigos de despertar por tráfego comum (`p`/`u`/`m`) acordavam computadores sozinhos com tráfego de rede normal — por isso sistemas deixam desativado até habilitar explicitamente apenas **Magic Packet (`g`)**.

---

## 8. Diagnóstico e solução de problemas

### 1. Acorda da suspensão (`systemctl suspend`), mas não quando está desligado (`poweroff`)
- Verifique se o carregador está conectado na tomada e o cabo RJ-45 está firme antes de desligar.
- Confira os **LEDs da porta RJ-45** após o desligamento: se apagarem completamente, entre na BIOS e desative **`ErP Ready`** / ative **`Power On By PCI-E` / `Wake on LAN`**.
- Rode `sudo ethtool eno1 | grep -i Wake-on` para confirmar `Wake-on: g` (e `Supports Wake-on` contendo `g`).

### 2. Cabo e LEDs estão acesos no desligado, mas o celular no Wi-Fi não liga o computador
- Alguns roteadores isolam clientes Wi-Fi da rede cabeada (**AP Isolation / Guest Network / Client Isolation**) ou descartam broadcast `255.255.255.255` vindo do Wi-Fi.
- Teste enviando pela sub-rede específica (ex.: `192.168.18.255`), use um **ESP32 / `oraculo-bridge` ligado por cabo ou no Wi-Fi principal** sem isolamento, ou confira se o endereço MAC configurado no app é o da placa **Ethernet** (e não o MAC randomizado de Wi-Fi).

### 3. O computador ligou fisicamente, mas o app exibiu *"Sinal enviado, mas o computador ainda não respondeu"*
- Confira se o boot completo levou mais de `55s` (ex.: disco lento, atualização do sistema ou descriptografia manual de LUKS/BitLocker na inicialização antes de subir rede).
- Confirme no computador se o serviço automático subiu no boot:
  ```bash
  systemctl --user status oraculo-host.service --no-pager
  loginctl show-user "$USER" -p Linger
  ```
  Deve mostrar `Active: active (running)` e `Linger=yes`.

### 4. Ponte MCU (`Cardputer / ESP32`) não recebe ou dá erro TLS
- Garanta que o Wi-Fi da casa conectado na placa tem acesso à internet para sincronizar o relógio via NTP e conectar ao broker na porta TCP `8883` (certificados TLS falham se a data/hora do ESP32 não sincronizar).
- No Cardputer, pressione **`Q`** para ver o status (`Wi-Fi`, `NTP` e `TLS OK`). Se precisar reconfigurar Wi-Fi/broker, pressione **`P`** (ou segure **BOOT** por `3s`), conecte ao AP `Oraculo-Ponte-XXXX` usando a senha WPA2 exibida na tela/Monitor Serial (`115200 baud`) e acesse `http://192.168.4.1`.

### 5. App Android mostrou ponte com QR antigo ou sem resposta
- No Cardputer, pressionar **`R`** rotaciona `bridge_id` e `secret`, invalidando todos os QR Codes antigos. Remova a ponte antiga no app Android e pareie novamente pelo novo QR Code (ou pelo comando `uri` no Serial).
