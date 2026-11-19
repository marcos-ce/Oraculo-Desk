# Uso por linha de comando

*Read this in [English](cli.md).*

Tudo que o app desktop faz também pode ser executado sem interface gráfica — útil para máquinas Linux sem monitor, scripts e administração sem SSH.

## Flags do app Desktop

```bash
oraculo-desktop              # abre a interface (embute o host ou conecta ao daemon em execução)
oraculo-desktop --background # inicia em silêncio em segundo plano
oraculo-desktop --quit       # encerra a interface e o serviço em segundo plano
```

## Daemon host sem interface (`oraculo-host`)

```bash
oraculo-host serve --pair             # inicia o daemon e exibe QR + código de 8 caracteres (tecla 'p' gera outro)
oraculo-host peers                    # lista aparelhos pareados
oraculo-host revoke <ENDPOINT_ID>     # revoga um aparelho na hora
oraculo-host permissions              # exibe as permissões atuais do settings.toml
oraculo-host permissions \
  --screen true --input true --terminal true \
  --files-read true --files-write false \
  --commands-run true --commands-edit false \
  --rdp-tunnel true --rdp-port 3389 \
  --wol-enabled true --wol-mac "" --wol-broadcast ""
oraculo-host info                     # exibe EndpointId e caminhos de configuração
oraculo-host monitors                 # escolhe monitores para transmissão multimonitor
oraculo-host wol --enable             # ativa Wake-on-LAN / Magic Packet nas placas de rede físicas
```

## Cliente de terminal (`oraculo-cli`)

```bash
oraculo-cli pair "<CODIGO_OU_URI_DE_PAREAMENTO>" --name "notebook"
oraculo-cli ping                      # aceita --host <ENDPOINT_ID>
oraculo-cli commands                  # aceita --host <ENDPOINT_ID>
oraculo-cli run <COMMAND_ID>          # aceita --host <ENDPOINT_ID>
oraculo-cli shell                     # aceita --host <ENDPOINT_ID>
oraculo-cli wake                      # liga o computador salvo na rede local
oraculo-cli wake --bridge <BRIDGE_ID> # liga via ponte WoL pareada
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9
```

## Ponte Wake-on-LAN independente (`oraculo-bridge`)

```bash
oraculo-bridge                                   # na primeira execução exibe QR Code e código curto
oraculo-bridge serve --pair --name "Home-Pi"     # nome personalizado + novo código de pareamento
oraculo-bridge peers                             # lista celulares autorizados
oraculo-bridge revoke <ENDPOINT_ID>              # revoga um celular
oraculo-bridge info                              # exibe EndpointId e caminhos de dados da ponte
```

Veja o [Guia de Wake-on-LAN](wake-on-lan.pt-BR.md) para configurar a ponte.
