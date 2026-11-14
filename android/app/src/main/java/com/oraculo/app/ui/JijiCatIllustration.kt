package com.oraculo.app.ui

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
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
import androidx.compose.ui.geometry.CornerRadius
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

private const val SCENE_WIDTH = 650f
private const val SCENE_HEIGHT = 514f

@Immutable
private data class JijiMascotPalette(
    val dropShadowCenter: Color,
    val dropShadowMid: Color,
    val dropShadowAlphaCenter: Float,
    val dropShadowAlphaMid: Float,
    val contactShadow: Color,
    val catContactAlpha: Float,
    val laptopContactAlpha: Float,
    val furTop: Color,
    val furMid: Color,
    val furBottom: Color,
    val furOutline: Color,
    val rimLight: Color,
    val tailStroke: Color,
    val tailRim: Color,
    val tailRimAlpha: Float,
    val earInnerTop: Color,
    val earInnerBottom: Color,
    val pawFill: Color,
    val pawStroke: Color,
    val whiskerStroke: Color,
    val eyeIris: Color,
    val eyePupil: Color,
    val eyeCatchlight: Color,
    val noseFill: Color,
    val laptopTop: Color,
    val laptopMid: Color,
    val laptopBottom: Color,
    val laptopStroke: Color,
    val screenTop: Color,
    val screenBottom: Color,
    val crackStroke: Color,
    val hingeFill: Color,
    val baseEdgeFill: Color,
    val keyWellFill: Color,
    val keyFill: Color,
    val accentKeyFill: Color,
    val touchpadFill: Color,
)

private val LightJijiPalette = JijiMascotPalette(
    dropShadowCenter = Color(0xFF0C0E12),
    dropShadowMid = Color(0xFF0C0E12),
    dropShadowAlphaCenter = 0.42f,
    dropShadowAlphaMid = 0.16f,
    contactShadow = Color(0xFF090B0E),
    catContactAlpha = 0.56f,
    laptopContactAlpha = 0.50f,
    furTop = Color(0xFF12151C),
    furMid = Color(0xFF080A0E),
    furBottom = Color(0xFF030406),
    furOutline = Color(0xFF020305),
    rimLight = Color.Transparent,
    tailStroke = Color(0xFF040508),
    tailRim = Color(0xFF2A3548),
    tailRimAlpha = 0.55f,
    earInnerTop = Color(0xFF243047),
    earInnerBottom = Color(0xFF131A29),
    pawFill = Color(0xFF06070B),
    pawStroke = Color(0xFF232936),
    whiskerStroke = Color(0xFF232936),
    eyeIris = Color(0xFF555E6E),
    eyePupil = Color(0xFF030406),
    eyeCatchlight = Color(0xFF8C96A8),
    noseFill = Color(0xFF1E283B),
    laptopTop = Color(0xFF27344B),
    laptopMid = Color(0xFF1E283B),
    laptopBottom = Color(0xFF131A26),
    laptopStroke = Color(0xFF05070B),
    screenTop = Color(0xFF131924),
    screenBottom = Color(0xFF090C11),
    crackStroke = Color(0xFF34435E),
    hingeFill = Color(0xFF151921),
    baseEdgeFill = Color(0xFF161E2E),
    keyWellFill = Color(0xFF0B0F17),
    keyFill = Color(0xFF1E283B),
    accentKeyFill = Color(0xFF2F405E),
    touchpadFill = Color(0xFF171F2E),
)

