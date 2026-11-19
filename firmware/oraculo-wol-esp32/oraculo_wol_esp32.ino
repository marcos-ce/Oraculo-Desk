/*
 * Oráculo WoL Bridge Firmware (Single-file .ino for M5Stack Cardputer & ESP32)
 *
 * Compatible targets (Espressif esp32 Arduino Core 2.x / 3.x):
 *   - M5Stack Cardputer (ESP32-S3 + Display + Keyboard -> auto QR Code + physical Wake key)
 *   - ESP32, ESP32-C3, ESP32-S2, ESP32-S3 (Headless mode -> SoftAP Portal + Serial CLI)
 *
 * Security model:
 *   - MQTT strictly over TLS (default port 8883) with Root CA verification (zero setInsecure())
 *   - Hardware TRNG (esp_fill_random) 256-bit secret + 64-bit bridge ID stored in NVS
 *   - Mutual Challenge-Response HMAC-SHA256 + 25s single-use nonce anti-replay
 *   - Target PC MAC & subnet broadcast encrypted via HMAC-SHA256 stream + Encrypt-then-MAC
 *
 * Uncomment the line below only if compiling for M5Cardputer using a generic ESP32-S3 board:
 */
// #define ORACULO_CARDPUTER 1

#if defined(ORACULO_CARDPUTER) || defined(ARDUINO_M5STACK_CARDPUTER) || \
    defined(ARDUINO_M5Stack_CARDPUTER) || defined(ARDUINO_M5STACK_Cardputer) || \
    defined(ARDUINO_M5Stack_Cardputer)
#define ORACULO_HAS_CARDPUTER 1
#include <M5Cardputer.h>
#else
#define ORACULO_HAS_CARDPUTER 0
#endif

#include <DNSServer.h>
#include <Preferences.h>
#include <WebServer.h>
#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <WiFiUdp.h>
#include <esp_system.h>
#include <mbedtls/md.h>
#include <time.h>

#ifndef ORACULO_BOOT_PIN
#if defined(CONFIG_IDF_TARGET_ESP32C3) || defined(CONFIG_IDF_TARGET_ESP32C6)
#define ORACULO_BOOT_PIN 9
#else
#define ORACULO_BOOT_PIN 0
#endif
#endif

static const char* DEFAULT_BROKER_HOST = "broker.emqx.io";
static const uint16_t DEFAULT_BROKER_PORT = 8883;
static const char* DEFAULT_BRIDGE_NAME = ORACULO_HAS_CARDPUTER ? "Cardputer" : "ESP32-Ponte";
static const char* NVS_NAMESPACE = "oraculo-wol";
static const size_t MAX_MQTT_RX = 768;
static const size_t MAX_WOL_TARGETS = 6;
static const unsigned long CHALLENGE_TTL_MS = 25000UL;
static const unsigned long MQTT_KEEPALIVE_MS = 25000UL;
static const unsigned long MQTT_RX_TIMEOUT_MS = 65000UL;

