---
name: oraculo-m3-design
description: >-
  Guia oficial do sistema de design do Oráculo, inspirado no Google Material Design 3
  e Material 3 Expressive. Use sempre que criar, alterar, estilizar ou revisar telas,
  componentes, diálogos, temas, animações ou ícones no app Android (Jetpack Compose em
  android/app/src/main/java/com/oraculo/app/ e res/values/) ou no painel Desktop
  (crates/oraculo-desktop/src/ui.html, ipc.rs e webview.rs), garantindo paridade de
  tokens (OraculoColors, OraculoMetrics, OraculoMotion), elevação tonal limpa, proporção
  harmônica entre telas, listas agrupadas expressivas e i18n PT/EN.
---

# Oráculo Material 3 Design System

Este documento define o padrão visual do **Oráculo** no cliente Android (Jetpack Compose + Material 3) e no painel Desktop (`oraculo-desktop` WebKitGTK/WebView2). A estética segue o **Google Material Design 3** com elementos do **Material 3 Expressive** (elevação tonal sem bordas redundantes, pílulas de ação limpas, listas agrupadas segmentadas e proporção adaptativa entre tamanhos de janela).

Para tabelas completas de hexadecimais, CSS variables, métricas exatas, razões de grade e receitas prontas de código, consulte [tokens-and-components.md](./references/tokens-and-components.md).

---

## 1. Pilares da Identidade Visual

1. **Elevação Tonal Pura (Zero "Caixas Dentro de Caixas")**
   - A profundidade entre planos nasce da diferença de luminância (`Background` `#131314` → `SurfaceRecessed` `#18191B` → `Surface` `#1E1F20` → `SurfaceInteractive` `#282A2C` no escuro; `#F8F9FA` → `#E9EEF6` → `#FFFFFF` → `#DFE3E7` no claro), **não de bordas repetidas**.
   - Use no máximo **2 níveis de contêiner visíveis** por bloco (`Fundo → Superfície` ou `Card → Superfície interna`). **Proibido** empilhar `.m3-item` com borda `1px` dentro de `.surface-card` com borda `1px`: no tema escuro, superfícies se separam por cor; no tema claro, borda sutil (`#E3E7EE`) vai apenas no plano externo ou em botões `Outlined`.
   - Evite `shadowElevation` pesada ou `box-shadow` difusa em cards fixos; reserve sombra apenas para camadas realmente flutuantes (`.m3-menu`, `.modal-card`).
2. **Paridade 1:1 Android ↔ Desktop ↔ Splash**
   - Os 17 tokens semânticos de `OraculoDarkPalette` / `OraculoLightPalette` em [Theme.kt](../../../android/app/src/main/java/com/oraculo/app/ui/Theme.kt) são idênticos aos tokens CSS de [ui.html](../../../crates/oraculo-desktop/src/ui.html).
   - O fundo inicial XML (`values/colors.xml` `#F8F9FA`, `values-night/colors.xml` `#131314`) e o fundo GTK/WebKitGTK em `webview.rs` (`#131314` escuro / `#F8F9FA` claro) devem sempre bater com `Background` para eliminar flash na abertura.
3. **Acento Google Blue Autêntico**
   - **Tema Escuro:** CTA primário em pílula usa `PrimaryAction` (`#A8C7FA`) com texto escuro de alto contraste `OnPrimaryAction` (`#062E6F`) e sem borda (`border: transparent`); acento tonal/pílula ativa da rail usa `SecondaryContainer` (`#004A77`) + `OnSecondaryContainer` (`#C2E7FF`).
   - **Tema Claro:** `Primary` e `PrimaryAction` convergem para Google Blue `#0B57D0` sobre `#FFFFFF` / `#F8F9FA`, com contêiner tonal `#D3E3FD` + `#041E49`.
4. **Geometria M3 & M3 Expressive**
   - **Botões padrão e busca:** pílula cheia (`border-radius: 999px` / `OraculoMetrics.CornerPill`), sem contorno extra em `Primary` e `Tonal`.
   - **Listas agrupadas:** cantos externos amplos (`20.dp`/`20px`) e internos suaves (`8.dp`/`8px`) com vão de `3.dp`/`3px` (`oraculoSegmentedItemShape` e `.m3-group > .m3-item`).
   - **Cards principais e modais:** `24.dp`/`24px` (`.surface-card`) a `28px` (`.modal-card`); caixas de ícone e inputs `12.dp`/`12px`.
