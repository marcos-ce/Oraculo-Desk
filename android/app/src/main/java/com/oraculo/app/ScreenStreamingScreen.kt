package com.oraculo.app

import android.content.Context
import android.content.pm.ActivityInfo
import android.graphics.SurfaceTexture
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.Surface
import android.view.TextureView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculateCentroidSize
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.HelpOutline
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DesktopWindows
import androidx.compose.material.icons.filled.Fullscreen
import androidx.compose.material.icons.filled.FullscreenExit
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.KeyboardHide
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.ScreenRotation
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
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
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoDeckButton
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.oraculoPrimaryButtonColors
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
import java.util.concurrent.atomic.AtomicLong
import kotlin.math.roundToInt

private const val TAG_STREAM = "OraculoScreen"
private const val PREF_SCREEN_PRESET = "screen_preset_v2"
private const val PREF_SCREEN_TUTORIAL_SEEN = "screen_tutorial_seen_v1"
private const val PREF_PORTRAIT_TOUCHPAD_HEIGHT_DP = "screen_portrait_touchpad_dp_v1"
private const val PRESET_VALUE_720P30 = "720p30"
private const val PRESET_VALUE_1080P30 = "1080p30"
private const val PRESET_VALUE_1080P60 = "1080p60"

private const val MIN_ZOOM_SCALE = 1f
private const val MAX_ZOOM_SCALE = 8f
private const val DOUBLE_TAP_ZOOM_SCALE = 2.5f
private const val KEYFRAME_REQUEST_MIN_INTERVAL_MS = 350L

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
    allowScreen: Boolean? = null,
    allowInput: Boolean? = null,
    allowRdpTunnel: Boolean = true,
    connectionRestoredNonce: Int = 0,
    onOpenRdpTunnel: () -> Unit = {},
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
    var screenConnectHint by remember { mutableStateOf(tr("Conectando vídeo...", "Connecting video...")) }
    var screenError by remember { mutableStateOf<String?>(null) }
    var streamInfo by remember { mutableStateOf<ScreenStreamInfo?>(null) }
    var videoWidth by remember { mutableIntStateOf(0) }
    var videoHeight by remember { mutableIntStateOf(0) }
    var screenRetryNonce by remember { mutableIntStateOf(0) }
    var screenAutoRetryCount by remember(host.endpointId) { mutableIntStateOf(0) }
    var pendingScreenDelayMs by remember(host.endpointId) { mutableStateOf(0L) }
    var monitorSwitching by remember { mutableStateOf(false) }
    var showMonitorSetupDialog by remember { mutableStateOf(false) }
    var monitorReauthPending by remember { mutableStateOf(false) }
    var streamNotice by remember { mutableStateOf<String?>(null) }
    var showGestureTutorial by remember {
        mutableStateOf(!prefs.getBoolean(PREF_SCREEN_TUTORIAL_SEEN, false))
    }
    var customPortraitTouchpadDp by remember {
        val saved = prefs.getFloat(PREF_PORTRAIT_TOUCHPAD_HEIGHT_DP, -1f)
        mutableStateOf(if (saved > 0f) saved else null)
    }

    var inputSession by remember { mutableStateOf<MobileInputSession?>(null) }
    var inputConnecting by remember { mutableStateOf(true) }
    var inputConnectHint by remember { mutableStateOf(tr("Conectando touchpad...", "Connecting touchpad...")) }
    var inputError by remember { mutableStateOf<String?>(null) }
    var inputRetryNonce by remember { mutableIntStateOf(0) }
    var inputAutoRetryCount by remember(host.endpointId) { mutableIntStateOf(0) }
    var pendingInputDelayMs by remember(host.endpointId) { mutableStateOf(0L) }
    var telemetryReadout by remember { mutableStateOf<String?>(null) }
    var lastTelemetryAtMs by remember { mutableStateOf(0L) }

    val scope = rememberCoroutineScope()
    val lastKeyframeRequestAtMs = remember { AtomicLong(0L) }
    var zoomScale by remember { mutableFloatStateOf(MIN_ZOOM_SCALE) }
    val isZoomed by remember { derivedStateOf { zoomScale > 1.05f } }
    var panOffset by remember { mutableStateOf(Offset.Zero) }
    var zoomJob by remember { mutableStateOf<Job?>(null) }

    fun dismissGestureTutorial() {
        if (!prefs.getBoolean(PREF_SCREEN_TUTORIAL_SEEN, false)) {
            prefs.edit().putBoolean(PREF_SCREEN_TUTORIAL_SEEN, true).apply()
        }
        showGestureTutorial = false
    }

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
                triggerInputReconnectRef(false, tr("Reconectando touchpad...", "Reconnecting touchpad..."))
            }
        },
        onPublishTelemetry = { publishTelemetry(it, 0L) },
    )

    fun triggerInputReconnect(resetAttempts: Boolean, hint: String) {
        if (resetAttempts) {
            inputAutoRetryCount = 0
            pendingInputDelayMs = 0L
        } else {
            if (inputAutoRetryCount >= 5) {
                inputSession = null
                inputConnecting = false
                inputError = tr("Touchpad desconectado. Toque para reconectar.", "Touchpad disconnected. Tap to reconnect.")
                keyboardController.releaseAllModifiers(null)
                return
            }
            val backoffSchedule = listOf(350L, 900L, 1800L, 3000L, 4500L)
            pendingInputDelayMs = backoffSchedule[inputAutoRetryCount.coerceAtMost(backoffSchedule.lastIndex)]
            inputAutoRetryCount += 1
        }
        inputSession = null
        inputError = null
        inputConnectHint = hint
        keyboardController.releaseAllModifiers(null)
        inputRetryNonce += 1
    }
    triggerInputReconnectRef = ::triggerInputReconnect

    fun triggerScreenReconnect(resetAttempts: Boolean, hint: String) {
        if (resetAttempts) {
            screenAutoRetryCount = 0
            pendingScreenDelayMs = 0L
        } else {
            val backoffSchedule = listOf(600L, 1500L, 3000L, 4500L)
            pendingScreenDelayMs = backoffSchedule[screenAutoRetryCount.coerceAtMost(backoffSchedule.lastIndex)]
            screenAutoRetryCount += 1
        }
        screenSession = null
        screenError = null
        screenConnectHint = hint
        screenRetryNonce += 1
    }

    LaunchedEffect(screenSession, streamInfo) {
        val activeSession = screenSession ?: return@LaunchedEffect
        if (streamInfo == null) return@LaunchedEffect
        delay(4_000L)
        if (screenSession === activeSession && activeSession.isAlive()) {
            screenAutoRetryCount = 0
        }
    }

    LaunchedEffect(connectionRestoredNonce) {
        if (connectionRestoredNonce == 0) return@LaunchedEffect
        val curScreen = screenSession
        val curScreenErr = screenError
        if (
            allowScreen != false &&
            (curScreenErr == null || !isPermanentRemoteError(curScreenErr)) &&
            ((curScreen == null && !screenConnecting) || (curScreen != null && !curScreen.isAlive()))
        ) {
            triggerScreenReconnect(
                resetAttempts = true,
                hint = tr("Reconectando vídeo...", "Reconnecting video..."),
            )
        }
        val curInput = inputSession
        val curInputErr = inputError
        if (
            allowInput != false &&
            (curInputErr == null || !isPermanentRemoteError(curInputErr)) &&
            ((curInput == null && !inputConnecting) || (curInput != null && !curInput.isAlive()))
        ) {
            triggerInputReconnect(
                resetAttempts = true,
                hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
            )
        }
    }

    LaunchedEffect(host.endpointId) {
        var skipFirst = true
        OraculoSessionCoordinator.networkChangeNonce.collect {
            if (skipFirst) {
                skipFirst = false
                return@collect
            }
            delay(450L)
            val curScreen = screenSession
            val curScreenErr = screenError
            if (
                allowScreen != false &&
                (curScreenErr == null || !isPermanentRemoteError(curScreenErr)) &&
                ((curScreen == null && !screenConnecting) || (curScreen != null && !curScreen.isAlive()))
            ) {
                triggerScreenReconnect(
                    resetAttempts = true,
                    hint = tr("Reconectando vídeo...", "Reconnecting video..."),
                )
            }
            val curInput = inputSession
            val curInputErr = inputError
            if (
                allowInput != false &&
                (curInputErr == null || !isPermanentRemoteError(curInputErr)) &&
                ((curInput == null && !inputConnecting) || (curInput != null && !curInput.isAlive()))
            ) {
                triggerInputReconnect(
                    resetAttempts = true,
                    hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
                )
            }
        }
    }

    LaunchedEffect(allowInput) {
        if (allowInput == false) {
            val prev = inputSession
            keyboardController.releaseAllModifiers(prev)
            inputSession = null
            inputConnecting = false
            inputAutoRetryCount = 0
            pendingInputDelayMs = 0L
            inputError = tr("Mouse e teclado desativados no computador.", "Mouse and keyboard disabled on computer.")
            if (prev != null) {
                withContext(Dispatchers.IO) {
                    runCatching {
                        prev.closeSession()
                        prev.close()
                    }
                }
            }
        } else if (allowInput == true) {
            val current = inputSession
            if ((current == null && !inputConnecting) || (current != null && !current.isAlive())) {
                triggerInputReconnect(
                    resetAttempts = true,
                    hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
                )
            }
        }
    }

    LaunchedEffect(allowScreen) {
        if (allowScreen == false) {
            val prev = screenSession
            screenSession = null
            screenConnecting = false
            screenError = tr("Transmissão de tela desativada no computador.", "Screen sharing disabled on computer.")
            if (prev != null) {
                withContext(Dispatchers.IO) {
                    runCatching {
                        prev.closeSession()
                        prev.close()
                    }
                }
            }
        } else if (allowScreen == true) {
            val current = screenSession
            if ((current == null && !screenConnecting) || (current != null && !current.isAlive())) {
                screenError = null
                screenRetryNonce += 1
            }
        }
    }

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
                val activeScreen = screenSession
                if (activeScreen != null && activeScreen.isAlive()) {
                    val nowMs = System.currentTimeMillis()
                    val prevMs = lastKeyframeRequestAtMs.get()
                    if (nowMs - prevMs >= KEYFRAME_REQUEST_MIN_INTERVAL_MS &&
                        lastKeyframeRequestAtMs.compareAndSet(prevMs, nowMs)
                    ) {
                        scope.launch(Dispatchers.IO) {
                            runCatching { activeScreen.requestKeyframe() }
                        }
                    }
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
            decoder.release(s)
        }
    }

    DisposableEffect(lifecycleOwner, host.endpointId) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                val curInput = inputSession
                if ((curInput == null && !inputConnecting) || (curInput != null && !curInput.isAlive())) {
                    triggerInputReconnect(resetAttempts = true, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
                }
                val curScreen = screenSession
                if ((curScreen == null && !screenConnecting) || (curScreen != null && !curScreen.isAlive())) {
                    screenError = null
                    screenConnectHint = tr("Reconectando vídeo...", "Reconnecting video...")
                    screenRetryNonce += 1
                } else if (curScreen != null && curScreen.isAlive()) {
                    decoder.ensureRenderedOrRequestKeyframe()
                }
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose {
            owner.lifecycle.removeObserver(observer)
        }
    }

    val currentAllowScreen by rememberUpdatedState(allowScreen)

    // Sessão de vídeo H.264 independente do canal de input.
    DisposableEffect(host.endpointId, screenRetryNonce) {
        if (currentAllowScreen == false) {
            screenConnecting = false
            screenError = tr("Transmissão de tela desativada no computador.", "Screen sharing disabled on computer.")
            return@DisposableEffect onDispose {}
        }
        var active = true
        var createdScreen: MobileScreenSession? = null
        val waitScreenMs = maxOf(pendingScreenDelayMs, 50L)
        pendingScreenDelayMs = 0L
        screenConnecting = true
        screenError = null
        streamInfo = null
        monitorSwitching = false
        monitorReauthPending = false
        if (screenAutoRetryCount == 0) {
            screenConnectHint = tr("Abrindo transmissão da tela...", "Opening screen stream...")
        }
        val openingPreset = preset

        val hintJob = CoroutineScope(Dispatchers.Main).launch {
            delay(1100)
            if (active && screenConnecting) {
                screenConnectHint = tr(
                    "Se aparecer um pedido no computador, autorize o compartilhamento.",
                    "If prompted on your computer, allow screen sharing.",
                )
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
                    monitorSwitching = false
                    monitorReauthPending = false
                    if (
                        currentAllowScreen != false &&
                        !isPermanentRemoteError(message) &&
                        screenAutoRetryCount < 4
                    ) {
                        triggerScreenReconnect(
                            resetAttempts = false,
                            hint = tr("Reconectando vídeo...", "Reconnecting video..."),
                        )
                    } else {
                        screenConnecting = false
                        screenSession = null
                        screenError = message
                    }
                }
            }

            override fun onStreamClosed(reason: String) {
                Log.w(TAG_STREAM, "onStreamClosed: $reason")
                mainHandler.post {
                    if (!active) return@post
                    monitorSwitching = false
                    monitorReauthPending = false
                    val msg = reason.ifBlank { tr("Transmissão de tela encerrada.", "Screen stream ended.") }
                    if (
                        currentAllowScreen != false &&
                        !isPermanentRemoteError(msg) &&
                        screenAutoRetryCount < 4
                    ) {
                        triggerScreenReconnect(
                            resetAttempts = false,
                            hint = tr("Reconectando vídeo...", "Reconnecting video..."),
                        )
                    } else {
                        screenConnecting = false
                        screenSession = null
                        screenError = msg
                    }
                }
            }
        }

        val openJob = CoroutineScope(Dispatchers.IO).launch {
            delay(waitScreenMs)
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
                    val msg = formatOraculoError(err)
                    if (
                        currentAllowScreen != false &&
                        !isPermanentRemoteError(msg) &&
                        screenAutoRetryCount < 4
                    ) {
                        triggerScreenReconnect(
                            resetAttempts = false,
                            hint = tr("Reconectando vídeo...", "Reconnecting video..."),
                        )
                    } else {
                        screenConnecting = false
                        screenSession = null
                        screenError = msg
                    }
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

    val currentAllowInput by rememberUpdatedState(allowInput)

    // Canal de input dedicado ao mini-touchpad e teclado enquanto estiver no modo streaming.
    DisposableEffect(host.endpointId, inputRetryNonce) {
        if (currentAllowInput == false) {
            inputConnecting = false
            inputError = tr("Mouse e teclado desativados no computador.", "Mouse and keyboard disabled on computer.")
            return@DisposableEffect onDispose {}
        }
        var active = true
        var createdInput: MobileInputSession? = null
        val waitMs = maxOf(pendingInputDelayMs, 110L)
        pendingInputDelayMs = 0L
        inputConnecting = true
        inputError = null
        if (inputAutoRetryCount == 0) {
            inputConnectHint = tr("Conectando touchpad...", "Connecting touchpad...")
        }
        keyboardController.resetState(null)

        val observer = object : InputObserver {
            override fun onSessionClosed(reason: String) {
                Log.w(TAG_STREAM, "input onSessionClosed: $reason")
                mainHandler.post {
                    if (!active) return@post
                    if (createdInput != null && inputSession === createdInput) {
                        createdInput = null
                        if (currentAllowInput == false) {
                            inputSession = null
                            inputConnecting = false
                            inputAutoRetryCount = 0
                            inputError = tr("Mouse e teclado desativados no computador.", "Mouse and keyboard disabled on computer.")
                        } else {
                            triggerInputReconnect(
                                resetAttempts = false,
                                hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
                            )
                        }
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
                    val msg = formatOraculoError(err)
                    if (!isPermanentRemoteError(msg) && inputAutoRetryCount < 5) {
                        triggerInputReconnect(
                            resetAttempts = false,
                            hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
                        )
                    } else {
                        inputConnecting = false
                        inputError = msg
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
        scope.launch(Dispatchers.IO) {
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
        scope.launch(Dispatchers.IO) {
            val sent = runCatching { activeSession.reauthorizeMonitors() }.isSuccess
            if (!sent) {
                mainHandler.post {
                    monitorReauthPending = false
                    streamNotice = tr("Sessão indisponível no momento.", "Session unavailable right now.")
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
            scope.launch(Dispatchers.IO) {
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
        val label = when {
            screenError != null -> "Offline"
            info != null && compactText -> "${info.height}p·${info.fps}"
            info != null -> "${info.width}×${info.height} · ${info.fps}fps"
            else -> "..."
        }
        Box(
            modifier = modifier
                .height(pillHeight)
                .padding(horizontal = 4.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            Text(
                text = label,
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontFeatureSettings = "tnum",
                ),
                color = if (screenError != null) OraculoColors.Error else OraculoColors.OnSurfaceMuted,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }

    @Composable
    fun ZoomResetChip(pillHeight: Dp = OraculoMetrics.ControlHeightSmall) {
        val zoomTenths by remember { derivedStateOf { (zoomScale * 10f).roundToInt() } }
        val zoomLabel = "${zoomTenths / 10}.${zoomTenths % 10}x"
        StreamToolbarButton(
            onClick = { resetZoom() },
            containerColor = OraculoColors.SecondaryContainer,
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
                color = OraculoColors.OnSecondaryContainer,
                maxLines = 1,
                softWrap = false,
            )
        }
    }

    @Composable
    fun TutorialHelpChip(pillHeight: Dp = OraculoMetrics.ControlHeightSmall) {
        StreamToolbarButton(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                showGestureTutorial = true
            },
            height = pillHeight,
        ) {
            Icon(
                imageVector = Icons.AutoMirrored.Outlined.HelpOutline,
                contentDescription = tr("Ajuda de gestos", "Gesture help"),
                tint = OraculoColors.OnSurface,
                modifier = Modifier.size(15.dp),
            )
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
            else -> tr("1/$available tel.", "1/$available disp.")
        }
        StreamToolbarButton(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                switchMonitor()
            },
            enabled = screenSession != null && !screenConnecting && !monitorSwitching,
            containerColor = if (multi) OraculoColors.SecondaryContainer else OraculoColors.Surface,
            height = pillHeight,
            modifier = modifier,
        ) {
            Icon(
                imageVector = Icons.Default.DesktopWindows,
                contentDescription = tr("Alternar monitor", "Switch monitor"),
                tint = if (multi) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurfaceMuted,
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
                color = if (multi) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
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
        StreamToolbarButton(
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                cyclePreset()
            },
            containerColor = if (isHigh) OraculoColors.SecondaryContainer else OraculoColors.Surface,
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
                color = if (isHigh) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
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
        val toggle = {
            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
            showKeyboard = !showKeyboard
        }
        if (showLabel) {
            StreamToolbarButton(
                onClick = toggle,
                containerColor = if (showKeyboard) OraculoColors.SecondaryContainer else OraculoColors.Surface,
                height = pillHeight,
                modifier = modifier,
            ) {
                Icon(
                    imageVector = if (showKeyboard) Icons.Default.KeyboardHide else Icons.Default.Keyboard,
                    contentDescription = if (showKeyboard) tr("Ocultar teclado", "Hide keyboard") else tr("Mostrar teclado", "Show keyboard"),
                    tint = if (showKeyboard) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
                    modifier = Modifier.size(16.dp),
                )
                Text(
                    text = if (showKeyboard) tr("Fechar", "Close") else tr("Teclado", "Keys"),
                    style = MaterialTheme.typography.labelMedium,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (showKeyboard) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
                    maxLines = 1,
                    softWrap = false,
                )
            }
        } else if (showKeyboard) {
            FilledTonalIconButton(
                onClick = toggle,
                colors = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = OraculoColors.SecondaryContainer,
                    contentColor = OraculoColors.OnSecondaryContainer,
                ),
                modifier = modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.KeyboardHide,
                    contentDescription = tr("Ocultar teclado", "Hide keyboard"),
                    modifier = Modifier.size(18.dp),
                )
            }
        } else {
            IconButton(
                onClick = toggle,
                modifier = modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.Keyboard,
                    contentDescription = tr("Mostrar teclado", "Show keyboard"),
                    tint = OraculoColors.OnSurface,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }

    @Composable
    fun RotateChip(
        showLabel: Boolean = false,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        val toggle = {
            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
            isLandscape = !isLandscape
            resetZoom(animated = false)
        }
        if (showLabel) {
            StreamToolbarButton(
                onClick = toggle,
                containerColor = if (isLandscape) OraculoColors.SecondaryContainer else OraculoColors.Surface,
                height = pillHeight,
            ) {
                Icon(
                    imageVector = Icons.Default.ScreenRotation,
                    contentDescription = tr("Girar tela", "Rotate screen"),
                    tint = if (isLandscape) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
                    modifier = Modifier.size(16.dp),
                )
                Text(
                    text = tr("Girar", "Rotate"),
                    style = MaterialTheme.typography.labelMedium,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (isLandscape) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface,
                    maxLines = 1,
                    softWrap = false,
                )
            }
        } else if (isLandscape) {
            FilledTonalIconButton(
                onClick = toggle,
                colors = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = OraculoColors.SecondaryContainer,
                    contentColor = OraculoColors.OnSecondaryContainer,
                ),
                modifier = Modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.ScreenRotation,
                    contentDescription = tr("Girar tela", "Rotate screen"),
                    modifier = Modifier.size(17.dp),
                )
            }
        } else {
            IconButton(
                onClick = toggle,
                modifier = Modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.ScreenRotation,
                    contentDescription = tr("Girar tela", "Rotate screen"),
                    tint = OraculoColors.OnSurface,
                    modifier = Modifier.size(17.dp),
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
        val clickHandler = {
            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
            onToggle()
        }
        if (translucent) {
            FilledIconButton(
                onClick = clickHandler,
                colors = IconButtonDefaults.filledIconButtonColors(
                    containerColor = Color.Black.copy(alpha = 0.58f),
                    contentColor = Color.White,
                ),
                modifier = modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = if (fullscreen) Icons.Default.FullscreenExit else Icons.Default.Fullscreen,
                    contentDescription = if (fullscreen) tr("Sair da tela cheia", "Exit fullscreen") else tr("Tela cheia", "Fullscreen"),
                    modifier = Modifier.size(18.dp),
                )
            }
        } else if (fullscreen) {
            FilledTonalIconButton(
                onClick = clickHandler,
                colors = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = OraculoColors.SecondaryContainer,
                    contentColor = OraculoColors.OnSecondaryContainer,
                ),
                modifier = modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.FullscreenExit,
                    contentDescription = tr("Sair da tela cheia", "Exit fullscreen"),
                    modifier = Modifier.size(18.dp),
                )
            }
        } else {
            IconButton(
                onClick = clickHandler,
                modifier = modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.Fullscreen,
                    contentDescription = tr("Tela cheia", "Fullscreen"),
                    tint = OraculoColors.OnSurface,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }

    @Composable
    fun ExitChip(
        compact: Boolean = false,
        pillHeight: Dp = OraculoMetrics.ControlHeightSmall,
    ) {
        if (compact) {
            IconButton(
                onClick = { handleExit() },
                modifier = Modifier.size(pillHeight),
            ) {
                Icon(
                    imageVector = Icons.Default.Close,
                    contentDescription = tr("Sair da transmissão", "Exit stream"),
                    tint = OraculoColors.Error,
                    modifier = Modifier.size(18.dp),
                )
            }
        } else {
            StreamToolbarButton(
                onClick = { handleExit() },
                containerColor = Color.Transparent,
                height = pillHeight,
            ) {
                Icon(
                    imageVector = Icons.Default.Close,
                    contentDescription = tr("Sair da transmissão", "Exit stream"),
                    tint = OraculoColors.Error,
                    modifier = Modifier.size(17.dp),
                )
                Text(
                    text = tr("Sair", "Exit"),
                    style = MaterialTheme.typography.labelMedium,
                    fontSize = 12.sp,
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
                        awaitEachGesture {
                            awaitFirstDown(requireUnconsumed = false)
                            var pastSlop = false
                            var totalPan = Offset.Zero
                            var totalZoom = 1f
                            val touchSlop = viewConfiguration.touchSlop

                            while (true) {
                                val event = awaitPointerEvent()
                                if (event.changes.any { it.isConsumed }) break
                                val pressed = event.changes.filter { it.pressed }
                                if (pressed.isEmpty()) break

                                // Em escala 1x com apenas 1 dedo, não intercepta toques para não bloquear toque duplo.
                                if (pressed.size < 2 && zoomScale <= 1.01f && !pastSlop) {
                                    continue
                                }

                                val zoomChange = if (pressed.size >= 2) event.calculateZoom() else 1f
                                val panChange = event.calculatePan()

                                if (!pastSlop) {
                                    totalZoom *= zoomChange
                                    totalPan += panChange
                                    val centroidSize = event.calculateCentroidSize(useCurrent = false)
                                    val zoomMotion = kotlin.math.abs(1f - totalZoom) * centroidSize
                                    val panMotion = totalPan.getDistance()
                                    if (zoomMotion > touchSlop || (zoomScale > 1.01f && panMotion > touchSlop) || (pressed.size >= 2 && panMotion > touchSlop)) {
                                        pastSlop = true
                                    }
                                }

                                if (pastSlop) {
                                    val centroid = event.calculateCentroid(useCurrent = false)
                                    if (zoomChange != 1f || panChange != Offset.Zero) {
                                        zoomJob?.cancel()
                                        val oldScale = zoomScale
                                        val nextScale = (oldScale * zoomChange).coerceIn(MIN_ZOOM_SCALE, MAX_ZOOM_SCALE)
                                        if (nextScale <= 1.01f) {
                                            if (oldScale > MIN_ZOOM_SCALE || panOffset != Offset.Zero) {
                                                resetZoom(animated = false)
                                            }
                                        } else {
                                            val ratio = nextScale / oldScale
                                            val center = Offset(viewportWidthPx / 2f, viewportHeightPx / 2f)
                                            val c = if (centroid == Offset.Unspecified) Offset.Zero else centroid - center
                                            val rawPan = panOffset * ratio + c * (1f - ratio) + panChange
                                            zoomScale = nextScale
                                            panOffset = clampPan(rawPan, nextScale)
                                        }
                                    }
                                    event.changes.forEach {
                                        if (it.positionChanged()) {
                                            it.consume()
                                        }
                                    }
                                }
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
                    color = OraculoColors.TerminalBackground.copy(alpha = 0.88f),
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(horizontal = 24.dp)
                        .border(1.dp, OraculoColors.TerminalBorder, RoundedCornerShape(OraculoMetrics.CornerMedium)),
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
                            color = OraculoColors.TerminalForeground,
                        )
                    }
                }
            } else if (screenError != null) {
                Surface(
                    color = OraculoColors.Surface.copy(alpha = 0.94f),
                    shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(horizontal = 24.dp)
                        .border(1.dp, OraculoColors.Error.copy(alpha = 0.5f), RoundedCornerShape(OraculoMetrics.CornerLarge)),
                ) {
                    Column(
                        modifier = Modifier.padding(horizontal = 18.dp, vertical = 14.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Text(
                            text = screenError ?: tr("Falha na transmissão.", "Stream failed."),
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
                            Text(tr("Tentar novamente", "Try again"))
                        }
                        if (allowRdpTunnel) {
                            TextButton(
                                onClick = {
                                    onExit()
                                    onOpenRdpTunnel()
                                },
                            ) {
                                Icon(
                                    imageVector = Icons.Default.Lock,
                                    contentDescription = null,
                                    modifier = Modifier.size(16.dp),
                                )
                                Spacer(modifier = Modifier.width(6.dp))
                                Text(tr("Destravar via login remoto (RDP)", "Unlock via remote login (RDP)"))
                            }
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
                    color = OraculoColors.TerminalBackground.copy(alpha = 0.90f),
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    modifier = Modifier.border(1.dp, OraculoColors.TerminalBorder, RoundedCornerShape(OraculoMetrics.CornerMedium)),
                ) {
                    Text(
                        text = streamNotice.orEmpty(),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.TerminalForeground,
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
            onRetry = { triggerInputReconnect(resetAttempts = true, hint = tr("Conectando touchpad...", "Connecting touchpad...")) },
            onSessionBroken = { broken ->
                if (inputSession === broken && !inputConnecting) {
                    triggerInputReconnect(resetAttempts = false, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
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
        val availableWidth = maxWidth
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

                    val landscapeRailWidth = (availableWidth * 0.31f).coerceIn(244.dp, 296.dp)
                    Surface(
                        color = OraculoColors.SurfaceRecessed,
                        modifier = Modifier
                            .width(landscapeRailWidth)
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
                                    if (isZoomed) {
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
                                TutorialHelpChip(pillHeight = railPillHeight)
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
                                    if (isZoomed) {
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
                                    TutorialHelpChip()
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

                    val minPortraitPadDp = 136f
                    val maxPortraitPadDp = (availableHeight.value * 0.48f).coerceIn(220f, 380f)
                    val defaultPortraitPadDp = (availableHeight.value * 0.30f).coerceIn(196f, 256f)
                    val resolvedPortraitPadDp = (customPortraitTouchpadDp ?: defaultPortraitPadDp)
                        .coerceIn(minPortraitPadDp, maxPortraitPadDp)
                    val resizeDensity = LocalDensity.current
                    val latestMinPadDp by rememberUpdatedState(minPortraitPadDp)
                    val latestMaxPadDp by rememberUpdatedState(maxPortraitPadDp)
                    val latestDefaultPadDp by rememberUpdatedState(defaultPortraitPadDp)
                    var lastResizeHandleTapMs by remember { mutableStateOf(0L) }

                    Surface(color = OraculoColors.SurfaceRecessed) {
                        Column(modifier = Modifier.fillMaxWidth()) {
                            HorizontalDivider(thickness = 1.dp, color = OraculoColors.Outline)
                            if (!showKeyboard) {
                                Box(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(24.dp)
                                        .pointerInput(Unit) {
                                            val dragSlopPx = viewConfiguration.touchSlop.coerceAtMost(
                                                with(resizeDensity) { 6.dp.toPx() },
                                            )
                                            awaitEachGesture {
                                                val down = awaitFirstDown(requireUnconsumed = false)
                                                val pointerId = down.id
                                                val downAtMs = SystemClock.uptimeMillis()
                                                var accumDy = 0f
                                                var dragging = false
                                                var cancelled = false
                                                try {
                                                    while (true) {
                                                        val event = awaitPointerEvent()
                                                        val change = event.changes.firstOrNull { it.id == pointerId }
                                                            ?: break
                                                        if (!change.pressed) {
                                                            if (dragging) change.consume()
                                                            break
                                                        }
                                                        val stepDy = change.position.y - change.previousPosition.y
                                                        if (!dragging) {
                                                            accumDy += stepDy
                                                            if (kotlin.math.abs(accumDy) >= dragSlopPx) {
                                                                dragging = true
                                                                change.consume()
                                                                val deltaDp = with(resizeDensity) { (-accumDy).toDp().value }
                                                                val current = (customPortraitTouchpadDp ?: latestDefaultPadDp)
                                                                    .coerceIn(latestMinPadDp, latestMaxPadDp)
                                                                customPortraitTouchpadDp = (current + deltaDp)
                                                                    .coerceIn(latestMinPadDp, latestMaxPadDp)
                                                            }
                                                        } else if (stepDy != 0f) {
                                                            change.consume()
                                                            val deltaDp = with(resizeDensity) { (-stepDy).toDp().value }
                                                            val current = (customPortraitTouchpadDp ?: latestDefaultPadDp)
                                                                .coerceIn(latestMinPadDp, latestMaxPadDp)
                                                            customPortraitTouchpadDp = (current + deltaDp)
                                                                .coerceIn(latestMinPadDp, latestMaxPadDp)
                                                        }
                                                    }
                                                } catch (ce: kotlinx.coroutines.CancellationException) {
                                                    cancelled = true
                                                    throw ce
                                                } finally {
                                                    if (dragging) {
                                                        customPortraitTouchpadDp?.let { savedDp ->
                                                            prefs.edit()
                                                                .putFloat(PREF_PORTRAIT_TOUCHPAD_HEIGHT_DP, savedDp)
                                                                .apply()
                                                        }
                                                    }
                                                }
                                                if (!dragging && !cancelled) {
                                                    val upAtMs = SystemClock.uptimeMillis()
                                                    if (upAtMs - downAtMs < 300L) {
                                                        if (upAtMs - lastResizeHandleTapMs in 20L..350L) {
                                                            lastResizeHandleTapMs = 0L
                                                            customPortraitTouchpadDp = null
                                                            prefs.edit().remove(PREF_PORTRAIT_TOUCHPAD_HEIGHT_DP).apply()
                                                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                                        } else {
                                                            lastResizeHandleTapMs = upAtMs
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(width = 36.dp, height = 4.dp)
                                            .clip(RoundedCornerShape(OraculoMetrics.CornerPill))
                                            .background(OraculoColors.Outline),
                                    )
                                }
                            }
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(
                                        start = 10.dp,
                                        end = 10.dp,
                                        top = if (showKeyboard) 8.dp else 2.dp,
                                        bottom = 8.dp,
                                    ),
                                verticalArrangement = Arrangement.spacedBy(6.dp),
                            ) {
                                if (!showKeyboard || !tightImeHeight) {
                                    MiniTouchpadSurface(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .height(if (showKeyboard) 104.dp else resolvedPortraitPadDp.dp),
                                    )
                                }

                                OraculoDeckButton(
                                    label = if (showKeyboard) tr("Fechar teclado", "Close keyboard") else tr("Abrir teclado", "Open keyboard"),
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
                        text = tr("Autorizar $avail monitores", "Authorize $avail monitors"),
                        style = MaterialTheme.typography.titleMedium,
                    )
                },
                text = {
                    Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                        Text(
                            text = tr(
                                "Este computador tem $avail monitores ativos, mas apenas 1 está autorizado.\n\n" +
                                    "Para alternar direto pelo celular:\n" +
                                    "1. Abra o seletor no computador (ou rode `oraculo-host monitors`).\n" +
                                    "2. Marque todos os $avail monitores.\n" +
                                    "3. Deixe \"Lembrar esta seleção\" marcado e clique em Compartilhar.",
                                "This computer has $avail active monitors, but only 1 is authorized.\n\n" +
                                    "To switch monitors from your phone:\n" +
                                    "1. Open the picker on your computer (or run `oraculo-host monitors`).\n" +
                                    "2. Select all $avail monitors.\n" +
                                    "3. Keep \"Remember this selection\" checked and click Share.",
                            ),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
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
                        Text(
                            if (monitorReauthPending) {
                                tr("Aberto no computador...", "Opened on computer...")
                            } else {
                                tr("Abrir no computador", "Open on computer")
                            }
                        )
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showMonitorSetupDialog = false }) {
                        Text(tr("Fechar", "Close"), color = OraculoColors.OnSurfaceMuted)
                    }
                },
            )
        }

        if (showGestureTutorial) {
            ScreenGestureTutorialDialog(onDismiss = ::dismissGestureTutorial)
        }
    }
}

@Composable
private fun ScreenGestureTutorialDialog(
    onDismiss: () -> Unit,
) {
    val isDark = OraculoColors.IsDark
    val touchSurfaceColor = MaterialTheme.colorScheme.surfaceContainerHigh
    val outlineColor = OraculoColors.Outline
    val primaryColor = OraculoColors.Primary
    val terminalSurfaceColor = OraculoColors.TerminalBackground
    val onSurfaceMuted = OraculoColors.OnSurfaceMuted
    val fingerFillColor = if (isDark) {
        primaryColor.copy(alpha = 0.24f)
    } else {
        primaryColor.copy(alpha = 0.16f)
    }
    val fingerStrokeColor = primaryColor
    val nailColor = primaryColor.copy(alpha = 0.45f)

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            contentAlignment = Alignment.Center,
        ) {
            val useSideBySide = maxWidth >= 520.dp
            Surface(
                shape = RoundedCornerShape(OraculoMetrics.CornerSurfaceExpanded),
                color = OraculoColors.Surface,
                border = BorderStroke(1.dp, outlineColor),
                modifier = Modifier.widthIn(max = if (useSideBySide) 620.dp else 400.dp),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(
                            text = tr("Gestos da tela", "Screen gestures"),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = OraculoColors.OnSurface,
                        )
                        Text(
                            text = tr(
                                "Use dois polegares para rolar e pinça para ampliar.",
                                "Scroll with two thumbs and pinch to zoom.",
                            ),
                            style = MaterialTheme.typography.bodySmall,
                            color = onSurfaceMuted,
                        )
                    }

                    val scrollCard: @Composable (Modifier) -> Unit = { cardModifier ->
                        GestureTutorialCard(
                            title = tr("Rolar com dois polegares", "Scroll with two thumbs"),
                            description = tr(
                                "Arraste dois polegares juntos no touchpad para rolar páginas e listas.",
                                "Drag two thumbs together on the touchpad to scroll pages and lists.",
                            ),
                            containerColor = touchSurfaceColor,
                            borderColor = outlineColor,
                            modifier = cardModifier,
                        ) { canvasModifier ->
                            TwoThumbsScrollIllustration(
                                touchpadColor = OraculoColors.Surface,
                                outlineColor = outlineColor,
                                accentColor = primaryColor,
                                fingerFill = fingerFillColor,
                                fingerStroke = fingerStrokeColor,
                                nailColor = nailColor,
                                modifier = canvasModifier,
                            )
                        }
                    }
                    val pinchCard: @Composable (Modifier) -> Unit = { cardModifier ->
                        GestureTutorialCard(
                            title = tr("Zoom com pinça na tela", "Pinch the screen to zoom"),
                            description = tr(
                                "Faça pinça no vídeo para ampliar, arraste para mover e dê 2 toques para alternar 1x/2,5x.",
                                "Pinch the video to zoom, drag to pan, and double-tap to toggle 1x/2.5x.",
                            ),
                            containerColor = touchSurfaceColor,
                            borderColor = outlineColor,
                            modifier = cardModifier,
                        ) { canvasModifier ->
                            PinchScreenZoomIllustration(
                                screenFill = terminalSurfaceColor,
                                outlineColor = outlineColor,
                                accentColor = primaryColor,
                                fingerFill = fingerFillColor,
                                fingerStroke = fingerStrokeColor,
                                nailColor = nailColor,
                                modifier = canvasModifier,
                            )
                        }
                    }

                    if (useSideBySide) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            scrollCard(Modifier.weight(1f))
                            pinchCard(Modifier.weight(1f))
                        }
                    } else {
                        Column(
                            modifier = Modifier.fillMaxWidth(),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            scrollCard(Modifier.fillMaxWidth())
                            pinchCard(Modifier.fillMaxWidth())
                        }
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.End,
                    ) {
                        Button(
                            onClick = onDismiss,
                            colors = oraculoPrimaryButtonColors(),
                            shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                            modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                        ) {
                            Text(tr("Entendi", "Got it"))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun GestureTutorialCard(
    title: String,
    description: String,
    containerColor: Color,
    borderColor: Color,
    modifier: Modifier = Modifier,
    illustration: @Composable (Modifier) -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
        color = containerColor,
        border = BorderStroke(1.dp, borderColor),
        modifier = modifier,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            illustration(
                Modifier
                    .fillMaxWidth()
                    .height(100.dp),
            )
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.labelLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = OraculoColors.OnSurface,
                )
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = OraculoColors.OnSurfaceMuted,
                )
            }
        }
    }
}

@Composable
private fun TwoThumbsScrollIllustration(
    touchpadColor: Color,
    outlineColor: Color,
    accentColor: Color,
    fingerFill: Color,
    fingerStroke: Color,
    nailColor: Color,
    modifier: Modifier = Modifier,
) {
    Canvas(modifier = modifier) {
        val padW = minOf(size.width * 0.92f, size.height * 2.25f)
        val padH = size.height * 0.86f
        val left = (size.width - padW) / 2f
        val top = (size.height - padH) / 2f
        val radius = CornerRadius(12.dp.toPx(), 12.dp.toPx())

        drawRoundRect(
            color = touchpadColor,
            topLeft = Offset(left, top),
            size = Size(padW, padH),
            cornerRadius = radius,
        )
        drawRoundRect(
            color = outlineColor,
            topLeft = Offset(left, top),
            size = Size(padW, padH),
            cornerRadius = radius,
            style = Stroke(width = 1.25.dp.toPx()),
        )

        val cx = size.width / 2f
        val cy = size.height / 2f
        val trackHalfH = padH * 0.29f
        val strokePx = 2.dp.toPx()
        val headPx = 5.5.dp.toPx()

        // Eixo central indicando rolagem vertical (subir/descer) entre os dois polegares.
        drawLine(
            color = accentColor,
            start = Offset(cx, cy - trackHalfH),
            end = Offset(cx, cy + trackHalfH),
            strokeWidth = strokePx,
            cap = StrokeCap.Round,
        )
        drawChevronArrow(
            tip = Offset(cx, cy - trackHalfH - 1.dp.toPx()),
            dx = headPx,
            dy = headPx,
            pointUp = true,
            color = accentColor,
            strokeWidth = strokePx,
        )
        drawChevronArrow(
            tip = Offset(cx, cy + trackHalfH + 1.dp.toPx()),
            dx = headPx,
            dy = headPx,
            pointUp = false,
            color = accentColor,
            strokeWidth = strokePx,
        )

        clipRect(
            left = left + 1.dp.toPx(),
            top = top + 1.dp.toPx(),
            right = left + padW - 1.dp.toPx(),
            bottom = top + padH - 1.dp.toPx(),
        ) {
            val thumbW = (padH * 0.28f).coerceIn(16.dp.toPx(), 26.dp.toPx())
            val thumbLen = padH * 0.68f
            val tipY = cy - padH * 0.08f
            val spreadX = minOf(padW * 0.21f, thumbW * 1.45f)

            drawFingerSilhouette(
                tipCenter = Offset(cx - spreadX, tipY),
                widthPx = thumbW,
                lengthPx = thumbLen,
                angleDeg = 16f,
                fillColor = fingerFill,
                strokeColor = fingerStroke,
                nailColor = nailColor,
            )
            drawFingerSilhouette(
                tipCenter = Offset(cx + spreadX, tipY),
                widthPx = thumbW,
                lengthPx = thumbLen,
                angleDeg = -16f,
                fillColor = fingerFill,
                strokeColor = fingerStroke,
                nailColor = nailColor,
            )
        }
    }
}

@Composable
private fun PinchScreenZoomIllustration(
    screenFill: Color,
    outlineColor: Color,
    accentColor: Color,
    fingerFill: Color,
    fingerStroke: Color,
    nailColor: Color,
    modifier: Modifier = Modifier,
) {
    val dashEffect = remember { PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f) }

    Canvas(modifier = modifier) {
        val scrW = minOf(size.width * 0.92f, size.height * 2.05f)
        val scrH = size.height * 0.88f
        val left = (size.width - scrW) / 2f
        val top = (size.height - scrH) / 2f
        val radius = CornerRadius(11.dp.toPx(), 11.dp.toPx())

        drawRoundRect(
            color = screenFill,
            topLeft = Offset(left, top),
            size = Size(scrW, scrH),
            cornerRadius = radius,
        )
        drawRoundRect(
            color = outlineColor,
            topLeft = Offset(left, top),
            size = Size(scrW, scrH),
            cornerRadius = radius,
            style = Stroke(width = 1.25.dp.toPx()),
        )

        // Detalhes sutis de janela de desktop para diferenciar a área de vídeo.
        val barH = scrH * 0.16f
        drawRoundRect(
            color = Color.White.copy(alpha = 0.08f),
            topLeft = Offset(left + 8.dp.toPx(), top + 6.dp.toPx()),
            size = Size(scrW - 16.dp.toPx(), barH),
            cornerRadius = CornerRadius(4.dp.toPx(), 4.dp.toPx()),
        )
        drawRoundRect(
            color = Color.White.copy(alpha = 0.05f),
            topLeft = Offset(left + 10.dp.toPx(), top + barH + 12.dp.toPx()),
            size = Size(scrW * 0.42f, scrH * 0.56f),
            cornerRadius = CornerRadius(6.dp.toPx(), 6.dp.toPx()),
        )

        val cx = size.width / 2f
        val cy = top + scrH * 0.54f
        val ringR = scrH * 0.21f
        val strokePx = 1.8.dp.toPx()

        drawCircle(
            color = accentColor.copy(alpha = 0.45f),
            radius = ringR,
            center = Offset(cx, cy),
            style = Stroke(width = 1.3.dp.toPx(), pathEffect = dashEffect),
        )

        // Setas diagonais indicando abertura/fechamento em pinça.
        val arrowSpan = ringR * 0.68f
        drawLine(
            color = accentColor,
            start = Offset(cx - arrowSpan * 0.35f, cy + arrowSpan * 0.24f),
            end = Offset(cx - arrowSpan * 0.92f, cy + arrowSpan * 0.62f),
            strokeWidth = strokePx,
            cap = StrokeCap.Round,
        )
        drawLine(
            color = accentColor,
            start = Offset(cx + arrowSpan * 0.35f, cy - arrowSpan * 0.24f),
            end = Offset(cx + arrowSpan * 0.92f, cy - arrowSpan * 0.62f),
            strokeWidth = strokePx,
            cap = StrokeCap.Round,
        )

        clipRect(
            left = left + 1.dp.toPx(),
            top = top + 1.dp.toPx(),
            right = left + scrW - 1.dp.toPx(),
            bottom = top + scrH - 1.dp.toPx(),
        ) {
            val pinchDx = scrH * 0.26f
            val pinchDy = scrH * 0.17f
            val thumbW = (scrH * 0.24f).coerceIn(14.dp.toPx(), 22.dp.toPx())
            val indexW = (thumbW * 0.84f).coerceAtLeast(12.dp.toPx())

            // Polegar (inferior esquerdo) + indicador (superior direito) formando a pinça.
            drawFingerSilhouette(
                tipCenter = Offset(cx - pinchDx, cy + pinchDy),
                widthPx = thumbW,
                lengthPx = scrH * 0.56f,
                angleDeg = 46f,
                fillColor = fingerFill,
                strokeColor = fingerStroke,
                nailColor = nailColor,
            )
            drawFingerSilhouette(
                tipCenter = Offset(cx + pinchDx, cy - pinchDy),
                widthPx = indexW,
                lengthPx = scrH * 0.56f,
                angleDeg = -134f,
                fillColor = fingerFill,
                strokeColor = fingerStroke,
                nailColor = nailColor,
            )
        }
    }
}

private fun DrawScope.drawChevronArrow(
    tip: Offset,
    dx: Float,
    dy: Float,
    pointUp: Boolean,
    color: Color,
    strokeWidth: Float,
) {
    val dir = if (pointUp) 1f else -1f
    drawLine(
        color = color,
        start = tip,
        end = Offset(tip.x - dx, tip.y + dir * dy),
        strokeWidth = strokeWidth,
        cap = StrokeCap.Round,
    )
    drawLine(
        color = color,
        start = tip,
        end = Offset(tip.x + dx, tip.y + dir * dy),
        strokeWidth = strokeWidth,
        cap = StrokeCap.Round,
    )
}

private fun DrawScope.drawFingerSilhouette(
    tipCenter: Offset,
    widthPx: Float,
    lengthPx: Float,
    angleDeg: Float,
    fillColor: Color,
    strokeColor: Color,
    nailColor: Color,
) {
    // Halo sutil de toque na ponta do dedo.
    drawCircle(
        color = strokeColor.copy(alpha = 0.16f),
        radius = widthPx * 0.62f,
        center = tipCenter,
    )
    drawCircle(
        color = strokeColor.copy(alpha = 0.50f),
        radius = widthPx * 0.62f,
        center = tipCenter,
        style = Stroke(width = 1.2.dp.toPx()),
    )

    withTransform({
        rotate(degrees = angleDeg, pivot = tipCenter)
    }) {
        val topOffset = widthPx * 0.48f
        val bodyTopLeft = Offset(tipCenter.x - widthPx / 2f, tipCenter.y - topOffset)
        val bodySize = Size(widthPx, lengthPx + topOffset)
        val radius = CornerRadius(widthPx * 0.48f, widthPx * 0.48f)

        drawRoundRect(
            color = fillColor,
            topLeft = bodyTopLeft,
            size = bodySize,
            cornerRadius = radius,
        )
        drawRoundRect(
            color = strokeColor,
            topLeft = bodyTopLeft,
            size = bodySize,
            cornerRadius = radius,
            style = Stroke(width = 1.8.dp.toPx(), join = StrokeJoin.Round),
        )

        // Contorno da unha para identificar claramente o formato do dedo/polegar.
        val nailW = widthPx * 0.58f
        val nailH = widthPx * 0.44f
        drawRoundRect(
            color = nailColor,
            topLeft = Offset(tipCenter.x - nailW / 2f, tipCenter.y - widthPx * 0.34f),
            size = Size(nailW, nailH),
            cornerRadius = CornerRadius(nailW * 0.45f, nailW * 0.45f),
            style = Stroke(width = 1.25.dp.toPx()),
        )

        // Marca sutil da articulação do dedo.
        val knuckleY = tipCenter.y + lengthPx * 0.36f
        val knuckleHalf = widthPx * 0.22f
        drawLine(
            color = strokeColor.copy(alpha = 0.45f),
            start = Offset(tipCenter.x - knuckleHalf, knuckleY),
            end = Offset(tipCenter.x + knuckleHalf, knuckleY),
            strokeWidth = 1.2.dp.toPx(),
            cap = StrokeCap.Round,
        )
    }
}

@Composable
private fun StreamToolbarButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    containerColor: Color = OraculoColors.Surface,
    height: Dp = OraculoMetrics.ControlHeightSmall,
    content: @Composable () -> Unit,
) {
    Surface(
        onClick = onClick,
        enabled = enabled,
        color = containerColor,
        shape = RoundedCornerShape(OraculoMetrics.CornerSmall),
        modifier = modifier
            .height(height)
            .widthIn(min = height),
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