// Verified Mozilla/CCADB Root CAs for EMQX Public, HiveMQ Public/Cloud, Let's Encrypt, and Google Trust Services.
static const char* ORACULO_DEFAULT_ROOT_CAS = R"PEM(
-----BEGIN CERTIFICATE-----
MIIDjjCCAnagAwIBAgIQAzrx5qcRqaC7KGSxHQn65TANBgkqhkiG9w0BAQsFADBh
MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
d3cuZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBH
MjAeFw0xMzA4MDExMjAwMDBaFw0zODAxMTUxMjAwMDBaMGExCzAJBgNVBAYTAlVT
MRUwEwYDVQQKEwxEaWdpQ2VydCBJbmMxGTAXBgNVBAsTEHd3dy5kaWdpY2VydC5j
b20xIDAeBgNVBAMTF0RpZ2lDZXJ0IEdsb2JhbCBSb290IEcyMIIBIjANBgkqhkiG
9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuzfNNNx7a8myaJCtSnX/RrohCgiN9RlUyfuI
2/Ou8jqJkTx65qsGGmvPrC3oXgkkRLpimn7Wo6h+4FR1IAWsULecYxpsMNzaHxmx
1x7e/dfgy5SDN67sH0NO3Xss0r0upS/kqbitOtSZpLYl6ZtrAGCSYP9PIUkY92eQ
q2EGnI/yuum06ZIya7XzV+hdG82MHauVBJVJ8zUtluNJbd134/tJS7SsVQepj5Wz
tCO7TG1F8PapspUwtP1MVYwnSlcUfIKdzXOS0xZKBgyMUNGPHgm+F6HmIcr9g+UQ
vIOlCsRnKPZzFBQ9RnbDhxSJITRNrw9FDKZJobq7nMWxM4MphQIDAQABo0IwQDAP
BgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBhjAdBgNVHQ4EFgQUTiJUIBiV
5uNu5g/6+rkS7QYXjzkwDQYJKoZIhvcNAQELBQADggEBAGBnKJRvDkhj6zHd6mcY
1Yl9PMWLSn/pvtsrF9+wX3N3KjITOYFnQoQj8kVnNeyIv/iPsGEMNKSuIEyExtv4
NeF22d+mQrvHRAiGfzZ0JFrabA0UWTW98kndth/Jsw1HKj2ZL7tcu7XUIOGZX1NG
Fdtom/DzMNU+MeKNhJ7jitralj41E6Vf8PlwUHBHQRFXGU7Aj64GxJUTFy8bJZ91
8rGOmaFvE7FBcf6IKshPECBV1/MUReXgRPTqh5Uykw7+U0b6LJ3/iyK5S9kJRaTe
pLiaWN0bfVKfjllDiIGknibVb63dDcY3fe0Dkhvld1927jyNxF1WW6LZZm6zNTfl
MrY=
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIIDQTCCAimgAwIBAgITBmyfz5m/jAo54vB4ikPmljZbyjANBgkqhkiG9w0BAQsF
ADA5MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6
b24gUm9vdCBDQSAxMB4XDTE1MDUyNjAwMDAwMFoXDTM4MDExNzAwMDAwMFowOTEL
MAkGA1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJv
b3QgQ0EgMTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALJ4gHHKeNXj
ca9HgFB0fW7Y14h29Jlo91ghYPl0hAEvrAIthtOgQ3pOsqTQNroBvo3bSMgHFzZM
9O6II8c+6zf1tRn4SWiw3te5djgdYZ6k/oI2peVKVuRF4fn9tBb6dNqcmzU5L/qw
IFAGbHrQgLKm+a/sRxmPUDgH3KKHOVj4utWp+UhnMJbulHheb4mjUcAwhmahRWa6
VOujw5H5SNz/0egwLX0tdHA114gk957EWW67c4cX8jJGKLhD+rcdqsq08p8kDi1L
93FcXmn/6pUCyziKrlA4b9v7LWIbxcceVOF34GfID5yHI9Y/QCB/IIDEgEw+OyQm
jgSubJrIqg0CAwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMC
AYYwHQYDVR0OBBYEFIQYzIU07LwMlJQuCFmcx7IQTgoIMA0GCSqGSIb3DQEBCwUA
A4IBAQCY8jdaQZChGsV2USggNiMOruYou6r4lK5IpDB/G/wkjUu0yKGX9rbxenDI
U5PMCCjjmCXPI6T53iHTfIUJrU6adTrCC2qJeHZERxhlbI1Bjjt/msv0tadQ1wUs
N+gDS63pYaACbvXy8MWy7Vu33PqUXHeeE6V/Uq2V8viTO96LXFvKWlJbYK8U90vv
o/ufQJVtMVT8QtPHRh8jrdkPSHCa2XV4cdFyQzR1bldZwgJcJmApzyMZFo6IQ6XU
5MsI+yMRQ+hDKXJioaldXgjUkK642M4UwtBV8ob2xJNDd2ZhwLnoQdeXeGADbkpy
rqXRfboQnoZsG4q5WTP468SQvvG5
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw
TzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh
cmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4
WhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu
ZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY
MTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc
h77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+
0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U
A5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW
T8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH
B5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC
B5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv
KBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn
OlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn
jh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw
qHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI
rU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV
HRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq
hkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL
ubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ
3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK
NFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5
ORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur
TkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC
jNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc
oyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq
4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA
mRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d
emyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIICGzCCAaGgAwIBAgIQQdKd0XLq7qeAwSxs6S+HUjAKBggqhkjOPQQDAzBPMQsw
CQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJuZXQgU2VjdXJpdHkgUmVzZWFyY2gg
R3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBYMjAeFw0yMDA5MDQwMDAwMDBaFw00
MDA5MTcxNjAwMDBaME8xCzAJBgNVBAYTAlVTMSkwJwYDVQQKEyBJbnRlcm5ldCBT
ZWN1cml0eSBSZXNlYXJjaCBHcm91cDEVMBMGA1UEAxMMSVNSRyBSb290IFgyMHYw
EAYHKoZIzj0CAQYFK4EEACIDYgAEzZvVn4CDCuwJSvMWSj5cz3es3mcFDR0HttwW
+1qLFNvicWDEukWVEYmO6gbf9yoWHKS5xcUy4APgHoIYOIvXRdgKam7mAHf7AlF9
ItgKbppbd9/w+kHsOdx1ymgHDB/qo0IwQDAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0T
AQH/BAUwAwEB/zAdBgNVHQ4EFgQUfEKWrt5LSDv6kviejM9ti6lyN5UwCgYIKoZI
zj0EAwMDaAAwZQIwe3lORlCEwkSHRhtFcP9Ymd70/aTSVaYgLXTWNLxBo1BfASdW
tL4ndQavEi51mI38AjEAi/V3bNTIZargCyzuFJ0nN6T5U6VR5CmD1/iQMVtCnwr1
/q4AaOeMSQ+2b1tbFfLn
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIIFVzCCAz+gAwIBAgINAgPlk28xsBNJiGuiFzANBgkqhkiG9w0BAQwFADBHMQsw
CQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2VzIExMQzEU
MBIGA1UEAxMLR1RTIFJvb3QgUjEwHhcNMTYwNjIyMDAwMDAwWhcNMzYwNjIyMDAw
MDAwWjBHMQswCQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZp
Y2VzIExMQzEUMBIGA1UEAxMLR1RTIFJvb3QgUjEwggIiMA0GCSqGSIb3DQEBAQUA
A4ICDwAwggIKAoICAQC2EQKLHuOhd5s73L+UPreVp0A8of2C+X0yBoJx9vaMf/vo
27xqLpeXo4xL+Sv2sfnOhB2x+cWX3u+58qPpvBKJXqeqUqv4IyfLpLGcY9vXmX7w
Cl7raKb0xlpHDU0QM+NOsROjyBhsS+z8CZDfnWQpJSMHobTSPS5g4M/SCYe7zUjw
TcLCeoiKu7rPWRnWr4+wB7CeMfGCwcDfLqZtbBkOtdh+JhpFAz2weaSUKK0Pfybl
qAj+lug8aJRT7oM6iCsVlgmy4HqMLnXWnOunVmSPlk9orj2XwoSPwLxAwAtcvfaH
szVsrBhQf4TgTM2S0yDpM7xSma8ytSmzJSq0SPly4cpk9+aCEI3oncKKiPo4Zor8
Y/kB+Xj9e1x3+naH+uzfsQ55lVe0vSbv1gHR6xYKu44LtcXFilWr06zqkUspzBmk
MiVOKvFlRNACzqrOSbTqn3yDsEB750Orp2yjj32JgfpMpf/VjsPOS+C12LOORc92
wO1AK/1TD7Cn1TsNsYqiA94xrcx36m97PtbfkSIS5r762DL8EGMUUXLeXdYWk70p
aDPvOmbsB4om3xPXV2V4J95eSRQAogB/mqghtqmxlbCluQ0WEdrHbEg8QOB+DVrN
VjzRlwW5y0vtOUucxD/SVRNuJLDWcfr0wbrM7Rv1/oFB2ACYPTrIrnqYNxgFlQID
AQABo0IwQDAOBgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4E
FgQU5K8rJnEaK0gnhS9SZizv8IkTcT4wDQYJKoZIhvcNAQEMBQADggIBAJ+qQibb
C5u+/x6Wki4+omVKapi6Ist9wTrYggoGxval3sBOh2Z5ofmmWJyq+bXmYOfg6LEe
QkEzCzc9zolwFcq1JKjPa7XSQCGYzyI0zzvFIoTgxQ6KfF2I5DUkzps+GlQebtuy
h6f88/qBVRRiClmpIgUxPoLW7ttXNLwzldMXG+gnoot7TiYaelpkttGsN/H9oPM4
7HLwEXWdyzRSjeZ2axfG34arJ45JK3VmgRAhpuo+9K4l/3wV3s6MJT/KYnAK9y8J
ZgfIPxz88NtFMN9iiMG1D53Dn0reWVlHxYciNuaCp+0KueIHoI17eko8cdLiA6Ef
MgfdG+RCzgwARWGAtQsgWSl4vflVy2PFPEz0tv/bal8xa5meLMFrUKTX5hgUvYU/
Z6tGn6D/Qqc6f1zLXbBwHSs09dR2CQzreExZBfMzQsNhFRAbd03OIozUhfJFfbdT
6u9AWpQKXCBfTkBdYiJ23//OYb2MI3jSNwLgjt7RETeJ9r/tSQdirpLsQBqvFAnZ
0E6yove+7u7Y/9waLd64NnHi/Hm3lCXRSHNboTXns5lndcEZOitHTtNCjv0xyBZm
2tIMPNuzjsmhDYAPexZ3FL//2wmUspO8IFgV6dtxQ/PeEMMA3KgqlbbC1j+Qa3bb
bP6MvPJwNQzcmRk13NfIRmPVNnGuV/u3gm3c
-----END CERTIFICATE-----
)PEM";

struct PendingChallenge {
  bool active;
  char nonceHex[33];
  unsigned long expiresMs;
};

static PendingChallenge gChallenges[8];

Preferences gPrefs;
WiFiClientSecure gTlsClient;
WiFiUDP gUdp;
WebServer gWeb(80);
DNSServer gDns;

String gWifiSsid;
String gWifiPass;
String gBrokerHost;
uint16_t gBrokerPort = DEFAULT_BROKER_PORT;
String gMqttUser;
String gMqttPass;
String gCustomCa;
String gBridgeName;
String gBridgeId;
uint8_t gSecret[32];
String gLastWokenMacs;

String gCmdTopic;
String gEvtTopic;

bool gPortalMode = false;
String gPortalApPass;
bool gMqttConnected = false;
bool gShowQrScreen = true;
bool gNeedsRedraw = true;
uint32_t gWakeCount = 0;
String gStatusNotice;

unsigned long gNextMqttRetryMs = 0;
unsigned long gMqttBackoffMs = 1500;
unsigned long gLastPingSentMs = 0;
unsigned long gLastMqttRxMs = 0;
unsigned long gLastChalCreatedMs = 0;
unsigned long gBootBtnDownMs = 0;
unsigned long gLastDisplayTickMs = 0;
unsigned long gLastWifiRetryMs = 0;

// -------------------- Low-level utilities --------------------

static void secureZero(void* ptr, size_t len) {
  volatile uint8_t* p = reinterpret_cast<volatile uint8_t*>(ptr);
  while (len--) {
    *p++ = 0;
  }
}

static bool ctEqual32(const uint8_t a[32], const uint8_t b[32]) {
  volatile uint8_t diff = 0;
  for (int i = 0; i < 32; ++i) {
    diff |= (a[i] ^ b[i]);
  }
  return diff == 0;
}

static String toLowerHex(const uint8_t* data, size_t len) {
  static const char* kHex = "0123456789abcdef";
  String out;
  out.reserve(len * 2);
  for (size_t i = 0; i < len; ++i) {
    out += kHex[(data[i] >> 4) & 0x0F];
    out += kHex[data[i] & 0x0F];
  }
  return out;
}