private val DarkJijiPalette = JijiMascotPalette(
    dropShadowCenter = Color(0xFF06080C),
    dropShadowMid = Color(0xFF080A0F),
    dropShadowAlphaCenter = 0.86f,
    dropShadowAlphaMid = 0.42f,
    contactShadow = Color(0xFF020305),
    catContactAlpha = 0.90f,
    laptopContactAlpha = 0.86f,
    furTop = Color(0xFF101319),
    furMid = Color(0xFF07090D),
    furBottom = Color(0xFF020305),
    furOutline = Color(0xFF020305),
    rimLight = Color(0xFF324059).copy(alpha = 0.58f),
    tailStroke = Color(0xFF030406),
    tailRim = Color(0xFF3B4C6B),
    tailRimAlpha = 0.72f,
    earInnerTop = Color(0xFF2B3B59),
    earInnerBottom = Color(0xFF161E2E),
    pawFill = Color(0xFF050609),
    pawStroke = Color(0xFF2B364A),
    whiskerStroke = Color(0xFF364359),
    eyeIris = Color(0xFF5E687A),
    eyePupil = Color(0xFF020305),
    eyeCatchlight = Color(0xFF8C96A8),
    noseFill = Color(0xFF26334A),
    laptopTop = Color(0xFF2B3A56),
    laptopMid = Color(0xFF1E283B),
    laptopBottom = Color(0xFF121824),
    laptopStroke = Color(0xFF05070B),
    screenTop = Color(0xFF151C29),
    screenBottom = Color(0xFF090C11),
    crackStroke = Color(0xFF3C4D6E),
    hingeFill = Color(0xFF151921),
    baseEdgeFill = Color(0xFF161E2E),
    keyWellFill = Color(0xFF0A0E15),
    keyFill = Color(0xFF1E283B),
    accentKeyFill = Color(0xFF2F405E),
    touchpadFill = Color(0xFF171F2E),
)

private class JijiScenePaths {
    // Notebook quebrado
    val laptopFrame = parse(
        "M 425 232 L 605 248 C 618 250, 624 258, 620 270 L 592 378 " +
            "C 588 388, 578 394, 566 392 L 398 368 C 386 366, 380 356, 383 344 " +
            "L 406 246 C 409 236, 416 231, 425 232 Z",
    )
    val laptopScreen = parse("M 421 250 L 598 265 L 574 372 L 399 348 Z")
    val crackMain = parse("M 525 259 L 495 308 L 535 322 L 482 359")
    val crackBranch = parse("M 495 308 L 452 295")
    val baseThickness = parse(
        "M 326 408 L 502 440 C 516 443, 528 438, 537 429 L 586 386 " +
            "L 586 405 L 535 450 C 524 459, 510 463, 494 460 L 326 426 " +
            "C 318 424, 316 413, 326 408 Z",
    )
    val keyboardDeck = parse(
        "M 395 360 L 578 384 C 588 386, 590 394, 582 401 L 532 440 " +
            "C 522 448, 510 450, 495 447 L 328 415 C 318 412, 316 404, 326 396 " +
            "L 378 364 C 384 360, 389 359, 395 360 Z",
    )
    val keyboardWell = parse("M 388 370 L 556 392 L 522 422 L 352 394 Z")
    val keyRow1 = poly(388f, 370f, 556f, 392f, 546f, 401f, 376f, 378f)
    val keyRow2 = poly(376f, 378f, 546f, 401f, 534f, 411f, 364f, 386f)
    val keyRow3 = poly(364f, 386f, 534f, 411f, 522f, 422f, 352f, 394f)
    val poppedKey = poly(452f, 382f, 478f, 380f, 474f, 392f, 448f, 393f)
    val touchpad = poly(402f, 408f, 452f, 417f, 440f, 430f, 390f, 421f)
    val fallenKey = poly(365f, 452f, 385f, 455f, 380f, 466f, 360f, 463f)

