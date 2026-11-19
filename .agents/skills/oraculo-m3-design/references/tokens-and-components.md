# Referência Completa de Tokens, Proporções e Componentes — Oráculo M3

Especificação exata extraída de `Theme.kt`, `colors.xml`, `themes.xml`, `JijiCatIllustration.kt`, `crates/oraculo-desktop/src/ui.html`, `ipc.rs` e `webview.rs`.

---

## 1. Paleta Principal (`OraculoColors` ↔ CSS Variables)

| Token Android (`OraculoPalette`) | Token CSS (`ui.html`) | Escuro (`Dark`) | Claro (`Light`) | Papel Semântico |
| :--- | :--- | :--- | :--- | :--- |
| `Background` | `--bg` | `#131314` | `#F8F9FA` | Fundo raiz da janela/tela e `oraculo_window_bg` |
| `Surface` | `--surface` | `#1E1F20` | `#FFFFFF` | Cartões, itens de lista na raiz, diálogos, `ModalBottomSheet` |
| `SurfaceRecessed` | `--surface-recessed` | `#18191B` | `#E9EEF6` | Navigation Rail, camada rebaixada M3, barras do modo tela |
| `SurfaceInteractive` | `--surface-interactive` | `#282A2C` | `#DFE3E7` | Hover no desktop, menus flutuantes, `kbd`, desabilitados |
| `Primary` | `--primary` | `#A8C7FA` | `#0B57D0` | Acento ativo, anel de foco, progresso, track de `Switch`, ícones de destaque |
| `OnPrimary` | `--on-primary` | `#062E6F` | `#FFFFFF` | Conteúdo sobre `Primary` (ex.: thumb de `Switch` ativo no escuro) |
| `PrimaryAction` | `--primary-action` | `#A8C7FA` | `#0B57D0` | Fundo de botões primários preenchidos em pílula (`Button` / `.btn-primary`) |
| `OnPrimaryAction` | `--on-primary-action` | `#062E6F` | `#FFFFFF` | Texto e ícones sobre `PrimaryAction` |
| `PrimaryActionBorder` | `--primary-action-border` | `#A8C7FA` | `#0B57D0` | Mantido em paridade (CTA em CSS usa `border-color: transparent`) |
| `SecondaryContainer` | `--secondary-container` | `#004A77` | `#D3E3FD` | Pílula ativa da rail, item selecionado em List-Detail, botões tonais, badges de ícone |
| `OnSecondaryContainer` | `--on-secondary-container` | `#C2E7FF` | `#041E49` | Texto e ícone sobre `SecondaryContainer` |
| `OnSurface` | `--on-surface` | `#E3E3E3` | `#1F1F1F` | Texto e ícones principais |
| `OnSurfaceMuted` | `--on-surface-muted` | `#9AA0A6` | `#5F6368` | Subtítulos, placeholders, captions, ícones inativos |
| `Outline` | `--outline` | `#333537` | `#DDE1E6` | Divisores, botões `Outlined`, trilho do switch desligado, borda sutil no claro |
| `Error` | `--error` | `#F87171` | `#DC2626` | Erros, offline, ações destrutivas |
| `OnError` | `--on-error` | `#2E0B0B` | `#FFFFFF` | Texto sobre botão destrutivo preenchido |
| `Success` | `--success` | `#6DD58C` | `#146C2E` | Status online, `exit 0`, selo "Conectado" |

### Variáveis Exclusivas de Profundidade no Desktop (`ui.html`)
- `--rail-bg`: `#18191B` (Escuro) / `#E9EEF6` (Claro) — separa a Navigation Rail por tom, sem linha vertical pesada.
- `--surface-inner` & `--input-bg`: `#161719` / `#131314` (Escuro) / `#F3F6FA` (Claro) — distingue itens, code-box, search pill e inputs **dentro** de um `.surface-card` sem empilhar bordas.
- `--card-border` / `--item-border`: `transparent` no Escuro (elevação puramente tonal); `#E3E7EE` / `#E5E9F0` no Claro com `--card-shadow: 0 1px 2px rgba(60, 64, 67, 0.06)`.

