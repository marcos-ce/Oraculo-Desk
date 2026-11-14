package com.oraculo.app.ui

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.PathParser
import kotlin.math.PI
import kotlin.math.sin

private const val CAT_VIEW_WIDTH = 400f
private const val CAT_VIEW_TOP = 34f
private const val CAT_VIEW_HEIGHT = 210f

@Immutable
private data class SleepingCatColors(
    val floorGlowOuter: Color,
    val floorGlowOuterAlpha: Float,
    val floorGlowInner: Color,
    val floorGlowInnerAlpha: Float,
    val bodyTop: Color,
    val bodyBottom: Color,
    val rimStroke: Color,
    val earLeftOuter: Color,
    val earRightOuter: Color,
    val accentTop: Color,
    val accentBottom: Color,
    val tailOuter: Color,
    val tailInner: Color,
    val pawFill: Color,
    val contourStroke: Color,
    val cheekFill: Color,
    val featureStroke: Color,
    val whiskerStroke: Color,
    val sleepZStroke: Color,
)

private val LightCatColors = SleepingCatColors(
    floorGlowOuter = Color(0xFFDBEAFE),
    floorGlowOuterAlpha = 1f,
    floorGlowInner = Color(0xFFBFDBFE),
    floorGlowInnerAlpha = 1f,
    bodyTop = Color(0xFF1E293B),
    bodyBottom = Color(0xFF0F172A),
    rimStroke = Color.Transparent,
    earLeftOuter = Color(0xFF0F172A),
    earRightOuter = Color(0xFF1E293B),
    accentTop = Color(0xFF60A5FA),
    accentBottom = Color(0xFF2563EB),
    tailOuter = Color(0xFF0F172A),
    tailInner = Color(0xFF1E293B),
    pawFill = Color(0xFF1E293B),
    contourStroke = Color(0xFF090D16),
    cheekFill = Color(0xFF38BDF8),
    featureStroke = Color(0xFF93C5FD),
    whiskerStroke = Color(0xFF60A5FA),
    sleepZStroke = Color(0xFF2563EB),
)

private val DarkCatColors = SleepingCatColors(
    floorGlowOuter = Color(0xFF1E40AF),
    floorGlowOuterAlpha = 0.24f,
    floorGlowInner = Color(0xFF3B82F6),
    floorGlowInnerAlpha = 0.20f,
    bodyTop = Color(0xFF27354B),
    bodyBottom = Color(0xFF11192A),
    rimStroke = Color(0xFF60A5FA).copy(alpha = 0.22f),
    earLeftOuter = Color(0xFF131C2E),
    earRightOuter = Color(0xFF233046),
    accentTop = Color(0xFF78B7FF),
    accentBottom = Color(0xFF3B82F6),
    tailOuter = Color(0xFF090E18),
    tailInner = Color(0xFF223046),
    pawFill = Color(0xFF223046),
    contourStroke = Color(0xFF070A12),
    cheekFill = Color(0xFF38BDF8),
    featureStroke = Color(0xFFA4CEFF),
    whiskerStroke = Color(0xFF76B4FF),
    sleepZStroke = Color(0xFF60A5FA),
)

private class SleepingCatPaths {
    val body = parse("M 125 218 C 90 218, 85 135, 165 125 C 240 115, 305 140, 305 192 C 305 212, 285 218, 250 218 Z")
    val bodyArch = parse("M 125 218 C 90 218, 85 135, 165 125 C 240 115, 305 140, 305 192 C 305 212, 285 218, 250 218")
    val bodyHighlight = parse("M 225 155 C 255 160, 275 185, 270 215")
    val tail = parse("M 292 206 C 325 206, 330 224, 280 225 C 240 226, 205 222, 185 218")
    val tailTip = parse("M 205 221 C 195 220, 190 219, 185 218")
    val earLeftOuter = parse("M 92 162 L 76 112 Q 75 104 83 108 L 125 135 Z")
    val earLeftInner = parse("M 95 154 L 84 118 L 115 138 Z")
    val earRightOuter = parse("M 152 135 L 184 104 Q 192 100 190 109 L 182 158 Z")
    val earRightInner = parse("M 158 138 L 182 114 L 176 150 Z")
    val headContour = parse("M 100 206 C 125 222, 165 218, 182 196")
    val eyeLeft = parse("M 98 174 Q 111 185 124 174")
    val eyeRight = parse("M 142 174 Q 155 185 168 174")
    val nose = parse("M 128 185 Q 133 182 138 185 L 133 192 Z")
    val mouth = parse("M 123 195 Q 128 201 133 193 Q 138 201 143 195")
    val zSmall = parse("M 195 110 L 207 110 L 195 122 L 207 122")
    val zMedium = parse("M 218 82 L 234 82 L 218 98 L 234 98")
    val zLarge = parse("M 248 48 L 268 48 L 248 68 L 268 68")