    // Gatinho preto estilo Ghibli (Jiji)
    val tail = parse("M 185 408 C 125 388, 128 282, 112 225 C 102 190, 78 172, 58 166")
    val tailRim = parse("M 182 396 C 132 376, 134 280, 118 223 C 108 188, 84 170, 62 163")
    val body = parse(
        "M 200 215 C 192 272, 155 335, 155 402 C 155 448, 178 468, 208 468 " +
            "L 258 468 C 288 468, 305 448, 305 402 C 305 335, 272 272, 264 215 Z",
    )
    val bodyRim = parse("M 202 225 C 194 278, 162 338, 162 400 C 162 432, 174 450, 192 460")
    val pawLeft = parse(
        "M 196 370 C 194 408, 195 445, 198 466 C 208 472, 224 472, 231 466 C 233 445, 232 408, 232 370",
    )
    val pawRight = parse(
        "M 232 370 C 232 408, 231 445, 233 466 C 240 472, 256 472, 266 466 C 269 445, 268 408, 265 370",
    )
    val earLeftOuter = parse("M 158 138 C 148 78, 160 18, 178 0 C 200 18, 218 70, 224 110 Z")
    val earLeftInner = parse("M 168 138 C 160 92, 168 44, 180 26 C 195 42, 208 86, 213 118 Z")
    val earRightOuter = parse("M 240 110 C 246 70, 264 18, 286 0 C 304 18, 316 78, 306 138 Z")
    val earRightInner = parse("M 251 118 C 256 86, 269 42, 284 26 C 296 44, 304 92, 296 138 Z")
    val head = parse(
        "M 148 160 C 148 114, 182 96, 232 96 C 282 96, 316 114, 316 160 " +
            "C 316 200, 280 222, 232 222 C 184 222, 148 200, 148 160 Z",
    )
    val whiskerL1 = parse("M 162 164 Q 128 160 98 164")
    val whiskerL2 = parse("M 160 174 Q 125 176 94 184")
    val whiskerL3 = parse("M 166 184 Q 135 192 108 205")
    val whiskerR1 = parse("M 302 164 Q 336 160 366 164")
    val whiskerR2 = parse("M 304 174 Q 339 176 370 184")
    val whiskerR3 = parse("M 298 184 Q 329 192 356 205")

    private fun parse(data: String): Path = PathParser().parsePathString(data).toPath()

    private fun poly(vararg coords: Float): Path = Path().apply {
        moveTo(coords[0], coords[1])
        var i = 2
        while (i < coords.size) {
            lineTo(coords[i], coords[i + 1])
            i += 2
        }
        close()
    }
}

private data class JijiAnimState(
    val breath: Float,
    val tailSwayDeg: Float,
    val headOffsetY: Float,
    val headTiltDeg: Float,
    val blinkScaleY: Float,
    val earTwitchDeg: Float,
    val whiskerFlexDeg: Float,
)

/**
 * Mascote vetorial do estado vazio (gatinho preto estilo Jiji ao lado do notebook)
 * convertido do SVG com sombras adaptadas e animações fluidas.
 */
@Composable
fun JijiCatIllustration(
    modifier: Modifier = Modifier,
) {
    val colors = if (OraculoColors.IsDark) DarkJijiPalette else LightJijiPalette
    val paths = remember { JijiScenePaths() }

    val transition = rememberInfiniteTransition(label = "jijiMascot")
    val breath by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 3200, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "breath",
    )
    val tailRad by transition.animateFloat(
        initialValue = 0f,
        targetValue = (2.0 * PI).toFloat(),
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 4200, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "tail",
    )
    val expressT by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 6400, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "expression",
    )

    val tailWave = sin(tailRad.toDouble()).toFloat()
    val blink = 1f - 0.94f * (
        smoothPulse(expressT, start = 0.24f, peak = 0.268f, end = 0.305f) +
            smoothPulse(expressT, start = 0.32f, peak = 0.342f, end = 0.372f) * 0.85f
        ).coerceIn(0f, 1f)
    val earTwitch = (
        smoothPulse(expressT, start = 0.62f, peak = 0.642f, end = 0.665f) * -6.5f +
            smoothPulse(expressT, start = 0.66f, peak = 0.680f, end = 0.705f) * 2.5f
        )

    val anim = JijiAnimState(
        breath = breath,
        tailSwayDeg = tailWave * 4.6f,
        headOffsetY = -1.6f * breath,
        headTiltDeg = tailWave * 1.1f,
        blinkScaleY = blink.coerceAtLeast(0.05f),
        earTwitchDeg = earTwitch,
        whiskerFlexDeg = tailWave * 1.1f + breath * 0.8f,
    )

    Canvas(
        modifier = modifier.aspectRatio(SCENE_WIDTH / SCENE_HEIGHT),
    ) {
        val scale = size.width / SCENE_WIDTH
        withTransform({
            scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
        }) {
            drawFloorShadows(colors, anim.breath)
            drawBrokenLaptop(colors, paths)
            drawJijiCat(colors, paths, anim)
        }
    }
}