### Tokens Exclusivos de Console/Preview (Android)

| Token | Escuro | Claro | Uso |
| :--- | :--- | :--- | :--- |
| `TerminalBackground` | `#0E0F10` | `#131314` | Fundo de `CommandOutputCard` e preview de texto/imagem em `FilesTab` |
| `TerminalForeground` | `#E6EDF3` | `#E6EDF3` | Texto monoespaçado de stdout/stderr e arquivos |
| `TerminalBorder` | `#282A2C` | `#282A2C` | Borda de `1.dp` do bloco escuro de saída |

### Mapeamento no `MaterialTheme.colorScheme` (`OraculoTheme`)

```kotlin
primary = palette.Primary
onPrimary = palette.OnPrimary
primaryContainer = palette.SecondaryContainer
onPrimaryContainer = palette.OnSecondaryContainer
secondary = palette.Primary
onSecondary = palette.OnPrimary
secondaryContainer = palette.SecondaryContainer
onSecondaryContainer = palette.OnSecondaryContainer
background = palette.Background
onBackground = palette.OnSurface
surface = palette.Surface
onSurface = palette.OnSurface
surfaceVariant = palette.SecondaryContainer
onSurfaceVariant = palette.OnSurfaceMuted
surfaceContainerLowest = palette.SurfaceRecessed
surfaceContainerLow = palette.SurfaceRecessed
surfaceContainer = palette.Surface
surfaceContainerHigh = palette.SurfaceInteractive
surfaceContainerHighest = palette.SurfaceInteractive
outline = palette.Outline
outlineVariant = palette.Outline
error = palette.Error
onError = palette.OnError
```

### Paridade de Inicialização (Anti-Flash & Chrome Unificado)

- Android Light: `res/values/colors.xml` → `oraculo_window_bg = #F8F9FA`, parent `android:Theme.Material.Light.NoActionBar`
- Android Dark: `res/values-night/colors.xml` → `oraculo_window_bg = #131314`, parent `android:Theme.Material.NoActionBar`
- Desktop GTK4/libadwaita (`crates/oraculo-desktop/src/webview.rs`):
  - `rgba_f32_for_dark(true)` e GTK3 `GdkRgbaF64`: `RGB(19, 19, 20)` (`19.0 / 255.0, 19.0 / 255.0, 20.0 / 255.0`, `#131314`)
  - `rgba_f32_for_dark(false)`: `RGB(248, 249, 250)` (`#F8F9FA`)
  - Janela padrão `1020×690`, mínima `760×540`.
  - Quando `AdwApplicationWindow` + `GtkOverlay` está ativo (`__ORACULO_CHROME__ = "overlay"`): faixa de arrasto superior `height: 58px`, `margin_start: 80px`, `margin_end: 370px`, controles nativos em `top: 16px, right: 14px`. O CSS reserva `padding-right: 68px` no `.topbar` (`58px` abaixo de `640px`) e limita `.status-pill` a `max-width: 170px`.

---

## 2. Paleta Ptyxis (Terminal Interativo)

Separada de `OraculoColors` para preservar fidelidade ao terminal GNOME Ptyxis:

| Papel | `PtyxisDarkPalette` | `PtyxisLightPalette` |
| :--- | :--- | :--- |
| `Background` | `#1C1C1F` | `#FFFFFF` |
| `HeaderBackground` | `#2E2E32` | `#F3F3F5` |
| `HeaderBorder` | `#3A3A40` | `#DFDFE4` |
| `Foreground` | `#FFFFFF` | `#1D1D20` |
| `ForegroundMuted` | `#B5B5BC` | `#5E5E66` |
| `Cursor` / `CursorText` | `#FFFFFF` / `#1C1C1F` | `#1D1D20` / `#FFFFFF` |

Cores ANSI 0–15 estão definidas em `PtyxisDarkPalette.Ansi16` e `PtyxisLightPalette.Ansi16` em `Theme.kt`.

---

## 3. Dimensões, Raios de Canto e Espaçamentos