static int hexNibble(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

static bool fromHex(const String& hex, uint8_t* out, size_t expectedBytes) {
  if (hex.length() != expectedBytes * 2) return false;
  for (size_t i = 0; i < expectedBytes; ++i) {
    int hi = hexNibble(hex[i * 2]);
    int lo = hexNibble(hex[i * 2 + 1]);
    if (hi < 0 || lo < 0) return false;
    out[i] = static_cast<uint8_t>((hi << 4) | lo);
  }
  return true;
}

static bool decodeVarHex(const String& hex, uint8_t* out, size_t maxOut, size_t* outLen) {
  size_t len = hex.length();
  if (len == 0 || (len % 2) != 0 || (len / 2) > maxOut) return false;
  for (size_t i = 0; i < len / 2; ++i) {
    int hi = hexNibble(hex[i * 2]);
    int lo = hexNibble(hex[i * 2 + 1]);
    if (hi < 0 || lo < 0) return false;
    out[i] = static_cast<uint8_t>((hi << 4) | lo);
  }
  *outLen = len / 2;
  return true;
}

static String encodeBase64UrlNoPad(const uint8_t* data, size_t len) {
  static const char* kTbl =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  String out;
  out.reserve(((len * 4) + 2) / 3);
  for (size_t i = 0; i < len; i += 3) {
    uint32_t b0 = data[i];
    uint32_t b1 = (i + 1 < len) ? data[i + 1] : 0;
    uint32_t b2 = (i + 2 < len) ? data[i + 2] : 0;
    uint32_t triple = (b0 << 16) | (b1 << 8) | b2;
    out += kTbl[(triple >> 18) & 0x3F];
    out += kTbl[(triple >> 12) & 0x3F];
    if (i + 1 < len) out += kTbl[(triple >> 6) & 0x3F];
    if (i + 2 < len) out += kTbl[triple & 0x3F];
  }
  return out;
}

static String percentEncode(const String& input) {
  static const char* kHex = "0123456789ABCDEF";
  String out;
  out.reserve(input.length());
  for (size_t i = 0; i < input.length(); ++i) {
    uint8_t b = static_cast<uint8_t>(input[i]);
    bool unreserved =
        (b >= 'A' && b <= 'Z') || (b >= 'a' && b <= 'z') || (b >= '0' && b <= '9') ||
        b == '-' || b == '_' || b == '.' || b == '~';
    if (unreserved) {
      out += static_cast<char>(b);
    } else {
      out += '%';
      out += kHex[(b >> 4) & 0x0F];
      out += kHex[b & 0x0F];
    }
  }
  return out;
}

static String htmlEscape(const String& s) {
  String out;
  out.reserve(s.length());
  for (size_t i = 0; i < s.length(); ++i) {
    char c = s[i];
    if (c == '&') out += F("&amp;");
    else if (c == '<') out += F("&lt;");
    else if (c == '>') out += F("&gt;");
    else if (c == '"') out += F("&quot;");
    else if (c == '\'') out += F("&#39;");
    else out += c;
  }
  return out;
}

static String jsonEscape(const String& s) {
  String out;
  out.reserve(s.length());
  for (size_t i = 0; i < s.length(); ++i) {
    uint8_t c = static_cast<uint8_t>(s[i]);
    if (c < 0x20) continue;
    if (c == '"' || c == '\\') {
      out += '\\';
    }
    out += static_cast<char>(c);
  }
  return out;
}

static String sanitizeName(const String& raw) {
  String out;
  for (size_t i = 0; i < raw.length() && out.length() < 48; ++i) {
    uint8_t c = static_cast<uint8_t>(raw[i]);
    if (c >= 0x20 && c != 0x7F) out += static_cast<char>(c);
  }
  out.trim();
  if (out.isEmpty()) return String(DEFAULT_BRIDGE_NAME);
  return out;
}

static bool jsonGetString(const String& json, const char* key, String& out) {
  String pattern = "\"";
  pattern += key;
  pattern += "\"";
  int keyPos = json.indexOf(pattern);
  if (keyPos < 0) return false;
  int colon = json.indexOf(':', keyPos + pattern.length());
  if (colon < 0) return false;
  int pos = colon + 1;
  while (pos < static_cast<int>(json.length()) &&
         (json[pos] == ' ' || json[pos] == '\t' || json[pos] == '\r' || json[pos] == '\n')) {
    pos++;
  }
  if (pos >= static_cast<int>(json.length()) || json[pos] != '"') return false;
  pos++;
  out = "";
  while (pos < static_cast<int>(json.length())) {
    char c = json[pos++];
    if (c == '\\' && pos < static_cast<int>(json.length())) {
      out += json[pos++];
    } else if (c == '"') {
      return true;
    } else {
      out += c;
    }
  }
  return false;
}

static bool hmacSha256Raw(const uint8_t secret[32], const uint8_t* msg, size_t len, uint8_t out[32]) {
  mbedtls_md_context_t ctx;
  mbedtls_md_init(&ctx);
  const mbedtls_md_info_t* info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA256);
  bool ok = (info != nullptr &&
             mbedtls_md_setup(&ctx, info, 1) == 0 &&
             mbedtls_md_hmac_starts(&ctx, secret, 32) == 0 &&
             mbedtls_md_hmac_update(&ctx, msg, len) == 0 &&
             mbedtls_md_hmac_finish(&ctx, out) == 0);
  mbedtls_md_free(&ctx);
  return ok;
}

static bool hmacSha256Hex(const String& msg, String& outHex) {
  uint8_t mac[32];
  if (!hmacSha256Raw(gSecret, reinterpret_cast<const uint8_t*>(msg.c_str()), msg.length(), mac)) {
    secureZero(mac, sizeof(mac));
    return false;
  }
  outHex = toLowerHex(mac, 32);
  secureZero(mac, sizeof(mac));
  return true;
}

static bool decryptKeystream(const String& nonceHex, const uint8_t* ct, size_t len, String& outPlain) {
  outPlain = "";
  outPlain.reserve(len);
  uint8_t pad[32];
  for (size_t offset = 0, blockIdx = 0; offset < len; offset += 32, ++blockIdx) {
    String label = "oraculo-mcu-v1|stream|";
    label += nonceHex;
    label += '|';
    label += String(static_cast<unsigned long>(blockIdx));
    if (!hmacSha256Raw(gSecret, reinterpret_cast<const uint8_t*>(label.c_str()), label.length(), pad)) {
      secureZero(pad, sizeof(pad));
      return false;
    }
    size_t chunk = (len - offset < 32) ? (len - offset) : 32;
    for (size_t j = 0; j < chunk; ++j) {
      outPlain += static_cast<char>(ct[offset + j] ^ pad[j]);
    }
  }
  secureZero(pad, sizeof(pad));
  return true;
}

// -------------------- Identity & NVS persistence --------------------

static String buildPairingUri() {
  String uri = "oraculo://mcu-bridge?id=";
  uri += gBridgeId;
  uri += "&s=";
  uri += encodeBase64UrlNoPad(gSecret, 32);
  uri += "&b=";
  uri += percentEncode(gBrokerHost);
  uri += "&n=";
  uri += percentEncode(gBridgeName);
  if (gBrokerPort != DEFAULT_BROKER_PORT) {
    uri += "&p=";
    uri += String(gBrokerPort);
  }
  if (!gMqttUser.isEmpty()) {
    uri += "&u=";
    uri += percentEncode(gMqttUser);
    if (!gMqttPass.isEmpty()) {
      uri += "&pw=";
      uri += percentEncode(gMqttPass);
    }
  }
  return uri;
}

static void generateFreshIdentity() {
  uint8_t idRaw[8];
  esp_fill_random(idRaw, sizeof(idRaw));
  esp_fill_random(gSecret, sizeof(gSecret));
  gBridgeId = toLowerHex(idRaw, sizeof(idRaw));
  gPrefs.begin(NVS_NAMESPACE, false);
  gPrefs.putString("bridge_id", gBridgeId);
  gPrefs.putBytes("secret", gSecret, sizeof(gSecret));
  gPrefs.end();
  secureZero(idRaw, sizeof(idRaw));

  gCmdTopic = "oraculo/v1/mcu/" + gBridgeId + "/cmd";
  gEvtTopic = "oraculo/v1/mcu/" + gBridgeId + "/evt";
  memset(gChallenges, 0, sizeof(gChallenges));
}

