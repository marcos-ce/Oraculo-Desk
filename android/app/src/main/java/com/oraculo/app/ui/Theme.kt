package com.oraculo.app.ui

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.TweenSpec
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonColors
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButtonColors
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SwitchColors
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.oraculo.app.R

/**
 * Tokens compartilhados de dimensões e cantos Material 3 para manter botões
 * e superfícies proporcionais e leves em todas as telas e orientações.
 */
object OraculoMetrics {
    val ControlHeightCompactChip: Dp = 32.dp
    val ControlHeightSmall: Dp = 36.dp
    val ControlHeightCompactDeck: Dp = 42.dp
    val ControlHeightDialog: Dp = 44.dp
    val ControlHeightInputCompact: Dp = 48.dp
    val ControlHeightDeck: Dp = 48.dp
    val ControlHeightCta: Dp = 50.dp
    val ControlHeightInput: Dp = 56.dp

    val CornerSmall: Dp = 8.dp
    val CornerMedium: Dp = 12.dp
    val CornerLarge: Dp = 16.dp
    val CornerSurface: Dp = 20.dp
    val CornerSurfaceExpanded: Dp = 24.dp
    val CornerGroupedOuter: Dp = 20.dp
    val CornerGroupedInner: Dp = 8.dp
    val CornerPill: Dp = 999.dp
}

/**
 * Curvas e tempos padronizados do Material 3 para transições leves e estáveis,
 * sem física custosa por widget em aparelhos mais modestos.
 */
object OraculoMotion {
    val Easing: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)

    const val MicroDurationMs: Int = 90
    const val StateDurationMs: Int = 140
    const val PanelEnterMs: Int = 160
    const val PanelExitMs: Int = 120
    const val RouteEnterMs: Int = 180
    const val RouteExitMs: Int = 140

    fun <T> microTween(): TweenSpec<T> = tween(durationMillis = MicroDurationMs, easing = Easing)

    fun <T> stateTween(): TweenSpec<T> = tween(durationMillis = StateDurationMs, easing = Easing)

    fun <T> panelTween(enter: Boolean = true): TweenSpec<T> = tween(
        durationMillis = if (enter) PanelEnterMs else PanelExitMs,
        easing = Easing,
    )

    val ExpandV: EnterTransition =
        expandVertically(tween<IntSize>(PanelEnterMs, easing = Easing)) +
            fadeIn(tween<Float>(StateDurationMs, easing = Easing))

    val CollapseV: ExitTransition =
        shrinkVertically(tween<IntSize>(PanelExitMs, easing = Easing)) +
            fadeOut(tween<Float>(100, easing = Easing))

    val ExpandH: EnterTransition =
        expandHorizontally(tween<IntSize>(PanelEnterMs, easing = Easing)) +
            fadeIn(tween<Float>(StateDurationMs, easing = Easing))

    val CollapseH: ExitTransition =
        shrinkHorizontally(tween<IntSize>(PanelExitMs, easing = Easing)) +
            fadeOut(tween<Float>(100, easing = Easing))

    val OverlayEnter: EnterTransition =
        slideInVertically(tween<IntOffset>(RouteEnterMs, easing = Easing)) { it / 18 } +
            fadeIn(tween<Float>(PanelEnterMs, easing = Easing))

    val OverlayExit: ExitTransition =
        slideOutVertically(tween<IntOffset>(RouteExitMs, easing = Easing)) { it / 18 } +
            fadeOut(tween<Float>(PanelExitMs, easing = Easing))
}

/**
 * Forma segmentada de grupo de cartões Material 3 (cantos externos maiores e internos suaves).
 */
fun oraculoSegmentedItemShape(
    index: Int,
    count: Int,
    outerRadius: Dp = OraculoMetrics.CornerGroupedOuter,
    innerRadius: Dp = OraculoMetrics.CornerGroupedInner,
): RoundedCornerShape {
    val top = if (index == 0) outerRadius else innerRadius
    val bottom = if (index >= count - 1) outerRadius else innerRadius
    return RoundedCornerShape(
        topStart = top,
        topEnd = top,
        bottomStart = bottom,
        bottomEnd = bottom,
    )
}

enum class ThemeMode(val prefValue: String, private val ptLabel: String, private val enLabel: String) {
    SYSTEM("system", "Auto", "Auto"),
    LIGHT("light", "Claro", "Light"),
    DARK("dark", "Escuro", "Dark");

