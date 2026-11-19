# Suporte de plataformas e limitações conhecidas

*Read this in [English](compatibility.md).*

O Oráculo é um **beta** em evolução ativa. O cenário principal de testes é **GNOME (Wayland / Fedora)** + **Android (`arm64-v8a`)**.

| Recurso | Linux | Windows | macOS | Android | iOS |
| :--- | :---: | :---: | :---: | :---: | :---: |
| Aplicativo Desktop (`oraculo-desktop`) | ✅ | em desenvolvimento | — | — | — |
| Terminal, comandos, arquivos, Wake-on-LAN | ✅ | em desenvolvimento | experimental | ✅ (cliente) | planejado |
| Transmissão de tela e controle remoto | ✅ (Wayland) | próximas atualizações | planejado | ✅ (cliente) | planejado |

## Ambientes gráficos no Linux

A transmissão de tela e o controle de mouse/teclado usam os padrões **XDG Desktop Portal** (`ScreenCast` + `RemoteDesktop`) e **PipeWire**.

- **GNOME no Wayland (Fedora):** alvo principal de desenvolvimento e testes.
- **Outros ambientes Wayland** que implementam ambos os portais (como **KDE Plasma 6**) também tendem a funcionar, mas tiveram cobertura limitada de testes em campo.
- **Sessões X11** (como Cinnamon ou XFCE em X11) suportam o app desktop, terminal, comandos, arquivos e Wake-on-LAN — **sem** transmissão de tela nem controle remoto.

## Tela bloqueada / login do GDM no Wayland

Dentro de uma sessão já desbloqueada, o espelhamento de tela e o controle rodam diretamente no app Android. Na tela de login do GDM ou com o sistema bloqueado por senha, o Wayland restringe capturas pelo portal; nesses momentos o desbloqueio acontece pelo túnel RDP local criptografado do Oráculo com um cliente RDP no celular (`aFreeRDP` ou `Windows App`). Ainda não há visualizador RDP embutido.

## Outras limitações

- Desktop: apenas Linux `x86_64`. Android: `arm64-v8a`, Android 8.0+.
- O firmware da ponte ESP32 / Cardputer teve cobertura limitada de testes em campo.
- Ligar o computador fora da rede local exige uma ponte ([guia](wake-on-lan.pt-BR.md)).

## Próximas atualizações

- **Suporte ao Windows (próxima prioridade):** captura de tela, entrada remota, empacotamento e testes ponta a ponta.
- **Maior compatibilidade no Linux:** KDE Plasma, compositores wlroots/COSMIC e fallback X11 opcional.
- **Depois:** visualizador RDP embutido no Android e cliente iOS compartilhando o mesmo núcleo Rust.

Relatos de bugs e compatibilidade em [Issues](https://github.com/marcos-ce/Oraculo-Desk/issues) são muito bem-vindos.
