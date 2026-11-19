# Command-line usage

*Leia em [Português (Brasil)](cli.pt-BR.md).*

Everything the desktop app does can also be run without a GUI — useful for headless Linux machines, scripts, and SSH-free administration.

## Desktop app flags

```bash
oraculo-desktop              # open the GUI (embeds host or attaches to running daemon)
oraculo-desktop --background # start silently in the background
oraculo-desktop --quit       # stop the GUI and background service completely
```

## Headless host daemon (`oraculo-host`)

```bash
oraculo-host serve --pair             # start daemon and print QR + 8-character code (press 'p' for a new code)
oraculo-host peers                    # list paired devices
oraculo-host revoke <ENDPOINT_ID>     # revoke a device immediately
oraculo-host permissions              # inspect current settings.toml permissions
oraculo-host permissions \
  --screen true --input true --terminal true \
  --files-read true --files-write false \
  --commands-run true --commands-edit false \
  --rdp-tunnel true --rdp-port 3389 \
  --wol-enabled true --wol-mac "" --wol-broadcast ""
oraculo-host info                     # print EndpointId and config paths
oraculo-host monitors                 # choose monitors for multi-monitor streaming
oraculo-host wol --enable             # enable Wake-on-LAN / Magic Packet on physical NICs
```

## Terminal client (`oraculo-cli`)

```bash
oraculo-cli pair "<CODE_OR_PAIRING_URI>" --name "laptop"
oraculo-cli ping                      # accepts --host <ENDPOINT_ID>
oraculo-cli commands                  # accepts --host <ENDPOINT_ID>
oraculo-cli run <COMMAND_ID>          # accepts --host <ENDPOINT_ID>
oraculo-cli shell                     # accepts --host <ENDPOINT_ID>
oraculo-cli wake                      # wake default saved computer on LAN
oraculo-cli wake --bridge <BRIDGE_ID> # wake via a paired WoL bridge
oraculo-cli wake --mac AA:BB:CC:DD:EE:FF --target 192.168.1.255 --port 9
```

## Standalone Wake-on-LAN bridge (`oraculo-bridge`)

```bash
oraculo-bridge                                   # first run prints QR code and short code
oraculo-bridge serve --pair --name "Home-Pi"     # custom label + fresh pairing code
oraculo-bridge peers                             # list authorized phones
oraculo-bridge revoke <ENDPOINT_ID>              # revoke a phone
oraculo-bridge info                              # show bridge EndpointId and data paths
```

See the [Wake-on-LAN guide](wake-on-lan.md) for bridge setup.
