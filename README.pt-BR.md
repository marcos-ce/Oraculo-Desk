# Oráculo

[![CI](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml/badge.svg)](https://github.com/marcos-ce/Oraculo-Desk/actions/workflows/ci.yml)
[![Licença](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](licenses/LICENSE)

*Read this in [English](README.md).*

Acesso remoto do celular Android ao desktop Linux por conexão P2P criptografada. Sem contas, sem abrir portas no roteador, sem servidor intermediário.

## O que faz

- Transmite a tela do seu desktop para o celular (H.264, decodificação por hardware)
- Controla mouse e teclado remotamente
- Abre sessões de terminal pelo celular
- Executa comandos pré-configurados com um toque
- Navega, baixa e envia arquivos (confinado ao diretório home)

O app Android tem interface Material 3 com troca PT/EN, temas claro/escuro, zoom por pinça, e quatro abas: Touchpad, Terminal, Comandos e Arquivos.

## Download

Acesse a página de [Releases](https://github.com/marcos-ce/Oraculo-Desk/releases/latest) e baixe os dois:

| | Arquivo | Para |
| :--- | :--- | :--- |
| 🖥️ | `oraculo-vX.Y.Z-linux-x86_64.tar.gz` | Seu computador Linux |
| 📱 | `oraculo-vX.Y.Z-android-arm64.apk` | Seu celular Android (8.0+) |

### Instalar no Linux

```bash
tar xzf oraculo-v*.tar.gz
sudo install -m 755 oraculo-host oraculo-cli /usr/local/bin/
```

A transmissão de tela precisa de uma sessão Wayland com PipeWire, GStreamer (com plugins PipeWire) e um encoder H.264. No Fedora:

```bash
sudo dnf install gstreamer1-plugins-base pipewire-gstreamer gstreamer1-plugin-openh264
```

Se você tem uma GPU NVIDIA, `nvh264enc` é usado automaticamente quando disponível (mais rápido, menos uso de CPU). Caso contrário, usa `openh264enc`.

### Instalar no Android

Transfira o `.apk` para o celular e instale. Pode ser necessário permitir "Instalar de fontes desconhecidas" nas configurações do aparelho.

## Como usar

### 1. Inicie o host no computador

```bash
oraculo-host serve --pair
```

Um QR code aparece no terminal, válido por 3 minutos. Pressione `p` + Enter para gerar um novo a qualquer momento.

### 2. Pareie o celular

Abra o app Oráculo, toque em "Parear novo computador" e escaneie o QR code. Também dá para colar o link `oraculo://pair?...` manualmente.

Pronto. Seu celular está conectado. Use as abas para alternar entre touchpad, terminal, comandos e navegador de arquivos.

### 3. Gerenciar dispositivos e permissões

```bash
oraculo-host peers                    # listar dispositivos pareados
oraculo-host revoke <ID>              # remover um dispositivo
oraculo-host permissions --screen true --input true --terminal true
oraculo-host permissions --files-read true --files-write false
oraculo-host info                     # mostrar ID do host e caminhos de config
oraculo-host monitors                 # escolher monitores para multi-monitor
```

## Cliente CLI

Também existe um cliente de terminal para parear e abrir shells remotos de outro computador:

```bash
oraculo-cli pair "<URI_DE_PAREAMENTO>" --name "meu-notebook"
oraculo-cli ping
oraculo-cli shell
```

Todos os comandos aceitam `--host <ID>` para direcionar a um host específico.

## Segurança

As conexões são criptografadas ponta a ponta com QUIC (TLS 1.3) e chaves Ed25519. Nenhum dado passa por servidores de terceiros.

Tokens de pareamento são de uso único, expiram em 3 minutos e travam após 5 tentativas erradas. Revogar um dispositivo ou mudar permissões encerra os streams ativos na hora.

O acesso a arquivos fica confinado ao diretório home com proteção contra symlinks e traversal. Diretórios sensíveis (`.ssh`, `.gnupg`, keyrings, configs do shell) são sempre bloqueados. Material criptográfico é armazenado com permissão 0600.

Veja [`.github/SECURITY.md`](.github/SECURITY.md) para reportar vulnerabilidades.

## Compilar do código-fonte

Se quiser compilar você mesmo em vez de usar os binários da release, precisa do [Podman](https://podman.io/). Os scripts de build cuidam do toolchain Rust, Android SDK e NDK dentro de contêineres.

```bash
git clone https://github.com/marcos-ce/Oraculo-Desk.git
cd Oraculo-Desk

./scripts/build.sh release            # binários desktop → ./dist/
./scripts/build-android.sh debug      # APK Android → ./dist/
./scripts/build.sh test               # rodar testes unitários
```

## Suporte de plataformas

| Componente | Linux | macOS / Windows | Android |
| :--- | :--- | :--- | :--- |
| Terminal, comandos, arquivos | ✅ | ✅ | ✅ (cliente) |
| Transmissão de tela + entrada remota | ✅ (Wayland) | planejado | ✅ (cliente) |

## Licença

[Apache License 2.0](licenses/LICENSE). Atribuições de terceiros em [`licenses/NOTICE`](licenses/NOTICE).