static void loadConfig() {
  gPrefs.begin(NVS_NAMESPACE, false);
  gWifiSsid = gPrefs.getString("wifi_ssid", "");
  gWifiPass = gPrefs.getString("wifi_pass", "");
  gBrokerHost = gPrefs.getString("broker_host", DEFAULT_BROKER_HOST);
  if (gBrokerHost.isEmpty()) gBrokerHost = DEFAULT_BROKER_HOST;
  gBrokerPort = gPrefs.getUShort("broker_port", DEFAULT_BROKER_PORT);
  if (gBrokerPort == 0) gBrokerPort = DEFAULT_BROKER_PORT;
  gMqttUser = gPrefs.getString("mqtt_user", "");
  gMqttPass = gPrefs.getString("mqtt_pass", "");
  gCustomCa = gPrefs.getString("custom_ca", "");
  gBridgeName = sanitizeName(gPrefs.getString("name", DEFAULT_BRIDGE_NAME));
  gBridgeId = gPrefs.getString("bridge_id", "");
  gLastWokenMacs = gPrefs.getString("last_macs", "");

  bool needIdentity = (gBridgeId.length() < 16);
  if (!needIdentity) {
    size_t got = gPrefs.getBytes("secret", gSecret, sizeof(gSecret));
    if (got != sizeof(gSecret)) needIdentity = true;
  }
  gPrefs.end();

  if (needIdentity) {
    generateFreshIdentity();
  } else {
    gCmdTopic = "oraculo/v1/mcu/" + gBridgeId + "/cmd";
    gEvtTopic = "oraculo/v1/mcu/" + gBridgeId + "/evt";
  }
}

// -------------------- Wake-on-LAN UDP engine --------------------

static bool parseMacBytes(const String& raw, uint8_t out[6]) {
  String clean;
  for (size_t i = 0; i < raw.length(); ++i) {
    char c = raw[i];
    if (c == ':' || c == '-' || c == '.' || c == ' ') continue;
    clean += c;
  }
  if (clean.length() != 12) return false;
  if (!fromHex(clean, out, 6)) return false;
  bool allZero = true, allFF = true;
  for (int i = 0; i < 6; ++i) {
    if (out[i] != 0x00) allZero = false;
    if (out[i] != 0xFF) allFF = false;
  }
  return !(allZero || allFF);
}

static bool isSafePrivateLanV4(const IPAddress& ip) {
  uint8_t a = ip[0], b = ip[1];
  if (a == 10) return true;
  if (a == 172 && b >= 16 && b <= 31) return true;
  if (a == 192 && b == 168) return true;
  if (a == 100 && b >= 64 && b <= 127) return true;
  if (a == 255 && b == 255 && ip[2] == 255 && ip[3] == 255) return true;
  return false;
}

static uint32_t sendMagicPacketsCsv(const String& macCsv, const String& optBroadcast, uint16_t port) {
  if (WiFi.status() != WL_CONNECTED) return 0;
  uint16_t effectivePort = (port == 0) ? 9 : port;

  uint8_t macs[MAX_WOL_TARGETS][6];
  size_t macCount = 0;
  int start = 0;
  while (start < static_cast<int>(macCsv.length()) && macCount < MAX_WOL_TARGETS) {
    int comma = macCsv.indexOf(',', start);
    String token = (comma < 0) ? macCsv.substring(start) : macCsv.substring(start, comma);
    token.trim();
    if (parseMacBytes(token, macs[macCount])) {
      macCount++;
    }
    if (comma < 0) break;
    start = comma + 1;
  }
  if (macCount == 0) return 0;

  IPAddress targets[4];
  uint16_t targetPorts[4];
  size_t tCount = 0;

  auto addTarget = [&](const IPAddress& ip, uint16_t p) {
    for (size_t i = 0; i < tCount; ++i) {
      if (targets[i] == ip && targetPorts[i] == p) return;
    }
    if (tCount < 4) {
      targets[tCount] = ip;
      targetPorts[tCount] = p;
      tCount++;
    }
  };

  IPAddress lip = WiFi.localIP();
  IPAddress smask = WiFi.subnetMask();
  IPAddress localBroadcast(
      static_cast<uint8_t>(lip[0] | (~smask[0] & 0xFF)),
      static_cast<uint8_t>(lip[1] | (~smask[1] & 0xFF)),
      static_cast<uint8_t>(lip[2] | (~smask[2] & 0xFF)),
      static_cast<uint8_t>(lip[3] | (~smask[3] & 0xFF)));
  addTarget(localBroadcast, effectivePort);
  if (effectivePort != 7) addTarget(localBroadcast, 7);

  if (!optBroadcast.isEmpty() && optBroadcast != "-") {
    IPAddress reqBc;
    if (reqBc.fromString(optBroadcast) && isSafePrivateLanV4(reqBc)) {
      addTarget(reqBc, effectivePort);
    }
  }

  addTarget(IPAddress(255, 255, 255, 255), effectivePort);

  uint32_t sentTotal = 0;
  uint8_t pkt[102];
  for (int burst = 0; burst < 3; ++burst) {
    if (burst > 0) delay(25);
    for (size_t m = 0; m < macCount; ++m) {
      memset(pkt, 0xFF, 6);
      for (int r = 0; r < 16; ++r) {
        memcpy(pkt + 6 + r * 6, macs[m], 6);
      }
      for (size_t t = 0; t < tCount; ++t) {
        if (gUdp.beginPacket(targets[t], targetPorts[t]) == 1) {
          gUdp.write(pkt, sizeof(pkt));
          if (gUdp.endPacket() == 1) {
            sentTotal++;
          }
        }
      }
    }
  }

  if (sentTotal > 0) {
    gWakeCount++;
    gLastWokenMacs = macCsv;
    gPrefs.begin(NVS_NAMESPACE, false);
    gPrefs.putString("last_macs", gLastWokenMacs);
    gPrefs.end();
    gStatusNotice = "WoL enviado (" + String(sentTotal) + "x)";
    gNeedsRedraw = true;
  }
  return sentTotal;
}

// -------------------- MQTT 3.1.1 TLS Engine --------------------

static void appendMqttU16(uint8_t* buf, size_t& pos, uint16_t v) {
  buf[pos++] = static_cast<uint8_t>((v >> 8) & 0xFF);
  buf[pos++] = static_cast<uint8_t>(v & 0xFF);
}

static void appendMqttStr(uint8_t* buf, size_t& pos, const String& s) {
  uint16_t len = static_cast<uint16_t>(s.length());
  appendMqttU16(buf, pos, len);
  memcpy(buf + pos, s.c_str(), len);
  pos += len;
}

static bool mqttSendPacket(uint8_t header, const uint8_t* body, size_t bodyLen) {
  if (!gTlsClient.connected()) return false;
  uint8_t prefix[5];
  size_t pLen = 0;
  prefix[pLen++] = header;
  size_t rem = bodyLen;
  do {
    uint8_t d = static_cast<uint8_t>(rem % 128);
    rem /= 128;
    if (rem > 0) d |= 0x80;
    prefix[pLen++] = d;
  } while (rem > 0 && pLen < sizeof(prefix));

  if (gTlsClient.write(prefix, pLen) != pLen) return false;
  if (bodyLen > 0 && gTlsClient.write(body, bodyLen) != bodyLen) return false;
  gTlsClient.flush();
  return true;
}

static bool mqttPublish(const String& topic, const String& payload) {
  size_t bodyLen = 2 + topic.length() + payload.length();
  if (bodyLen > 1024) return false;
  uint8_t body[1024];
  size_t pos = 0;
  appendMqttStr(body, pos, topic);
  memcpy(body + pos, payload.c_str(), payload.length());
  pos += payload.length();
  return mqttSendPacket(0x30, body, pos);
}

static bool readExactWithDeadline(uint8_t* dst, size_t n, unsigned long timeoutMs) {
  size_t got = 0;
  unsigned long deadline = millis() + timeoutMs;
  while (got < n) {
    if (!gTlsClient.connected() && gTlsClient.available() == 0) return false;
    int avail = gTlsClient.available();
    if (avail > 0) {
      int r = gTlsClient.read(dst + got, n - got);
      if (r > 0) got += static_cast<size_t>(r);
    } else {
      if (static_cast<long>(millis() - deadline) >= 0) return false;
      delay(2);
    }
  }
  return true;
}

static bool mqttReadPacket(uint8_t& outType, uint8_t& outFlags, uint8_t* outBody, size_t maxBody, size_t& outLen) {
  uint8_t hdr = 0;
  if (!readExactWithDeadline(&hdr, 1, 1500)) return false;
  outType = hdr >> 4;
  outFlags = hdr & 0x0F;

  size_t remLen = 0;
  size_t mult = 1;
  uint8_t count = 0;
  while (true) {
    if (count >= 4) return false;
    uint8_t b = 0;
    if (!readExactWithDeadline(&b, 1, 1000)) return false;
    remLen += static_cast<size_t>(b & 0x7F) * mult;
    if (remLen > 65536U) return false;
    count++;
    if ((b & 0x80) == 0) break;
    mult *= 128;
  }

  if (remLen > maxBody) {
    // Drena e descarta pacotes retidos/excedentes no broker público sem derrubar a sessão TLS.
    uint8_t scratch[128];
    size_t left = remLen;
    while (left > 0) {
      size_t chunk = (left > sizeof(scratch)) ? sizeof(scratch) : left;
      if (!readExactWithDeadline(scratch, chunk, 2000)) return false;
      left -= chunk;
    }
    outType = 0;
    outFlags = 0;
    outLen = 0;
    return true;
  }

  outLen = remLen;
  if (remLen > 0 && !readExactWithDeadline(outBody, remLen, 2000)) return false;
  return true;
}