private fun DrawScope.drawFloorShadows(
    c: JijiMascotPalette,
    breath: Float,
) {
    drawEllipticalRadialGradient(
        cx = 355f,
        cy = 468f,
        rx = 265f,
        ry = 38f,
        colorStops = arrayOf(
            0f to c.dropShadowCenter.copy(alpha = c.dropShadowAlphaCenter),
            0.65f to c.dropShadowMid.copy(alpha = c.dropShadowAlphaMid),
            1f to Color.Transparent,
        ),
    )

    drawCenteredOval(
        color = c.contactShadow,
        cx = 232f,
        cy = 466f,
        rx = 85f + breath * 2f,
        ry = 18f,
        alpha = c.catContactAlpha,
    )

    withTransform({
        rotate(degrees = 4f, pivot = Offset(465f, 445f))
    }) {
        drawCenteredOval(
            color = c.contactShadow,
            cx = 465f,
            cy = 445f,
            rx = 145f,
            ry = 26f,
            alpha = c.laptopContactAlpha,
        )
    }
}

private fun DrawScope.drawBrokenLaptop(
    c: JijiMascotPalette,
    p: JijiScenePaths,
) {
    val frameBrush = Brush.linearGradient(
        colorStops = arrayOf(0f to c.laptopTop, 0.55f to c.laptopMid, 1f to c.laptopBottom),
        start = Offset(380f, 232f),
        end = Offset(620f, 392f),
    )
    val deckBrush = Brush.linearGradient(
        colorStops = arrayOf(0f to c.laptopTop, 0.55f to c.laptopMid, 1f to c.laptopBottom),
        start = Offset(320f, 360f),
        end = Offset(586f, 448f),
    )
    val screenBrush = Brush.linearGradient(
        colors = listOf(c.screenTop, c.screenBottom),
        start = Offset(399f, 250f),
        end = Offset(598f, 372f),
    )

    // Moldura da tela e tela escura com trinca
    drawPath(p.laptopFrame, brush = frameBrush)
    drawPath(p.laptopFrame, color = c.laptopStroke, style = Stroke(width = 5f, join = StrokeJoin.Round))
    if (c.rimLight.alpha > 0f) {
        drawPath(p.laptopFrame, color = c.rimLight, style = Stroke(width = 1.2f, join = StrokeJoin.Round))
    }

    drawPath(p.laptopScreen, brush = screenBrush)
    drawPath(p.laptopScreen, color = c.laptopStroke, style = Stroke(width = 4f, join = StrokeJoin.Round))

    drawPath(
        p.crackMain,
        color = c.crackStroke,
        style = Stroke(width = 2.5f, cap = StrokeCap.Round, join = StrokeJoin.Round),
    )
    drawPath(
        p.crackBranch,
        color = c.crackStroke,
        style = Stroke(width = 2.0f, cap = StrokeCap.Round),
    )

    // Dobradiças soltas
    drawRotatedHinge(415f, 352f, 8f, c)
    drawRotatedHinge(530f, 368f, 14f, c)

    // Base 3D e teclado
    drawPath(p.baseThickness, color = c.baseEdgeFill)
    drawPath(p.baseThickness, color = c.laptopStroke, style = Stroke(width = 5f, join = StrokeJoin.Round))

    drawPath(p.keyboardDeck, brush = deckBrush)
    drawPath(p.keyboardDeck, color = c.laptopStroke, style = Stroke(width = 5f, join = StrokeJoin.Round))
    if (c.rimLight.alpha > 0f) {
        drawPath(p.keyboardDeck, color = c.rimLight, style = Stroke(width = 1.2f, join = StrokeJoin.Round))
    }

    drawPath(p.keyboardWell, color = c.keyWellFill)
    drawPath(p.keyboardWell, color = c.laptopStroke, style = Stroke(width = 3.5f, join = StrokeJoin.Round))

    for (row in arrayOf(p.keyRow1, p.keyRow2, p.keyRow3)) {
        drawPath(row, color = c.keyFill)
        drawPath(row, color = c.laptopStroke, style = Stroke(width = 2.5f, join = StrokeJoin.Round))
    }

    drawLine(c.laptopStroke, Offset(418f, 374f), Offset(385f, 399f), strokeWidth = 2.5f)
    drawLine(c.laptopStroke, Offset(452f, 378f), Offset(420f, 405f), strokeWidth = 2.5f)
    drawLine(c.laptopStroke, Offset(486f, 383f), Offset(455f, 411f), strokeWidth = 2.5f)
    drawLine(c.laptopStroke, Offset(520f, 387f), Offset(490f, 416f), strokeWidth = 2.5f)

    // Tecla saltada, touchpad e tecla caída
    drawPath(p.poppedKey, color = c.accentKeyFill)
    drawPath(p.poppedKey, color = c.laptopStroke, style = Stroke(width = 2.5f, join = StrokeJoin.Round))

    drawPath(p.touchpad, color = c.touchpadFill)
    drawPath(p.touchpad, color = c.laptopStroke, style = Stroke(width = 3f, join = StrokeJoin.Round))

    drawPath(p.fallenKey, color = c.accentKeyFill)
    drawPath(p.fallenKey, color = c.laptopStroke, style = Stroke(width = 2.5f, join = StrokeJoin.Round))
}