    val label: String
        get() = com.oraculo.app.tr(ptLabel, enLabel)

    companion object {
        fun fromPref(value: String?): ThemeMode =
            entries.firstOrNull { it.prefValue == value } ?: SYSTEM
    }
}

@Immutable
data class PtyxisPalette(
    val Background: Color,
    val HeaderBackground: Color,
    val HeaderBorder: Color,
    val Foreground: Color,
    val ForegroundMuted: Color,
    val Cursor: Color,
    val CursorText: Color,
    val Ansi16: List<Color>,
)

val PtyxisDarkPalette = PtyxisPalette(
    Background = Color(0xFF1C1C1F),
    HeaderBackground = Color(0xFF2E2E32),
    HeaderBorder = Color(0xFF3A3A40),
    Foreground = Color(0xFFFFFFFF),
    ForegroundMuted = Color(0xFFB5B5BC),
    Cursor = Color(0xFFFFFFFF),
    CursorText = Color(0xFF1C1C1F),
    Ansi16 = listOf(
        Color(0xFF241F31), // 0 Black
        Color(0xFFC01C28), // 1 Red
        Color(0xFF2EC27E), // 2 Green
        Color(0xFFF5C211), // 3 Yellow
        Color(0xFF1E78E4), // 4 Blue
        Color(0xFF9841BB), // 5 Magenta
        Color(0xFF0AB9DC), // 6 Cyan
        Color(0xFFC0BFBC), // 7 White
        Color(0xFF5E5C64), // 8 Bright Black
        Color(0xFFED333B), // 9 Bright Red
        Color(0xFF57E389), // 10 Bright Green
        Color(0xFFF8E45C), // 11 Bright Yellow
        Color(0xFF51A1FF), // 12 Bright Blue
        Color(0xFFC061CB), // 13 Bright Magenta
        Color(0xFF4FD2FD), // 14 Bright Cyan
        Color(0xFFF6F5F4), // 15 Bright White
    ),
)

val PtyxisLightPalette = PtyxisPalette(
    Background = Color(0xFFFFFFFF),
    HeaderBackground = Color(0xFFF3F3F5),
    HeaderBorder = Color(0xFFDFDFE4),
    Foreground = Color(0xFF1D1D20),
    ForegroundMuted = Color(0xFF5E5E66),
    Cursor = Color(0xFF1D1D20),
    CursorText = Color(0xFFFFFFFF),
    Ansi16 = listOf(
        Color(0xFF1D1D20), // 0 Black
        Color(0xFFC01C28), // 1 Red
        Color(0xFF26A269), // 2 Green
        Color(0xFFA2734C), // 3 Yellow
        Color(0xFF12488B), // 4 Blue
        Color(0xFFA347BA), // 5 Magenta
        Color(0xFF2AA1B3), // 6 Cyan
        Color(0xFFCFCFCF), // 7 White
        Color(0xFF5D5D5D), // 8 Bright Black
        Color(0xFFF66151), // 9 Bright Red
        Color(0xFF33D17A), // 10 Bright Green
        Color(0xFFE9AD0C), // 11 Bright Yellow
        Color(0xFF2A7BDE), // 12 Bright Blue
        Color(0xFFC061CB), // 13 Bright Magenta
        Color(0xFF33C7DE), // 14 Bright Cyan
        Color(0xFFFFFFFF), // 15 Bright White
    ),
)

fun ptyxisPaletteFor(isDark: Boolean): PtyxisPalette =
    if (isDark) PtyxisDarkPalette else PtyxisLightPalette

@Immutable
data class OraculoPalette(
    val IsDark: Boolean,
    val Background: Color,
    val Surface: Color,
    val SurfaceRecessed: Color,
    val SurfaceInteractive: Color,
    val Primary: Color,
    val OnPrimary: Color,
    val PrimaryAction: Color,
    val OnPrimaryAction: Color,
    val PrimaryActionBorder: Color,
    val SecondaryContainer: Color,
    val OnSecondaryContainer: Color,
    val OnSurface: Color,
    val OnSurfaceMuted: Color,
    val Outline: Color,
    val Error: Color,
    val OnError: Color,
    val Success: Color,
    val TerminalBackground: Color,
    val TerminalForeground: Color,
    val TerminalBorder: Color,
)