### `OraculoMetrics` (Android) e Equivalentes Desktop

| Token `OraculoMetrics` | Valor Android | Equivalente Desktop (`ui.html`) | Onde usar |
| :--- | :--- | :--- | :--- |
| `ControlHeightCompactChip` | `32.dp` | `height: 32px` (`.rail-pill`, `.btn-compact`, `.seg-btn`) | `FilterChip`, `RemoteModifierRow(compact = true)`, controles secundários compactos |
| `ControlHeightSmall` | `36.dp` | `height: 36px` (`.btn`, `.m3-menu-item`), `34px` (`.status-pill`, `.icon-btn`) | Botões desktop padrão, toolbar do streaming, menus |
| `ControlHeightCompactDeck` | `42.dp` | `height: 40px–42px` (`.search-pill`, `.m3-input`), `min-height: 48px` (`.m3-item`) | Botões do deck no modo tela; busca e inputs no desktop |
| `ControlHeightDialog` | `44.dp` | `min-width: 280px; max-width: 460px` (`.modal-card`, teto `560px`) | Todos os botões de `AlertDialog` no Android e modais centralizados no Desktop |
| `ControlHeightInputCompact` | `48.dp` | — | `RemoteKeyboardBar(compact = true)` no modo paisagem |
| `ControlHeightDeck` | `48.dp` | — | `OraculoDeckButton` e `TouchpadMouseButtonsRow` no modo retrato normal |
| `ControlHeightCta` | `50.dp` | — | CTAs primários full-width (`DeviceListScreen`, `QrScannerScreen`) |
| `ControlHeightInput` | `56.dp` | — | `OutlinedTextField` padrão + botão Send Enter ao lado |
| `CornerSmall` | `8.dp` | `8px` (`.m3-item` interno, `kbd`, `.timer-track`) | Chips de atalho/tecla, raio interno de lista agrupada |
| `CornerMedium` | `12.dp` | `10px–12px` (`.m3-input`, `.item-icon`, `.m3-menu-item`, `.snackbar`) | Inputs de formulário, caixas de ícone `36px`, banners |
| `CornerLarge` | `16.dp` | `16px` (`.rail-pill`, `.code-box`, `.m3-menu`) | `OraculoDeckButton`, `TouchpadSettingsDrawer`, moldura do terminal Ptyxis, menus flutuantes |
| `CornerSurface` | `20.dp` | `20px` (`.m3-group` externo, `.pair-qr-wrap`) | Cantos externos de listas agrupadas, contêiner do QR Code |
| `CornerSurfaceExpanded` | `24.dp` | `24px–28px` (`.surface-card`, `.modal-card`) | Card do estado vazio com Jiji, `ModalBottomSheet` (topo), cards Desktop e diálogos |
| `CornerPill` | `999.dp` | `999px` (`.btn`, `.search-pill`, `.status-pill`, `.segmented-row`, `.m3-switch`) | CTAs principais/tonais/outlined, barras de busca, pílulas de status e switches |
| `CornerGroupedOuter` / `CornerGroupedInner` | `20.dp` / `8.dp` | `20px` / `8px` + `gap: 3px` | `oraculoSegmentedItemShape(index, count)` e `.m3-group` |

---

## 4. Matriz de Proporção e Alocação de Espaço Entre Telas

### Ritmo Base (`4px / 8px`) e Regras de Massa Visual
- **Unidade base:** `4px` / `4.dp`. Margens e respiros estruturais sempre múltiplos (`12`, `16`, `20`, `24`, `28`).
- **Máximo de itens por grupo:** `4–5` linhas irmãs em `.m3-group`; acima disso, divida com `.group-caption` (`margin: 16px 0 8px 4px`).
- **Regra Raio × Padding:** `surface-card` (`radius 24px`) usa `padding: 22px 24px`; `pair-steps-well` / `detail-subpanel` (`radius 18px`) usam `padding: 14px–16px`; `.m3-item` (`radius 8/20px`) usa `padding: 11px 16px`.
- **Alinhamento topográfico:** em qualquer grade de 2 colunas, os dois cabeçalhos iniciais começam no mesmo `Y`. Nunca misture título fora do card à esquerda com título dentro do card à direita.

