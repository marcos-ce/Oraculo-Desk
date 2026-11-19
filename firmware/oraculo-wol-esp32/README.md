# Oráculo Wake-on-LAN Bridge — Cardputer / ESP32 (`oraculo_wol_esp32.ino`)

[English](#english) · [Português (Brasil)](#português-brasil)

---

## English

Self-contained single-file `.ino` firmware for **M5Stack Cardputer** (display, keyboard, on-screen QR Code, and physical Wake key) and generic **ESP32 / ESP32-C3 / ESP32-S2 / ESP32-S3** boards (headless mode with local Wi-Fi setup portal and `115200 baud` Serial CLI).

### Flashing (Arduino IDE 2.x)

1. Install the **esp32** board package (*Espressif Systems*) in the Boards Manager.
2. **Additional libraries:**
   - **Generic ESP32 / ESP32-C3 / ESP32-S2 / ESP32-S3:** no external libraries are required (uses only built-in ESP32 Arduino core libraries).
   - **M5Stack Cardputer:** install the **M5Cardputer** library in Library Manager and select the **M5Cardputer** board (or uncomment `#define ORACULO_CARDPUTER 1` at the top of the `.ino` if compiling with a generic ESP32-S3 target).
3. Open `oraculo_wol_esp32.ino` and click **Upload**.

### Initial Setup (Wi-Fi & TLS Broker)

1. On first boot (with no saved Wi-Fi), the board starts the **`Oraculo-Ponte-XXXX`** SoftAP protected by a **random per-session 10-character WPA2 password** (excluding ambiguous characters `0`, `O`, `1`, `I`).
   - The WPA2 password is displayed on the **Cardputer screen** and printed to the **Serial Monitor (`115200 baud`)**.
   - To reopen the setup portal later: hold the **BOOT** button (`GPIO 0`, or `GPIO 9` on ESP32-C3/C6) for **3 seconds**, press **`P`** on Cardputer, or run **`portal`** in the Serial Monitor.
2. Connect your phone or computer to `Oraculo-Ponte-XXXX` using that WPA2 password, open `http://192.168.4.1`, select your home Wi-Fi (and optionally configure a private MQTT TLS broker, credentials, or custom Root CA PEM), and save.
3. For security, the HTTP setup portal **never exposes the pairing URI/secret**. After saving, the SoftAP closes automatically and the board restarts onto your home Wi-Fi.
4. **Pair with the Oráculo Android app:**
   - **M5Stack Cardputer:** once connected (`TLS OK`), scan the on-screen **QR Code** (`oraculo://mcu-bridge?...`) using **Pair device** in the Android app.
   - **Headless ESP32 boards:** copy the pairing URI printed on boot or by sending **`uri`** in the Serial Monitor (`115200 baud`), then paste it into **Code** / **Enter pairing code** (or **Pair with code** in the `☰` menu) in the Oráculo Android app.

### M5Stack Cardputer Shortcuts

- **`Q`**: Toggles between the QR Code screen and the status panel (IP, MQTT TLS broker, wake counter, battery, and last MAC). In portal mode (when Wi-Fi is already saved), **`Q`** or **`Del`** restarts the board to exit the portal.
- **`Enter`**: Re-sends local Wake-on-LAN to the last woken computer.
- **`P`**: Opens the Wi-Fi setup portal (`http://192.168.4.1`) with a new per-session WPA2 password.
- **`R`**: Rotates the cryptographic identity (`secret` + `bridge_id`), immediately invalidating previous QR Codes and URIs.

### Serial Console Commands (`115200 baud`)

- `uri` — Print the current `oraculo://mcu-bridge?...` pairing URI.
- `status` — Show bridge name, `bridge_id`, broker, Wi-Fi IP, and MQTT TLS status.
- `portal` — Reopen the Wi-Fi setup SoftAP and print its new per-session WPA2 password.
- `rotate` — Rotate `bridge_id` and `secret` and print the new pairing URI.
- `wake <AA:BB:CC:DD:EE:FF>` — Send local Wake-on-LAN Magic Packets immediately.
- `factory-reset` — Erase all saved NVS settings and restart.

### Embedded Security

- **No factory secrets:** each board generates a random `64`-bit `bridge_id` and `256`-bit `secret` via hardware TRNG (`esp_fill_random`) stored in NVS.
- **WPA2 setup portal without secret exposure:** `Oraculo-Ponte-XXXX` uses a fresh random 10-character WPA2 password on every portal session and never exposes the pairing secret or MQTT password over HTTP.
- **MQTT strictly over TLS (`8883`):** X.509 certificate verification after NTP clock synchronization (`setCACert` with built-in DigiCert Global Root G2, Amazon Root CA 1, ISRG Root X1/X2, and GTS Root R1, or an optional custom Root CA PEM). Defaults to `broker.emqx.io` and supports private brokers.
- **Challenge-Response + anti-replay:** every wake request uses a single-use random `nonce` (`25s` TTL), an `HMAC-SHA256` encrypted payload stream (target MACs never travel in plaintext through the broker), and an authenticated `ACK`.

---

## Português (Brasil)

Firmware autocontido em arquivo único (`.ino`), compatível com **M5Stack Cardputer** (display, teclado, QR Code na tela e tecla física Wake) e qualquer placa **ESP32 / ESP32-C3 / ESP32-S2 / ESP32-S3** (modo sem tela com portal Wi-Fi local e CLI serial a `115200 baud`).

### Como gravar (Arduino IDE 2.x)

1. Instale o suporte de placas **esp32** (*Espressif Systems*) no Gerenciador de Placas.
2. **Bibliotecas adicionais:**
   - **ESP32 / ESP32-C3 / ESP32-S2 / ESP32-S3 comum:** nenhuma biblioteca externa é necessária (usa apenas bibliotecas nativas do core ESP32).
   - **M5Stack Cardputer:** instale a biblioteca **M5Cardputer** no Gerenciador de Bibliotecas e selecione a placa **M5Cardputer** (ou descomente `#define ORACULO_CARDPUTER 1` no topo do `.ino` se estiver compilando com alvo genérico ESP32-S3).
3. Abra `oraculo_wol_esp32.ino` e envie (**Upload**).

### Configuração inicial (Wi-Fi e Broker TLS)

1. Na primeira inicialização (sem Wi-Fi salvo), a placa abre o ponto de acesso **`Oraculo-Ponte-XXXX`** protegido por uma **senha WPA2 aleatória de 10 caracteres por sessão** (sem caracteres ambíguos `0`, `O`, `1`, `I`).
   - A senha WPA2 é exibida na **tela do Cardputer** e impressa no **Monitor Serial (`115200 baud`)**.
   - Para reabrir o portal depois: segure o botão **BOOT** (`GPIO 0`, ou `GPIO 9` no ESP32-C3/C6) por **3 segundos**, pressione **`P`** no Cardputer ou envie **`portal`** no Monitor Serial.
2. Conecte o celular ou computador ao Wi-Fi `Oraculo-Ponte-XXXX` usando essa senha WPA2, abra `http://192.168.4.1`, selecione o Wi-Fi da sua casa (e, se quiser, configure um broker MQTT TLS privado, credenciais ou Root CA PEM própria) e salve.
3. Por segurança, o portal HTTP **não expõe a URI nem o segredo de pareamento**. Após salvar, o ponto de acesso fecha automaticamente e a placa reinicia conectando ao Wi-Fi da casa.
4. **Pareamento no aplicativo Oráculo (Android):**
   - **M5Stack Cardputer:** assim que conectar (`TLS OK`), escaneie o **QR Code** (`oraculo://mcu-bridge?...`) na tela tocando em **Parear dispositivo** no app Android.
   - **Placas ESP32 sem tela:** copie a URI exibida na inicialização ou executando **`uri`** no Monitor Serial (`115200 baud`) e cole-a em **Código** / **Digitar código de pareamento** (ou **Parear com código** no menu `☰`) no aplicativo Oráculo.

### Atalhos no M5Stack Cardputer

- **`Q`**: Alterna entre a tela de QR Code e o painel de status (IP, broker TLS, contador de sinais, bateria e último MAC). No modo portal (com Wi-Fi já salvo), **`Q`** ou **`Del`** reinicia a placa para sair do portal.
- **`Enter`**: Reenvia Wake-on-LAN localmente para o último computador acordado.
- **`P`**: Abre o portal Wi-Fi de configuração (`http://192.168.4.1`) com uma nova senha WPA2 por sessão.
- **`R`**: Rotaciona o segredo criptográfico (`secret` + `bridge_id`), invalidando QR Codes e URIs antigos na hora.

### Comandos do Console Serial (`115200 baud`)

- `uri` — Imprime a URI de pareamento atual (`oraculo://mcu-bridge?...`).
- `status` — Exibe nome da ponte, `bridge_id`, broker, IP Wi-Fi e estado MQTT TLS.
- `portal` — Reabre o SoftAP de configuração Wi-Fi e exibe a nova senha WPA2 da sessão.
- `rotate` — Rotaciona `bridge_id` e `secret` e imprime a nova URI de pareamento.
- `wake <AA:BB:CC:DD:EE:FF>` — Dispara Magic Packets Wake-on-LAN localmente na hora.
- `factory-reset` — Apaga as configurações salvas na NVS e reinicia a placa.

### Segurança embarcada

- **Zero chave fixa de fábrica:** cada placa gera `bridge_id` (`64 bits`) e `secret` (`256 bits`) aleatórios via TRNG de hardware (`esp_fill_random`) gravados na partição NVS.
- **Portal Wi-Fi com WPA2 e sem vazamento de segredos:** o ponto de acesso `Oraculo-Ponte-XXXX` gera uma senha WPA2 aleatória de 10 caracteres a cada sessão e nunca expõe o segredo de pareamento nem a senha MQTT via HTTP.
- **MQTT somente sobre TLS (`8883`):** verificação de certificado X.509 com sincronização NTP prévia (`setCACert` nativo para DigiCert Global Root G2, Amazon Root CA 1, ISRG Root X1/X2 e GTS Root R1, ou Root CA privada opcional). Usa `broker.emqx.io` por padrão e suporta brokers privados.
- **Challenge-Response + anti-replay:** todo disparo passa por desafio com `nonce` aleatório de uso único (TTL `25s`), payload cifrado por fluxo `HMAC-SHA256` (MACs do computador nunca trafegam em claro pelo broker) e confirmação autenticada (`ACK`).