private val OraculoDarkPalette = OraculoPalette(
    IsDark = true,
    Background = Color(0xFF131314),
    Surface = Color(0xFF1E1F20),
    SurfaceRecessed = Color(0xFF18191B),
    SurfaceInteractive = Color(0xFF282A2C),
    Primary = Color(0xFFA8C7FA),
    OnPrimary = Color(0xFF062E6F),
    PrimaryAction = Color(0xFFA8C7FA),
    OnPrimaryAction = Color(0xFF062E6F),
    PrimaryActionBorder = Color(0xFFA8C7FA),
    SecondaryContainer = Color(0xFF004A77),
    OnSecondaryContainer = Color(0xFFC2E7FF),
    OnSurface = Color(0xFFE3E3E3),
    OnSurfaceMuted = Color(0xFF9AA0A6),
    Outline = Color(0xFF333537),
    Error = Color(0xFFF87171),
    OnError = Color(0xFF2E0B0B),
    Success = Color(0xFF6DD58C),
    TerminalBackground = Color(0xFF0E0F10),
    TerminalForeground = Color(0xFFE6EDF3),
    TerminalBorder = Color(0xFF282A2C),
)

private val OraculoLightPalette = OraculoPalette(
    IsDark = false,
    Background = Color(0xFFF8F9FA),
    Surface = Color(0xFFFFFFFF),
    SurfaceRecessed = Color(0xFFE9EEF6),
    SurfaceInteractive = Color(0xFFDFE3E7),
    Primary = Color(0xFF0B57D0),
    OnPrimary = Color(0xFFFFFFFF),
    PrimaryAction = Color(0xFF0B57D0),
    OnPrimaryAction = Color(0xFFFFFFFF),
    PrimaryActionBorder = Color(0xFF0B57D0),
    SecondaryContainer = Color(0xFFD3E3FD),
    OnSecondaryContainer = Color(0xFF041E49),
    OnSurface = Color(0xFF1F1F1F),
    OnSurfaceMuted = Color(0xFF5F6368),
    Outline = Color(0xFFDDE1E6),
    Error = Color(0xFFDC2626),
    OnError = Color(0xFFFFFFFF),
    Success = Color(0xFF146C2E),
    TerminalBackground = Color(0xFF131314),
    TerminalForeground = Color(0xFFE6EDF3),
    TerminalBorder = Color(0xFF282A2C),
)

private val LocalOraculoPalette = compositionLocalOf { OraculoDarkPalette }

val OraculoColors: OraculoPalette
    @Composable
    @ReadOnlyComposable
    get() = LocalOraculoPalette.current

val OraculoDarkOverlayPalette: OraculoPalette
    get() = OraculoDarkPalette

@Composable
fun isOraculoDarkTheme(themeMode: ThemeMode): Boolean = when (themeMode) {
    ThemeMode.SYSTEM -> isSystemInDarkTheme()
    ThemeMode.LIGHT -> false
    ThemeMode.DARK -> true
}

val OraculoFontFamily = FontFamily(
    Font(R.font.google_sans_regular, FontWeight.Normal),
    Font(R.font.google_sans_medium, FontWeight.Medium),
    Font(R.font.google_sans_semibold, FontWeight.SemiBold),
    Font(R.font.google_sans_bold, FontWeight.Bold),
)

private val OraculoTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 57.sp,
        fontWeight = FontWeight.Normal,
        letterSpacing = (-0.25).sp,
        lineHeight = 64.sp,
    ),
    displayMedium = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 45.sp,
        fontWeight = FontWeight.Normal,
        lineHeight = 52.sp,
    ),
    displaySmall = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 32.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = (-0.75).sp,
        lineHeight = 38.sp,
    ),
    headlineLarge = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 32.sp,
        fontWeight = FontWeight.Medium,
        lineHeight = 40.sp,
    ),
    headlineMedium = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 28.sp,
        fontWeight = FontWeight.Medium,
        lineHeight = 36.sp,
    ),
    headlineSmall = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 24.sp,
        fontWeight = FontWeight.Medium,
        lineHeight = 32.sp,
    ),
    titleLarge = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 22.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = (-0.3).sp,
        lineHeight = 28.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 16.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = (-0.1).sp,
        lineHeight = 24.sp,
    ),
    titleSmall = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 14.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = 0.1.sp,
        lineHeight = 20.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 16.sp,
        lineHeight = 22.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    ),
    bodySmall = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 12.sp,
        lineHeight = 16.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 14.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = 0.1.sp,
        lineHeight = 20.sp,
    ),
    labelMedium = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 12.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = 0.4.sp,
        lineHeight = 16.sp,
    ),
    labelSmall = TextStyle(
        fontFamily = OraculoFontFamily,
        fontSize = 11.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = 0.4.sp,
        lineHeight = 16.sp,
    ),
)