### Grades por Classe de Tela

| Contexto | Geometria da Grade | Respiro / Largura Máxima | Regra de Ocupação |
| :--- | :--- | :--- | :--- |
| **Desktop (`>= 840px`) — Início** | `grid-template-columns: minmax(330px, clamp(356px, 31vw, 460px)) 1fr; flex: 1; align-items: stretch` | `.page { max-width: 1840px; margin: 0 auto; }`, padding fluido `24px–44px` | Coluna esquerda escala QR (`212px–288px`, auto-renovável) + controles ancorados; direita mantém Jiji (`204px–280px`) centralizado verticalmente quando vazia |
| **Desktop (`>= 840px`) — Acesso** | `grid-template-columns: minmax(400px, 1.35fr) minmax(300px, 1fr); align-items: start` | Sem cartão externo envolvente; dois cabeçalhos alinhados | Lista principal dividida em 2 grupos (`3` + `4` itens) com `.item-icon`; painel WoL sem bordas aninhadas |
| **Desktop (`>= 840px`) — Comandos** | `grid-template-columns: minmax(310px, clamp(336px, 29vw, 430px)) 1fr; flex: 1; align-items: stretch` | `.detail-form-wrap { flex: 1; width: 100%; }` com `.form-row-two` | Busca pílula (`40px`); seleção tonal `--secondary-container`; campos duplos em telas largas + resumo de execução e ações ancoradas na base |
| **Desktop (`>= 840px`) — Sistema** | `grid-template-columns: repeat(2, minmax(320px, 1fr)); align-items: start` | 4 itens × 4 itens com `.item-icon` simétrico | Divisão `50/50` justificada pela simetria exata de conteúdo |
| **Desktop Estreito (`641px–840px`)** | `grid-template-columns: 1fr` (mantém Rail `80px`) | Padding `4px 28px 28px` | Colapsa antes que colunas de `~300px` comprimam textos ou botões |
| **Desktop Compacto (`<= 640px`)** | `grid-template-columns: 1fr` (mantém Rail `80px`) | Padding `4px 18px 20px`; `.topbar` `14px 18px 10px` | Reduz margens mantendo legibilidade em janelas pequenas |
| **Android Portrait (`< 600.dp`)** | `Column` / `LazyColumn` (`1fr`) | Padding horizontal `20.dp` (raiz) / `14.dp` (abas) | Alvos `>= 48.dp`; botões primários em pílula cheia no rodapé (`50.dp`) |
| **Android Landscape (`altura < 480.dp`)** | Divisão lateral `~68% / 32%` | Padding `8–12.dp`; deck compacto `42–48.dp` | Preserva área útil de vídeo/touchpad; nunca empilha barras horizontais espessas |

---

## 5. Tipografia (`Google Sans` + `Google Sans Code`)

### Android (`OraculoFontFamily` / `OraculoTypography`)
Empacotada em `android/app/src/main/res/font/google_sans_{regular,medium,semibold,bold}.ttf` (SIL OFL 1.1):
- `displaySmall`: `OraculoFontFamily`, `32.sp`, `Medium`, `letterSpacing = (-0.75).sp`, `lineHeight = 38.sp`
- `titleLarge`: `OraculoFontFamily`, `22.sp`, `Medium`, `letterSpacing = (-0.3).sp`
- `titleMedium`: `OraculoFontFamily`, `16.sp`, `Medium`, `letterSpacing = (-0.1).sp`
- `bodyLarge`: `OraculoFontFamily`, `16.sp`, `lineHeight = 22.sp`
- `bodyMedium`: `OraculoFontFamily`, `14.sp`, `lineHeight = 20.sp`
- `bodySmall`: `OraculoFontFamily`, `12.sp`, `lineHeight = 16.sp`
- `labelLarge`: `OraculoFontFamily`, `14.sp`, `Medium`, `letterSpacing = 0.1.sp`
- **Monospace tabular** (`FontFamily.Monospace`, `fontFeatureSettings = "tnum"`): exclusivo para IDs, códigos, caminhos e telemetria (`11.sp`–`13.sp`, e código de pareamento em `headlineSmall` `Bold` `letterSpacing = 2.sp`).