static bool ensureNtpClockSynced() {
  time_t now = time(nullptr);
  if (now >= 1700000000L) return true;
  configTime(0, 0, "pool.ntp.org", "time.cloudflare.com", "time.google.com");
  unsigned long deadline = millis() + 4500UL;
  while (static_cast<long>(millis() - deadline) < 0) {
    now = time(nullptr);
    if (now >= 1700000000L) return true;
    delay(100);
  }
  return time(nullptr) >= 1700000000L;
}

static bool connectMqttBroker() {
  if (WiFi.status() != WL_CONNECTED) return false;
  if (!ensureNtpClockSynced()) {
    gStatusNotice = "Aguardando relógio NTP";
    gNeedsRedraw = true;
    return false;
  }

  gTlsClient.stop();
  if (!gCustomCa.isEmpty()) {
    gTlsClient.setCACert(gCustomCa.c_str());
  } else {
    gTlsClient.setCACert(ORACULO_DEFAULT_ROOT_CAS);
  }
  gTlsClient.setTimeout(5);

  if (!gTlsClient.connect(gBrokerHost.c_str(), gBrokerPort)) {
    gStatusNotice = "Falha TLS MQTT";
    gNeedsRedraw = true;
    return false;
  }

  uint8_t cidRand[4];
  esp_fill_random(cidRand, sizeof(cidRand));
  String clientId = "ora-mcu-" + toLowerHex(cidRand, sizeof(cidRand));

  uint8_t connectBody[512];
  size_t pos = 0;
  appendMqttStr(connectBody, pos, "MQTT");
  connectBody[pos++] = 0x04;
  uint8_t cFlags = 0x02;  // Clean Session
  // [MQTT-3.1.2-22]: Password Flag exige User Name Flag ativo.
  const bool hasUser = !gMqttUser.isEmpty();
  const bool hasPass = hasUser && !gMqttPass.isEmpty();
  if (hasUser) cFlags |= 0x80;
  if (hasPass) cFlags |= 0x40;
  connectBody[pos++] = cFlags;
  appendMqttU16(connectBody, pos, 30);
  if (pos + 2 + clientId.length() + (hasUser ? gMqttUser.length() : 0) + (hasPass ? gMqttPass.length() : 0) + 4 > sizeof(connectBody)) {
    gTlsClient.stop();
    return false;
  }
  appendMqttStr(connectBody, pos, clientId);
  if (hasUser) appendMqttStr(connectBody, pos, gMqttUser);
  if (hasPass) appendMqttStr(connectBody, pos, gMqttPass);

  if (!mqttSendPacket(0x10, connectBody, pos)) {
    gTlsClient.stop();
    return false;
  }

  uint8_t rxType = 0, rxFlags = 0;
  uint8_t rxBuf[MAX_MQTT_RX];
  size_t rxLen = 0;
  if (!mqttReadPacket(rxType, rxFlags, rxBuf, sizeof(rxBuf), rxLen) || rxType != 2 || rxLen < 2 || rxBuf[1] != 0) {
    gTlsClient.stop();
    gStatusNotice = "CONNACK recusado";
    gNeedsRedraw = true;
    return false;
  }

  uint8_t subBody[128];
  size_t subPos = 0;
  appendMqttU16(subBody, subPos, 1);
  appendMqttStr(subBody, subPos, gCmdTopic);
  subBody[subPos++] = 0x01;  // QoS 1
  if (!mqttSendPacket(0x82, subBody, subPos)) {
    gTlsClient.stop();
    return false;
  }

  // Aguarda SUBACK em loop para tolerar mensagens PUBLISH retained entregues antes do SUBACK.
  bool gotSuback = false;
  for (int tries = 0; tries < 16; ++tries) {
    if (!mqttReadPacket(rxType, rxFlags, rxBuf, sizeof(rxBuf), rxLen)) {
      gTlsClient.stop();
      return false;
    }
    if (rxType == 9) {
      if (rxLen < 3 || rxBuf[2] == 0x80) {
        gTlsClient.stop();
        return false;
      }
      gotSuback = true;
      break;
    }
  }
  if (!gotSuback) {
    gTlsClient.stop();
    return false;
  }

  gMqttConnected = true;
  gMqttBackoffMs = 1500;
  gLastPingSentMs = millis();
  gLastMqttRxMs = millis();
  gStatusNotice = "MQTT TLS conectado";
  gNeedsRedraw = true;
  return true;
}

// -------------------- Challenge-Response Handler --------------------

static bool allocateChallengeSlot(char outNonceHex[33]) {
  unsigned long now = millis();
  int freeIdx = -1;
  int oldestIdx = 0;
  unsigned long oldestExp = gChallenges[0].expiresMs;

  for (int i = 0; i < 8; ++i) {
    if (!gChallenges[i].active || static_cast<long>(now - gChallenges[i].expiresMs) >= 0) {
      freeIdx = i;
      break;
    }
    if (static_cast<long>(gChallenges[i].expiresMs - oldestExp) < 0) {
      oldestExp = gChallenges[i].expiresMs;
      oldestIdx = i;
    }
  }

  if (freeIdx < 0) {
    if (now - gLastChalCreatedMs < 250UL) {
      return false;
    }
    freeIdx = oldestIdx;
  }

  uint8_t rnd[16];
  esp_fill_random(rnd, sizeof(rnd));
  String nonceStr = toLowerHex(rnd, sizeof(rnd));
  secureZero(rnd, sizeof(rnd));

  gChallenges[freeIdx].active = true;
  memcpy(gChallenges[freeIdx].nonceHex, nonceStr.c_str(), 32);
  gChallenges[freeIdx].nonceHex[32] = '\0';
  gChallenges[freeIdx].expiresMs = now + CHALLENGE_TTL_MS;
  gLastChalCreatedMs = now;
  memcpy(outNonceHex, gChallenges[freeIdx].nonceHex, 33);
  return true;
}

static int findActiveChallenge(const String& nonceHex) {
  if (nonceHex.length() != 32) return -1;
  unsigned long now = millis();
  for (int i = 0; i < 8; ++i) {
    if (!gChallenges[i].active) continue;
    if (static_cast<long>(now - gChallenges[i].expiresMs) >= 0) {
      gChallenges[i].active = false;
      continue;
    }
    if (memcmp(gChallenges[i].nonceHex, nonceHex.c_str(), 32) == 0) {
      return i;
    }
  }
  return -1;
}