@Composable
fun OraculoTheme(
    themeMode: ThemeMode = ThemeMode.SYSTEM,
    content: @Composable () -> Unit,
) {
    val dark = isOraculoDarkTheme(themeMode)
    val target = if (dark) OraculoDarkPalette else OraculoLightPalette
    val animSpec = OraculoMotion.stateTween<Color>()

    val palette = OraculoPalette(
        IsDark = dark,
        Background = animateColorAsState(target.Background, animSpec, label = "bg").value,
        Surface = animateColorAsState(target.Surface, animSpec, label = "surface").value,
        SurfaceRecessed = animateColorAsState(target.SurfaceRecessed, animSpec, label = "recessed").value,
        SurfaceInteractive = animateColorAsState(target.SurfaceInteractive, animSpec, label = "interactive").value,
        Primary = animateColorAsState(target.Primary, animSpec, label = "primary").value,
        OnPrimary = animateColorAsState(target.OnPrimary, animSpec, label = "onPrimary").value,
        PrimaryAction = animateColorAsState(target.PrimaryAction, animSpec, label = "primaryAction").value,
        OnPrimaryAction = animateColorAsState(target.OnPrimaryAction, animSpec, label = "onPrimaryAction").value,
        PrimaryActionBorder = animateColorAsState(target.PrimaryActionBorder, animSpec, label = "primaryActionBorder").value,
        SecondaryContainer = animateColorAsState(target.SecondaryContainer, animSpec, label = "secContainer").value,
        OnSecondaryContainer = animateColorAsState(target.OnSecondaryContainer, animSpec, label = "onSecContainer").value,
        OnSurface = animateColorAsState(target.OnSurface, animSpec, label = "onSurface").value,
        OnSurfaceMuted = animateColorAsState(target.OnSurfaceMuted, animSpec, label = "muted").value,
        Outline = animateColorAsState(target.Outline, animSpec, label = "outline").value,
        Error = animateColorAsState(target.Error, animSpec, label = "error").value,
        OnError = animateColorAsState(target.OnError, animSpec, label = "onError").value,
        Success = animateColorAsState(target.Success, animSpec, label = "success").value,
        TerminalBackground = animateColorAsState(target.TerminalBackground, animSpec, label = "termBg").value,
        TerminalForeground = animateColorAsState(target.TerminalForeground, animSpec, label = "termFg").value,
        TerminalBorder = animateColorAsState(target.TerminalBorder, animSpec, label = "termBorder").value,
    )

    val colorScheme = if (dark) {
        darkColorScheme(
            primary = palette.Primary,
            onPrimary = palette.OnPrimary,
            primaryContainer = palette.SecondaryContainer,
            onPrimaryContainer = palette.OnSecondaryContainer,
            secondary = palette.Primary,
            onSecondary = palette.OnPrimary,
            secondaryContainer = palette.SecondaryContainer,
            onSecondaryContainer = palette.OnSecondaryContainer,
            background = palette.Background,
            onBackground = palette.OnSurface,
            surface = palette.Surface,
            onSurface = palette.OnSurface,
            surfaceVariant = palette.SurfaceInteractive,
            onSurfaceVariant = palette.OnSurfaceMuted,
            surfaceContainerLowest = palette.SurfaceRecessed,
            surfaceContainerLow = palette.SurfaceRecessed,
            surfaceContainer = palette.Surface,
            surfaceContainerHigh = palette.SurfaceInteractive,
            surfaceContainerHighest = palette.SurfaceInteractive,
            outline = palette.Outline,
            outlineVariant = palette.Outline,
            error = palette.Error,
            onError = palette.OnError,
        )
    } else {
        lightColorScheme(
            primary = palette.Primary,
            onPrimary = palette.OnPrimary,
            primaryContainer = palette.SecondaryContainer,
            onPrimaryContainer = palette.OnSecondaryContainer,
            secondary = palette.Primary,
            onSecondary = palette.OnPrimary,
            secondaryContainer = palette.SecondaryContainer,
            onSecondaryContainer = palette.OnSecondaryContainer,
            background = palette.Background,
            onBackground = palette.OnSurface,
            surface = palette.Surface,
            onSurface = palette.OnSurface,
            surfaceVariant = palette.SurfaceInteractive,
            onSurfaceVariant = palette.OnSurfaceMuted,
            surfaceContainerLowest = palette.SurfaceRecessed,
            surfaceContainerLow = palette.SurfaceRecessed,
            surfaceContainer = palette.Surface,
            surfaceContainerHigh = palette.SurfaceInteractive,
            surfaceContainerHighest = palette.SurfaceInteractive,
            outline = palette.Outline,
            outlineVariant = palette.Outline,
            error = palette.Error,
            onError = palette.OnError,
        )
    }

    CompositionLocalProvider(LocalOraculoPalette provides palette) {
        MaterialTheme(
            colorScheme = colorScheme,
            typography = OraculoTypography,
            content = content,
        )
    }
}