private fun DrawScope.drawRotatedHinge(
    x: Float,
    y: Float,
    degrees: Float,
    c: JijiMascotPalette,
) {
    withTransform({
        rotate(degrees = degrees, pivot = Offset(x, y))
    }) {
        drawRoundRect(
            color = c.hingeFill,
            topLeft = Offset(x, y),
            size = Size(26f, 10f),
            cornerRadius = CornerRadius(3f, 3f),
        )
        drawRoundRect(
            color = c.laptopStroke,
            topLeft = Offset(x, y),
            size = Size(26f, 10f),
            cornerRadius = CornerRadius(3f, 3f),
            style = Stroke(width = 3.5f),
        )
    }
}

private fun DrawScope.drawJijiCat(
    c: JijiMascotPalette,
    p: JijiScenePaths,
    anim: JijiAnimState,
) {
    val bodyBrush = Brush.linearGradient(
        colorStops = arrayOf(0f to c.furTop, 0.45f to c.furMid, 1f to c.furBottom),
        start = Offset(185f, 215f),
        end = Offset(282f, 468f),
    )

    // Rabo longo com balanço natural
    withTransform({
        rotate(degrees = anim.tailSwayDeg, pivot = Offset(185f, 408f))
    }) {
        drawPath(p.tail, color = c.tailStroke, style = Stroke(width = 28f, cap = StrokeCap.Round))
        drawPath(
            p.tailRim,
            color = c.tailRim,
            alpha = c.tailRimAlpha,
            style = Stroke(width = 3.5f, cap = StrokeCap.Round),
        )
    }

    // Tronco esguio com respiração sutil
    withTransform({
        scale(scaleX = 1f, scaleY = 1f + anim.breath * 0.014f, pivot = Offset(230f, 468f))
    }) {
        drawPath(p.body, brush = bodyBrush)
        drawPath(p.body, color = c.furOutline, style = Stroke(width = 4f, join = StrokeJoin.Round))
        if (c.rimLight.alpha > 0f) {
            drawPath(p.body, color = c.rimLight, style = Stroke(width = 1.6f, join = StrokeJoin.Round))
        }
        drawPath(
            p.bodyRim,
            color = c.tailRim,
            alpha = if (c.rimLight.alpha > 0f) 0.85f else 0.75f,
            style = Stroke(width = 4.5f, cap = StrokeCap.Round),
        )

        drawPath(p.pawLeft, color = c.pawFill)
        drawPath(p.pawLeft, color = c.pawStroke, style = Stroke(width = 2.5f, cap = StrokeCap.Round))
        drawPath(p.pawRight, color = c.pawFill)
        drawPath(p.pawRight, color = c.pawStroke, style = Stroke(width = 2.5f, cap = StrokeCap.Round))
    }

    // Cabeça, orelhas, bigodes e olhos animados em bloco único pelo pescoço
    withTransform({
        translate(top = anim.headOffsetY)
        rotate(degrees = anim.headTiltDeg, pivot = Offset(232f, 220f))
    }) {
        drawJijiHead(c, p, anim)
    }
}

