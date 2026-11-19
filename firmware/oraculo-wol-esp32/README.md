# Oráculo Wake-on-LAN Bridge: Cardputer / ESP32

[English](#english) · [Português (Brasil)](#português-brasil)

> **Status: experimental.** This firmware has had limited real-world testing. It is optional: `oraculo-bridge` or a spare Android phone can do the same job.

---

## English

Single-file Arduino firmware (`oraculo_wol_esp32.ino`) that wakes your computer from outside your home network. Runs on **M5Stack Cardputer** (screen, keyboard, on-screen QR Code) and generic **ESP32 / C3 / S2 / S3** boards (headless, with Wi-Fi setup portal and Serial CLI at `115200 baud`).

### Flash (Arduino IDE 2.x)

1. Install the **esp32** board package (*Espressif Systems*).
2. **Cardputer only:** install the **M5Cardputer** library and select the **M5Cardputer** board (or uncomment `#define ORACULO_CARDPUTER 1` at the top of the `.ino` when building for a generic ESP32-S3). Generic boards need no extra libraries.
3. Open `oraculo_wol_esp32.ino` and click **Upload**.

### Set up and pair

1. On first boot the board opens a **`Oraculo-Ponte-XXXX`** Wi-Fi network protected by a random 10-character WPA2 password, shown on the Cardputer screen and in the Serial Monitor.
2. Join it, open `http://192.168.4.1`, pick your home Wi-Fi (optionally a private MQTT TLS broker and Root CA), and save. The board restarts onto your Wi-Fi.
3. **Pair in the Android app:**
   - **Cardputer:** once `TLS OK` appears, tap **Pair device** and scan the on-screen QR Code.
   - **Headless boards:** send `uri` in the Serial Monitor and paste the result into **Enter pairing code** (or **Pair with code** in the `☰` menu).

To reopen the setup portal later, hold **BOOT** for 3 seconds (`GPIO 0`, or `GPIO 9` on C3/C6), press **`P`** on the Cardputer, or send `portal`.

### Cardputer keys

| Key | Action |
| :--- | :--- |
| `Q` | Toggle QR Code / status panel (in portal mode, `Q` or `Del` restarts the board) |
| `Enter` | Re-send Wake-on-LAN to the last woken computer |
| `P` | Open the Wi-Fi setup portal with a new WPA2 password |
| `R` | Rotate `secret` + `bridge_id`, invalidating old QR Codes and URIs |

### Serial commands (`115200 baud`)

| Command | Action |
| :--- | :--- |
| `uri` | Print the current `oraculo://mcu-bridge?...` pairing URI |
| `status` | Show bridge name, `bridge_id`, broker, Wi-Fi IP, and MQTT TLS state |
| `portal` | Reopen the setup portal and print its new WPA2 password |
| `rotate` | Rotate `bridge_id` and `secret`, then print the new URI |
| `wake <AA:BB:CC:DD:EE:FF>` | Send Magic Packets locally |
| `factory-reset` | Erase saved settings and restart |

### Security

- Each board generates its own 64-bit `bridge_id` and 256-bit `secret` with the hardware TRNG, stored in NVS. No factory secrets.
- The pairing secret and MQTT password are never served over HTTP.
- MQTT runs only over TLS (`8883`) with X.509 verification after NTP sync. Defaults to `broker.emqx.io`; private brokers and custom Root CA are supported.
- Challenge-response with anti-replay: single-use `nonce` (`25s` TTL), `HMAC-SHA256` encrypted payload (target MACs never travel in plaintext), and authenticated `ACK`.

---

## Português (Brasil)

> **Status: experimental.** Este firmware teve cobertura limitada de testes em campo. Ele é opcional: o `oraculo-bridge` ou um celular Android sobrando fazem o mesmo trabalho.

Firmware Arduino em arquivo único (`oraculo_wol_esp32.ino`) que liga seu computador de fora da rede local. Roda no **M5Stack Cardputer** (tela, teclado e QR Code na tela) e em placas **ESP32 / C3 / S2 / S3** comuns (sem tela, com portal Wi-Fi de configuração e CLI serial a `115200 baud`).

### Como gravar (Arduino IDE 2.x)

1. Instale o suporte de placas **esp32** (*Espressif Systems*).
2. **Somente Cardputer:** instale a biblioteca **M5Cardputer** e selecione a placa **M5Cardputer** (ou descomente `#define ORACULO_CARDPUTER 1` no topo do `.ino` ao compilar para um ESP32-S3 genérico). Placas comuns não precisam de bibliotecas extras.
3. Abra `oraculo_wol_esp32.ino` e clique em **Upload**.

### Configurar e parear

1. Na primeira inicialização, a placa abre o Wi-Fi **`Oraculo-Ponte-XXXX`** protegido por senha WPA2 aleatória de 10 caracteres, exibida na tela do Cardputer e no Monitor Serial.
2. Conecte-se a ele, abra `http://192.168.4.1`, escolha o Wi-Fi de casa (opcionalmente um broker MQTT TLS privado e Root CA) e salve. A placa reinicia no seu Wi-Fi.
3. **Pareie no app Android:**
   - **Cardputer:** quando aparecer `TLS OK`, toque em **Parear dispositivo** e escaneie o QR Code da tela.
   - **Placas sem tela:** envie `uri` no Monitor Serial e cole o resultado em **Digitar código de pareamento** (ou **Parear com código** no menu `☰`).

Para reabrir o portal depois, segure **BOOT** por 3 segundos (`GPIO 0`, ou `GPIO 9` no C3/C6), pressione **`P`** no Cardputer ou envie `portal`.

### Teclas do Cardputer

| Tecla | Ação |
| :--- | :--- |
| `Q` | Alterna QR Code / painel de status (no modo portal, `Q` ou `Del` reinicia a placa) |
| `Enter` | Reenvia Wake-on-LAN ao último computador acordado |
| `P` | Abre o portal Wi-Fi de configuração com nova senha WPA2 |
| `R` | Rotaciona `secret` + `bridge_id`, invalidando QR Codes e URIs antigos |

### Comandos seriais (`115200 baud`)

| Comando | Ação |
| :--- | :--- |
| `uri` | Imprime a URI de pareamento `oraculo://mcu-bridge?...` atual |
| `status` | Exibe nome da ponte, `bridge_id`, broker, IP Wi-Fi e estado MQTT TLS |
| `portal` | Reabre o portal de configuração e imprime a nova senha WPA2 |
| `rotate` | Rotaciona `bridge_id` e `secret` e imprime a nova URI |
| `wake <AA:BB:CC:DD:EE:FF>` | Dispara Magic Packets localmente |
| `factory-reset` | Apaga as configurações salvas e reinicia |

### Segurança

- Cada placa gera seu próprio `bridge_id` (`64 bits`) e `secret` (`256 bits`) com o TRNG de hardware, gravados na NVS. Sem chaves de fábrica.
- O segredo de pareamento e a senha MQTT nunca são servidos por HTTP.
- MQTT roda somente sobre TLS (`8883`) com verificação X.509 após sincronização NTP. Usa `broker.emqx.io` por padrão; suporta brokers privados e Root CA própria.
- Challenge-response com anti-replay: `nonce` de uso único (TTL `25s`), payload cifrado com `HMAC-SHA256` (MACs de destino nunca trafegam em claro) e `ACK` autenticado.
