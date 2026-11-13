package com.oraculo.app

import android.content.Context
import android.content.pm.ActivityInfo
import android.graphics.SurfaceTexture
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Surface
import android.view.TextureView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DesktopWindows
import androidx.compose.material.icons.filled.Fullscreen
import androidx.compose.material.icons.filled.FullscreenExit
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.KeyboardHide
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.ScreenRotation
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.movableContentOf
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoDeckButton
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.oraculoPressScale
import com.oraculo.app.ui.rememberPressedInteraction
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.oraculo_mobile_core.InputObserver
import uniffi.oraculo_mobile_core.MobileInputSession
import uniffi.oraculo_mobile_core.MobileScreenPreset
import uniffi.oraculo_mobile_core.MobileScreenSession
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.SavedHostInfo
import uniffi.oraculo_mobile_core.ScreenObserver
import uniffi.oraculo_mobile_core.ScreenStreamInfo
import uniffi.oraculo_mobile_core.ScreenVideoFrame
import kotlin.math.roundToInt

private const val TAG_STREAM = "OraculoScreen"
private const val PREF_SCREEN_PRESET = "screen_preset_v2"
private const val PRESET_VALUE_720P30 = "720p30"
private const val PRESET_VALUE_1080P30 = "1080p30"
private const val PRESET_VALUE_1080P60 = "1080p60"

private const val MIN_ZOOM_SCALE = 1f
private const val MAX_ZOOM_SCALE = 8f
private const val DOUBLE_TAP_ZOOM_SCALE = 2.5f

private fun MobileScreenPreset.toPrefValue(): String = when (this) {
    MobileScreenPreset.CRISP1080P30 -> PRESET_VALUE_1080P30
    MobileScreenPreset.SMOOTH1080P60 -> PRESET_VALUE_1080P60
    MobileScreenPreset.BALANCED720P30 -> PRESET_VALUE_720P30
}

private fun presetFromPref(raw: String?): MobileScreenPreset = when (raw) {
    PRESET_VALUE_1080P60 -> MobileScreenPreset.SMOOTH1080P60
    PRESET_VALUE_720P30 -> MobileScreenPreset.BALANCED720P30
    else -> MobileScreenPreset.CRISP1080P30
}

private fun MobileScreenPreset.nextPreset(): MobileScreenPreset = when (this) {
    MobileScreenPreset.CRISP1080P30 -> MobileScreenPreset.SMOOTH1080P60
    MobileScreenPreset.SMOOTH1080P60 -> MobileScreenPreset.BALANCED720P30
    MobileScreenPreset.BALANCED720P30 -> MobileScreenPreset.CRISP1080P30
}