private fun DrawScope.drawJijiHead(
    c: JijiMascotPalette,
    p: JijiScenePaths,
    anim: JijiAnimState,
) {
    val headBrush = Brush.linearGradient(
        colorStops = arrayOf(0f to c.furTop, 0.45f to c.furMid, 1f to c.furBottom),
        start = Offset(182f, 96f),
        end = Offset(290f, 222f),
    )
    val leftEarBrush = Brush.linearGradient(
        colorStops = arrayOf(0f to c.furTop, 0.45f to c.furMid, 1f to c.furBottom),
        start = Offset(168f, 0f),
        end = Offset(214f, 138f),
    )
    val rightEarBrush = Brush.linearGradient(
        colorStops = arrayOf(0f to c.furTop, 0.45f to c.furMid, 1f to c.furBottom),
        start = Offset(254f, 0f),
        end = Offset(300f, 138f),
    )
    val earInnerBrushLeft = Brush.linearGradient(
        colors = listOf(c.earInnerTop, c.earInnerBottom),
        start = Offset(180f, 26f),
        end = Offset(180f, 138f),
    )
    val earInnerBrushRight = Brush.linearGradient(
        colors = listOf(c.earInnerTop, c.earInnerBottom),
        start = Offset(284f, 26f),
        end = Offset(284f, 138f),
    )

    // Orelha esquerda
    withTransform({ translate(top = 12f) }) {
        drawPath(p.earLeftOuter, brush = leftEarBrush)
        drawPath(p.earLeftOuter, color = c.furOutline, style = Stroke(width = 4f, join = StrokeJoin.Round))
        if (c.rimLight.alpha > 0f) {
            drawPath(p.earLeftOuter, color = c.rimLight, style = Stroke(width = 1.5f, join = StrokeJoin.Round))
        }
    }
    drawPath(p.earLeftInner, brush = earInnerBrushLeft)

    // Orelha direita (com tremor pontual)
    withTransform({
        rotate(degrees = anim.earTwitchDeg, pivot = Offset(276f, 132f))
    }) {
        withTransform({ translate(top = 12f) }) {
            drawPath(p.earRightOuter, brush = rightEarBrush)
            drawPath(p.earRightOuter, color = c.furOutline, style = Stroke(width = 4f, join = StrokeJoin.Round))
            if (c.rimLight.alpha > 0f) {
                drawPath(p.earRightOuter, color = c.rimLight, style = Stroke(width = 1.5f, join = StrokeJoin.Round))
            }
        }
        drawPath(p.earRightInner, brush = earInnerBrushRight)
    }

    // Cabeça arredondada
    drawPath(p.head, brush = headBrush)
    drawPath(p.head, color = c.furOutline, style = Stroke(width = 4f))
    if (c.rimLight.alpha > 0f) {
        drawPath(p.head, color = c.rimLight, style = Stroke(width = 1.5f))
    }

    // Bigodes finos com micro-balanço
    withTransform({
        rotate(degrees = -anim.whiskerFlexDeg, pivot = Offset(162f, 174f))
    }) {
        drawPath(p.whiskerL1, color = c.whiskerStroke, style = Stroke(width = 3f, cap = StrokeCap.Round))
        drawPath(p.whiskerL2, color = c.whiskerStroke, style = Stroke(width = 3f, cap = StrokeCap.Round))
        drawPath(p.whiskerL3, color = c.whiskerStroke, style = Stroke(width = 3f, cap = StrokeCap.Round))
    }
    withTransform({
        rotate(degrees = anim.whiskerFlexDeg, pivot = Offset(302f, 174f))
    }) {
        drawPath(p.whiskerR1, color = c.whiskerStroke, style = Stroke(width = 3f, cap = StrokeCap.Round))
        drawPath(p.whiskerR2, color = c.whiskerStroke, style = Stroke(width = 3f, cap = StrokeCap.Round))
        drawPath(p.whiskerR3, color = c.whiskerStroke, style = Stroke(width = 3f, cap = StrokeCap.Round))
    }

    // Olhos grandes cinza escuro (sem branco puro), centralizados olhando direto para o usuário
    drawJijiEye(
        c = c,
        cx = 198f,
        cy = 154f,
        rx = 23f,
        ry = 25f,
        tiltDeg = -4f,
        pupilCx = 198f,
        pupilCy = 154f,
        catchCx = 193.5f,
        catchCy = 147.5f,
        blinkScaleY = anim.blinkScaleY,
    )
    drawJijiEye(
        c = c,
        cx = 266f,
        cy = 154f,
        rx = 22f,
        ry = 25f,
        tiltDeg = 4f,
        pupilCx = 266f,
        pupilCy = 154f,
        catchCx = 261.5f,
        catchCy = 147.5f,
        blinkScaleY = anim.blinkScaleY,
    )

    // Focinho discreto
    drawCenteredOval(c.noseFill, 232f, 174f, 5f, 2.5f)
}