/**
 * Cores padronizadas do Material 3 para os botões principais preenchidos do app,
 * mantendo azul claro de alto contraste (#A8C7FA) no tema escuro e azul Google no tema claro.
 */
@Composable
fun oraculoPrimaryButtonColors(): ButtonColors = ButtonDefaults.buttonColors(
    containerColor = OraculoColors.PrimaryAction,
    contentColor = OraculoColors.OnPrimaryAction,
    disabledContainerColor = OraculoColors.SurfaceInteractive,
    disabledContentColor = OraculoColors.OnSurfaceMuted.copy(alpha = 0.65f),
)

/**
 * Cores padronizadas para `SingleChoiceSegmentedButtonRow` no padrão Material 3.
 */
@Composable
fun oraculoSegmentedButtonColors(): SegmentedButtonColors = SegmentedButtonDefaults.colors(
    activeContainerColor = OraculoColors.SecondaryContainer,
    activeContentColor = OraculoColors.OnSecondaryContainer,
    activeBorderColor = OraculoColors.Outline,
    inactiveContainerColor = Color.Transparent,
    inactiveContentColor = OraculoColors.OnSurfaceMuted,
    inactiveBorderColor = OraculoColors.Outline,
)

/**
 * Cores padronizadas para `Switch` com alto contraste no estado desligado
 * nos temas claro e escuro.
 */
@Composable
fun oraculoSwitchColors(): SwitchColors = SwitchDefaults.colors(
    checkedThumbColor = OraculoColors.OnPrimaryAction,
    checkedTrackColor = OraculoColors.PrimaryAction,
    checkedBorderColor = OraculoColors.PrimaryAction,
    uncheckedThumbColor = OraculoColors.OnSurfaceMuted,
    uncheckedTrackColor = OraculoColors.SurfaceInteractive,
    uncheckedBorderColor = OraculoColors.OnSurfaceMuted.copy(alpha = 0.65f),
)

/**
 * Botão de ação padronizado do deck inferior utilizando componentes nativos
 * `Button` / `FilledTonalButton` do Material 3 sem animações ou bordas extras.
 */
@Composable
fun OraculoDeckButton(
    label: String,
    icon: ImageVector,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    active: Boolean = false,
    accent: Boolean = false,
    height: Dp = OraculoMetrics.ControlHeightDeck,
    cornerRadius: Dp = OraculoMetrics.CornerLarge,
) {
    val shape = RoundedCornerShape(cornerRadius)
    val contentRow: @Composable () -> Unit = {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(19.dp),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = label,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }

    if (active) {
        Button(
            onClick = onClick,
            enabled = enabled,
            shape = shape,
            colors = oraculoPrimaryButtonColors(),
            contentPadding = PaddingValues(horizontal = 12.dp),
            modifier = modifier.height(height),
        ) {
            contentRow()
        }
    } else {
        FilledTonalButton(
            onClick = onClick,
            enabled = enabled,
            shape = shape,
            colors = ButtonDefaults.filledTonalButtonColors(
                containerColor = if (accent) OraculoColors.SecondaryContainer else OraculoColors.Surface,
                contentColor = if (accent) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
                disabledContainerColor = OraculoColors.Surface.copy(alpha = 0.55f),
                disabledContentColor = OraculoColors.OnSurfaceMuted,
            ),
            contentPadding = PaddingValues(horizontal = 12.dp),
            modifier = modifier.height(height),
        ) {
            contentRow()
        }
    }
}