    private fun parse(data: String): Path = PathParser().parsePathString(data).toPath()
}

/**
 * Ilustração vetorial do estado vazio (gatinho dormindo adaptado do SVG original)
 * com calibragem automática de contraste para tema claro e escuro.
 */
@Composable
fun SleepingCatIllustration(
    modifier: Modifier = Modifier,
) {
    val colors = if (OraculoColors.IsDark) DarkCatColors else LightCatColors
    val paths = remember { SleepingCatPaths() }

    val transition = rememberInfiniteTransition(label = "sleepingCat")
    val breathPhase by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 3600, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "breath",
    )
    val sleepCycle by transition.animateFloat(
        initialValue = 0f,
        targetValue = (2.0 * PI).toFloat(),
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 3400),
            repeatMode = RepeatMode.Restart,
        ),
        label = "sleepZ",
    )

    val breathScaleY = 1f + 0.022f * breathPhase

    Canvas(
        modifier = modifier.aspectRatio(CAT_VIEW_WIDTH / CAT_VIEW_HEIGHT),
    ) {
        val scale = size.width / CAT_VIEW_WIDTH
        withTransform({
            scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
            translate(left = 0f, top = -CAT_VIEW_TOP)
        }) {
            drawFloor(colors)
            drawCatBody(colors, paths, breathScaleY)
            drawCatHead(colors, paths)
            drawSleepZzz(colors, paths, sleepCycle)
        }
    }
}

private fun DrawScope.drawFloor(c: SleepingCatColors) {
    drawCenteredOval(c.floorGlowOuter, 200f, 222f, 145f, 18f, alpha = c.floorGlowOuterAlpha)
    drawCenteredOval(c.floorGlowInner, 195f, 220f, 115f, 12f, alpha = c.floorGlowInnerAlpha)
}

private fun DrawScope.drawCatBody(
    c: SleepingCatColors,
    p: SleepingCatPaths,
    breathScaleY: Float,
) {
    val bodyBrush = Brush.linearGradient(
        colors = listOf(c.bodyTop, c.bodyBottom),
        start = Offset(100f, 122f),
        end = Offset(305f, 218f),
    )

    withTransform({
        scale(scaleX = 1f, scaleY = breathScaleY, pivot = Offset(205f, 218f))
    }) {
        drawPath(path = p.body, brush = bodyBrush)
        if (c.rimStroke.alpha > 0f) {
            drawPath(
                path = p.bodyArch,
                color = c.rimStroke,
                style = Stroke(width = 1.6f, cap = StrokeCap.Round, join = StrokeJoin.Round),
            )
        }
        drawPath(
            path = p.bodyHighlight,
            color = Color(0xFF3B82F6),
            alpha = 0.40f,
            style = Stroke(width = 3f, cap = StrokeCap.Round),
        )
    }

    drawPath(path = p.tail, color = c.tailOuter, style = Stroke(width = 18f, cap = StrokeCap.Round))
    drawPath(path = p.tail, color = c.tailInner, style = Stroke(width = 14f, cap = StrokeCap.Round))
    drawPath(
        path = p.tailTip,
        color = Color(0xFF3B82F6),
        alpha = 0.50f,
        style = Stroke(width = 4f, cap = StrokeCap.Round),
    )

    drawCenteredOval(c.pawFill, 168f, 216f, 18f, 9f)
    drawCenteredOvalStroke(c.contourStroke, 168f, 216f, 18f, 9f, strokeWidth = 2f)
}