private fun DrawScope.drawJijiEye(
    c: JijiMascotPalette,
    cx: Float,
    cy: Float,
    rx: Float,
    ry: Float,
    tiltDeg: Float,
    pupilCx: Float,
    pupilCy: Float,
    catchCx: Float,
    catchCy: Float,
    blinkScaleY: Float,
) {
    withTransform({
        rotate(degrees = tiltDeg, pivot = Offset(cx, cy))
        scale(scaleX = 1f, scaleY = blinkScaleY, pivot = Offset(cx, cy))
    }) {
        drawCenteredOval(c.eyeIris, cx, cy, rx, ry)
        drawCenteredOvalStroke(c.furOutline, cx, cy, rx, ry, strokeWidth = 3.5f)
    }

    if (blinkScaleY > 0.25f) {
        withTransform({
            scale(scaleX = 1f, scaleY = blinkScaleY, pivot = Offset(cx, cy))
        }) {
            drawCenteredOval(c.eyePupil, pupilCx, pupilCy, 9.5f, 13.5f)
            drawCircle(c.eyeCatchlight, radius = 2.5f, center = Offset(catchCx, catchCy))
        }
    }
}

private fun DrawScope.drawEllipticalRadialGradient(
    cx: Float,
    cy: Float,
    rx: Float,
    ry: Float,
    vararg colorStops: Pair<Float, Color>,
) {
    withTransform({
        translate(left = cx, top = cy)
        scale(scaleX = 1f, scaleY = ry / rx, pivot = Offset.Zero)
    }) {
        drawCircle(
            brush = Brush.radialGradient(
                colorStops = colorStops,
                center = Offset.Zero,
                radius = rx,
            ),
            radius = rx,
            center = Offset.Zero,
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

private fun smoothPulse(t: Float, start: Float, peak: Float, end: Float): Float {
    if (t <= start || t >= end) return 0f
    val raw = if (t < peak) (t - start) / (peak - start) else (end - t) / (end - peak)
    val u = raw.coerceIn(0f, 1f)
    return u * u * (3f - 2f * u)
}