### Desktop (`ui.html` + `ipc.rs`)
Fontes locais servidas em `/assets/*.woff2?t=__ORACULO_SESSION_TOKEN__` (`crates/oraculo-desktop/assets/`, SIL OFL 1.1):
- Sans-serif: `"Google Sans", "Adwaita Sans", "Inter", "Segoe UI", system-ui, -apple-system, sans-serif` (`14px`, `line-height: 1.45`)
  - `h1` (topbar): `26px`, `font-weight: 500`, `letter-spacing: -0.45px`
  - `.section-title`: `16px`, `font-weight: 500`, `letter-spacing: -0.1px`
  - `.item-title`: `14px`, `font-weight: 500`
  - `.item-sub` / `.section-sub`: `12.5px–13px` em `--on-surface-muted`
- Monospace: `"Google Sans Code", "Adwaita Mono", "JetBrains Mono", ui-monospace, monospace` com `font-variant-numeric: tabular-nums; user-select: text;`

---

## 6. Movimento (`OraculoMotion`)

- `Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)` (Desktop: `cubic-bezier(0.2, 0, 0, 1)`)
- `MicroDurationMs = 90` — pressão de botão (`0.98` CSS / `0.965f` Compose), fade de telemetria
- `StateDurationMs = 140` — troca de tema (`animateColorAsState` / `transition` CSS)
- `PanelEnterMs = 160` / `PanelExitMs = 120` — `ExpandV`, `CollapseV`, `ExpandH`, `CollapseH`
- `RouteEnterMs = 180` / `RouteExitMs = 140` — `OverlayEnter` / `OverlayExit` e transição horizontal de rotas

---

## 7. Receitas Canônicas

### Botão Primário Pílula (Android)

```kotlin
Button(
    onClick = onAction,
    colors = oraculoPrimaryButtonColors(),
    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
    modifier = Modifier
        .fillMaxWidth()
        .height(OraculoMetrics.ControlHeightCta),
) {
    Text(text = label, fontWeight = FontWeight.Medium)
}
```

### Lista Agrupada M3 Expressive (Android)

```kotlin
LazyColumn(verticalArrangement = Arrangement.spacedBy(3.dp)) {
    itemsIndexed(items, key = { _, item -> item.id }) { index, item ->
        Surface(
            onClick = { onClick(item) },
            color = OraculoColors.Surface,
            shape = oraculoSegmentedItemShape(index, items.size),
            border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
            modifier = Modifier.fillMaxWidth(),
        ) {
            // Ícone leading: Box 40.dp com CornerMedium (12.dp) e SecondaryContainer
        }
    }
}
```

### Lista Agrupada M3 Expressive (Desktop)

```html
<div class="group-caption">Grupo lógico</div>
<div class="m3-group">
  <div class="m3-item">
    <div class="item-lead">
      <div class="item-icon"><!-- SVG 18px --></div>
      <div class="item-texts">
        <div class="item-title">Título</div>
        <div class="item-sub wrap">Subtítulo curto</div>
      </div>
    </div>
    <label class="m3-switch"><input type="checkbox"><span class="m3-track"><span class="m3-thumb"></span></span></label>
  </div>
</div>
```

### Busca Pílula M3 (Desktop)

```html
<div class="search-pill-wrap">
  <span class="search-pill-icon"><!-- Lupa 16px --></span>
  <input class="search-pill" type="search" placeholder="Filtrar...">
</div>
```

### Switch M3 no Desktop (`46×28px`)

- Track desligado: `--surface-interactive`, borda `2px solid var(--on-surface-muted)`, thumb `14×14px`.
- Track ligado: `--primary`, borda `--primary`, thumb expande para `20×20px` em `--on-primary` com `translate(18px, -50%)`.