5. **Tipografia `Google Sans` + Contraste Real de Peso**
   - Android empacota `OraculoFontFamily` (`res/font/google_sans_*.ttf`) e Desktop serve `@font-face` local (`Google Sans` + `Google Sans Code` via `ipc.rs`).
   - Valorize pesos **`500` (`Medium`)** nos títulos (`26px` no header de página, `16px` em seções, `14px` em itens) em vez de negrito `700` pesado em tudo.
   - `Google Sans Code` / `FontFamily.Monospace` com `tabular-nums` (`tnum`) é exclusivo para dados técnicos (`EndpointId`, MAC, código `K7P4-M9X2`, caminhos `~/`, comandos shell, latência, `exit 0`, atalhos). Nunca use monoespaço em cabeçalhos ou pílulas de identificação humana na topbar.
6. **Movimento Leve e Responsivo**
   - Curva única M3: `OraculoMotion.Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)` (`cubic-bezier(0.2, 0, 0, 1)` no CSS), durações `90ms`–`180ms`, sem *spring physics* pesada. No Desktop, animações contínuas respeitam `prefers-reduced-motion`.

---

## 2. Proporção, Hierarquia e Alocação Harmônica de Espaço Entre Telas

Telas profissionais parecem calmas e intencionais porque **cada classe de tela recebe proporções próprias**, e o espaço negativo trabalha a favor da leitura em vez de sobrar como "buraco vazio":

### Princípios de Proporção
- **Alinhamento no mesmo eixo Y:** em layouts de 2 colunas, títulos principais de cada coluna (`Permissões` e `Wake-on-LAN`, `Parear celular` e `Dispositivos autorizados`) nascem na mesma linha de base horizontal. Nunca coloque título solto sobre `--bg` numa coluna contra título dentro de card com padding extra na vizinha.
- **Equilíbrio de altura em painéis paralelos:** evite um cartão alto (`420px`) ao lado de outro pela metade (`210px`) flutuando com um parágrafo solto no meio. Se duas colunas são cartões irmãos (`Início`, `Comandos`), use `align-items: stretch` e dê estrutura útil ao painel menor (ex.: passos numerados com respiro real e CTA ancorado). Se uma coluna for naturalmente assimétrica e não exigir cartão envolvente (`Acesso`), **remova o cartão externo** e use grupos abertos.
- **Teto de densidade por grupo (Chunking):** nunca empilhe mais de **4 ou 5 linhas idênticas** num bloco único sem separar em subgrupos lógicos com `group-caption` e `14px–16px` de vão (ex.: `Permissões` dividido em *Controle do computador* e *Arquivos e atalhos*).
- **Proporção raio × respiro interno:** mantenha `padding >= raio` opticamente (`surface-card` de `24px` usa `padding: 22px 24px`; listas de `20px` usam `padding: 11px 16px`; pílulas `999px` usam `padding-inline >= 16px`).
- **Escala ótica por dispositivo:** controles no desktop têm proporção mais fina que os `48.dp` do dedo no celular (`switch` `46×28px` no desktop vs `52×32.dp` no mobile; botão padrão `36px` vs `48–50.dp` no mobile).

### Alocação por Classe de Tela (*Window Size Classes*)

| Classe de Tela | Colunas & Navegação | Largura & Margens | Proporção / Comportamento Obrigatório |
| :--- | :--- | :--- | :--- |
| **Compact Portrait** (`< 600.dp` — Celular) | **1 coluna**; sheet ou barra superior/abas | Margem lateral `16–20.dp`; largura `100%` | Alvos `>= 48.dp`; CTA principal no terço inferior (`50.dp`); ilustração vazia entre `220–264.dp` centrada verticalmente |
| **Compact Landscape** (`altura < 480.dp` — Vídeo/Touchpad) | **Assimetria lateral `~68% / 32%`** | Padding `8–12.dp`; deck compacto `42–48.dp` | Altura é o recurso escasso: coloque touchpad/vídeo à esquerda e controles rápidos à direita; nunca empilhe barras altas no topo/base |
| **Medium / Desktop Estreito** (`600px–839px` / `< 840px`) | **Navigation Rail `80px` + 1 coluna** | Padding conteúdo `18–24px`; card máx `680px` | Quebre grades de 2 colunas abaixo de `840px` para impedir texto espremido quebrando em 4 linhas e switches apertados |
| **Expanded** (`840px–1199px` — Desktop padrão `1020×690`) | **Navigation Rail `80px` + 2 colunas canônicas** | Padding fluido `24–44px`; `flex: 1` vertical | Escolha a razão correta pelo peso de cada coluna (tabela abaixo) |
| **Wide / Fullscreen** (`>= 1200px` — `1920×1080`+) | **Navigation Rail `80px` + canvas fluido alinhado à topbar** | Container máx `1840px` centrado; cards ocupam altura útil (`flex: 1`) | Alinhe margens laterais do `.page` com `.topbar` para eliminar vazios à direita e embaixo em tela cheia |