@Composable
internal fun ScreenStreamingScreen(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    onExit: () -> Unit,
) {
    val context = LocalContext.current
    val view = LocalView.current
    val haptic = LocalHapticFeedback.current
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    val activity = remember(context) { context.findActivity() }
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() }
    val prefs = remember(context) { context.getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE) }

    val pointerSpeed = remember { prefs.getFloat("touchpad_speed", 1.5f) }
    val naturalScroll = remember { prefs.getBoolean("touchpad_natural_scroll", false) }

    var preset by remember(host.endpointId) {
        mutableStateOf(presetFromPref(prefs.getString(PREF_SCREEN_PRESET, PRESET_VALUE_1080P30)))
    }
    var isLandscape by remember { mutableStateOf(false) }
    var showKeyboard by remember { mutableStateOf(false) }

    var screenSession by remember { mutableStateOf<MobileScreenSession?>(null) }
    var screenConnecting by remember { mutableStateOf(true) }
    var screenConnectHint by remember { mutableStateOf("Conectando vídeo...") }
    var screenError by remember { mutableStateOf<String?>(null) }
    var streamInfo by remember { mutableStateOf<ScreenStreamInfo?>(null) }
    var videoWidth by remember { mutableIntStateOf(0) }
    var videoHeight by remember { mutableIntStateOf(0) }
    var screenRetryNonce by remember { mutableIntStateOf(0) }
    var monitorSwitching by remember { mutableStateOf(false) }
    var showMonitorSetupDialog by remember { mutableStateOf(false) }
    var monitorReauthPending by remember { mutableStateOf(false) }
    var streamNotice by remember { mutableStateOf<String?>(null) }

    var inputSession by remember { mutableStateOf<MobileInputSession?>(null) }
    var inputConnecting by remember { mutableStateOf(true) }
    var inputConnectHint by remember { mutableStateOf("Conectando touchpad...") }
    var inputError by remember { mutableStateOf<String?>(null) }
    var inputRetryNonce by remember { mutableIntStateOf(0) }
    var inputAutoRetryCount by remember(host.endpointId) { mutableIntStateOf(0) }
    var pendingInputDelayMs by remember(host.endpointId) { mutableStateOf(0L) }
    var telemetryReadout by remember { mutableStateOf<String?>(null) }
    var lastTelemetryAtMs by remember { mutableStateOf(0L) }

    val scope = rememberCoroutineScope()
    var zoomScale by remember { mutableFloatStateOf(MIN_ZOOM_SCALE) }
    var panOffset by remember { mutableStateOf(Offset.Zero) }
    var zoomJob by remember { mutableStateOf<Job?>(null) }

    fun publishTelemetry(text: String, throttleMs: Long = 0L) {
        val now = System.currentTimeMillis()
        if (throttleMs > 0L && now - lastTelemetryAtMs < throttleMs) {
            return
        }
        lastTelemetryAtMs = now
        telemetryReadout = text
    }

    lateinit var triggerInputReconnectRef: (Boolean, String) -> Unit
    val keyboardController = rememberRemoteInputController(
        inputSession = inputSession,
        onSessionBroken = { broken ->
            if (inputSession === broken && !inputConnecting) {
                triggerInputReconnectRef(false, "Reconectando touchpad...")
            }
        },
        onPublishTelemetry = { publishTelemetry(it, 0L) },
    )

    fun triggerInputReconnect(resetAttempts: Boolean, hint: String) {
        if (resetAttempts) {
            inputAutoRetryCount = 0
            pendingInputDelayMs = 0L
        } else {
            if (inputAutoRetryCount >= 3) {
                inputSession = null
                inputConnecting = false
                inputError = "Touchpad encerrado por inatividade."
                keyboardController.releaseAllModifiers(null)
                return
            }
            inputAutoRetryCount += 1
            pendingInputDelayMs = if (inputAutoRetryCount == 1) 250L else 850L
        }
        inputSession = null
        inputError = null
        inputConnectHint = hint
        keyboardController.releaseAllModifiers(null)
        inputRetryNonce += 1
    }
    triggerInputReconnectRef = ::triggerInputReconnect

    val currentOnExit by rememberUpdatedState(onExit)

    fun handleExit() {
        showKeyboard = false
        keyboardController.releaseAllModifiers(inputSession)
        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        currentOnExit()
    }

    var isLandscapeFullscreen by remember(host.endpointId) { mutableStateOf(false) }

    BackHandler {
        if (isLandscapeFullscreen) {
            isLandscapeFullscreen = false
        } else if (showKeyboard) {
            showKeyboard = false
        } else {
            handleExit()
        }
    }

    DisposableEffect(view) {
        val previousKeepScreenOn = view.keepScreenOn
        view.keepScreenOn = true
        onDispose {
            view.keepScreenOn = previousKeepScreenOn
        }
    }

    LaunchedEffect(isLandscape, activity) {
        activity?.requestedOrientation = if (isLandscape) {
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        } else {
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        }
    }

    DisposableEffect(activity) {
        onDispose {
            activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
    }

    val decoder = remember {
        ScreenVideoDecoder(
            onVideoSizeChanged = { width, height ->
                mainHandler.post {
                    videoWidth = width
                    videoHeight = height
                }
            },
            onRequestKeyframe = {
                val activeScreen = screenSession ?: return@ScreenVideoDecoder
                CoroutineScope(Dispatchers.IO).launch {
                    runCatching { activeScreen.requestKeyframe() }
                }
            },
            onDecoderError = { message ->
                mainHandler.post {
                    screenError = message
                    screenConnecting = false
                }
            },
        )
    }

    var attachedSurface by remember { mutableStateOf<Surface?>(null) }

    DisposableEffect(decoder) {
        onDispose {
            val s = attachedSurface
            attachedSurface = null
            decoder.detachSurface(s)
            decoder.release()
            s?.release()
        }
    }

    DisposableEffect(lifecycleOwner, host.endpointId) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                val curInput = inputSession
                if ((curInput == null && !inputConnecting) || (curInput != null && !curInput.isAlive())) {
                    triggerInputReconnect(resetAttempts = true, hint = "Reconectando touchpad...")
                }
                val curScreen = screenSession
                if ((curScreen == null && !screenConnecting) || (curScreen != null && !curScreen.isAlive())) {
                    screenError = null
                    screenConnectHint = "Reconectando vídeo..."
                    screenRetryNonce += 1
                }
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose {
            owner.lifecycle.removeObserver(observer)
        }
    }

    // Sessão de vídeo H.264 independente do canal de input.
    DisposableEffect(host.endpointId, screenRetryNonce) {
        var active = true
        var createdScreen: MobileScreenSession? = null
        screenConnecting = true
        screenError = null
        streamInfo = null
        monitorSwitching = false
        monitorReauthPending = false
        screenConnectHint = "Abrindo transmissão da tela..."
        val openingPreset = preset

        val hintJob = CoroutineScope(Dispatchers.Main).launch {
            delay(1100)
            if (active && screenConnecting) {
                screenConnectHint = "Se aparecer aviso no PC, autorize o compartilhamento de tela"
            }
        }

        val observer = object : ScreenObserver {
            override fun onStreamReady(info: ScreenStreamInfo) {
                Log.i(
                    TAG_STREAM,
                    "onStreamReady: ${info.width}x${info.height} @${info.fps} (${info.encoder}) mon=${info.monitorIndex}/${info.monitorCount} (avail=${info.availableMonitors})",
                )
                decoder.onStreamReady(info)
                mainHandler.post {
                    if (!active) return@post
                    streamInfo = info
                    if (videoWidth == 0 && info.width > 0u && info.height > 0u) {
                        videoWidth = info.width.toInt()
                        videoHeight = info.height.toInt()
                    }
                    screenConnecting = false
                    screenError = null
                    monitorSwitching = false
                    monitorReauthPending = false
                    if (info.monitorCount > 1u) {
                        showMonitorSetupDialog = false
                    }
                }
            }

            override fun onVideoFrame(frame: ScreenVideoFrame) {
                decoder.submitFrame(frame)
            }

            override fun onStreamNotice(message: String) {
                Log.i(TAG_STREAM, "onStreamNotice: $message")
                mainHandler.post {
                    if (!active) return@post
                    monitorSwitching = false
                    monitorReauthPending = false
                    streamNotice = message
                }
            }

            override fun onStreamError(message: String) {
                Log.w(TAG_STREAM, "onStreamError: $message")
                mainHandler.post {
                    if (!active) return@post
                    screenConnecting = false
                    monitorSwitching = false
                    monitorReauthPending = false
                    screenSession = null
                    screenError = message
                }
            }

            override fun onStreamClosed(reason: String) {
                Log.w(TAG_STREAM, "onStreamClosed: $reason")
                mainHandler.post {
                    if (!active) return@post
                    screenConnecting = false
                    monitorSwitching = false
                    monitorReauthPending = false
                    screenSession = null
                    screenError = reason.ifBlank { "Transmissão de tela encerrada." }
                }
            }
        }

        val openJob = CoroutineScope(Dispatchers.IO).launch {
            delay(50)
            if (!active) return@launch
            Log.i(TAG_STREAM, "openScreen iniciando preset=$openingPreset")
            val result = runCatching { client.openScreen(host.endpointId, openingPreset, observer) }
            withContext(Dispatchers.Main) {
                if (!active) {
                    result.onSuccess { session ->
                        CoroutineScope(Dispatchers.IO).launch {
                            runCatching {
                                session.closeSession()
                                session.close()
                            }
                        }
                    }
                    return@withContext
                }
                hintJob.cancel()
                result.onSuccess { session ->
                    Log.i(TAG_STREAM, "openScreen conectado")
                    createdScreen = session
                    screenSession = session
                }.onFailure { err ->
                    Log.w(TAG_STREAM, "openScreen falhou", err)
                    screenConnecting = false
                    screenSession = null
                    screenError = formatOraculoError(err)
                }
            }
        }

        onDispose {
            active = false
            hintJob.cancel()
            openJob.cancel()
            val toClose = createdScreen ?: screenSession
            screenSession = null
            if (toClose != null) {
                CoroutineScope(Dispatchers.IO).launch {
                    runCatching {
                        toClose.closeSession()
                        toClose.close()
                    }
                }
            }
        }
    }

    LaunchedEffect(inputSession) {
        val session = inputSession ?: return@LaunchedEffect
        delay(4_000L)
        if (inputSession === session && session.isAlive()) {
            inputAutoRetryCount = 0
        }
    }

    // Canal de input dedicado ao mini-touchpad e teclado enquanto estiver no modo streaming.
    DisposableEffect(host.endpointId, inputRetryNonce) {
        var active = true
        var createdInput: MobileInputSession? = null
        val waitMs = maxOf(pendingInputDelayMs, 110L)
        pendingInputDelayMs = 0L
        inputConnecting = true
        inputError = null
        if (inputAutoRetryCount == 0) {
            inputConnectHint = "Conectando touchpad..."
        }
        keyboardController.resetState(null)

        val observer = object : InputObserver {
            override fun onSessionClosed(reason: String) {
                Log.w(TAG_STREAM, "input onSessionClosed: $reason")
                mainHandler.post {
                    if (!active) return@post
                    if (createdInput != null && inputSession === createdInput) {
                        createdInput = null
                        triggerInputReconnect(resetAttempts = false, hint = "Reconectando touchpad...")
                    }
                }
            }
        }

        val openJob = CoroutineScope(Dispatchers.IO).launch {
            delay(waitMs)
            if (!active) return@launch
            val result = runCatching { client.openInput(host.endpointId, observer) }
            withContext(Dispatchers.Main) {
                if (!active) {
                    result.onSuccess { session ->
                        CoroutineScope(Dispatchers.IO).launch {
                            runCatching {
                                session.closeSession()
                                session.close()
                            }
                        }
                    }
                    return@withContext
                }
                result.onSuccess { session ->
                    inputConnecting = false
                    pendingInputDelayMs = 0L
                    createdInput = session
                    inputSession = session
                }.onFailure { err ->
                    Log.w(TAG_STREAM, "openInput falhou", err)
                    if (inputAutoRetryCount in 1 until 3) {
                        triggerInputReconnect(resetAttempts = false, hint = "Reconectando touchpad...")
                    } else {
                        inputConnecting = false
                        inputError = formatOraculoError(err)
                    }
                }
            }
        }

        onDispose {
            active = false
            openJob.cancel()
            val toClose = createdInput ?: inputSession
            keyboardController.releaseAllModifiers(toClose)
            inputSession = null
            if (toClose != null) {
                CoroutineScope(Dispatchers.IO).launch {
                    runCatching {
                        toClose.closeSession()
                        toClose.close()
                    }
                }
            }
        }
    }

    LaunchedEffect(streamNotice) {
        val current = streamNotice ?: return@LaunchedEffect
        delay(4_200L)
        if (streamNotice == current) {
            streamNotice = null
        }
    }

    LaunchedEffect(monitorSwitching) {
        if (monitorSwitching) {
            delay(4_000L)
            monitorSwitching = false
        }
    }

    fun switchMonitor() {
        val activeSession = screenSession
        val info = streamInfo
        if (activeSession == null || !activeSession.isAlive() || monitorSwitching) {
            return
        }
        val count = info?.monitorCount?.toInt() ?: 0
        if (count <= 1) {
            showMonitorSetupDialog = true
            return
        }
        monitorSwitching = true
        CoroutineScope(Dispatchers.IO).launch {
            val sent = runCatching { activeSession.switchMonitor() }.isSuccess
            if (!sent) {
                mainHandler.post {
                    monitorSwitching = false
                    screenError = null
                    screenRetryNonce += 1
                }
            }
        }
    }

    fun requestPcMonitorReauth() {
        val activeSession = screenSession
        if (activeSession == null || !activeSession.isAlive() || monitorReauthPending) {
            return
        }
        monitorReauthPending = true
        showMonitorSetupDialog = false
        CoroutineScope(Dispatchers.IO).launch {
            val sent = runCatching { activeSession.reauthorizeMonitors() }.isSuccess
            if (!sent) {
                mainHandler.post {
                    monitorReauthPending = false
                    streamNotice = "Sessão indisponível no momento."
                }
            }
        }
    }

    fun cyclePreset() {
        val next = preset.nextPreset()
        preset = next
        prefs.edit().putString(PREF_SCREEN_PRESET, next.toPrefValue()).apply()
        val activeSession = screenSession
        if (activeSession != null && activeSession.isAlive()) {
            CoroutineScope(Dispatchers.IO).launch {
                val ok = runCatching { activeSession.setPreset(next) }.isSuccess
                if (!ok) {
                    mainHandler.post {
                        screenError = null
                        screenRetryNonce += 1
                    }
                }
            }
        } else if (!screenConnecting) {
            screenError = null
            screenRetryNonce += 1
        }
    }

    fun animateZoomTo(targetScale: Float, targetPan: Offset) {
        val startScale = zoomScale
        val startPan = panOffset
        zoomJob?.cancel()
        zoomJob = scope.launch {
            val progress = Animatable(0f)
            progress.animateTo(
                targetValue = 1f,
                animationSpec = OraculoMotion.panelTween(enter = true),
            ) {
                zoomScale = startScale + (targetScale - startScale) * value
                panOffset = Offset(
                    x = startPan.x + (targetPan.x - startPan.x) * value,
                    y = startPan.y + (targetPan.y - startPan.y) * value,
                )
            }
        }
    }

    fun resetZoom(animated: Boolean = true) {
        if (animated && (zoomScale > 1.01f || panOffset != Offset.Zero)) {
            animateZoomTo(MIN_ZOOM_SCALE, Offset.Zero)
        } else {
            zoomJob?.cancel()
            zoomScale = MIN_ZOOM_SCALE
            panOffset = Offset.Zero
        }
    }

    @Composable
    fun StreamStatusChip(
        compactText: Boolean = false,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
        modifier: Modifier = Modifier,
    ) {
        val info = streamInfo
        val dotColor = when {
            screenError != null -> OraculoColors.Error
            info != null -> OraculoColors.Success
            else -> OraculoColors.Primary
        }
        val label = when {
            screenError != null -> "Offline"
            info != null && compactText -> "${info.height}p·${info.fps}"
            info != null -> "${info.width}×${info.height} · ${info.fps}fps"
            else -> "..."
        }
        StreamControlPill(height = pillHeight, modifier = modifier) {
            Box(
                modifier = Modifier
                    .size(6.dp)
                    .clip(CircleShape)
                    .background(dotColor),
            )
            Text(
                text = label,
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontFeatureSettings = "tnum",
                ),
                color = OraculoColors.OnSurfaceMuted,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }

    @Composable
    fun ZoomResetChip(pillHeight: Dp = OraculoMetrics.ControlHeightSmall) {
        if (zoomScale > 1.05f) {
            val zoomTenths = (zoomScale * 10f).roundToInt()
            val zoomLabel = "${zoomTenths / 10}.${zoomTenths % 10}x"
            StreamControlPill(
                onClick = { resetZoom() },
                containerColor = OraculoColors.SecondaryContainer,
                accentBorder = OraculoColors.Primary.copy(alpha = 0.65f),
                height = pillHeight,
            ) {
                Text(
                    text = "$zoomLabel · 1x",
                    style = TextStyle(
                        fontFamily = FontFamily.Monospace,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold,
                        fontFeatureSettings = "tnum",
                    ),
                    color = OraculoColors.Primary,
                    maxLines = 1,
                    softWrap = false,
                )
            }
        }
    }

    @Composable
    fun MonitorSwitchChip(
        compact: Boolean = false,
        modifier: Modifier = Modifier,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        val info = streamInfo ?: return
        val count = info.monitorCount.toInt()
        val available = maxOf(info.availableMonitors.toInt(), count)
        if (count <= 1 && available <= 1) {
            return
        }
        val index = info.monitorIndex.toInt()
        val multi = count > 1
        val label = when {
            multi -> "M${index + 1}/$count"
            compact -> "1/$available"
            else -> "1/$available tel."
        }
        StreamControlPill(
            onClick = if (screenSession != null && !screenConnecting && !monitorSwitching) {
                {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    switchMonitor()
                }
            } else {
                null
            },
            containerColor = if (multi) OraculoColors.SecondaryContainer else OraculoColors.Surface,
            accentBorder = if (multi) OraculoColors.Primary.copy(alpha = 0.65f) else OraculoColors.Outline,
            height = pillHeight,
            modifier = modifier,
        ) {
            Icon(
                imageVector = Icons.Default.DesktopWindows,
                contentDescription = "Alternar monitor",
                tint = if (multi) OraculoColors.Primary else OraculoColors.OnSurfaceMuted,
                modifier = Modifier.size(15.dp),
            )
            Text(
                text = label,
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    fontFeatureSettings = "tnum",
                ),
                color = if (multi) OraculoColors.Primary else OraculoColors.OnSurface,
                maxLines = 1,
                softWrap = false,
            )
        }
    }

    @Composable
    fun PresetCycleChip(
        modifier: Modifier = Modifier,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        val isHigh = preset != MobileScreenPreset.BALANCED720P30
        StreamControlPill(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                cyclePreset()
            },
            accentBorder = if (isHigh) OraculoColors.Primary.copy(alpha = 0.5f) else OraculoColors.Outline,
            height = pillHeight,
            modifier = modifier,
        ) {
            Text(
                text = preset.toPrefValue(),
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    fontFeatureSettings = "tnum",
                ),
                color = if (isHigh) OraculoColors.Primary else OraculoColors.OnSurface,
                maxLines = 1,
                softWrap = false,
            )
        }
    }

    @Composable
    fun KeyboardToggleChip(
        showLabel: Boolean = false,
        modifier: Modifier = Modifier,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        StreamControlPill(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                showKeyboard = !showKeyboard
            },
            containerColor = if (showKeyboard) OraculoColors.SecondaryContainer else OraculoColors.Surface,
            accentBorder = if (showKeyboard) OraculoColors.Primary.copy(alpha = 0.7f) else OraculoColors.Outline,
            height = pillHeight,
            modifier = modifier,
        ) {
            Icon(
                imageVector = if (showKeyboard) Icons.Default.KeyboardHide else Icons.Default.Keyboard,
                contentDescription = if (showKeyboard) "Ocultar teclado" else "Mostrar teclado",
                tint = if (showKeyboard) OraculoColors.Primary else OraculoColors.OnSurface,
                modifier = Modifier.size(16.dp),
            )
            if (showLabel) {
                Text(
                    text = if (showKeyboard) "Fechar" else "Teclado",
                    style = MaterialTheme.typography.labelMedium,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (showKeyboard) OraculoColors.Primary else OraculoColors.OnSurface,
                    maxLines = 1,
                    softWrap = false,
                )
            }
        }
    }

    @Composable
    fun RotateChip(
        showLabel: Boolean = false,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        StreamControlPill(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                isLandscape = !isLandscape
                resetZoom(animated = false)
            },
            containerColor = if (isLandscape) OraculoColors.SecondaryContainer else OraculoColors.Surface,
            accentBorder = if (isLandscape) OraculoColors.Primary.copy(alpha = 0.65f) else OraculoColors.Outline,
            height = pillHeight,
        ) {
            Icon(
                imageVector = Icons.Default.ScreenRotation,
                contentDescription = "Girar tela",
                tint = if (isLandscape) OraculoColors.Primary else OraculoColors.OnSurface,
                modifier = Modifier.size(15.dp),
            )
            if (showLabel) {
                Text(
                    text = "Girar",
                    style = MaterialTheme.typography.labelMedium,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (isLandscape) OraculoColors.Primary else OraculoColors.OnSurface,
                    maxLines = 1,
                    softWrap = false,
                )
            }
        }
    }

    @Composable
    fun FullscreenChip(
        fullscreen: Boolean,
        onToggle: () -> Unit,
        modifier: Modifier = Modifier,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
        translucent: Boolean = false,
    ) {
        StreamControlPill(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onToggle()
            },
            containerColor = when {
                translucent -> Color.Black.copy(alpha = 0.56f)
                fullscreen -> OraculoColors.SecondaryContainer
                else -> OraculoColors.Surface
            },
            accentBorder = when {
                translucent -> Color.White.copy(alpha = 0.24f)
                fullscreen -> OraculoColors.Primary.copy(alpha = 0.65f)
                else -> OraculoColors.Outline
            },
            height = pillHeight,
            modifier = modifier,
        ) {
            Icon(
                imageVector = if (fullscreen) Icons.Default.FullscreenExit else Icons.Default.Fullscreen,
                contentDescription = if (fullscreen) "Sair da tela cheia" else "Tela cheia",
                tint = when {
                    translucent -> Color.White.copy(alpha = 0.92f)
                    fullscreen -> OraculoColors.Primary
                    else -> OraculoColors.OnSurface
                },
                modifier = Modifier.size(16.dp),
            )
        }
    }

    @Composable
    fun ExitChip(
        compact: Boolean = false,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        StreamControlPill(
            onClick = { handleExit() },
            accentBorder = OraculoColors.Error.copy(alpha = 0.55f),
            height = pillHeight,
        ) {
            Icon(
                imageVector = Icons.Default.Close,
                contentDescription = "Sair do streaming",
                tint = OraculoColors.Error,
                modifier = Modifier.size(15.dp),
            )
            if (!compact) {
                Text(
                    text = "Sair",
                    style = MaterialTheme.typography.labelMedium,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = OraculoColors.OnSurface,
                    maxLines = 1,
                    softWrap = false,
                )
            }
        }
    }

    @Composable
    fun VideoViewport(modifier: Modifier) {
        BoxWithConstraints(
            modifier = modifier
                .background(Color.Black)
                .clipToBounds(),
        ) {
            val density = LocalDensity.current
            val viewportWidthPx = with(density) { maxWidth.toPx() }
            val viewportHeightPx = with(density) { maxHeight.toPx() }
            val videoAspect = if (videoWidth > 0 && videoHeight > 0) {
                videoWidth.toFloat() / videoHeight.toFloat()
            } else {
                16f / 9f
            }
            val containerAspect = if (viewportHeightPx > 0f) {
                viewportWidthPx / viewportHeightPx
            } else {
                16f / 9f
            }
            val (fittedWidthPx, fittedHeightPx) = if (containerAspect > videoAspect) {
                val h = viewportHeightPx
                (h * videoAspect) to h
            } else {
                val w = viewportWidthPx
                w to (w / videoAspect)
            }
            val fittedWidthDp = with(density) { fittedWidthPx.toDp() }
            val fittedHeightDp = with(density) { fittedHeightPx.toDp() }

            // Permite levar qualquer borda do desktop até o centro da tela quando houver zoom,
            // mantendo o enquadramento livre mesmo com tarjas em modo retrato.
            fun clampPan(offset: Offset, scale: Float): Offset {
                if (scale <= 1.01f) return Offset.Zero
                val maxX = ((fittedWidthPx * scale) / 2f).coerceAtLeast(0f)
                val maxY = ((fittedHeightPx * scale) / 2f).coerceAtLeast(0f)
                return Offset(
                    x = offset.x.coerceIn(-maxX, maxX),
                    y = offset.y.coerceIn(-maxY, maxY),
                )
            }

            LaunchedEffect(viewportWidthPx, viewportHeightPx, fittedWidthPx, fittedHeightPx) {
                panOffset = clampPan(panOffset, zoomScale)
            }

            // Área de vídeo nunca envia cursor/cliques ao PC; realiza apenas zoom e pan locais.
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .pointerInput(viewportWidthPx, viewportHeightPx, fittedWidthPx, fittedHeightPx) {
                        detectTapGestures(
                            onDoubleTap = { tapOffset ->
                                if (zoomScale > 1.05f) {
                                    resetZoom()
                                } else {
                                    val center = Offset(viewportWidthPx / 2f, viewportHeightPx / 2f)
                                    val c = tapOffset - center
                                    val nextScale = DOUBLE_TAP_ZOOM_SCALE
                                    animateZoomTo(
                                        nextScale,
                                        clampPan(-c * (nextScale - 1f), nextScale),
                                    )
                                }
                            },
                        )
                    }
                    .pointerInput(viewportWidthPx, viewportHeightPx, fittedWidthPx, fittedHeightPx) {
                        detectTransformGestures { centroid, pan, zoom, _ ->
                            zoomJob?.cancel()
                            val oldScale = zoomScale
                            val nextScale = (oldScale * zoom).coerceIn(MIN_ZOOM_SCALE, MAX_ZOOM_SCALE)
                            if (nextScale <= 1.01f) {
                                resetZoom(animated = false)
                            } else {
                                val ratio = nextScale / oldScale
                                val center = Offset(viewportWidthPx / 2f, viewportHeightPx / 2f)
                                val c = centroid - center
                                val rawPan = panOffset * ratio + c * (1f - ratio) + pan
                                zoomScale = nextScale
                                panOffset = clampPan(rawPan, nextScale)
                            }
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    modifier = Modifier
                        .size(width = fittedWidthDp, height = fittedHeightDp)
                        .graphicsLayer {
                            scaleX = zoomScale
                            scaleY = zoomScale
                            translationX = panOffset.x
                            translationY = panOffset.y
                        },
                ) {
                    AndroidView(
                        factory = { ctx ->
                            TextureView(ctx).apply {
                                isOpaque = true
                                surfaceTextureListener = object : TextureView.SurfaceTextureListener {
                                    override fun onSurfaceTextureAvailable(
                                        st: SurfaceTexture,
                                        width: Int,
                                        height: Int,
                                    ) {
                                        val created = Surface(st)
                                        attachedSurface = created
                                        decoder.attachSurface(created)
                                    }

                                    override fun onSurfaceTextureSizeChanged(
                                        st: SurfaceTexture,
                                        width: Int,
                                        height: Int,
                                    ) = Unit

                                    override fun onSurfaceTextureDestroyed(st: SurfaceTexture): Boolean {
                                        val s = attachedSurface
                                        attachedSurface = null
                                        decoder.detachSurface(s)
                                        s?.release()
                                        return true
                                    }

                                    override fun onSurfaceTextureUpdated(st: SurfaceTexture) = Unit
                                }
                            }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }

            if (screenConnecting) {
                Surface(
                    color = Color(0xD90D1117),
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(horizontal = 24.dp)
                        .border(1.dp, OraculoColors.Outline, RoundedCornerShape(14.dp)),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 11.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        CircularProgressIndicator(
                            color = OraculoColors.Primary,
                            modifier = Modifier.size(16.dp),
                            strokeWidth = 2.dp,
                        )
                        Text(
                            text = screenConnectHint,
                            style = MaterialTheme.typography.bodySmall,
                            color = OraculoColors.OnSurface,
                        )
                    }
                }
            } else if (screenError != null) {
                Surface(
                    color = Color(0xE60D1117),
                    shape = RoundedCornerShape(16.dp),
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(horizontal = 24.dp)
                        .border(1.dp, OraculoColors.Error.copy(alpha = 0.5f), RoundedCornerShape(16.dp)),
                ) {
                    Column(
                        modifier = Modifier.padding(horizontal = 18.dp, vertical = 14.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Text(
                            text = screenError ?: "Falha na transmissão.",
                            style = MaterialTheme.typography.bodySmall,
                            color = OraculoColors.Error,
                            textAlign = TextAlign.Center,
                        )
                        Button(
                            onClick = {
                                screenError = null
                                screenRetryNonce += 1
                            },
                            shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = OraculoColors.SecondaryContainer,
                                contentColor = OraculoColors.OnSecondaryContainer,
                            ),
                            modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                        ) {
                            Icon(
                                imageVector = Icons.Default.Refresh,
                                contentDescription = null,
                                modifier = Modifier.size(16.dp),
                            )
                            Spacer(modifier = Modifier.width(6.dp))
                            Text("Tentar novamente")
                        }
                    }
                }
            }
            AnimatedVisibility(
                visible = streamNotice != null && !screenConnecting && screenError == null,
                enter = OraculoMotion.OverlayEnter,
                exit = OraculoMotion.OverlayExit,
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(horizontal = 16.dp, vertical = 12.dp),
            ) {
                Surface(
                    color = Color(0xE60D1117),
                    shape = RoundedCornerShape(12.dp),
                    modifier = Modifier.border(1.dp, OraculoColors.Outline, RoundedCornerShape(12.dp)),
                ) {
                    Text(
                        text = streamNotice.orEmpty(),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurface,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                }
            }
        }
    }

    @Composable
    fun MiniTouchpadSurface(modifier: Modifier) {
        TouchpadSurface(
            inputSession = inputSession,
            isConnecting = inputConnecting,
            connectHint = inputConnectHint,
            errorMsg = inputError,
            pointerSpeed = pointerSpeed,
            naturalScroll = naturalScroll,
            onRetry = { triggerInputReconnect(resetAttempts = true, hint = "Conectando touchpad...") },
            onSessionBroken = { broken ->
                if (inputSession === broken && !inputConnecting) {
                    triggerInputReconnect(resetAttempts = false, hint = "Reconectando touchpad...")
                }
            },
            onDisarmModifiers = { keyboardController.disarmArmedModifiers() },
            telemetryProvider = { telemetryReadout },
            onPublishTelemetry = { text, throttleMs -> publishTelemetry(text, throttleMs) },
            compactMode = true,
            modifier = modifier,
        )
    }

    @Composable
    fun MouseButtonsRow(buttonHeightDp: Int, compactLabels: Boolean) {
        TouchpadMouseButtonsRow(
            enabled = inputSession != null,
            onButtonPress = { button, pressed ->
                if (pressed) haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                keyboardController.sendOnSession { it.pressButton(button, pressed) }
            },
            buttonHeight = buttonHeightDp.dp,
            compactLabels = compactLabels,
        )
    }

    // Mantém a instância de TextureView/Surface viva ao alternar Retrato <-> Paisagem via Girar.
    val sharedVideoViewport = remember {
        movableContentOf<Modifier> { vpModifier ->
            VideoViewport(vpModifier)
        }
    }

    BoxWithConstraints(
        modifier = Modifier
            .fillMaxSize()
            .background(OraculoColors.Background),
    ) {
        val wideSplit = maxWidth > maxHeight
        val availableHeight = maxHeight
        val tightImeHeight = availableHeight < 500.dp
        val showLandscapeKeyboardExtras = showKeyboard && availableHeight >= 300.dp
        val showPortraitShortcuts = showKeyboard && availableHeight >= 520.dp
        val fullscreenActive = wideSplit && isLandscapeFullscreen

        LaunchedEffect(wideSplit) {
            if (!wideSplit) {
                isLandscapeFullscreen = false
            }
        }

        DisposableEffect(activity, view, fullscreenActive) {
            val controller = activity?.window?.let { WindowCompat.getInsetsController(it, view) }
            if (fullscreenActive && controller != null) {
                controller.systemBarsBehavior =
                    WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                controller.hide(WindowInsetsCompat.Type.systemBars())
            } else {
                controller?.show(WindowInsetsCompat.Type.systemBars())
            }
            onDispose {
                controller?.show(WindowInsetsCompat.Type.systemBars())
            }
        }

        val insetsModifier = if (fullscreenActive) {
            Modifier.fillMaxSize()
        } else {
            Modifier
                .fillMaxSize()
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .imePadding()
        }

        Box(modifier = insetsModifier) {
            if (fullscreenActive) {
                sharedVideoViewport(Modifier.fillMaxSize())
                FullscreenChip(
                    fullscreen = true,
                    onToggle = { isLandscapeFullscreen = false },
                    pillHeight = 36.dp,
                    translucent = true,
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(14.dp),
                )
            } else if (wideSplit) {
                val railPillHeight = 34.dp
                Row(modifier = Modifier.fillMaxSize()) {
                    Column(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxHeight(),
                    ) {
                        sharedVideoViewport(
                            Modifier
                                .weight(1f)
                                .fillMaxWidth(),
                        )
                        AnimatedVisibility(
                            visible = showKeyboard,
                            enter = OraculoMotion.ExpandV,
                            exit = OraculoMotion.CollapseV,
                        ) {
                            Surface(color = OraculoColors.SurfaceRecessed) {
                                Column {
                                    HorizontalDivider(thickness = 1.dp, color = OraculoColors.Outline)
                                    RemoteKeyboardBar(
                                        visible = true,
                                        controller = keyboardController,
                                        enabled = inputSession != null,
                                        compact = true,
                                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
                                    )
                                }
                            }
                        }
                    }

                    VerticalDivider(thickness = 1.dp, color = OraculoColors.Outline)

                    Surface(
                        color = OraculoColors.SurfaceRecessed,
                        modifier = Modifier
                            .width(236.dp)
                            .fillMaxHeight(),
                    ) {
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                ExitChip(compact = false, pillHeight = railPillHeight)
                                Row(
                                    modifier = Modifier
                                        .weight(1f, fill = false)
                                        .padding(start = 4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                                ) {
                                    if (zoomScale > 1.05f) {
                                        ZoomResetChip(pillHeight = railPillHeight)
                                    } else {
                                        StreamStatusChip(
                                            compactText = true,
                                            pillHeight = railPillHeight,
                                            modifier = Modifier.weight(1f, fill = false),
                                        )
                                    }
                                    FullscreenChip(
                                        fullscreen = false,
                                        onToggle = {
                                            showKeyboard = false
                                            isLandscapeFullscreen = true
                                        },
                                        pillHeight = railPillHeight,
                                    )
                                    RotateChip(showLabel = false, pillHeight = railPillHeight)
                                }
                            }

                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                KeyboardToggleChip(
                                    showLabel = false,
                                    pillHeight = railPillHeight,
                                )
                                MonitorSwitchChip(
                                    compact = true,
                                    pillHeight = railPillHeight,
                                    modifier = Modifier.weight(1f),
                                )
                                PresetCycleChip(
                                    pillHeight = railPillHeight,
                                    modifier = Modifier.weight(1f),
                                )
                            }

                            AnimatedVisibility(
                                visible = showLandscapeKeyboardExtras,
                                enter = OraculoMotion.ExpandV,
                                exit = OraculoMotion.CollapseV,
                            ) {
                                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                    RemoteModifierRow(
                                        controller = keyboardController,
                                        enabled = inputSession != null,
                                        compact = true,
                                    )
                                    RemoteShortcutStrip(
                                        controller = keyboardController,
                                        enabled = inputSession != null,
                                        compact = true,
                                    )
                                }
                            }

                            MiniTouchpadSurface(
                                modifier = Modifier
                                    .weight(1f)
                                    .fillMaxWidth(),
                            )

                            MouseButtonsRow(
                                buttonHeightDp = 38,
                                compactLabels = true,
                            )
                        }
                    }
                }
            } else {
                val deckBtnHeightDp = 42
                Column(modifier = Modifier.fillMaxSize()) {
                    Surface(color = OraculoColors.SurfaceRecessed) {
                        Column {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 10.dp, vertical = 7.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                                ) {
                                    ExitChip(compact = false)
                                    if (zoomScale > 1.05f) {
                                        ZoomResetChip()
                                    } else {
                                        StreamStatusChip(compactText = true)
                                    }
                                }

                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                                ) {
                                    MonitorSwitchChip(compact = true)
                                    PresetCycleChip()
                                    RotateChip(showLabel = false)
                                }
                            }
                            HorizontalDivider(thickness = 1.dp, color = OraculoColors.Outline)
                        }
                    }

                    sharedVideoViewport(
                        Modifier
                            .weight(1f)
                            .fillMaxWidth(),
                    )

                    Surface(color = OraculoColors.SurfaceRecessed) {
                        Column(modifier = Modifier.fillMaxWidth()) {
                            HorizontalDivider(thickness = 1.dp, color = OraculoColors.Outline)
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 10.dp, vertical = 8.dp),
                                verticalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                if (!showKeyboard || !tightImeHeight) {
                                    MiniTouchpadSurface(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .height(if (showKeyboard) 72.dp else 124.dp),
                                    )
                                }

                                OraculoDeckButton(
                                    label = if (showKeyboard) "Fechar teclado" else "Abrir teclado",
                                    icon = if (showKeyboard) Icons.Default.KeyboardHide else Icons.Default.Keyboard,
                                    onClick = {
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        showKeyboard = !showKeyboard
                                    },
                                    active = showKeyboard,
                                    height = deckBtnHeightDp.dp,
                                    cornerRadius = OraculoMetrics.CornerMedium,
                                    modifier = Modifier.fillMaxWidth(),
                                )

                                AnimatedVisibility(
                                    visible = showKeyboard,
                                    enter = OraculoMotion.ExpandV,
                                    exit = OraculoMotion.CollapseV,
                                ) {
                                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                        RemoteModifierRow(
                                            controller = keyboardController,
                                            enabled = inputSession != null,
                                            compact = true,
                                        )
                                        if (showPortraitShortcuts) {
                                            RemoteShortcutStrip(
                                                controller = keyboardController,
                                                enabled = inputSession != null,
                                                compact = true,
                                            )
                                        }
                                        RemoteKeyboardBar(
                                            visible = true,
                                            controller = keyboardController,
                                            enabled = inputSession != null,
                                            compact = true,
                                        )
                                    }
                                }

                                MouseButtonsRow(
                                    buttonHeightDp = deckBtnHeightDp,
                                    compactLabels = false,
                                )
                            }
                        }
                    }
                }
            }
        }

        if (showMonitorSetupDialog) {
            val avail = maxOf(streamInfo?.availableMonitors?.toInt() ?: 2, 2)
            AlertDialog(
                onDismissRequest = { showMonitorSetupDialog = false },
                containerColor = OraculoColors.Surface,
                titleContentColor = OraculoColors.OnSurface,
                textContentColor = OraculoColors.OnSurfaceMuted,
                title = {
                    Text(
                        text = "Liberar troca dos $avail monitores",
                        style = MaterialTheme.typography.titleMedium,
                    )
                },
                text = {
                    Text(
                        text = "O PC possui $avail monitores ativos, mas o GNOME só autorizou 1.\n\n" +
                            "Para alternar direto do celular sem abrir janela no desktop:\n" +
                            "1. No PC, rode `oraculo-host monitors` (ou abra o seletor agora se estiver na frente dele).\n" +
                            "2. Clique para marcar TODOS os $avail monitores.\n" +
                            "3. Deixe \"Lembrar esta seleção\" marcado e clique em Compartilhar.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                },
                confirmButton = {
                    Button(
                        onClick = { requestPcMonitorReauth() },
                        enabled = !monitorReauthPending,
                        shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OraculoColors.SecondaryContainer,
                            contentColor = OraculoColors.OnSecondaryContainer,
                        ),
                    ) {
                        Text(if (monitorReauthPending) "Aberto no PC..." else "Abrir no PC agora")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showMonitorSetupDialog = false }) {
                        Text("Fechar", color = OraculoColors.OnSurfaceMuted)
                    }
                },
            )
        }
    }
}

@Composable
private fun StreamControlPill(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    containerColor: Color = OraculoColors.Surface,
    accentBorder: Color = OraculoColors.Outline,
    height: Dp = OraculoMetrics.ControlHeightSmall,
    content: @Composable () -> Unit,
) {
    val (interactionSource, pressed) = rememberPressedInteraction()
    val shape = RoundedCornerShape(OraculoMetrics.CornerSmall)
    val base = modifier
        .height(height)
        .widthIn(min = height)
        .oraculoPressScale(pressed && onClick != null)
        .clip(shape)
        .border(1.dp, accentBorder, shape)
    val clickableModifier = if (onClick != null) {
        base.clickable(
            interactionSource = interactionSource,
            indication = LocalIndication.current,
            onClick = onClick,
        )
    } else {
        base
    }
    Surface(
        color = containerColor,
        shape = shape,
        modifier = clickableModifier,
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(5.dp, Alignment.CenterHorizontally),
        ) {
            content()
        }
    }
}