static void handleIncomingCommandJson(const String& json) {
  String op, req;
  if (!jsonGetString(json, "op", op) || !jsonGetString(json, "req", req)) return;
  if (req.length() != 16) return;
  for (size_t i = 0; i < req.length(); ++i) {
    if (hexNibble(req[i]) < 0) return;
  }

  if (op == "chal") {
    char nonceHex[33];
    if (!allocateChallengeSlot(nonceHex)) return;
    String sigInput = "oraculo-mcu-v1|chal|" + gBridgeId + "|" + req + "|" + String(nonceHex);
    String sigHex;
    if (!hmacSha256Hex(sigInput, sigHex)) return;

    String resp = "{\"op\":\"chal\",\"req\":\"";
    resp += req;
    resp += "\",\"nonce\":\"";
    resp += String(nonceHex);
    resp += "\",\"sig\":\"";
    resp += sigHex;
    resp += "\",\"name\":\"";
    resp += jsonEscape(gBridgeName);
    resp += "\"}";
    mqttPublish(gEvtTopic, resp);
    return;
  }

  if (op == "wake") {
    String nonceHex, ctHex, sigHex;
    if (!jsonGetString(json, "nonce", nonceHex) ||
        !jsonGetString(json, "ct", ctHex) ||
        !jsonGetString(json, "sig", sigHex)) {
      return;
    }

    int slot = findActiveChallenge(nonceHex);
    if (slot < 0) return;

    uint8_t sigBytes[32];
    if (!fromHex(sigHex, sigBytes, 32)) return;

    String verifyInput = "oraculo-mcu-v1|wake|" + gBridgeId + "|" + req + "|" + nonceHex + "|" + ctHex;
    uint8_t expectedSig[32];
    bool hmacOk = hmacSha256Raw(
        gSecret,
        reinterpret_cast<const uint8_t*>(verifyInput.c_str()),
        verifyInput.length(),
        expectedSig);
    bool valid = hmacOk && ctEqual32(sigBytes, expectedSig);
    secureZero(expectedSig, sizeof(expectedSig));
    secureZero(sigBytes, sizeof(sigBytes));
    if (!valid) return;

    // Consuma o nonce imediatamente após validação HMAC bem-sucedida (anti-replay estrito).
    gChallenges[slot].active = false;
    secureZero(gChallenges[slot].nonceHex, sizeof(gChallenges[slot].nonceHex));

    uint8_t ctBytes[256];
    size_t ctLen = 0;
    if (!decodeVarHex(ctHex, ctBytes, sizeof(ctBytes), &ctLen)) return;

    String plain;
    if (!decryptKeystream(nonceHex, ctBytes, ctLen, plain)) return;
    secureZero(ctBytes, sizeof(ctBytes));

    // Formato: <port>|<broadcast_ou_hifen>|<MAC1,MAC2,...>
    int sep1 = plain.indexOf('|');
    int sep2 = (sep1 >= 0) ? plain.indexOf('|', sep1 + 1) : -1;
    if (sep1 <= 0 || sep2 <= sep1 + 1) return;

    int parsedPort = plain.substring(0, sep1).toInt();
    uint16_t port = (parsedPort >= 1 && parsedPort <= 65535) ? static_cast<uint16_t>(parsedPort) : 9;
    String bc = plain.substring(sep1 + 1, sep2);
    String macCsv = plain.substring(sep2 + 1);

    uint32_t sent = sendMagicPacketsCsv(macCsv, bc, port);
    bool ok = (sent > 0);
    String ackInput = "oraculo-mcu-v1|ack|" + gBridgeId + "|" + req + "|" + nonceHex + "|" + String(sent);
    String ackSig;
    if (!hmacSha256Hex(ackInput, ackSig)) return;

    String ack = "{\"op\":\"ack\",\"req\":\"";
    ack += req;
    ack += "\",\"nonce\":\"";
    ack += nonceHex;
    ack += "\",\"ok\":";
    ack += (ok ? "true" : "false");
    ack += ",\"sent\":";
    ack += String(sent);
    ack += ",\"sig\":\"";
    ack += ackSig;
    ack += "\"}";
    mqttPublish(gEvtTopic, ack);
  }
}

static void pollMqttLoop() {
  if (!gMqttConnected || !gTlsClient.connected()) {
    gMqttConnected = false;
    unsigned long now = millis();
    if (static_cast<long>(now - gNextMqttRetryMs) >= 0) {
      if (!connectMqttBroker()) {
        gNextMqttRetryMs = now + gMqttBackoffMs;
        if (gMqttBackoffMs < 20000UL) gMqttBackoffMs *= 2;
      }
    }
    return;
  }

  unsigned long now = millis();
  if (now - gLastMqttRxMs > MQTT_RX_TIMEOUT_MS) {
    gTlsClient.stop();
    gMqttConnected = false;
    gNeedsRedraw = true;
    return;
  }

  if (now - gLastPingSentMs >= MQTT_KEEPALIVE_MS) {
    uint8_t pingReq[2] = {0xC0, 0x00};
    if (gTlsClient.write(pingReq, 2) != 2) {
      gTlsClient.stop();
      gMqttConnected = false;
      gNeedsRedraw = true;
      return;
    }
    gTlsClient.flush();
    gLastPingSentMs = now;
  }

  while (gTlsClient.available() > 0) {
    uint8_t pktType = 0, flags = 0;
    uint8_t body[MAX_MQTT_RX];
    size_t bodyLen = 0;
    if (!mqttReadPacket(pktType, flags, body, sizeof(body), bodyLen)) {
      gTlsClient.stop();
      gMqttConnected = false;
      gNeedsRedraw = true;
      return;
    }
    gLastMqttRxMs = millis();

    if (pktType == 3 && bodyLen >= 2) {
      uint8_t qos = (flags >> 1) & 0x03;
      uint16_t tLen = (static_cast<uint16_t>(body[0]) << 8) | body[1];
      size_t pos = 2 + tLen;
      if (pos <= bodyLen) {
        String topic;
        topic.reserve(tLen);
        for (size_t i = 2; i < pos; ++i) topic += static_cast<char>(body[i]);

        if (qos > 0 && pos + 2 <= bodyLen) {
          uint16_t pid = (static_cast<uint16_t>(body[pos]) << 8) | body[pos + 1];
          pos += 2;
          if (qos == 1) {
            uint8_t puback[4] = {0x40, 0x02, static_cast<uint8_t>(pid >> 8), static_cast<uint8_t>(pid & 0xFF)};
            gTlsClient.write(puback, 4);
            gTlsClient.flush();
          }
        }
        if (topic == gCmdTopic && pos < bodyLen) {
          String json;
          json.reserve(bodyLen - pos);
          for (size_t i = pos; i < bodyLen; ++i) json += static_cast<char>(body[i]);
          handleIncomingCommandJson(json);
        }
      }
    }
  }
}

// -------------------- Captive Setup Web Portal --------------------

static String apSsidName() {
  return "Oraculo-Ponte-" + gBridgeId.substring(0, 4);
}

static String generatePortalApPassword() {
  static const char kAlpha[] = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  uint8_t rnd[10];
  esp_fill_random(rnd, sizeof(rnd));
  String pass;
  pass.reserve(10);
  for (size_t i = 0; i < sizeof(rnd); ++i) {
    pass += kAlpha[rnd[i] % (sizeof(kAlpha) - 1)];
  }
  secureZero(rnd, sizeof(rnd));
  return pass;
}

static bool isValidBrokerHostInput(const String& host) {
  if (host.isEmpty() || host.length() > 128) return false;
  if (host.equalsIgnoreCase("localhost") || host.endsWith(".localhost")) return false;
  for (size_t i = 0; i < host.length(); ++i) {
    char c = host[i];
    if (c <= 0x20 || c >= 0x7F || c == '/' || c == '\\' || c == '@' || c == '?' || c == '#' || c == ':') {
      return false;
    }
  }
  return true;
}