### Razões Canônicas de Duas Colunas (Expanded & Wide)
- **List-Detail com Fluxo Guiado (`Início`):** `minmax(330px, clamp(356px, 31vw, 460px)) 1fr`, `flex: 1`, `align-items: stretch` — QR Code escala em tela cheia (`212px`–`288px`) e renova automaticamente ao expirar.
- **Supporting Pane Assimétrico (`Acesso`):** `minmax(400px, 1.35fr) minmax(300px, 1fr)` — o fluxo principal de permissões ganha largura confortável enquanto o painel auxiliar de rede fica enxuto.
- **List-Detail Mestre-Detalhe (`Comandos`):** `minmax(310px, clamp(336px, 29vw, 430px)) 1fr`, `flex: 1`, `align-items: stretch` — formulário ocupa a largura/altura do card com grade dupla em telas largas e rodapé ancorado.
- **Equal Split (`Sistema`):** `repeat(2, minmax(320px, 1fr))` (`50% / 50%`) — use apenas quando os dois lados têm massa visual equivalente.

---

## 3. Regras Obrigatórias para Qualquer Mudança de UI

### Cores e Superfícies
- **Use sempre tokens**: `OraculoColors.*` (ou `MaterialTheme.colorScheme.*` mapeado por `OraculoTheme`) no Android; `var(--...)` no Desktop.
- **Nunca crie hex avulsos** em telas comuns. As únicas exceções permitidas são:
  1. Scrims escuros sobre vídeo ao vivo ou câmera (`ScreenStreamingScreen.kt`, `QrScannerScreen.kt`).
  2. A paleta `PtyxisPalette` (`PtyxisDarkPalette` / `PtyxisLightPalette`) no terminal interativo.
  3. Arte vetorial do mascote (`JijiCatIllustration.kt` / SVG `.jiji-svg`).
- **Blocos de código/saída/preview** usam `TerminalBackground` + `TerminalForeground` + `TerminalBorder`.

### Hierarquia de Botões e Ações
| Papel da ação | Android Compose | Desktop (`ui.html`) | Uso típico |
| :--- | :--- | :--- | :--- |
| **CTA Principal** | `Button(colors = oraculoPrimaryButtonColors(), shape = RoundedCornerShape(OraculoMetrics.CornerPill))` | `.btn.btn-primary` | Parear, Salvar, Enviar, Ativar |
| **Ação Tonal / Estado Ativo** | `FilledTonalButton` / `SecondaryContainer` (`OraculoDeckButton(accent = true)` ou `active = true`) | `.btn.btn-tonal` | Novo comando, Copiar código, Monitores |
| **Secundária Neutra** | `OutlinedButton` com `BorderStroke(1.dp, OraculoColors.Outline)` e `CornerPill` | `.btn.btn-outlined` | Digitar código manual, Duplicar, Compartilhar |
| **Dispensar / Cancelar** | `TextButton` com `OraculoColors.OnSurfaceMuted` | `.btn.btn-outlined` / `.btn-compact` | Cancelar, Fechar |
| **Destrutiva** | `Button` com `Error`/`OnError` (confirmação) | `.btn.btn-danger-ghost` / `.btn-danger` | Remover dispositivo ou comando |

### Padrões de Navegação e Chrome Nativo
- **Android**:
  - Tela inicial usa cabeçalho tipográfico `displaySmall` + `IconButton` de menu abrindo `ModalBottomSheet` (`OraculoSettingsSheet`) com `CornerSurfaceExpanded` no topo e `BottomSheetDefaults.DragHandle(color = OraculoColors.Outline)`.
  - Dashboard conectado (`HostControlScreen`) usa barra compacta com `ArrowBack`, título + subtítulo monospace de latência clicável para ping, e `PrimaryTabRow` (`54.dp`).
- **Desktop (`oraculo-desktop`)**:
  - **Chrome unificado (`webview.rs`):** quando `AdwApplicationWindow` + `GtkOverlay` está ativo (`data-chrome="overlay"`), **não há HeaderBar dupla**; `GtkWindowHandle` ocupa os `58px` superiores entre `x=80px` e `width-370px`, e os controles da janela flutuam em `top:16px, right:14px`. Mantenha `padding-right: 68px` em `.topbar` e `.status-pill { max-width: 170px; }` para que ações nunca sejam bloqueadas pela faixa de arrasto nem colidam com o botão fechar.
  - **Navigation Rail (`80px`)**: `role="tablist"` vertical com ícones *outlined* (inativo) / *filled* (ativo) dentro da pílula `56×32px` em `--secondary-container`, setas `↑`/`↓` e atalhos `Alt+1..4`.
  - **Topbar contextual**: `<h1>` exibe a seção atual (`Início`, `Acesso e rede`, `Comandos rápidos`, `Sistema`) em vez de repetir o nome do app em todas as abas; o badge de status no topo mostra o nome da máquina de forma humana (ID técnico fica em tooltip e na aba `Sistema`).
  - **Modais, menus e teclado**: `.modal-card` centralizado (`min-width: 280px; max-width: 460px; border-radius: 28px`), `.m3-menu` próximo ao cursor, `:focus-visible` nítido, atalhos (`Alt+1..4`, `Ctrl+N`, `Ctrl+S`, `Ctrl+P`, `Ctrl+Q`, `Esc`, `?`) e atualização incremental no polling.

