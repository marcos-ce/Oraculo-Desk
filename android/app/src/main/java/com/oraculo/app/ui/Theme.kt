package com.oraculo.app.ui

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.TweenSpec
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Tokens compartilhados de dimensões e cantos para manter botões e superfícies
 * proporcionais em todas as telas e orientações.
 */
object OraculoMetrics {
    val ControlHeightCompactChip: Dp = 32.dp
    val ControlHeightSmall: Dp = 36.dp
    val ControlHeightCompactDeck: Dp = 42.dp
    val ControlHeightDialog: Dp = 44.dp
    val ControlHeightInputCompact: Dp = 48.dp
    val ControlHeightDeck: Dp = 50.dp
    val ControlHeightCta: Dp = 52.dp
    val ControlHeightInput: Dp = 56.dp

    val CornerSmall: Dp = 10.dp
    val CornerMedium: Dp = 12.dp
    val CornerLarge: Dp = 14.dp
    val CornerCard: Dp = 18.dp
    val CornerSurface: Dp = 20.dp
}

/**
 * Curvas e durações padronizadas para deixar feedback, transições e painéis
 * ágeis e consistentes em todo o aplicativo.
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

    val TabIndicatorSpring = spring<Dp>(
        dampingRatio = 0.88f,
        stiffness = 700f,
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
        slideInVertically(tween<IntOffset>(RouteEnterMs, easing = Easing)) { it / 16 } +
            fadeIn(tween<Float>(PanelEnterMs, easing = Easing))

    val OverlayExit: ExitTransition =
        slideOutVertically(tween<IntOffset>(RouteExitMs, easing = Easing)) { it / 18 } +
            fadeOut(tween<Float>(PanelExitMs, easing = Easing))
}

@Composable
fun rememberPressedInteraction(): Pair<MutableInteractionSource, Boolean> {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    return source to pressed
}

@Composable
fun Modifier.oraculoPressScale(
    pressed: Boolean,
    targetScale: Float = 0.965f,
): Modifier {
    val scale by animateFloatAsState(
        targetValue = if (pressed) targetScale else 1f,
        animationSpec = OraculoMotion.microTween(),
        label = "oraculoPressScale",
    )
    return graphicsLayer {
        scaleX = scale
        scaleY = scale
    }
}

@Composable
fun OraculoSquareIconButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    active: Boolean = false,
    size: Dp = OraculoMetrics.ControlHeightSmall,
    containerColor: Color = Color.Transparent,
    borderColor: Color? = null,
    content: @Composable () -> Unit,
) {
    val (interactionSource, pressed) = rememberPressedInteraction()
    val shape = RoundedCornerShape(OraculoMetrics.CornerSmall)
    val resolvedBg by animateColorAsState(
        targetValue = if (active) OraculoColors.SecondaryContainer else containerColor,
        animationSpec = OraculoMotion.stateTween(),
        label = "squareIconBg",
    )
    val resolvedBorderColor = when {
        borderColor != null -> borderColor
        active -> OraculoColors.Primary.copy(alpha = 0.55f)
        containerColor != Color.Transparent -> OraculoColors.Outline
        else -> null
    }

    Box(
        modifier = modifier
            .size(size)
            .oraculoPressScale(pressed && enabled)
            .clip(shape)
            .background(resolvedBg)
            .then(
                if (resolvedBorderColor != null) {
                    Modifier.border(1.dp, resolvedBorderColor, shape)
                } else {
                    Modifier
                }
            )
            .clickable(
                interactionSource = interactionSource,
                indication = LocalIndication.current,
                enabled = enabled,
                onClick = onClick,
            ),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

enum class ThemeMode(val prefValue: String, private val ptLabel: String, private val enLabel: String) {
    SYSTEM("system", "Auto", "Auto"),
    LIGHT("light", "Claro", "Light"),
    DARK("dark", "Escuro", "Dark");

    val label: String
        get() = com.oraculo.app.tr(ptLabel, enLabel)

    fun next(): ThemeMode = when (this) {
        SYSTEM -> LIGHT
        LIGHT -> DARK
        DARK -> SYSTEM
    }

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
    Background = Color(0xFF0C0E12),
    Surface = Color(0xFF151921),
    SurfaceRecessed = Color(0xFF090B0E),
    SurfaceInteractive = Color(0xFF1C212C),
    Primary = Color(0xFF5B8DEF),
    OnPrimary = Color(0xFFFFFFFF),
    SecondaryContainer = Color(0xFF1E283B),
    OnSecondaryContainer = Color(0xFFD9E5FF),
    OnSurface = Color(0xFFF0F3F8),
    OnSurfaceMuted = Color(0xFF8C96A8),
    Outline = Color(0xFF232936),
    Error = Color(0xFFF87171),
    OnError = Color(0xFF2E0B0B),
    Success = Color(0xFF34D399),
    TerminalBackground = Color(0xFF090B0E),
    TerminalForeground = Color(0xFFE6EDF3),
    TerminalBorder = Color(0xFF212733),
)

private val OraculoLightPalette = OraculoPalette(
    IsDark = false,
    Background = Color(0xFFF3F5F8),
    Surface = Color(0xFFFFFFFF),
    SurfaceRecessed = Color(0xFFE8ECF2),
    SurfaceInteractive = Color(0xFFF0F3F7),
    Primary = Color(0xFF2563EB),
    OnPrimary = Color(0xFFFFFFFF),
    SecondaryContainer = Color(0xFFE2EBFE),
    OnSecondaryContainer = Color(0xFF173D91),
    OnSurface = Color(0xFF111418),
    OnSurfaceMuted = Color(0xFF5E6775),
    Outline = Color(0xFFE0E4EC),
    Error = Color(0xFFDC2626),
    OnError = Color(0xFFFFFFFF),
    Success = Color(0xFF16A34A),
    TerminalBackground = Color(0xFF101318),
    TerminalForeground = Color(0xFFE6EDF3),
    TerminalBorder = Color(0xFF262D38),
)

private val LocalOraculoPalette = staticCompositionLocalOf { OraculoDarkPalette }

val OraculoColors: OraculoPalette
    @Composable
    @ReadOnlyComposable
    get() = LocalOraculoPalette.current

@Composable
fun isOraculoDarkTheme(themeMode: ThemeMode): Boolean = when (themeMode) {
    ThemeMode.SYSTEM -> isSystemInDarkTheme()
    ThemeMode.LIGHT -> false
    ThemeMode.DARK -> true
}

private val OraculoTypography = Typography(
    displaySmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 30.sp,
        fontWeight = FontWeight.SemiBold,
        letterSpacing = (-0.8).sp,
        lineHeight = 36.sp,
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 20.sp,
        fontWeight = FontWeight.SemiBold,
        letterSpacing = (-0.4).sp,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 16.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = (-0.1).sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 12.sp,
        lineHeight = 16.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 14.sp,
        fontWeight = FontWeight.SemiBold,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontSize = 16.sp,
        lineHeight = 22.sp,
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
            surfaceVariant = palette.SecondaryContainer,
            onSurfaceVariant = palette.OnSurfaceMuted,
            outline = palette.Outline,
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
            surfaceVariant = palette.SecondaryContainer,
            onSurfaceVariant = palette.OnSurfaceMuted,
            outline = palette.Outline,
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
 * Botão de ação padronizado do deck inferior e barra de atalhos principais,
 * com altura fixa, resposta tátil rápida e transição uniforme de estado.
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
    val (interactionSource, pressed) = rememberPressedInteraction()
    val shape = RoundedCornerShape(cornerRadius)
    val bg by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.Surface.copy(alpha = 0.55f)
            active -> OraculoColors.Primary
            accent -> OraculoColors.SecondaryContainer
            else -> OraculoColors.Surface
        },
        animationSpec = OraculoMotion.stateTween(),
        label = "deckBtnBg",
    )
    val border by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.Outline.copy(alpha = 0.5f)
            active -> OraculoColors.Primary
            accent -> OraculoColors.Primary.copy(alpha = 0.5f)
            else -> OraculoColors.Primary.copy(alpha = 0.36f)
        },
        animationSpec = OraculoMotion.stateTween(),
        label = "deckBtnBorder",
    )
    val textColor by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.OnSurfaceMuted
            active -> OraculoColors.OnPrimary
            accent -> OraculoColors.OnSecondaryContainer
            else -> OraculoColors.OnSurface
        },
        animationSpec = OraculoMotion.stateTween(),
        label = "deckBtnText",
    )
    val iconColor by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.OnSurfaceMuted
            active -> OraculoColors.OnPrimary
            else -> OraculoColors.Primary
        },
        animationSpec = OraculoMotion.stateTween(),
        label = "deckBtnIcon",
    )

    Box(
        modifier = modifier
            .height(height)
            .oraculoPressScale(pressed && enabled)
            .clip(shape)
            .background(bg)
            .border(1.dp, border, shape)
            .clickable(
                interactionSource = interactionSource,
                indication = LocalIndication.current,
                enabled = enabled,
                onClick = onClick,
            )
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = iconColor,
                modifier = Modifier.size(19.dp),
            )
            Text(
                text = label,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = textColor,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