static void handlePortalRoot() {
  int n = WiFi.scanNetworks();
  String ssidOpts;
  for (int i = 0; i < n && i < 20; ++i) {
    String s = WiFi.SSID(i);
    if (s.isEmpty()) continue;
    String esc = htmlEscape(s);
    ssidOpts += "<option value=\"" + esc + "\"";
    if (s == gWifiSsid) ssidOpts += " selected";
    ssidOpts += ">" + esc + " (" + String(WiFi.RSSI(i)) + " dBm)</option>";
  }
  WiFi.scanDelete();

  String html = F(
      "<!DOCTYPE html><html lang='pt-BR'><head><meta charset='utf-8'>"
      "<meta name='viewport' content='width=device-width,initial-scale=1'>"
      "<title>Oráculo Ponte WoL</title><style>"
      "body{font-family:system-ui,sans-serif;background:#111418;color:#e3e8ee;margin:0;padding:18px}"
      ".card{max-width:460px;margin:0 auto;background:#1a1f26;border-radius:18px;padding:18px}"
      "h1{font-size:20px;margin:0 0 6px}p{color:#9ca7b5;font-size:13px;line-height:1.4}"
      "label{display:block;font-size:12px;color:#9ca7b5;margin:12px 0 4px}"
      "input,select,textarea{width:100%;box-sizing:border-box;background:#111418;color:#fff;"
      "border:1px solid #303946;border-radius:10px;padding:10px;font-size:14px}"
      "button{width:100%;margin-top:14px;background:#7cc4ff;color:#062338;border:none;"
      "border-radius:999px;padding:12px;font-weight:700;font-size:15px;cursor:pointer}"
      ".note{background:#111418;border:1px solid #303946;border-radius:10px;padding:10px;margin-top:14px;font-size:12px;color:#9ca7b5}"
      "</style></head><body><div class='card'><h1>Oráculo · Ponte Wake-on-LAN</h1>"
      "<p>Configure o Wi-Fi da casa. Após salvar, este ponto de acesso fecha automaticamente.</p>"
      "<form method='POST' action='/save'>"
      "<label>Rede Wi-Fi (SSID)</label><select id='sel' onchange=\"document.getElementById('ssid').value=this.value\">"
      "<option value=''>-- Selecionar rede encontrada --</option>");
  html += ssidOpts;
  html += F("</select><label>Ou digite o SSID manualmente</label><input id='ssid' name='ssid' maxlength='64' required value='");
  html += htmlEscape(gWifiSsid);
  html += F("'><label>Senha do Wi-Fi</label><input name='wpass' type='password' maxlength='64' placeholder='");
  html += gWifiSsid.isEmpty() ? F("Senha da rede Wi-Fi") : F("Manter atual se em branco");
  html += F("'><label>Nome da ponte</label><input name='bname' maxlength='48' value='");
  html += htmlEscape(gBridgeName);
  html += F("'><label>Broker MQTT TLS</label><input name='broker' maxlength='128' required value='");
  html += htmlEscape(gBrokerHost);
  html += F("'><label>Porta TLS</label><input name='port' type='number' min='1' max='65535' value='");
  html += String(gBrokerPort);
  html += F(
      "'><details style='margin-top:12px'><summary style='cursor:pointer;color:#7cc4ff;font-size:13px'>"
      "Credenciais / CA opcional (broker privado)</summary>"
      "<label>Usuário MQTT</label><input name='muser' maxlength='64' value='");
  html += htmlEscape(gMqttUser);
  html += F("'><label>Senha MQTT</label><input name='mpass' type='password' maxlength='128' placeholder='Manter atual se em branco (vazio com usuário limpo remove)'>"
            "<label>Root CA PEM opcional</label><textarea name='ca' rows='3' placeholder='-----BEGIN CERTIFICATE-----'></textarea></details>"
            "<button type='submit'>Salvar e conectar</button></form>"
            "<div class='note'>Por segurança, a chave de pareamento não é exposta via HTTP. "
            "Após salvar e conectar ao Wi-Fi, escaneie o QR Code na tela do Cardputer ou execute <code>uri</code> no console Serial.</div>"
            "</div></body></html>");
  gWeb.send(200, "text/html; charset=utf-8", html);
}

static void handlePortalSave() {
  String ssid = gWeb.arg("ssid");
  ssid.trim();
  if (ssid.isEmpty() || ssid.length() > 64) {
    gWeb.send(400, "text/plain; charset=utf-8", "SSID obrigatório (máx. 64 caracteres).");
    return;
  }

  String wpass = gWeb.arg("wpass");
  if (wpass.length() > 64) {
    gWeb.send(400, "text/plain; charset=utf-8", "Senha do Wi-Fi excede 64 caracteres.");
    return;
  }
  String bname = sanitizeName(gWeb.arg("bname"));
  String broker = gWeb.arg("broker");
  broker.trim();
  if (broker.isEmpty()) broker = DEFAULT_BROKER_HOST;
  if (!isValidBrokerHostInput(broker)) {
    gWeb.send(400, "text/plain; charset=utf-8", "Hostname do broker MQTT inválido.");
    return;
  }
  int portVal = gWeb.arg("port").toInt();
  uint16_t port = (portVal >= 1 && portVal <= 65535) ? static_cast<uint16_t>(portVal) : DEFAULT_BROKER_PORT;

  gPrefs.begin(NVS_NAMESPACE, false);
  gPrefs.putString("wifi_ssid", ssid);
  if (!wpass.isEmpty() || gWifiSsid != ssid) gPrefs.putString("wifi_pass", wpass);
  gPrefs.putString("name", bname);
  gPrefs.putString("broker_host", broker);
  gPrefs.putUShort("broker_port", port);
  if (gWeb.hasArg("muser")) {
    String muser = gWeb.arg("muser");
    muser.trim();
    if (muser.length() <= 64) {
      gPrefs.putString("mqtt_user", muser);
    }
    if (muser.isEmpty()) {
      // [MQTT-3.1.2-22]: remove senha antiga quando usuário MQTT é limpo.
      gPrefs.remove("mqtt_pass");
    } else if (!gWeb.arg("mpass").isEmpty() && gWeb.arg("mpass").length() <= 128) {
      gPrefs.putString("mqtt_pass", gWeb.arg("mpass"));
    }
  }
  String ca = gWeb.arg("ca");
  ca.trim();
  if (ca == "-") {
    gPrefs.remove("custom_ca");
  } else if (!ca.isEmpty() && ca.length() <= 4096) {
    gPrefs.putString("custom_ca", ca);
  }
  gPrefs.end();

  gWeb.send(200, "text/html; charset=utf-8",
            "<!DOCTYPE html><html lang='pt-BR'><head><meta charset='utf-8'></head>"
            "<body style='background:#111418;color:#e3e8ee;font-family:sans-serif;padding:24px'>"
            "<h2>Configuração salva!</h2><p>A ponte está conectando ao Wi-Fi. Se houver erro, segure BOOT por 3s.</p></body></html>");
  delay(800);
  ESP.restart();
}

static void startPortalMode() {
  gTlsClient.stop();
  gMqttConnected = false;
  WiFi.disconnect(true);
  WiFi.mode(WIFI_AP_STA);
  IPAddress apIp(192, 168, 4, 1);
  WiFi.softAPConfig(apIp, apIp, IPAddress(255, 255, 255, 0));
  String apName = apSsidName();
  gPortalApPass = generatePortalApPassword();
  WiFi.softAP(apName.c_str(), gPortalApPass.c_str());
  gDns.start(53, "*", apIp);
  gWeb.on("/", HTTP_GET, handlePortalRoot);
  gWeb.on("/save", HTTP_POST, handlePortalSave);
  gWeb.onNotFound(handlePortalRoot);
  gWeb.begin();
  gPortalMode = true;
  gNeedsRedraw = true;

  Serial.println("\n=== MODO CONFIGURAÇÃO Wi-Fi ===");
  Serial.print("1. Conecte-se ao Wi-Fi: ");
  Serial.println(apName);
  Serial.print("2. Senha WPA2 do AP:    ");
  Serial.println(gPortalApPass);
  Serial.println("3. Abra no navegador:   http://192.168.4.1");
  Serial.println("URI de pareamento atual (console local):");
  Serial.println(buildPairingUri());
}

// -------------------- Cardputer UI --------------------

#if ORACULO_HAS_CARDPUTER
static uint8_t pickQrVersion(size_t len) {
  if (len <= 102) return 5;
  if (len <= 130) return 6;
  if (len <= 150) return 7;
  if (len <= 188) return 8;
  if (len <= 226) return 9;
  if (len <= 265) return 10;
  return 12;
}