### Métricas, Ícones, Bilinguismo e Ergonomia
- **Dimensões**: no Android, use `OraculoMetrics.ControlHeight*` e `OraculoMetrics.Corner*` em novos componentes. Não copie literais históricos (`13.dp`, `14.dp`, `22.dp`) — ao editar trechos antigos, migre para o token mais próximo.
- **Ícones direcionais**: no Android, use sempre `Icons.AutoMirrored.Filled.*` / `Icons.AutoMirrored.Outlined.*` para `ArrowBack`, `Backspace`, `KeyboardReturn`, `OpenInNew`, `KeyboardArrowLeft/Right`, `Volume*` e `HelpOutline`.
- **Bilinguismo PT/EN obrigatório**: todo texto visível usa `tr(pt, en)` no Android e `data-i18n` / `t(...)` com paridade exata `I18N.pt` ↔ `I18N.en` no Desktop (inclusive `contentDescription` e *placeholders*).
- **Insets e Edge-to-Edge**: no Android, toda tela raiz respeita `statusBarsPadding()`, `navigationBarsPadding()` e `imePadding()` (exceto fullscreen imersivo), alternando `SystemBarStyle.dark`/`light` conforme `isOraculoDarkTheme(themeMode)`.
- **Feedback Tátil**: controles remotos disparam `HapticFeedbackType.TextHandleMove`.

### Microcopy & Tom de Voz (Sem "Cara de IA" nem Jargão Dev)
- **Fale do efeito, não da tecnologia:** evite citar bibliotecas, protocolos, codecs ou arquivos internos (`Iroh`, `P2P`, `H.264`, `Ed25519`, `PTY`, `commands.toml`, `Material Design 3`) em fluxos principais; reserve termos técnicos para campos de diagnóstico ou quando o usuário precisa copiá-los/executá-los.
- **Zero texto promocional:** proíba advérbios e chavões vazios (*"sem esforço"*, *"rapidamente"*, *"intuitivo"*, *"completo"*, *"de forma segura"*). Descreva a ação em uma frase curta e objetiva.
- **Vocabulário único por conceito:** use sempre **"computador"** (evite misturar "PC" / "host" / "computador" na mesma tela, exceto em comandos como `oraculo-host`), **"Enviar"** para upload e **"Compartilhar"** para share. Diferencie **"salvo/pareado"** (autorizado) de **"conectado"** (sessão ativa).
- **Sentence case, sem badges em caixa alta e sem redundância:** use maiúscula apenas no início da frase, nomes próprios e siglas (`QR Code`, `Wi-Fi`, `Wake-on-LAN`, `MAC`). Evite selos como `EDITAR` ao lado do título `"Editar comando"` e subtítulos que repetem os controles ao lado.
- **Parênteses mínimos e erros acionáveis:** evite parênteses em cascata explicando detalhes internos; mensagens de erro informam o que falhou e qual ação tomar.

---

## 4. Fluxo de Trabalho ao Criar ou Alterar UI

1. **Consultar tokens e proporções**: leia [tokens-and-components.md](./references/tokens-and-components.md) para escolher paleta, cantos, escala tipográfica e razão de colunas conforme a classe de tela.
2. **Projetar com paridade**: mantenha nomes, estados, cores semânticas, `Google Sans` e rótulos PT/EN alinhados entre Android e Desktop.
3. **Reutilizar blocos nativos**: prefira `OraculoDeckButton`, `oraculoPrimaryButtonColors()`, `oraculoSegmentedButtonColors()`, `oraculoSegmentedItemShape()`, `OraculoSegmentedSelector`, `JijiCatIllustration` e `.m3-group` / `.m3-switch` / `.search-pill` / `.segmented-row`.
4. **Validar automaticamente**:
   ```bash
   python3 .agents/skills/oraculo-m3-design/scripts/check_design_tokens.py
   ```
   Corrija qualquer divergência de hex entre Android e Desktop, chaves `data-i18n` faltantes, quebras de regras Desktop ou ícones não-`AutoMirrored`.
5. **Compilar e testar pelo fluxo obrigatório do repositório** (via Podman, nunca `cargo`/`gradle` direto no host):
   - Se alterou `crates/oraculo-desktop/`: execute `./scripts/build.sh test` (e `./scripts/build.sh check`).
   - Se alterou `android/`: execute `./scripts/build-android.sh debug` e `./scripts/build.sh test` antes de finalizar.