private fun DrawScope.drawCatHead(
    c: SleepingCatColors,
    p: SleepingCatPaths,
) {
    val leftEarBrush = Brush.linearGradient(
        colors = listOf(c.accentTop, c.accentBottom),
        start = Offset(84f, 118f),
        end = Offset(84f, 154f),
    )
    val rightEarBrush = Brush.linearGradient(
        colors = listOf(c.accentTop, c.accentBottom),
        start = Offset(170f, 114f),
        end = Offset(170f, 150f),
    )
    val headBrush = Brush.linearGradient(
        colors = listOf(c.bodyTop, c.bodyBottom),
        start = Offset(84f, 130f),
        end = Offset(188f, 218f),
    )
    val noseBrush = Brush.linearGradient(
        colors = listOf(c.accentTop, c.accentBottom),
        start = Offset(133f, 182f),
        end = Offset(133f, 192f),
    )

    drawPath(path = p.earLeftOuter, color = c.earLeftOuter)
    drawPath(path = p.earLeftInner, brush = leftEarBrush, alpha = 0.85f)

    drawPath(path = p.earRightOuter, color = c.earRightOuter)
    drawPath(path = p.earRightInner, brush = rightEarBrush, alpha = 0.85f)

    drawOval(
        brush = headBrush,
        topLeft = Offset(136f - 52f, 174f - 44f),
        size = Size(104f, 88f),
    )
    if (c.rimStroke.alpha > 0f) {
        drawCenteredOvalStroke(c.rimStroke, 136f, 174f, 52f, 44f, strokeWidth = 1.4f)
    }

    drawPath(
        path = p.headContour,
        color = c.contourStroke,
        style = Stroke(width = 3f, cap = StrokeCap.Round),
    )

    // Patinha dianteira perto do rosto
    drawCenteredOval(c.pawFill, 96f, 214f, 16f, 10f)
    drawCenteredOvalStroke(c.contourStroke, 96f, 214f, 16f, 10f, strokeWidth = 2f)
    drawLine(c.contourStroke, Offset(88f, 212f), Offset(88f, 219f), strokeWidth = 2f, cap = StrokeCap.Round)
    drawLine(c.contourStroke, Offset(95f, 213f), Offset(95f, 221f), strokeWidth = 2f, cap = StrokeCap.Round)

    // Bochechas e expressões
    drawCenteredOval(c.cheekFill, 100f, 188f, 9f, 5f, alpha = 0.35f)
    drawCenteredOval(c.cheekFill, 164f, 188f, 9f, 5f, alpha = 0.35f)

    drawPath(p.eyeLeft, color = c.featureStroke, style = Stroke(width = 3.5f, cap = StrokeCap.Round))
    drawPath(p.eyeRight, color = c.featureStroke, style = Stroke(width = 3.5f, cap = StrokeCap.Round))

    drawPath(p.nose, brush = noseBrush)
    drawPath(
        p.mouth,
        color = c.featureStroke,
        style = Stroke(width = 2.5f, cap = StrokeCap.Round, join = StrokeJoin.Round),
    )

    // Bigodes
    val whiskerAlpha = 0.62f
    drawLine(c.whiskerStroke, Offset(82f, 182f), Offset(60f, 177f), strokeWidth = 2f, cap = StrokeCap.Round, alpha = whiskerAlpha)
    drawLine(c.whiskerStroke, Offset(80f, 190f), Offset(56f, 190f), strokeWidth = 2f, cap = StrokeCap.Round, alpha = whiskerAlpha)
    drawLine(c.whiskerStroke, Offset(182f, 182f), Offset(204f, 177f), strokeWidth = 2f, cap = StrokeCap.Round, alpha = whiskerAlpha)
    drawLine(c.whiskerStroke, Offset(184f, 190f), Offset(208f, 190f), strokeWidth = 2f, cap = StrokeCap.Round, alpha = whiskerAlpha)
}

private fun DrawScope.drawSleepZzz(
    c: SleepingCatColors,
    p: SleepingCatPaths,
    phaseRad: Float,
) {
    val w1 = 0.5f + 0.5f * sin(phaseRad.toDouble()).toFloat()
    val w2 = 0.5f + 0.5f * sin((phaseRad - 1.1f).toDouble()).toFloat()
    val w3 = 0.5f + 0.5f * sin((phaseRad - 2.2f).toDouble()).toFloat()

    withTransform({ translate(top = -2.5f * w1) }) {
        drawPath(
            path = p.zSmall,
            color = c.sleepZStroke,
            alpha = 0.45f + 0.35f * w1,
            style = Stroke(width = 2.5f, cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
    withTransform({ translate(top = -3.0f * w2) }) {
        drawPath(
            path = p.zMedium,
            color = c.sleepZStroke,
            alpha = 0.60f + 0.30f * w2,
            style = Stroke(width = 3.0f, cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
    withTransform({ translate(top = -3.5f * w3) }) {
        drawPath(
            path = p.zLarge,
            color = c.sleepZStroke,
            alpha = 0.72f + 0.28f * w3,
            style = Stroke(width = 3.5f, cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
}

private fun DrawScope.drawCenteredOval(
    color: Color,
    cx: Float,
    cy: Float,
    rx: Float,
    ry: Float,
    alpha: Float = 1f,
) {
    drawOval(
        color = color,
        topLeft = Offset(cx - rx, cy - ry),
        size = Size(rx * 2f, ry * 2f),
        alpha = alpha,
    )
}

private fun DrawScope.drawCenteredOvalStroke(
    color: Color,
    cx: Float,
    cy: Float,
    rx: Float,
    ry: Float,
    strokeWidth: Float,
) {
    drawOval(
        color = color,
        topLeft = Offset(cx - rx, cy - ry),
        size = Size(rx * 2f, ry * 2f),
        style = Stroke(width = strokeWidth),
    )
}