static void drawCardputerScreen() {
  auto& d = M5Cardputer.Display;
  d.startWrite();
  d.fillScreen(d.color565(17, 20, 24));

  if (gPortalMode) {
    d.setTextColor(d.color565(124, 196, 255));
    d.setTextSize(1.5);
    d.setCursor(10, 8);
    d.println("ORACULO · CONFIGURAR");
    d.setTextSize(1.1);
    d.setTextColor(TFT_WHITE);
    d.setCursor(10, 32);
    d.print("Wi-Fi: ");
    d.println(apSsidName());
    d.setCursor(10, 52);
    d.print("Senha: ");
    d.println(gPortalApPass);
    d.setCursor(10, 72);
    d.println("Abrir: http://192.168.4.1");
    d.setTextColor(d.color565(156, 167, 181));
    d.setCursor(10, 102);
    d.println("[R] Novo segredo  [Q/DEL] Reiniciar");
    d.endWrite();
    gNeedsRedraw = false;
    return;
  }

  if (gShowQrScreen) {
    String uri = buildPairingUri();
    d.fillRoundRect(3, 3, 129, 129, 6, TFT_WHITE);
    d.qrcode(uri.c_str(), 7, 7, 121, pickQrVersion(uri.length()));

    d.setTextColor(d.color565(124, 196, 255));
    d.setTextSize(1.2);
    d.setCursor(140, 8);
    d.println("ORACULO WOL");

    d.setTextSize(1.0);
    d.setTextColor(TFT_WHITE);
    d.setCursor(140, 26);
    d.println(gBridgeName.substring(0, 15));

    bool wifiOk = (WiFi.status() == WL_CONNECTED);
    d.fillCircle(144, 46, 4, gMqttConnected ? TFT_GREEN : (wifiOk ? TFT_YELLOW : TFT_RED));
    d.setCursor(153, 42);
    d.setTextColor(gMqttConnected ? TFT_GREEN : TFT_YELLOW);
    d.println(gMqttConnected ? "TLS OK" : (wifiOk ? "MQTT..." : "Wi-Fi..."));

    d.setTextColor(d.color565(156, 167, 181));
    d.setCursor(140, 64);
    d.println("[Q] Painel");
    d.setCursor(140, 80);
    d.println("[Enter] Ligar PC");
    d.setCursor(140, 96);
    d.println("[P] Portal Wi-Fi");
    d.setCursor(140, 112);
    d.println("[R] Novo QR");
  } else {
    d.setTextColor(d.color565(124, 196, 255));
    d.setTextSize(1.4);
    d.setCursor(10, 8);
    d.print("ORACULO · ");
    d.println(gBridgeName.substring(0, 14));

    d.setTextSize(1.05);
    bool wifiOk = (WiFi.status() == WL_CONNECTED);
    d.setCursor(10, 32);
    d.setTextColor(wifiOk ? TFT_WHITE : TFT_RED);
    d.print("Wi-Fi: ");
    d.println(wifiOk ? WiFi.localIP().toString() : String("Desconectado"));

    d.setCursor(10, 50);
    d.setTextColor(gMqttConnected ? TFT_GREEN : TFT_YELLOW);
    d.print("MQTT TLS: ");
    d.println(gMqttConnected ? ("Conectado (" + gBrokerHost + ")") : String("Conectando..."));

    d.setCursor(10, 68);
    d.setTextColor(TFT_WHITE);
    int batPct = M5.Power.getBatteryLevel();
    d.print("Sinais: ");
    d.print(gWakeCount);
    d.print("   Bat: ");
    if (batPct >= 0) {
      d.print(batPct);
      d.println("%");
    } else {
      d.println("USB");
    }

    d.setCursor(10, 88);
    d.setTextColor(d.color565(156, 167, 181));
    d.print("Ult. MAC: ");
    d.println(gLastWokenMacs.isEmpty() ? String("-") : gLastWokenMacs.substring(0, 17));

    d.setCursor(10, 112);
    d.setTextColor(d.color565(124, 196, 255));
    d.println("[Q] QR [Enter] Ligar [P] Wi-Fi [R] Novo");
  }

  d.endWrite();
  gNeedsRedraw = false;
}

static void pollCardputerKeys() {
  M5Cardputer.update();
  if (!M5Cardputer.Keyboard.isChange() || !M5Cardputer.Keyboard.isPressed()) return;
  auto keys = M5Cardputer.Keyboard.keysState();
  if (gPortalMode && keys.del && !gWifiSsid.isEmpty()) {
    ESP.restart();
    return;
  }
  if (keys.enter && !gLastWokenMacs.isEmpty() && !gPortalMode) {
    sendMagicPacketsCsv(gLastWokenMacs, "-", 9);
  }
  for (char c : keys.word) {
    char ch = static_cast<char>(tolower(static_cast<unsigned char>(c)));
    if (gPortalMode && ch == 'q' && !gWifiSsid.isEmpty()) {
      ESP.restart();
      return;
    }
    if (ch == 'q') {
      gShowQrScreen = !gShowQrScreen;
      gNeedsRedraw = true;
    } else if (ch == 'p') {
      startPortalMode();
    } else if (ch == 'r') {
      generateFreshIdentity();
      gTlsClient.stop();
      gMqttConnected = false;
      gStatusNotice = "Novo QR gerado";
      gNeedsRedraw = true;
      Serial.println("\nNovo URI de pareamento:");
      Serial.println(buildPairingUri());
    }
  }
}
#endif

// -------------------- Serial CLI & Boot Button --------------------

static void printSerialBanner() {
  Serial.println("\n=== Oráculo Ponte Wake-on-LAN (ESP32 / Cardputer) ===");
  Serial.print("Nome:      ");
  Serial.println(gBridgeName);
  Serial.print("Bridge ID: ");
  Serial.println(gBridgeId);
  Serial.print("Broker:    ");
  Serial.print(gBrokerHost);
  Serial.print(':');
  Serial.println(gBrokerPort);
  Serial.print("Wi-Fi:     ");
  Serial.println(WiFi.status() == WL_CONNECTED ? WiFi.localIP().toString() : String("offline"));
  Serial.print("MQTT TLS:  ");
  Serial.println(gMqttConnected ? "conectado" : "reconectando");
  Serial.println("Comandos Serial: uri | status | portal | rotate | wake <MAC>");
}

static void pollSerialCli() {
  if (!Serial.available()) return;
  String line = Serial.readStringUntil('\n');
  line.trim();
  if (line.isEmpty()) return;

  if (line == "uri") {
    Serial.println(buildPairingUri());
  } else if (line == "status") {
    printSerialBanner();
  } else if (line == "portal") {
    startPortalMode();
  } else if (line == "rotate") {
    generateFreshIdentity();
    gTlsClient.stop();
    gMqttConnected = false;
    Serial.println("Segredo rotacionado! Novo link de pareamento:");
    Serial.println(buildPairingUri());
    gNeedsRedraw = true;
  } else if (line.startsWith("wake ")) {
    String mac = line.substring(5);
    mac.trim();
    uint32_t sent = sendMagicPacketsCsv(mac, "-", 9);
    Serial.printf("WoL serial enviado (%lu pacotes).\n", static_cast<unsigned long>(sent));
  } else if (line == "factory-reset") {
    gPrefs.begin(NVS_NAMESPACE, false);
    gPrefs.clear();
    gPrefs.end();
    Serial.println("Configurações apagadas. Reiniciando...");
    delay(300);
    ESP.restart();
  } else {
    Serial.println("Comandos: uri | status | portal | rotate | wake <AA:BB:CC:DD:EE:FF> | factory-reset");
  }
}

static void pollBootButton() {
  bool pressed = (digitalRead(ORACULO_BOOT_PIN) == LOW);
  if (pressed) {
    if (gBootBtnDownMs == 0) gBootBtnDownMs = millis();
    if (!gPortalMode && millis() - gBootBtnDownMs >= 3000UL) {
      gBootBtnDownMs = 0;
      startPortalMode();
    }
  } else {
    gBootBtnDownMs = 0;
  }
}

// -------------------- Arduino lifecycle --------------------

void setup() {
  Serial.begin(115200);
  pinMode(ORACULO_BOOT_PIN, INPUT_PULLUP);

#if ORACULO_HAS_CARDPUTER
  auto cfg = M5.config();
  M5Cardputer.begin(cfg, true);
  M5Cardputer.Display.setRotation(1);
#endif

  WiFi.mode(WIFI_STA);
  WiFi.setAutoReconnect(true);
  WiFi.setSleep(false);

  loadConfig();

  if (gWifiSsid.isEmpty()) {
    startPortalMode();
    return;
  }

  WiFi.begin(gWifiSsid.c_str(), gWifiPass.c_str());
  unsigned long waitEnd = millis() + 12000UL;
  while (WiFi.status() != WL_CONNECTED && static_cast<long>(millis() - waitEnd) < 0) {
    delay(250);
    pollBootButton();
    if (gPortalMode) return;
  }

  gUdp.begin(0);
  printSerialBanner();
  Serial.println("Link de pareamento (para usar sem câmera):");
  Serial.println(buildPairingUri());
  gNeedsRedraw = true;
}

void loop() {
  pollSerialCli();
  pollBootButton();

#if ORACULO_HAS_CARDPUTER
  pollCardputerKeys();
#endif

  if (gPortalMode) {
    gDns.processNextRequest();
    gWeb.handleClient();
#if ORACULO_HAS_CARDPUTER
    if (gNeedsRedraw) drawCardputerScreen();
#endif
    delay(10);
    return;
  }

  if (WiFi.status() == WL_CONNECTED) {
    pollMqttLoop();
  } else {
    gMqttConnected = false;
    unsigned long now = millis();
    if (now - gLastWifiRetryMs >= 30000UL) {
      gLastWifiRetryMs = now;
      WiFi.begin(gWifiSsid.c_str(), gWifiPass.c_str());
    }
  }

#if ORACULO_HAS_CARDPUTER
  unsigned long now = millis();
  if (gNeedsRedraw || now - gLastDisplayTickMs > 5000UL) {
    gLastDisplayTickMs = now;
    drawCardputerScreen();
  }
#endif

  delay(8);
}
