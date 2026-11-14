package com.oraculo.app

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Backspace
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.filled.KeyboardReturn
import androidx.compose.material.icons.automirrored.filled.VolumeDown
import androidx.compose.material.icons.automirrored.filled.VolumeOff
import androidx.compose.material.icons.automirrored.filled.VolumeUp
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ClearAll
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.DesktopWindows
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.KeyboardHide
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.TouchApp
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.ZoomIn
import androidx.compose.material.icons.filled.ZoomOut
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.changedToUpIgnoreConsumed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.graphicsLayer
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoDeckButton
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.OraculoSquareIconButton
import com.oraculo.app.ui.PtyxisPalette
import com.oraculo.app.ui.oraculoPressScale
import com.oraculo.app.ui.ptyxisPaletteFor
import com.oraculo.app.ui.rememberPressedInteraction
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.oraculo_mobile_core.CommandRunResult
import uniffi.oraculo_mobile_core.InputObserver
import uniffi.oraculo_mobile_core.MobileInputSession
import uniffi.oraculo_mobile_core.MobileMouseButton
import uniffi.oraculo_mobile_core.MobileRemoteKey
import uniffi.oraculo_mobile_core.MobileTerminalSession
import uniffi.oraculo_mobile_core.OraculoException
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.QuickCommandItem
import uniffi.oraculo_mobile_core.SavedHostInfo
import uniffi.oraculo_mobile_core.TerminalColorKind
import uniffi.oraculo_mobile_core.TerminalObserver
import uniffi.oraculo_mobile_core.TerminalScreenSnapshot
import uniffi.oraculo_mobile_core.TerminalSpan
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.roundToInt

internal fun formatOraculoError(error: Throwable): String {
    val failure = error as? OraculoException.Failure
    if (failure != null && failure.reason.isNotBlank()) {
        return failure.reason
    }
    val msg = error.message?.removePrefix("reason=")?.trim()
    return if (!msg.isNullOrEmpty()) msg else error.javaClass.simpleName
}

private enum class ControlTab(
    private val ptLabel: String,
    private val enLabel: String,
    private val ptCompactLabel: String,
    private val enCompactLabel: String,
    val icon: ImageVector,
) {
    TOUCHPAD("Touchpad", "Touchpad", "Touch", "Touch", Icons.Default.TouchApp),
    TERMINAL("Terminal", "Terminal", "Terminal", "Terminal", Icons.Default.Terminal),
    COMMANDS("Comandos", "Commands", "Ações", "Actions", Icons.Default.Bolt),
    FILES("Arquivos", "Files", "Arquivos", "Files", Icons.Default.Folder);

    val label: String
        get() = tr(ptLabel, enLabel)

    val compactLabel: String
        get() = tr(ptCompactLabel, enCompactLabel)
}

internal fun Context.findLifecycleOwner(): LifecycleOwner? {
    var current: Context? = this
    while (current is ContextWrapper) {
        if (current is LifecycleOwner) return current
        current = current.baseContext
    }
    return current as? LifecycleOwner
}

internal fun Context.findActivity(): Activity? {
    var current: Context? = this
    while (current is ContextWrapper) {
        if (current is Activity) return current
        current = current.baseContext
    }
    return current as? Activity
}

@Composable
fun HostControlScreen(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() }
    val scope = rememberCoroutineScope()
    val focusManager = LocalFocusManager.current
    var selectedTab by remember { mutableStateOf(ControlTab.TOUCHPAD) }
    var visitedTabs by remember(host.endpointId) { mutableStateOf(setOf(ControlTab.TOUCHPAD)) }
    var showRemoteKeyboard by remember { mutableStateOf(false) }
    var showTouchpadSettings by remember { mutableStateOf(false) }
    var isStreamingMode by remember(host.endpointId) { mutableStateOf(false) }
    var latencyMs by remember { mutableStateOf<ULong?>(null) }
    var isConnected by remember { mutableStateOf<Boolean?>(null) }
    var isPinging by remember { mutableStateOf(false) }
    var pingGeneration by remember(host.endpointId) { mutableIntStateOf(0) }
    var isAppStarted by remember(lifecycleOwner) {
        mutableStateOf(
            lifecycleOwner?.lifecycle?.currentState?.isAtLeast(Lifecycle.State.STARTED) ?: true,
        )
    }

    HostSessionNotificationEffect(
        client = client,
        host = host,
        modeLabel = if (isStreamingMode) tr("Espelhamento de tela", "Screen mirroring") else selectedTab.label,
        onDisconnectRequested = onBack,
    )

    LaunchedEffect(selectedTab, isStreamingMode) {
        visitedTabs = visitedTabs + selectedTab
        if (isStreamingMode || selectedTab != ControlTab.TOUCHPAD) {
            focusManager.clearFocus(force = true)
            if (showRemoteKeyboard) {
                showRemoteKeyboard = false
            }
        }
        if (isStreamingMode && showTouchpadSettings) {
            showTouchpadSettings = false
        }
    }

    suspend fun performPing(showSpinner: Boolean) {
        val gen = ++pingGeneration
        if (showSpinner || isConnected != true) {
            isPinging = true
        }
        val result = withContext(Dispatchers.IO) {
            runCatching { client.ping(host.endpointId) }
        }
        if (gen == pingGeneration) {
            result.onSuccess { ping ->
                isConnected = true
                latencyMs = ping.rttMs
            }.onFailure {
                isConnected = false
                latencyMs = null
            }
            isPinging = false
        }
    }

    fun refreshPing(manual: Boolean = false) {
        if (isPinging && !manual) return
        scope.launch { performPing(showSpinner = manual) }
    }

    LaunchedEffect(host.endpointId, isAppStarted, isStreamingMode) {
        if (!isAppStarted || isStreamingMode) return@LaunchedEffect
        performPing(showSpinner = isConnected != true)
        delay(1800L)
        if (!isAppStarted || isStreamingMode) return@LaunchedEffect
        performPing(showSpinner = false)
        while (true) {
            delay(5000L)
            performPing(showSpinner = false)
        }
    }

    DisposableEffect(lifecycleOwner, host.endpointId) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> isAppStarted = true
                Lifecycle.Event.ON_STOP -> isAppStarted = false
                else -> Unit
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose {
            owner.lifecycle.removeObserver(observer)
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(OraculoColors.Background)
                .statusBarsPadding()
                .navigationBarsPadding()
                .imePadding(),
        ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OraculoSquareIconButton(
                onClick = onBack,
                size = OraculoMetrics.ControlHeightSmall,
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = "Voltar",
                    tint = OraculoColors.OnSurface,
                    modifier = Modifier.size(20.dp),
                )
            }

            Text(
                text = host.hostName,
                style = MaterialTheme.typography.titleLarge,
                color = OraculoColors.OnSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )

            val (pingSource, pingPressed) = rememberPressedInteraction()
            val msLabel = when {
                isPinging && (isConnected != true || latencyMs == null) -> "..."
                isConnected == true && latencyMs == 0uL -> "<1 ms"
                isConnected == true && latencyMs != null -> "${latencyMs} ms"
                isConnected == false -> "offline"
                else -> "..."
            }
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .height(OraculoMetrics.ControlHeightSmall)
                    .oraculoPressScale(pingPressed)
                    .clip(RoundedCornerShape(OraculoMetrics.CornerSmall))
                    .clickable(
                        interactionSource = pingSource,
                        indication = LocalIndication.current,
                    ) { refreshPing(manual = true) }
                    .padding(horizontal = 6.dp),
            ) {
                Text(
                    text = msLabel,
                    style = TextStyle(
                        fontFamily = FontFamily.Monospace,
                        fontSize = 12.sp,
                        fontFeatureSettings = "tnum",
                    ),
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Ellipsis,
                    color = when (isConnected) {
                        false -> OraculoColors.Error
                        else -> OraculoColors.OnSurfaceMuted
                    },
                )
            }

            AnimatedVisibility(
                visible = selectedTab == ControlTab.TOUCHPAD,
                enter = OraculoMotion.ExpandH,
                exit = OraculoMotion.CollapseH,
            ) {
                OraculoSquareIconButton(
                    onClick = { showTouchpadSettings = !showTouchpadSettings },
                    active = showTouchpadSettings,
                    containerColor = OraculoColors.Surface,
                    size = OraculoMetrics.ControlHeightSmall,
                ) {
                    Icon(
                        imageVector = Icons.Default.Tune,
                        contentDescription = "Ajustes do touchpad",
                        tint = if (showTouchpadSettings) OraculoColors.Primary else OraculoColors.OnSurfaceMuted,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }

        ModeCapsuleBar(
            selected = selectedTab,
            onSelect = { selectedTab = it },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 2.dp),
        )

        Spacer(modifier = Modifier.height(4.dp))

        KeepAliveControlTabs(
            selectedTab = selectedTab,
            visitedTabs = visitedTabs,
            modifier = Modifier.weight(1f),
        ) { tab ->
            when (tab) {
                ControlTab.TOUCHPAD -> TouchpadTab(
                    client = client,
                    host = host,
                    showKeyboardBar = showRemoteKeyboard,
                    showSettingsBar = showTouchpadSettings,
                    onToggleKeyboardBar = { showRemoteKeyboard = !showRemoteKeyboard },
                    onOpenScreenStream = { isStreamingMode = true },
                    isPaused = isStreamingMode,
                    modifier = Modifier.fillMaxSize(),
                )
                ControlTab.TERMINAL -> TerminalTab(
                    client = client,
                    host = host,
                    modifier = Modifier.fillMaxSize(),
                )
                ControlTab.COMMANDS -> CommandsTab(
                    client = client,
                    host = host,
                    modifier = Modifier.fillMaxSize(),
                )
                ControlTab.FILES -> FilesTab(
                    client = client,
                    host = host,
                    isActive = selectedTab == ControlTab.FILES,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        }

        AnimatedVisibility(
            visible = isStreamingMode,
            enter = OraculoMotion.OverlayEnter,
            exit = OraculoMotion.OverlayExit,
        ) {
            ScreenStreamingScreen(
                client = client,
                host = host,
                onExit = { isStreamingMode = false },
            )
        }
    }
}

@Composable
private fun KeepAliveControlTabs(
    selectedTab: ControlTab,
    visitedTabs: Set<ControlTab>,
    modifier: Modifier = Modifier,
    renderTab: @Composable (ControlTab) -> Unit,
) {
    // Mantém abas já visitadas vivas na composição sem medir/desenhar/capturar toques das inativas.
    Layout(
        content = {
            ControlTab.entries.forEach { tab ->
                if (tab in visitedTabs) {
                    key(tab) {
                        Box(modifier = Modifier.layoutId(tab)) {
                            renderTab(tab)
                        }
                    }
                }
            }
        },
        modifier = modifier,
    ) { measurables, constraints ->
        val activeMeasurable = measurables.firstOrNull { it.layoutId == selectedTab }
        val placeable = activeMeasurable?.measure(constraints)
        layout(constraints.maxWidth, constraints.maxHeight) {
            placeable?.placeRelative(0, 0)
        }
    }
}

@Composable
private fun ModeCapsuleBar(
    selected: ControlTab,
    onSelect: (ControlTab) -> Unit,
    modifier: Modifier = Modifier,
) {
    val trackColor = OraculoColors.SurfaceRecessed
    val pillColor = if (OraculoColors.IsDark) OraculoColors.SecondaryContainer else OraculoColors.Surface
    val pillBorderColor = OraculoColors.Outline
    val tabHeight = 46.dp

    BoxWithConstraints(
        modifier = modifier
            .clip(RoundedCornerShape(14.dp))
            .background(trackColor)
            .border(1.dp, OraculoColors.Outline, RoundedCornerShape(14.dp))
            .padding(4.dp),
    ) {
        val tabCount = ControlTab.entries.size
        val tabWidth = maxWidth / tabCount
        val indicatorOffset by animateDpAsState(
            targetValue = tabWidth * selected.ordinal,
            animationSpec = OraculoMotion.TabIndicatorSpring,
            label = "modePillOffset",
        )

        Box(
            modifier = Modifier
                .offset(x = indicatorOffset)
                .width(tabWidth)
                .height(tabHeight)
                .clip(RoundedCornerShape(OraculoMetrics.CornerSmall))
                .background(pillColor)
                .border(1.dp, pillBorderColor, RoundedCornerShape(OraculoMetrics.CornerSmall)),
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
        ) {
            ControlTab.entries.forEach { tab ->
                val isSelected = tab == selected
                val iconTint by animateColorAsState(
                    targetValue = if (isSelected) OraculoColors.Primary else OraculoColors.OnSurfaceMuted,
                    animationSpec = OraculoMotion.microTween(),
                    label = "tabIconTint",
                )
                val labelTint by animateColorAsState(
                    targetValue = if (isSelected) OraculoColors.OnSurface else OraculoColors.OnSurfaceMuted,
                    animationSpec = OraculoMotion.microTween(),
                    label = "tabLabelTint",
                )
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(tabHeight)
                        .clip(RoundedCornerShape(10.dp))
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                        ) { onSelect(tab) },
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier.padding(horizontal = 4.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(2.dp),
                    ) {
                        Icon(
                            imageVector = tab.icon,
                            contentDescription = tab.label,
                            tint = iconTint,
                            modifier = Modifier.size(16.dp),
                        )
                        Text(
                            text = tab.label,
                            style = MaterialTheme.typography.labelMedium,
                            fontSize = 11.sp,
                            fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
                            color = labelTint,
                            maxLines = 1,
                            softWrap = false,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun TouchpadTab(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    showKeyboardBar: Boolean,
    showSettingsBar: Boolean,
    onToggleKeyboardBar: () -> Unit,
    onOpenScreenStream: () -> Unit,
    isPaused: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val prefs = remember(context) { context.getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE) }
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    val haptic = LocalHapticFeedback.current
    var reconnectNonce by remember { mutableIntStateOf(0) }
    var autoRetryAttempts by remember(host.endpointId) { mutableIntStateOf(0) }
    var pendingReconnectDelayMs by remember(host.endpointId) { mutableStateOf(0L) }
    var resumeAfterPause by remember(host.endpointId) { mutableStateOf(false) }
    var inputSession by remember { mutableStateOf<MobileInputSession?>(null) }
    var isConnecting by remember { mutableStateOf(true) }
    var connectHint by remember { mutableStateOf(tr("Conectando touchpad...", "Connecting touchpad...")) }
    var errorMsg by remember { mutableStateOf<String?>(null) }

    var pointerSpeed by remember {
        mutableStateOf(prefs.getFloat("touchpad_speed", 1.5f))
    }
    var naturalScroll by remember {
        mutableStateOf(prefs.getBoolean("touchpad_natural_scroll", false))
    }

    var telemetryReadout by remember { mutableStateOf<String?>(null) }
    var lastTelemetryAtMs by remember { mutableStateOf(0L) }

    fun publishTelemetry(text: String, throttleMs: Long = 0L) {
        val now = System.currentTimeMillis()
        if (throttleMs > 0L && now - lastTelemetryAtMs < throttleMs) {
            return
        }
        lastTelemetryAtMs = now
        telemetryReadout = text
    }

    lateinit var triggerReconnectRef: (Boolean, String) -> Unit
    val keyboardController = rememberRemoteInputController(
        inputSession = inputSession,
        onSessionBroken = { broken ->
            if (inputSession === broken && !isConnecting) {
                triggerReconnectRef(false, tr("Reconectando touchpad...", "Reconnecting touchpad..."))
            }
        },
        onPublishTelemetry = { publishTelemetry(it, 0L) },
    )

    fun triggerReconnect(resetAttempts: Boolean, hint: String) {
        if (resetAttempts) {
            autoRetryAttempts = 0
            pendingReconnectDelayMs = 0L
        } else {
            if (autoRetryAttempts >= 3) {
                inputSession = null
                isConnecting = false
                errorMsg = tr("Conexão do touchpad encerrada por inatividade.", "Touchpad connection closed due to inactivity.")
                keyboardController.releaseAllModifiers(null)
                return
            }
            autoRetryAttempts += 1
            pendingReconnectDelayMs = if (autoRetryAttempts == 1) 250L else 850L
        }
        inputSession = null
        errorMsg = null
        connectHint = hint
        keyboardController.releaseAllModifiers(null)
        reconnectNonce += 1
    }
    triggerReconnectRef = ::triggerReconnect

    fun sendOnSession(target: MobileInputSession?, action: (MobileInputSession) -> Unit): Boolean =
        keyboardController.sendOnSession(target, action)

    val currentIsPaused by rememberUpdatedState(isPaused)
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() }
    DisposableEffect(lifecycleOwner, host.endpointId) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val lifecycleObserver = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME && !currentIsPaused) {
                val current = inputSession
                if ((current == null && !isConnecting) || (current != null && !current.isAlive())) {
                    triggerReconnect(resetAttempts = true, hint = "Reconectando touchpad...")
                }
            }
        }
        owner.lifecycle.addObserver(lifecycleObserver)
        onDispose {
            owner.lifecycle.removeObserver(lifecycleObserver)
        }
    }

    LaunchedEffect(inputSession, isPaused) {
        val session = inputSession ?: return@LaunchedEffect
        if (isPaused) return@LaunchedEffect
        delay(4_000L)
        if (inputSession === session && session.isAlive()) {
            autoRetryAttempts = 0
        }
    }

    DisposableEffect(host.endpointId, reconnectNonce, isPaused) {
        if (isPaused) {
            resumeAfterPause = true
            isConnecting = false
            errorMsg = null
            return@DisposableEffect onDispose {}
        }
        var active = true
        var createdSession: MobileInputSession? = null
        val resumeDelay = if (resumeAfterPause) {
            resumeAfterPause = false
            190L
        } else {
            0L
        }
        val waitBeforeConnectMs = maxOf(pendingReconnectDelayMs, resumeDelay)
        pendingReconnectDelayMs = 0L
        isConnecting = true
        errorMsg = null
        if (autoRetryAttempts == 0) {
            connectHint = tr("Conectando touchpad...", "Connecting touchpad...")
        }
        keyboardController.resetState(null)

        val hintJob = CoroutineScope(Dispatchers.Main).launch {
            delay(1300)
            if (active && isConnecting) {
                connectHint = tr(
                    "Se aparecer aviso no Fedora, clique em Compartilhar",
                    "If prompted on the PC, click Share to allow remote input",
                )
            }
        }

        val inputObserver = object : InputObserver {
            override fun onSessionClosed(reason: String) {
                mainHandler.post {
                    if (!active) return@post
                    if (createdSession != null && inputSession === createdSession) {
                        createdSession = null
                        triggerReconnect(resetAttempts = false, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
                    }
                }
            }
        }

        val openJob = CoroutineScope(Dispatchers.IO).launch {
            if (waitBeforeConnectMs > 0L) {
                delay(waitBeforeConnectMs)
            }
            if (!active) return@launch
            val result = runCatching { client.openInput(host.endpointId, inputObserver) }
            withContext(Dispatchers.Main) {
                if (!active) {
                    result.onSuccess { s ->
                        CoroutineScope(Dispatchers.IO).launch {
                            runCatching {
                                s.closeSession()
                                s.close()
                            }
                        }
                    }
                    return@withContext
                }
                hintJob.cancel()
                result.onSuccess { created ->
                    isConnecting = false
                    pendingReconnectDelayMs = 0L
                    createdSession = created
                    inputSession = created
                }.onFailure { err ->
                    if (autoRetryAttempts in 1 until 3) {
                        triggerReconnect(resetAttempts = false, hint = "Reconectando touchpad...")
                    } else {
                        isConnecting = false
                        errorMsg = formatOraculoError(err)
                    }
                }
            }
        }

        onDispose {
            active = false
            hintJob.cancel()
            openJob.cancel()
            val toClose = createdSession ?: inputSession
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

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp)
            .padding(bottom = 10.dp, top = 2.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        TouchpadSettingsDrawer(
            visible = showSettingsBar,
            pointerSpeed = pointerSpeed,
            naturalScroll = naturalScroll,
            onSelectSpeed = { speed ->
                pointerSpeed = speed
                prefs.edit().putFloat("touchpad_speed", speed).apply()
            },
            onToggleNaturalScroll = {
                val next = !naturalScroll
                naturalScroll = next
                prefs.edit().putBoolean("touchpad_natural_scroll", next).apply()
            },
        )

        TouchpadSurface(
            inputSession = inputSession,
            isConnecting = isConnecting,
            connectHint = connectHint,
            errorMsg = errorMsg,
            pointerSpeed = pointerSpeed,
            naturalScroll = naturalScroll,
            onRetry = { triggerReconnect(resetAttempts = true, hint = tr("Conectando touchpad...", "Connecting touchpad...")) },
            onSessionBroken = { broken ->
                if (inputSession === broken && !isConnecting) {
                    triggerReconnect(resetAttempts = false, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
                }
            },
            onDisarmModifiers = { keyboardController.disarmArmedModifiers() },
            telemetryProvider = { telemetryReadout },
            onPublishTelemetry = { text, throttleMs -> publishTelemetry(text, throttleMs) },
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OraculoDeckButton(
                label = if (showKeyboardBar) tr("Fechar teclado", "Close keyboard") else tr("Abrir teclado", "Open keyboard"),
                icon = if (showKeyboardBar) Icons.Default.KeyboardHide else Icons.Default.Keyboard,
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onToggleKeyboardBar()
                },
                active = showKeyboardBar,
                height = OraculoMetrics.ControlHeightDeck,
                modifier = Modifier.weight(1f),
            )
            OraculoDeckButton(
                label = tr("Abrir tela", "Open screen"),
                icon = Icons.Default.DesktopWindows,
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onOpenScreenStream()
                },
                accent = true,
                height = OraculoMetrics.ControlHeightDeck,
                modifier = Modifier.weight(1f),
            )
        }

        RemoteModifierRow(
            controller = keyboardController,
            enabled = inputSession != null,
        )

        RemoteShortcutStrip(
            controller = keyboardController,
            enabled = inputSession != null,
        )

        RemoteKeyboardBar(
            visible = showKeyboardBar,
            controller = keyboardController,
            enabled = inputSession != null,
        )

        TouchpadMouseButtonsRow(
            enabled = inputSession != null,
            onButtonPress = { button, pressed ->
                if (pressed) haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                val current = inputSession
                sendOnSession(current) { it.pressButton(button, pressed) }
            },
        )
    }
}

@Composable
private fun TouchpadSettingsDrawer(
    visible: Boolean,
    pointerSpeed: Float,
    naturalScroll: Boolean,
    onSelectSpeed: (Float) -> Unit,
    onToggleNaturalScroll: () -> Unit,
) {
    AnimatedVisibility(
        visible = visible,
        enter = OraculoMotion.ExpandV,
        exit = OraculoMotion.CollapseV,
    ) {
        val chipShape = RoundedCornerShape(OraculoMetrics.CornerSmall)
        Surface(
            color = OraculoColors.Surface,
            shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
            modifier = Modifier
                .fillMaxWidth()
                .border(1.dp, OraculoColors.Outline, RoundedCornerShape(OraculoMetrics.CornerLarge)),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = 10.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text(
                    text = "Cursor:",
                    style = MaterialTheme.typography.labelSmall,
                    color = OraculoColors.OnSurfaceMuted,
                )
                listOf(1.0f to "1.0x", 1.5f to "1.5x", 2.2f to "2.2x", 3.0f to "3.0x").forEach { (speed, label) ->
                    val selected = abs(pointerSpeed - speed) < 0.05f
                    val (speedSource, speedPressed) = rememberPressedInteraction()
                    Surface(
                        color = if (selected) OraculoColors.Primary else OraculoColors.SurfaceRecessed,
                        shape = chipShape,
                        modifier = Modifier
                            .height(OraculoMetrics.ControlHeightCompactChip)
                            .oraculoPressScale(speedPressed)
                            .clip(chipShape)
                            .border(
                                1.dp,
                                if (selected) OraculoColors.Primary else OraculoColors.Outline,
                                chipShape,
                            )
                            .clickable(
                                interactionSource = speedSource,
                                indication = LocalIndication.current,
                            ) { onSelectSpeed(speed) },
                    ) {
                        Box(
                            modifier = Modifier.padding(horizontal = 10.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = label,
                                style = MaterialTheme.typography.labelSmall,
                                fontFamily = FontFamily.Monospace,
                                color = if (selected) OraculoColors.OnPrimary else OraculoColors.OnSurface,
                            )
                        }
                    }
                }

                Spacer(modifier = Modifier.width(4.dp))

                val (scrollSource, scrollPressed) = rememberPressedInteraction()
                Surface(
                    color = if (naturalScroll) OraculoColors.SecondaryContainer else OraculoColors.SurfaceRecessed,
                    shape = chipShape,
                    modifier = Modifier
                        .height(OraculoMetrics.ControlHeightCompactChip)
                        .oraculoPressScale(scrollPressed)
                        .clip(chipShape)
                        .border(
                            1.dp,
                            if (naturalScroll) OraculoColors.Primary else OraculoColors.Outline,
                            chipShape,
                        )
                        .clickable(
                            interactionSource = scrollSource,
                            indication = LocalIndication.current,
                        ) { onToggleNaturalScroll() },
                ) {
                    Box(
                        modifier = Modifier.padding(horizontal = 10.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = if (naturalScroll) {
                                tr("Scroll natural: ON", "Natural scroll: ON")
                            } else {
                                tr("Scroll natural: OFF", "Natural scroll: OFF")
                            },
                            style = MaterialTheme.typography.labelSmall,
                            color = if (naturalScroll) OraculoColors.Primary else OraculoColors.OnSurfaceMuted,
                        )
                    }
                }
            }
        }
    }
}

@Composable
internal fun TouchpadSurface(
    inputSession: MobileInputSession?,
    isConnecting: Boolean,
    connectHint: String,
    errorMsg: String?,
    pointerSpeed: Float,
    naturalScroll: Boolean,
    onRetry: () -> Unit,
    onSessionBroken: (MobileInputSession) -> Unit,
    onDisarmModifiers: () -> Unit,
    telemetryProvider: () -> String?,
    onPublishTelemetry: (String, Long) -> Unit,
    modifier: Modifier = Modifier,
    compactMode: Boolean = false,
) {
    val haptic = LocalHapticFeedback.current
    var hasTouched by remember { mutableStateOf(false) }
    var touchSpot by remember { mutableStateOf<Offset?>(null) }
    var touchActive by remember { mutableStateOf(false) }

    val guideAlpha by animateFloatAsState(
        targetValue = if (hasTouched) 0f else 0.9f,
        animationSpec = tween(OraculoMotion.RouteEnterMs, easing = OraculoMotion.Easing),
        label = "touchpadGuideAlpha",
    )
    val touchGlowAlpha by animateFloatAsState(
        targetValue = if (touchActive) 1f else 0f,
        animationSpec = tween(
            durationMillis = if (touchActive) OraculoMotion.MicroDurationMs else OraculoMotion.RouteEnterMs,
            easing = OraculoMotion.Easing,
        ),
        label = "touchGlowAlpha",
    )

    val currentPointerSpeed by rememberUpdatedState(pointerSpeed)
    val currentNaturalScroll by rememberUpdatedState(naturalScroll)
    val currentDisarmArmed by rememberUpdatedState(onDisarmModifiers)
    val currentPublishTelemetry by rememberUpdatedState(onPublishTelemetry)
    val currentOnSessionBroken by rememberUpdatedState(onSessionBroken)

    fun sendOrReport(session: MobileInputSession, block: () -> Unit): Boolean =
        runCatching { block() }
            .onFailure { currentOnSessionBroken(session) }
            .isSuccess

    val cornerShape = RoundedCornerShape(if (compactMode) 16.dp else 22.dp)

    Box(
        modifier = modifier
            .clip(cornerShape)
            .background(OraculoColors.Surface)
            .border(1.dp, OraculoColors.Outline, cornerShape)
            .pointerInput(inputSession) {
                val activeSession = inputSession ?: return@pointerInput
                var lastLeftTapUpMs = 0L
                awaitEachGesture {
                    val firstDown = awaitFirstDown(requireUnconsumed = false)
                    hasTouched = true
                    touchSpot = firstDown.position
                    touchActive = true
                    val gestureStartMs = System.currentTimeMillis()
                    val isPotentialDrag = (gestureStartMs - lastLeftTapUpMs) <= 280L
                    var isDragging = false
                    var maxPointers = 1
                    var totalMove = 0f

                    try {
                        while (true) {
                            val event = awaitPointerEvent()
                            val pressed = event.changes.filter { it.pressed }
                            maxPointers = maxOf(maxPointers, pressed.size)
                            if (pressed.isNotEmpty()) {
                                touchSpot = pressed[0].position
                            }

                            if (pressed.isEmpty()) {
                                val wentUpCount = event.changes.count { it.changedToUpIgnoreConsumed() }
                                val elapsed = System.currentTimeMillis() - gestureStartMs
                                if (isDragging) {
                                    sendOrReport(activeSession) {
                                        activeSession.pressButton(MobileMouseButton.LEFT, false)
                                    }
                                    isDragging = false
                                    lastLeftTapUpMs = 0L
                                    currentDisarmArmed()
                                    currentPublishTelemetry(tr("solto", "released"), 0L)
                                } else if (wentUpCount > 0) {
                                    if (maxPointers >= 2) {
                                        if (totalMove < viewConfiguration.touchSlop * 2.2f && elapsed < 320L) {
                                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                            if (sendOrReport(activeSession) {
                                                    activeSession.clickButton(MobileMouseButton.RIGHT)
                                                }
                                            ) {
                                                currentDisarmArmed()
                                                currentPublishTelemetry(tr("botão direito", "right click"), 0L)
                                            }
                                        }
                                        lastLeftTapUpMs = 0L
                                    } else if (totalMove < viewConfiguration.touchSlop) {
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        if (sendOrReport(activeSession) {
                                                activeSession.clickButton(MobileMouseButton.LEFT)
                                            }
                                        ) {
                                            currentDisarmArmed()
                                            lastLeftTapUpMs = System.currentTimeMillis()
                                            currentPublishTelemetry(tr("clique", "click"), 0L)
                                        }
                                    } else {
                                        lastLeftTapUpMs = 0L
                                    }
                                }
                                break
                            }

                            if (pressed.size == 1 && maxPointers == 1) {
                                val change = pressed[0]
                                val delta = change.positionChange()
                                if (abs(delta.x) > 0.25f || abs(delta.y) > 0.25f) {
                                    val stepDist = delta.getDistance()
                                    totalMove += stepDist
                                    if (!isDragging && isPotentialDrag && totalMove >= viewConfiguration.touchSlop * 0.5f) {
                                        isDragging = true
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        sendOrReport(activeSession) {
                                            activeSession.pressButton(MobileMouseButton.LEFT, true)
                                        }
                                    }
                                    val boost = 1.0 + (stepDist / 48.0).coerceAtMost(0.5)
                                    val sx = delta.x * currentPointerSpeed * boost
                                    val sy = delta.y * currentPointerSpeed * boost
                                    sendOrReport(activeSession) {
                                        activeSession.movePointer(sx, sy)
                                    }
                                    currentPublishTelemetry(
                                        if (isDragging) tr("arrastando", "dragging") else "dx ${delta.x.roundToInt()}  dy ${delta.y.roundToInt()}",
                                        55L,
                                    )
                                    change.consume()
                                }
                            } else if (pressed.size >= 2) {
                                if (isDragging) {
                                    sendOrReport(activeSession) {
                                        activeSession.pressButton(MobileMouseButton.LEFT, false)
                                    }
                                    isDragging = false
                                }
                                var sumX = 0.0
                                var sumY = 0.0
                                pressed.forEach {
                                    val d = it.positionChange()
                                    sumX += d.x
                                    sumY += d.y
                                }
                                val avgDx = sumX / pressed.size
                                val avgDy = sumY / pressed.size
                                totalMove += hypot(avgDx, avgDy).toFloat()
                                if (abs(avgDx) > 0.25 || abs(avgDy) > 0.25) {
                                    val scrollSign = if (currentNaturalScroll) 1.0 else -1.0
                                    sendOrReport(activeSession) {
                                        activeSession.scrollPointer(
                                            avgDx * 0.42 * scrollSign,
                                            avgDy * 0.42 * scrollSign,
                                        )
                                    }
                                    currentPublishTelemetry("scroll", 55L)
                                    pressed.forEach { it.consume() }
                                }
                            }
                        }
                    } finally {
                        touchActive = false
                        if (isDragging) {
                            sendOrReport(activeSession) {
                                activeSession.pressButton(MobileMouseButton.LEFT, false)
                            }
                        }
                    }
                }
            },
    ) {
        CornerReticles(
            touchSpotProvider = { touchSpot },
            touchGlowAlphaProvider = { touchGlowAlpha },
            modifier = Modifier.fillMaxSize(),
        )

        val overlayPadding = if (compactMode) 8.dp else 14.dp

        if (errorMsg != null) {
            Column(
                modifier = Modifier
                    .align(Alignment.Center)
                    .padding(if (compactMode) 10.dp else 24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(if (compactMode) 6.dp else 12.dp),
            ) {
                Text(
                    text = errorMsg,
                    style = if (compactMode) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
                    color = OraculoColors.Error,
                    textAlign = TextAlign.Center,
                )
                Button(
                    onClick = onRetry,
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    contentPadding = if (compactMode) PaddingValues(horizontal = 12.dp) else ButtonDefaults.ContentPadding,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.SecondaryContainer,
                        contentColor = OraculoColors.OnSecondaryContainer,
                    ),
                    modifier = Modifier.height(
                        if (compactMode) OraculoMetrics.ControlHeightSmall else OraculoMetrics.ControlHeightDialog,
                    ),
                ) {
                    Text(
                        text = tr("Tentar novamente", "Try again"),
                        style = if (compactMode) MaterialTheme.typography.labelMedium else MaterialTheme.typography.labelLarge,
                    )
                }
            }
        } else {
            Column(
                modifier = Modifier
                    .align(Alignment.Center)
                    .padding(horizontal = if (compactMode) 16.dp else 28.dp)
                    .graphicsLayer { alpha = guideAlpha },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(if (compactMode) 4.dp else 10.dp),
            ) {
                if (compactMode) {
                    Text(
                        text = "Touchpad",
                        style = MaterialTheme.typography.labelLarge,
                        color = OraculoColors.OnSurface,
                        textAlign = TextAlign.Center,
                    )
                    Text(
                        text = tr("Toque: clicar · 2 dedos: direito/rolar", "Tap: click · 2 fingers: right/scroll"),
                        style = MaterialTheme.typography.labelSmall,
                        color = OraculoColors.OnSurfaceMuted,
                        textAlign = TextAlign.Center,
                    )
                } else {
                    Text(
                        text = tr("Toque para clicar", "Tap to click"),
                        style = MaterialTheme.typography.titleMedium,
                        color = OraculoColors.OnSurface,
                        textAlign = TextAlign.Center,
                    )
                    Text(
                        text = tr("Toque duplo e arraste para mover janelas", "Double-tap and drag to move windows"),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                        textAlign = TextAlign.Center,
                    )
                    Text(
                        text = tr("Dois dedos: clique direito ou rolar", "Two fingers: right-click or scroll"),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }

        if (isConnecting) {
            Surface(
                color = OraculoColors.SurfaceRecessed,
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .padding(overlayPadding)
                    .border(1.dp, OraculoColors.Outline, RoundedCornerShape(12.dp)),
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 7.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    CircularProgressIndicator(
                        color = OraculoColors.Primary,
                        modifier = Modifier.size(13.dp),
                        strokeWidth = 2.dp,
                    )
                    Text(
                        text = connectHint,
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurface,
                    )
                }
            }
        }

        TouchpadTelemetryPill(
            textProvider = telemetryProvider,
            isConnected = inputSession != null,
            modifier = Modifier
                .align(Alignment.BottomEnd)
                .padding(overlayPadding),
        )
    }
}

@Composable
private fun TouchpadTelemetryPill(
    textProvider: () -> String?,
    isConnected: Boolean,
    modifier: Modifier = Modifier,
) {
    val label = textProvider()
    AnimatedVisibility(
        visible = label != null && isConnected,
        enter = fadeIn(OraculoMotion.microTween()),
        exit = fadeOut(OraculoMotion.microTween()),
        modifier = modifier,
    ) {
        Surface(
            color = OraculoColors.SurfaceRecessed,
            shape = RoundedCornerShape(10.dp),
            modifier = Modifier.border(
                1.dp,
                OraculoColors.Outline,
                RoundedCornerShape(10.dp),
            ),
        ) {
            Text(
                text = label.orEmpty(),
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontFeatureSettings = "tnum",
                ),
                color = OraculoColors.Primary,
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 5.dp),
            )
        }
    }
}

@Composable
internal fun TouchpadMouseButtonsRow(
    enabled: Boolean,
    onButtonPress: (MobileMouseButton, Boolean) -> Unit,
    modifier: Modifier = Modifier,
    buttonHeight: Dp = OraculoMetrics.ControlHeightDeck,
    compactLabels: Boolean = false,
) {
    val labelSize = if (compactLabels) 12.sp else 13.5.sp
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(buttonHeight),
        horizontalArrangement = Arrangement.spacedBy(if (compactLabels) 6.dp else 8.dp),
    ) {
        MouseKeyButton(
            label = if (compactLabels) tr("Esq", "L") else tr("Esquerdo", "Left"),
            enabled = enabled,
            fontSize = labelSize,
            cornerRadius = if (compactLabels) OraculoMetrics.CornerMedium else OraculoMetrics.CornerLarge,
            onPressChanged = { onButtonPress(MobileMouseButton.LEFT, it) },
            modifier = Modifier
                .weight(1f)
                .height(buttonHeight),
        )

        MouseKeyButton(
            label = if (compactLabels) tr("Meio", "M") else tr("Meio", "Middle"),
            enabled = enabled,
            fontSize = labelSize,
            cornerRadius = if (compactLabels) OraculoMetrics.CornerMedium else OraculoMetrics.CornerLarge,
            onPressChanged = { onButtonPress(MobileMouseButton.MIDDLE, it) },
            modifier = Modifier
                .weight(1f)
                .height(buttonHeight),
        )

        MouseKeyButton(
            label = if (compactLabels) tr("Dir", "R") else tr("Direito", "Right"),
            enabled = enabled,
            fontSize = labelSize,
            cornerRadius = if (compactLabels) OraculoMetrics.CornerMedium else OraculoMetrics.CornerLarge,
            onPressChanged = { onButtonPress(MobileMouseButton.RIGHT, it) },
            modifier = Modifier
                .weight(1f)
                .height(buttonHeight),
        )
    }
}

@Composable
private fun MouseKeyButton(
    label: String,
    enabled: Boolean,
    onPressChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    fontSize: androidx.compose.ui.unit.TextUnit = 13.5.sp,
    cornerRadius: Dp = OraculoMetrics.CornerLarge,
) {
    var isPressed by remember { mutableStateOf(false) }
    val shape = RoundedCornerShape(cornerRadius)
    val bg by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.Surface.copy(alpha = 0.55f)
            isPressed -> OraculoColors.SecondaryContainer
            else -> OraculoColors.Surface
        },
        animationSpec = OraculoMotion.microTween(),
        label = "mouseBtnBg",
    )
    val borderColor by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.Outline.copy(alpha = 0.5f)
            isPressed -> OraculoColors.Primary
            else -> OraculoColors.Outline
        },
        animationSpec = OraculoMotion.microTween(),
        label = "mouseBtnBorder",
    )
    val contentColor by animateColorAsState(
        targetValue = when {
            !enabled -> OraculoColors.OnSurfaceMuted.copy(alpha = 0.6f)
            isPressed -> OraculoColors.Primary
            else -> OraculoColors.OnSurface
        },
        animationSpec = OraculoMotion.microTween(),
        label = "mouseBtnText",
    )
    val pressScale by animateFloatAsState(
        targetValue = if (isPressed) 0.965f else 1f,
        animationSpec = OraculoMotion.microTween(),
        label = "mouseBtnScale",
    )

    Box(
        modifier = modifier
            .graphicsLayer {
                scaleX = pressScale
                scaleY = pressScale
            }
            .clip(shape)
            .background(bg)
            .border(1.dp, borderColor, shape)
            .pointerInput(enabled) {
                if (!enabled) return@pointerInput
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    isPressed = true
                    onPressChanged(true)
                    try {
                        while (true) {
                            val event = awaitPointerEvent()
                            if (event.changes.none { it.pressed }) {
                                break
                            }
                        }
                    } finally {
                        isPressed = false
                        onPressChanged(false)
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            fontSize = fontSize,
            color = contentColor,
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Clip,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
    }
}

@Composable
private fun CornerReticles(
    touchSpotProvider: () -> Offset?,
    touchGlowAlphaProvider: () -> Float,
    modifier: Modifier = Modifier,
) {
    val reticleColor = OraculoColors.Outline.copy(alpha = 0.9f)
    val glowColor = OraculoColors.Primary

    Canvas(modifier = modifier) {
        val inset = 22.dp.toPx()
        val len = 10.dp.toPx()
        val stroke = 1.5.dp.toPx()
        val touchSpot = touchSpotProvider()
        val touchGlowAlpha = touchGlowAlphaProvider()

        if (touchSpot != null && touchGlowAlpha > 0.01f) {
            val radius = 84.dp.toPx()
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(
                        glowColor.copy(alpha = 0.18f * touchGlowAlpha),
                        glowColor.copy(alpha = 0.06f * touchGlowAlpha),
                        Color.Transparent,
                    ),
                    center = touchSpot,
                    radius = radius,
                ),
                radius = radius,
                center = touchSpot,
            )
        }

        drawLine(reticleColor, Offset(inset, inset), Offset(inset + len, inset), stroke)
        drawLine(reticleColor, Offset(inset, inset), Offset(inset, inset + len), stroke)
        drawLine(reticleColor, Offset(size.width - inset, inset), Offset(size.width - inset - len, inset), stroke)
        drawLine(reticleColor, Offset(size.width - inset, inset), Offset(size.width - inset, inset + len), stroke)
        drawLine(reticleColor, Offset(inset, size.height - inset), Offset(inset + len, size.height - inset), stroke)
        drawLine(reticleColor, Offset(inset, size.height - inset), Offset(inset, size.height - inset - len), stroke)
        drawLine(reticleColor, Offset(size.width - inset, size.height - inset), Offset(size.width - inset - len, size.height - inset), stroke)
        drawLine(reticleColor, Offset(size.width - inset, size.height - inset), Offset(size.width - inset, size.height - inset - len), stroke)
    }
}

private data class TerminalQuickKeySpec(
    val label: String,
    val bytes: ByteArray,
    val icon: ImageVector? = null,
    val clearsDraft: Boolean = true,
)

private val TERMINAL_QUICK_KEYS = listOf(
    TerminalQuickKeySpec("Tab", byteArrayOf(9)),
    TerminalQuickKeySpec("Apagar", byteArrayOf(0x7f), Icons.AutoMirrored.Filled.Backspace),
    TerminalQuickKeySpec("Ctrl+C", byteArrayOf(3)),
    TerminalQuickKeySpec("Ctrl+D", byteArrayOf(4)),
    TerminalQuickKeySpec("Ctrl+L", byteArrayOf(12), clearsDraft = false),
    TerminalQuickKeySpec("Esc", byteArrayOf(27)),
    TerminalQuickKeySpec("Esquerda", "\u001b[D".encodeToByteArray(), Icons.AutoMirrored.Filled.KeyboardArrowLeft),
    TerminalQuickKeySpec("Cima", "\u001b[A".encodeToByteArray(), Icons.Default.KeyboardArrowUp),
    TerminalQuickKeySpec("Baixo", "\u001b[B".encodeToByteArray(), Icons.Default.KeyboardArrowDown),
    TerminalQuickKeySpec("Direita", "\u001b[C".encodeToByteArray(), Icons.AutoMirrored.Filled.KeyboardArrowRight),
    TerminalQuickKeySpec("Enter", byteArrayOf(13)),
)

@Composable
private fun TerminalTab(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val haptic = LocalHapticFeedback.current
    val clipboard = LocalClipboardManager.current
    val prefs = remember(context) {
        context.getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE)
    }
    var fontSizeSp by remember {
        mutableStateOf(prefs.getFloat("terminal_font_sp", 12.5f).coerceIn(10f, 18f))
    }

    fun updateFontSize(delta: Float) {
        val next = (fontSizeSp + delta).coerceIn(10f, 18f)
        if (next != fontSizeSp) {
            fontSizeSp = next
            prefs.edit().putFloat("terminal_font_sp", next).apply()
        }
    }

    val terminalStyle = remember(fontSizeSp) {
        TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = fontSizeSp.sp,
            lineHeight = (fontSizeSp * 1.42f).sp,
        )
    }

    val ptyxis = ptyxisPaletteFor(OraculoColors.IsDark)
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    val inputFocusRequester = remember { FocusRequester() }
    val pendingSnapshot = remember { AtomicReference<TerminalScreenSnapshot?>(null) }
    val renderScheduled = remember { AtomicBoolean(false) }

    var reconnectNonce by remember { mutableIntStateOf(0) }
    var session by remember { mutableStateOf<MobileTerminalSession?>(null) }
    var screenSnapshot by remember { mutableStateOf<TerminalScreenSnapshot?>(null) }
    var screenRevision by remember { mutableIntStateOf(0) }
    var fallbackMessage by remember {
        mutableStateOf(tr("Conectando PTY a ${host.hostName}...", "Connecting PTY to ${host.hostName}..."))
    }
    var terminalStatus by remember { mutableStateOf(tr("Conectando...", "Connecting...")) }
    var remoteField by remember { mutableStateOf(TextFieldValue("")) }
    var syncedText by remember { mutableStateOf("") }
    var isConnecting by remember { mutableStateOf(true) }
    var termCols by remember { mutableIntStateOf(56) }
    var termRows by remember { mutableIntStateOf(24) }
    var copiedRecently by remember { mutableStateOf(false) }
    var lastBellCount by remember { mutableStateOf(0UL) }
    var visualBellActive by remember { mutableStateOf(false) }

    val plainCopyText = remember(screenSnapshot, fallbackMessage) {
        (screenSnapshot?.plainText ?: fallbackMessage).trimEnd('\n', '\r', ' ').ifEmpty { " " }
    }

    LaunchedEffect(visualBellActive) {
        if (visualBellActive) {
            delay(220)
            visualBellActive = false
        }
    }

    LaunchedEffect(copiedRecently) {
        if (copiedRecently) {
            delay(1500)
            copiedRecently = false
        }
    }

    LaunchedEffect(session, termCols, termRows) {
        val activeSession = session ?: return@LaunchedEffect
        delay(90)
        withContext(Dispatchers.IO) {
            runCatching { activeSession.resize(termCols.toUShort(), termRows.toUShort()) }
        }
        if (terminalStatus.startsWith("Shell ativo") || terminalStatus.startsWith("Active shell")) {
            terminalStatus = tr("Shell ativo (${termCols}x${termRows})", "Active shell (${termCols}x${termRows})")
        }
    }

    DisposableEffect(host.endpointId, reconnectNonce) {
        var active = true
        var openedSession: MobileTerminalSession? = null
        pendingSnapshot.set(null)
        renderScheduled.set(false)
        isConnecting = true
        screenSnapshot = null
        screenRevision = 0
        fallbackMessage = tr("Conectando PTY a ${host.hostName}...", "Connecting PTY to ${host.hostName}...")
        terminalStatus = tr("Abrindo shell...", "Opening shell...")
        remoteField = TextFieldValue("")
        syncedText = ""

        val job = CoroutineScope(Dispatchers.IO).launch {
            val observer = object : TerminalObserver {
                override fun onScreenUpdated(snapshot: TerminalScreenSnapshot) {
                    if (!active) return
                    pendingSnapshot.set(snapshot)
                    if (renderScheduled.compareAndSet(false, true)) {
                        mainHandler.post {
                            renderScheduled.set(false)
                            if (!active) return@post
                            val latest = pendingSnapshot.getAndSet(null) ?: return@post
                            screenSnapshot = latest
                            screenRevision += 1
                            if (latest.bellCount > lastBellCount) {
                                lastBellCount = latest.bellCount
                                visualBellActive = true
                            }
                            terminalStatus = tr(
                                "Shell ativo (${latest.cols}x${latest.rows})",
                                "Active shell (${latest.cols}x${latest.rows})",
                            )
                            isConnecting = false
                        }
                    }
                }

                override fun onSessionEnded(exitCode: Int) {
                    mainHandler.post {
                        if (active) {
                            terminalStatus = tr("Encerrado (exit $exitCode)", "Ended (exit $exitCode)")
                            isConnecting = false
                        }
                    }
                }

                override fun onError(message: String) {
                    mainHandler.post {
                        if (active) {
                            terminalStatus = tr("Erro: $message", "Error: $message")
                            isConnecting = false
                        }
                    }
                }
            }

            val result = runCatching {
                client.openTerminal(
                    endpointId = host.endpointId,
                    cols = termCols.toUShort(),
                    rows = termRows.toUShort(),
                    observer = observer,
                )
            }

            result.onSuccess { created ->
                if (active) {
                    openedSession = created
                    withContext(Dispatchers.Main) {
                        session = created
                        terminalStatus = tr(
                            "Shell ativo (${termCols}x${termRows})",
                            "Active shell (${termCols}x${termRows})",
                        )
                        isConnecting = false
                    }
                } else {
                    runCatching {
                        created.closeSession()
                        created.close()
                    }
                }
            }.onFailure { err ->
                withContext(Dispatchers.Main) {
                    if (active) {
                        isConnecting = false
                        screenSnapshot = null
                        screenRevision = 0
                        terminalStatus = tr("Falha na conexão", "Connection failed")
                        fallbackMessage = tr(
                            "Não foi possível abrir o terminal:\n${formatOraculoError(err)}",
                            "Could not open terminal:\n${formatOraculoError(err)}",
                        )
                    }
                }
            }
        }

        onDispose {
            active = false
            pendingSnapshot.set(null)
            renderScheduled.set(false)
            job.cancel()
            remoteField = TextFieldValue("")
            syncedText = ""
            val toClose = openedSession ?: session
            session = null
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

    fun clearDraft() {
        remoteField = TextFieldValue("")
        syncedText = ""
    }

    fun sendBytes(bytes: ByteArray, triggerHaptic: Boolean = true) {
        val current = session ?: return
        if (bytes.isEmpty()) return
        if (triggerHaptic) {
            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        }
        runCatching { current.sendInput(bytes) }
            .onFailure { err ->
                terminalStatus = "Erro ao enviar: ${formatOraculoError(err)}"
            }
    }

    fun sendLiveDelta(deletes: Int, inserted: String) {
        if (session == null) return
        val insertedBytes = if (inserted.isEmpty()) {
            ByteArray(0)
        } else {
            inserted.replace("\r\n", "\r").replace('\n', '\r').encodeToByteArray()
        }
        if (deletes == 0 && insertedBytes.isEmpty()) return
        val payload = ByteArray(deletes + insertedBytes.size)
        for (i in 0 until deletes) {
            payload[i] = 0x7f
        }
        insertedBytes.copyInto(payload, destinationOffset = deletes)
        sendBytes(payload, triggerHaptic = false)
    }

    fun sendEnterAndClear() {
        if (session == null) return
        clearDraft()
        sendBytes(byteArrayOf(13))
    }

    val terminalBorderColor by animateColorAsState(
        targetValue = if (visualBellActive) OraculoColors.Primary else ptyxis.HeaderBorder,
        animationSpec = OraculoMotion.stateTween(),
        label = "ptyxisBellBorder",
    )
    val windowTitle = screenSnapshot?.title?.takeIf { it.isNotBlank() } ?: terminalStatus
    val displayCols = screenSnapshot?.cols?.toInt() ?: termCols
    val displayRows = screenSnapshot?.rows?.toInt() ?: termRows

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp)
            .padding(bottom = 10.dp, top = 2.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .clip(RoundedCornerShape(16.dp))
                .background(ptyxis.Background)
                .border(1.dp, terminalBorderColor, RoundedCornerShape(16.dp)),
        ) {
            PtyxisTerminalHeader(
                title = windowTitle,
                cols = displayCols,
                rows = displayRows,
                ptyxis = ptyxis,
                fontSizeSp = fontSizeSp,
                copiedRecently = copiedRecently,
                canSendClear = session != null,
                onDecreaseFont = { updateFontSize(-1f) },
                onIncreaseFont = { updateFontSize(1f) },
                onCopy = {
                    clipboard.setText(AnnotatedString(plainCopyText))
                    copiedRecently = true
                },
                onClearTerminal = { sendBytes(byteArrayOf(12)) },
                onReconnect = { reconnectNonce += 1 },
            )

            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(1.dp)
                    .background(ptyxis.HeaderBorder),
            )

            PtyxisViewport(
                screenSnapshot = screenSnapshot,
                screenRevision = screenRevision,
                fallbackMessage = fallbackMessage,
                terminalStyle = terminalStyle,
                ptyxis = ptyxis,
                isConnecting = isConnecting,
                onGridMeasured = { cols, rows ->
                    termCols = cols
                    termRows = rows
                },
                onTapViewport = {
                    runCatching { inputFocusRequester.requestFocus() }
                },
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth(),
            )
        }

        TerminalQuickKeysBar(
            onKey = { keySpec ->
                sendBytes(keySpec.bytes)
                if (keySpec.clearsDraft) {
                    clearDraft()
                }
            },
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedTextField(
                value = remoteField,
                onValueChange = { next ->
                    if (session == null) {
                        clearDraft()
                        return@OutlinedTextField
                    }
                    val newText = next.text
                    if (newText != syncedText) {
                        val (deletes, inserted) = computeLiveTextDelta(syncedText, newText)
                        sendLiveDelta(deletes, inserted)
                        if (newText.contains('\n')) {
                            val tail = newText.substringAfterLast('\n')
                            val (nextSynced, nextField) = applyLiveDraftWindow(
                                TextFieldValue(
                                    text = tail,
                                    selection = TextRange(tail.length),
                                ),
                            )
                            syncedText = nextSynced
                            remoteField = nextField
                        } else {
                            val (nextSynced, nextField) = applyLiveDraftWindow(next)
                            syncedText = nextSynced
                            remoteField = nextField
                        }
                    } else {
                        remoteField = next
                    }
                },
                placeholder = {
                    Text(
                        if (session != null) {
                            tr("Digite em tempo real no shell...", "Type live into the shell...")
                        } else {
                            tr("Aguardando shell...", "Waiting for shell...")
                        },
                    )
                },
                singleLine = true,
                shape = RoundedCornerShape(12.dp),
                trailingIcon = if (remoteField.text.isNotEmpty()) {
                    {
                        IconButton(onClick = { clearDraft() }) {
                            Icon(
                                imageVector = Icons.Default.Close,
                                contentDescription = "Limpar campo local",
                                tint = OraculoColors.OnSurfaceMuted,
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                } else {
                    null
                },
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    autoCorrectEnabled = false,
                    imeAction = ImeAction.Send,
                ),
                keyboardActions = KeyboardActions(onSend = { sendEnterAndClear() }),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = OraculoColors.Primary,
                    unfocusedBorderColor = OraculoColors.Outline,
                    focusedContainerColor = OraculoColors.Surface,
                    unfocusedContainerColor = OraculoColors.Surface,
                    focusedTextColor = OraculoColors.OnSurface,
                    unfocusedTextColor = OraculoColors.OnSurface,
                ),
                modifier = Modifier
                    .weight(1f)
                    .focusRequester(inputFocusRequester)
                    .onPreviewKeyEvent { event ->
                        if (event.type == KeyEventType.KeyDown &&
                            event.key == Key.Backspace &&
                            syncedText.isEmpty()
                        ) {
                            sendBytes(byteArrayOf(0x7f))
                            true
                        } else {
                            false
                        }
                    },
            )
            Button(
                onClick = { sendEnterAndClear() },
                enabled = session != null,
                colors = ButtonDefaults.buttonColors(
                    containerColor = OraculoColors.Primary,
                    contentColor = OraculoColors.OnPrimary,
                ),
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                contentPadding = PaddingValues(horizontal = 16.dp),
                modifier = Modifier
                    .height(OraculoMetrics.ControlHeightInput)
                    .widthIn(min = OraculoMetrics.ControlHeightInput),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.KeyboardReturn,
                    contentDescription = "Enviar Enter",
                    modifier = Modifier.size(20.dp),
                )
            }
        }
    }
}

@Composable
private fun PtyxisTerminalHeader(
    title: String,
    cols: Int,
    rows: Int,
    ptyxis: PtyxisPalette,
    fontSizeSp: Float,
    copiedRecently: Boolean,
    canSendClear: Boolean,
    onDecreaseFont: () -> Unit,
    onIncreaseFont: () -> Unit,
    onCopy: () -> Unit,
    onClearTerminal: () -> Unit,
    onReconnect: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(ptyxis.HeaderBackground)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.Medium,
            color = ptyxis.Foreground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Text(
            text = "${cols}×${rows}",
            style = MaterialTheme.typography.labelSmall,
            fontFamily = FontFamily.Monospace,
            color = ptyxis.ForegroundMuted,
            modifier = Modifier
                .clip(RoundedCornerShape(6.dp))
                .background(ptyxis.Background.copy(alpha = 0.6f))
                .padding(horizontal = 6.dp, vertical = 2.dp),
        )
        IconButton(
            onClick = onDecreaseFont,
            enabled = fontSizeSp > 10f,
            modifier = Modifier.size(28.dp),
        ) {
            Icon(
                imageVector = Icons.Default.ZoomOut,
                contentDescription = "Diminuir fonte",
                tint = ptyxis.ForegroundMuted,
                modifier = Modifier.size(16.dp),
            )
        }
        IconButton(
            onClick = onIncreaseFont,
            enabled = fontSizeSp < 18f,
            modifier = Modifier.size(28.dp),
        ) {
            Icon(
                imageVector = Icons.Default.ZoomIn,
                contentDescription = "Aumentar fonte",
                tint = ptyxis.ForegroundMuted,
                modifier = Modifier.size(16.dp),
            )
        }
        IconButton(
            onClick = onCopy,
            modifier = Modifier.size(28.dp),
        ) {
            Icon(
                imageVector = if (copiedRecently) Icons.Default.Check else Icons.Default.ContentCopy,
                contentDescription = "Copiar terminal",
                tint = if (copiedRecently) OraculoColors.Primary else ptyxis.ForegroundMuted,
                modifier = Modifier.size(15.dp),
            )
        }
        IconButton(
            onClick = onClearTerminal,
            enabled = canSendClear,
            modifier = Modifier.size(28.dp),
        ) {
            Icon(
                imageVector = Icons.Default.ClearAll,
                contentDescription = "Limpar terminal",
                tint = ptyxis.ForegroundMuted,
                modifier = Modifier.size(16.dp),
            )
        }
        IconButton(
            onClick = onReconnect,
            modifier = Modifier.size(28.dp),
        ) {
            Icon(
                imageVector = Icons.Default.Refresh,
                contentDescription = "Reconectar terminal",
                tint = ptyxis.ForegroundMuted,
                modifier = Modifier.size(16.dp),
            )
        }
    }
}

@Composable
private fun PtyxisViewport(
    screenSnapshot: TerminalScreenSnapshot?,
    screenRevision: Int,
    fallbackMessage: String,
    terminalStyle: TextStyle,
    ptyxis: PtyxisPalette,
    isConnecting: Boolean,
    onGridMeasured: (cols: Int, rows: Int) -> Unit,
    onTapViewport: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val density = LocalDensity.current
    val textMeasurer = rememberTextMeasurer()
    val cellMetrics = remember(textMeasurer, terminalStyle) {
        val measured = textMeasurer.measure("MMMMMMMMMM", style = terminalStyle)
        val charWidth = (measured.size.width / 10f).coerceAtLeast(1f)
        val lineHeight = measured.size.height.toFloat().coerceAtLeast(1f)
        charWidth to lineHeight
    }

    var cursorBlinkOn by remember { mutableStateOf(true) }
    LaunchedEffect(screenSnapshot?.cursorVisible, isConnecting, screenRevision) {
        cursorBlinkOn = true
        if (screenSnapshot?.cursorVisible == true && !isConnecting) {
            while (true) {
                delay(530L)
                cursorBlinkOn = !cursorBlinkOn
            }
        }
    }

    val showCursor = (screenSnapshot?.cursorVisible == true) && !isConnecting && cursorBlinkOn
    val renderedScreen = remember(screenSnapshot, fallbackMessage, ptyxis, showCursor) {
        val snap = screenSnapshot
        if (snap != null) {
            buildPtyxisAnnotatedScreen(snap, ptyxis, showCursor)
        } else {
            AnnotatedString(fallbackMessage.trimEnd('\n', '\r', ' ').ifEmpty { " " })
        }
    }

    val verticalScroll = rememberScrollState()
    val horizontalScroll = rememberScrollState()
    LaunchedEffect(screenRevision, screenSnapshot?.cursorRow) {
        verticalScroll.scrollTo(verticalScroll.maxValue)
    }

    Box(
        modifier = modifier
            .onSizeChanged { size ->
                val padPx = with(density) { 20.dp.toPx() }
                val innerW = (size.width - padPx).coerceAtLeast(cellMetrics.first * 36)
                val innerH = (size.height - padPx).coerceAtLeast(cellMetrics.second * 10)
                val cols = (innerW / cellMetrics.first).toInt().coerceIn(36, 180)
                val rows = (innerH / cellMetrics.second).toInt().coerceIn(10, 72)
                onGridMeasured(cols, rows)
            }
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = { onTapViewport() },
                )
            }
            .padding(10.dp),
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(verticalScroll)
                .horizontalScroll(horizontalScroll),
        ) {
            SelectionContainer {
                Text(
                    text = renderedScreen,
                    style = terminalStyle,
                    color = ptyxis.Foreground,
                    softWrap = false,
                )
            }
        }
        if (isConnecting) {
            CircularProgressIndicator(
                color = OraculoColors.Primary,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .size(16.dp),
                strokeWidth = 2.dp,
            )
        }
    }
}

@Composable
private fun TerminalQuickKeysBar(
    onKey: (TerminalQuickKeySpec) -> Unit,
) {
    val shape = RoundedCornerShape(OraculoMetrics.CornerSmall)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        TERMINAL_QUICK_KEYS.forEach { keySpec ->
            val (interactionSource, pressed) = rememberPressedInteraction()
            Surface(
                color = OraculoColors.Surface,
                shape = shape,
                modifier = Modifier
                    .height(OraculoMetrics.ControlHeightSmall)
                    .oraculoPressScale(pressed)
                    .clip(shape)
                    .border(1.dp, OraculoColors.Outline, shape)
                    .clickable(
                        interactionSource = interactionSource,
                        indication = LocalIndication.current,
                    ) { onKey(keySpec) },
            ) {
                if (keySpec.icon != null) {
                    Box(
                        contentAlignment = Alignment.Center,
                        modifier = Modifier.padding(horizontal = 11.dp),
                    ) {
                        Icon(
                            imageVector = keySpec.icon,
                            contentDescription = keySpec.label,
                            tint = OraculoColors.OnSurface,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                } else {
                    Box(
                        contentAlignment = Alignment.Center,
                        modifier = Modifier.padding(horizontal = 11.dp),
                    ) {
                        Text(
                            text = keySpec.label,
                            style = MaterialTheme.typography.bodySmall,
                            fontFamily = FontFamily.Monospace,
                            color = OraculoColors.OnSurface,
                        )
                    }
                }
            }
        }
    }
}

private fun resolvePtyxisColor(
    kind: TerminalColorKind,
    value: UInt,
    palette: PtyxisPalette,
): Color? = when (kind) {
    TerminalColorKind.DEFAULT -> null
    TerminalColorKind.INDEXED -> {
        val idx = value.toInt()
        when (idx) {
            in 0..15 -> palette.Ansi16[idx]
            in 16..231 -> {
                val n = idx - 16
                val rIdx = n / 36
                val gIdx = (n % 36) / 6
                val bIdx = n % 6
                Color(
                    red = if (rIdx == 0) 0 else 55 + rIdx * 40,
                    green = if (gIdx == 0) 0 else 55 + gIdx * 40,
                    blue = if (bIdx == 0) 0 else 55 + bIdx * 40,
                )
            }
            in 232..255 -> {
                val gray = 8 + (idx - 232) * 10
                Color(gray, gray, gray)
            }
            else -> null
        }
    }
    TerminalColorKind.RGB -> {
        val rgb = value.toInt()
        Color(
            red = (rgb shr 16) and 0xFF,
            green = (rgb shr 8) and 0xFF,
            blue = rgb and 0xFF,
        )
    }
}

private fun terminalSpanStyle(span: TerminalSpan, palette: PtyxisPalette): SpanStyle {
    var fg = resolvePtyxisColor(span.fgKind, span.fgVal, palette)
    var bg = resolvePtyxisColor(span.bgKind, span.bgVal, palette)
    if (span.inverse) {
        val prevFg = fg ?: palette.Foreground
        val prevBg = bg ?: palette.Background
        fg = prevBg
        bg = prevFg
    }
    if (span.dim) {
        fg = (fg ?: palette.Foreground).copy(alpha = 0.65f)
    }
    return SpanStyle(
        color = fg ?: Color.Unspecified,
        background = bg ?: Color.Unspecified,
        fontWeight = if (span.bold) FontWeight.Bold else null,
        fontStyle = if (span.italic) FontStyle.Italic else null,
        textDecoration = if (span.underline) TextDecoration.Underline else null,
    )
}

private fun buildPtyxisAnnotatedScreen(
    snapshot: TerminalScreenSnapshot,
    palette: PtyxisPalette,
    showCursor: Boolean,
): AnnotatedString = buildAnnotatedString {
    val lines = snapshot.lines
    if (lines.isEmpty()) {
        if (showCursor) {
            pushStyle(SpanStyle(color = palette.CursorText, background = palette.Cursor))
            append(" ")
            pop()
        } else {
            append(" ")
        }
        return@buildAnnotatedString
    }

    val cursorRow = snapshot.cursorRow.toInt()
    val cursorCol = snapshot.cursorCol.toInt().coerceAtLeast(0)
    val cursorStyle = SpanStyle(color = palette.CursorText, background = palette.Cursor)

    lines.forEachIndexed { rowIdx, line ->
        if (rowIdx > 0) {
            append("\n")
        }
        val activeCursorHere = showCursor && rowIdx == cursorRow
        var col = 0
        var cursorDrawn = false

        line.spans.forEach { span ->
            val style = terminalSpanStyle(span, palette)
            val text = span.text
            val len = text.length
            if (activeCursorHere && !cursorDrawn && cursorCol >= col && cursorCol < col + len) {
                val splitIdx = cursorCol - col
                if (splitIdx > 0) {
                    pushStyle(style)
                    append(text.substring(0, splitIdx))
                    pop()
                }
                pushStyle(cursorStyle)
                append(text[splitIdx])
                pop()
                if (splitIdx + 1 < len) {
                    pushStyle(style)
                    append(text.substring(splitIdx + 1))
                    pop()
                }
                cursorDrawn = true
            } else {
                pushStyle(style)
                append(text)
                pop()
            }
            col += len
        }

        if (activeCursorHere && !cursorDrawn) {
            if (cursorCol > col) {
                append(" ".repeat(cursorCol - col))
            }
            pushStyle(cursorStyle)
            append(" ")
            pop()
        }
    }
}

@Composable
private fun CommandsTab(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    modifier: Modifier = Modifier,
) {
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    var commands by remember { mutableStateOf<List<QuickCommandItem>>(emptyList()) }
    var isLoading by remember { mutableStateOf(true) }
    var runningCommandId by remember { mutableStateOf<String?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var lastCommandLabel by remember { mutableStateOf<String?>(null) }
    var lastResult by remember { mutableStateOf<CommandRunResult?>(null) }

    var isEditorOpen by remember { mutableStateOf(false) }
    var editCommandId by remember { mutableStateOf<String?>(null) }
    var editLabel by remember { mutableStateOf("") }
    var editDescription by remember { mutableStateOf("") }
    var editCommandLine by remember { mutableStateOf("") }
    var isSavingCommand by remember { mutableStateOf(false) }
    var editorError by remember { mutableStateOf<String?>(null) }
    var pendingDeleteCommand by remember { mutableStateOf<QuickCommandItem?>(null) }
    var deletingCommandId by remember { mutableStateOf<String?>(null) }

    fun loadCommands() {
        scope.launch {
            isLoading = true
            errorMessage = null
            val res = withContext(Dispatchers.IO) {
                runCatching { client.listCommands(host.endpointId) }
            }
            res.onSuccess { list ->
                commands = list
            }.onFailure { err ->
                errorMessage = formatOraculoError(err)
            }
            isLoading = false
        }
    }

    fun openCreateEditor() {
        editCommandId = null
        editLabel = ""
        editDescription = ""
        editCommandLine = ""
        editorError = null
        isEditorOpen = true
    }

    fun openEditEditor(item: QuickCommandItem) {
        editCommandId = item.id
        editLabel = item.label
        editDescription = item.description
        editCommandLine = item.commandLine
        editorError = null
        isEditorOpen = true
    }

    fun closeEditor() {
        isEditorOpen = false
        editorError = null
    }

    fun submitEditor() {
        val label = editLabel.trim()
        val commandLine = editCommandLine.trim()
        val description = editDescription.trim()
        if (label.isEmpty()) {
            editorError = tr("Informe um nome para o comando.", "Enter a name for the command.")
            return
        }
        if (commandLine.isEmpty()) {
            editorError = tr("Informe a linha de comando a executar.", "Enter the command line to execute.")
            return
        }

        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        scope.launch {
            isSavingCommand = true
            editorError = null
            val res = withContext(Dispatchers.IO) {
                runCatching {
                    client.saveCommand(
                        endpointId = host.endpointId,
                        id = editCommandId,
                        label = label,
                        description = description,
                        commandLine = commandLine,
                    )
                }
            }
            res.onSuccess { updated ->
                commands = updated
                isEditorOpen = false
                errorMessage = null
            }.onFailure { err ->
                editorError = formatOraculoError(err)
            }
            isSavingCommand = false
        }
    }

    fun confirmDelete(item: QuickCommandItem) {
        pendingDeleteCommand = null
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        scope.launch {
            deletingCommandId = item.id
            errorMessage = null
            val res = withContext(Dispatchers.IO) {
                runCatching { client.deleteCommand(host.endpointId, item.id) }
            }
            res.onSuccess { updated ->
                commands = updated
                if (editCommandId == item.id) {
                    isEditorOpen = false
                }
            }.onFailure { err ->
                errorMessage = formatOraculoError(err)
            }
            deletingCommandId = null
        }
    }

    fun executeCommand(item: QuickCommandItem) {
        if (runningCommandId != null) return
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        scope.launch {
            runningCommandId = item.id
            errorMessage = null
            val res = withContext(Dispatchers.IO) {
                runCatching { client.runCommand(host.endpointId, item.id) }
            }
            res.onSuccess { output ->
                lastCommandLabel = item.label
                lastResult = output
            }.onFailure { err ->
                errorMessage = formatOraculoError(err)
            }
            runningCommandId = null
        }
    }

    LaunchedEffect(host.endpointId) {
        loadCommands()
    }

    if (isEditorOpen) {
        CommandEditorDialog(
            isEditing = editCommandId != null,
            label = editLabel,
            commandLine = editCommandLine,
            description = editDescription,
            error = editorError,
            isSaving = isSavingCommand,
            onLabelChange = { editLabel = it },
            onCommandLineChange = { editCommandLine = it },
            onDescriptionChange = { editDescription = it },
            onDismiss = { closeEditor() },
            onSubmit = { submitEditor() },
        )
    }

    pendingDeleteCommand?.let { target ->
        AlertDialog(
            onDismissRequest = { pendingDeleteCommand = null },
            containerColor = OraculoColors.Surface,
            title = {
                Text(
                    text = tr("Remover comando?", "Delete command?"),
                    color = OraculoColors.OnSurface,
                )
            },
            text = {
                Text(
                    text = tr(
                        "\"${target.label}\" será removido de commands.toml no computador.",
                        "\"${target.label}\" will be removed from commands.toml on the computer.",
                    ),
                    color = OraculoColors.OnSurfaceMuted,
                )
            },
            confirmButton = {
                TextButton(
                    onClick = { confirmDelete(target) },
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Remover", "Delete"), color = OraculoColors.Error)
                }
            },
            dismissButton = {
                TextButton(
                    onClick = { pendingDeleteCommand = null },
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
                }
            },
        )
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp)
            .padding(bottom = 10.dp, top = 2.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = tr("Comandos", "Commands"),
                style = MaterialTheme.typography.titleMedium,
                color = OraculoColors.OnSurface,
                modifier = Modifier.weight(1f),
            )
            val (newCmdSource, newCmdPressed) = rememberPressedInteraction()
            val cmdBtnShape = RoundedCornerShape(OraculoMetrics.CornerSmall)
            Surface(
                color = OraculoColors.Surface,
                shape = cmdBtnShape,
                modifier = Modifier
                    .height(OraculoMetrics.ControlHeightSmall)
                    .oraculoPressScale(newCmdPressed)
                    .clip(cmdBtnShape)
                    .border(
                        width = 1.dp,
                        color = OraculoColors.Outline,
                        shape = cmdBtnShape,
                    )
                    .clickable(
                        interactionSource = newCmdSource,
                        indication = LocalIndication.current,
                    ) { openCreateEditor() },
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(5.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.Add,
                        contentDescription = null,
                        tint = OraculoColors.Primary,
                        modifier = Modifier.size(16.dp),
                    )
                    Text(
                        text = tr("Novo", "New"),
                        style = MaterialTheme.typography.labelLarge,
                        fontSize = 13.sp,
                        color = OraculoColors.OnSurface,
                    )
                }
            }
            OraculoSquareIconButton(
                onClick = { loadCommands() },
                containerColor = OraculoColors.Surface,
                size = OraculoMetrics.ControlHeightSmall,
            ) {
                Icon(
                    imageVector = Icons.Default.Refresh,
                    contentDescription = "Atualizar lista",
                    tint = OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(18.dp),
                )
            }
        }

        if (errorMessage != null) {
            Surface(
                color = OraculoColors.Error.copy(alpha = 0.14f),
                shape = RoundedCornerShape(14.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, OraculoColors.Error.copy(alpha = 0.4f), RoundedCornerShape(14.dp)),
            ) {
                Text(
                    text = errorMessage.orEmpty(),
                    color = OraculoColors.Error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(12.dp),
                )
            }
        }

        if (isLoading) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(color = OraculoColors.Primary)
            }
        } else if (commands.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = tr(
                        "Nenhum comando salvo ainda.\nToque em \"+ Novo\" para criar o primeiro.",
                        "No saved commands yet.\nTap \"+ New\" to create the first one.",
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    color = OraculoColors.OnSurfaceMuted,
                    textAlign = TextAlign.Center,
                )
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                itemsIndexed(commands, key = { _, item -> item.id }) { _, item ->
                    CommandCardRow(
                        item = item,
                        isRunning = runningCommandId == item.id,
                        isDeleting = deletingCommandId == item.id,
                        canRun = runningCommandId == null,
                        onExecute = { executeCommand(item) },
                        onEdit = { openEditEditor(item) },
                        onDelete = { pendingDeleteCommand = item },
                    )
                }
            }
        }

        AnimatedVisibility(
            visible = lastResult != null,
            enter = OraculoMotion.ExpandV,
            exit = OraculoMotion.CollapseV,
        ) {
            val result = lastResult
            if (result != null) {
                CommandOutputCard(
                    commandLabel = lastCommandLabel ?: "Saída",
                    result = result,
                    onDismiss = { lastResult = null },
                )
            }
        }
    }
}

@Composable
private fun CommandEditorDialog(
    isEditing: Boolean,
    label: String,
    commandLine: String,
    description: String,
    error: String?,
    isSaving: Boolean,
    onLabelChange: (String) -> Unit,
    onCommandLineChange: (String) -> Unit,
    onDescriptionChange: (String) -> Unit,
    onDismiss: () -> Unit,
    onSubmit: () -> Unit,
) {
    val fieldColors = OutlinedTextFieldDefaults.colors(
        focusedBorderColor = OraculoColors.Primary,
        unfocusedBorderColor = OraculoColors.Outline,
        focusedContainerColor = OraculoColors.SurfaceRecessed.copy(alpha = 0.7f),
        unfocusedContainerColor = OraculoColors.SurfaceRecessed.copy(alpha = 0.45f),
        focusedTextColor = OraculoColors.OnSurface,
        unfocusedTextColor = OraculoColors.OnSurface,
        focusedLabelColor = OraculoColors.Primary,
        unfocusedLabelColor = OraculoColors.OnSurfaceMuted,
    )

    AlertDialog(
        onDismissRequest = {
            if (!isSaving) onDismiss()
        },
        containerColor = OraculoColors.Surface,
        title = {
            Text(
                text = if (isEditing) tr("Editar comando", "Edit command") else tr("Novo comando", "New command"),
                color = OraculoColors.OnSurface,
            )
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = label,
                    onValueChange = onLabelChange,
                    label = { Text(tr("Nome", "Name")) },
                    placeholder = { Text(tr("Ex.: Reiniciar Áudio", "e.g. Restart Audio")) },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = commandLine,
                    onValueChange = onCommandLineChange,
                    label = { Text(tr("Comando shell", "Shell command")) },
                    placeholder = { Text("Ex.: systemctl --user restart pipewire") },
                    singleLine = true,
                    textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp),
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = description,
                    onValueChange = onDescriptionChange,
                    label = { Text(tr("Descrição (opcional)", "Description (optional)")) },
                    placeholder = { Text(tr("Resumo curto exibido no cartão", "Short summary shown on card")) },
                    singleLine = true,
                    shape = RoundedCornerShape(12.dp),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    text = tr(
                        "Suporta argumentos com aspas; salvo em commands.toml no computador.",
                        "Supports quoted arguments; saved in commands.toml on the computer.",
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    color = OraculoColors.OnSurfaceMuted,
                )
                if (error != null) {
                    Text(
                        text = error,
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.Error,
                    )
                }
            }
        },
        confirmButton = {
            Button(
                onClick = onSubmit,
                enabled = !isSaving,
                colors = ButtonDefaults.buttonColors(
                    containerColor = OraculoColors.Primary,
                    contentColor = OraculoColors.OnPrimary,
                ),
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
            ) {
                if (isSaving) {
                    CircularProgressIndicator(
                        color = OraculoColors.OnPrimary,
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp,
                    )
                } else {
                    Text(tr("Salvar", "Save"))
                }
            }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                enabled = !isSaving,
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
            ) {
                Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
            }
        },
    )
}

@Composable
private fun CommandCardRow(
    item: QuickCommandItem,
    isRunning: Boolean,
    isDeleting: Boolean,
    canRun: Boolean,
    onExecute: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(OraculoColors.Surface)
            .border(
                width = 1.dp,
                color = if (isRunning) OraculoColors.Primary else OraculoColors.Outline,
                shape = RoundedCornerShape(14.dp),
            )
            .clickable(enabled = canRun && !isDeleting, onClick = onExecute)
            .padding(start = 14.dp, end = 10.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.label,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = OraculoColors.OnSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (item.description.isNotBlank()) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = item.description,
                    style = MaterialTheme.typography.bodySmall,
                    color = OraculoColors.OnSurfaceMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (item.commandLine.isNotBlank()) {
                Spacer(modifier = Modifier.height(3.dp))
                Text(
                    text = item.commandLine,
                    style = TextStyle(
                        fontFamily = FontFamily.Monospace,
                        fontSize = 11.sp,
                    ),
                    color = OraculoColors.OnSurfaceMuted.copy(alpha = 0.85f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(modifier = Modifier.width(6.dp))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            OraculoSquareIconButton(
                onClick = onEdit,
                enabled = !isDeleting,
                size = OraculoMetrics.ControlHeightSmall,
            ) {
                Icon(
                    imageVector = Icons.Default.Edit,
                    contentDescription = "Editar ${item.label}",
                    tint = OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(17.dp),
                )
            }
            OraculoSquareIconButton(
                onClick = onDelete,
                enabled = !isDeleting && canRun,
                size = OraculoMetrics.ControlHeightSmall,
            ) {
                Icon(
                    imageVector = Icons.Default.DeleteOutline,
                    contentDescription = "Remover ${item.label}",
                    tint = OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(17.dp),
                )
            }
            Spacer(modifier = Modifier.width(2.dp))
            Box(
                modifier = Modifier
                    .size(OraculoMetrics.ControlHeightSmall)
                    .clip(RoundedCornerShape(OraculoMetrics.CornerSmall))
                    .background(OraculoColors.SecondaryContainer),
                contentAlignment = Alignment.Center,
            ) {
                if (isRunning || isDeleting) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        color = OraculoColors.Primary,
                        strokeWidth = 2.dp,
                    )
                } else {
                    Icon(
                        imageVector = Icons.Default.PlayArrow,
                        contentDescription = "Executar ${item.label}",
                        tint = OraculoColors.Primary,
                        modifier = Modifier.size(19.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun CommandOutputCard(
    commandLabel: String,
    result: CommandRunResult,
    onDismiss: () -> Unit,
) {
    val combined = remember(result) {
        buildString {
            if (result.stdout.isNotBlank()) append(result.stdout.trimEnd())
            if (result.stderr.isNotBlank()) {
                if (isNotEmpty()) append("\n--- stderr ---\n")
                append(result.stderr.trimEnd())
            }
            if (isEmpty()) append(tr("(sem saída)", "(no output)"))
        }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(OraculoColors.TerminalBackground)
            .border(1.dp, OraculoColors.TerminalBorder, RoundedCornerShape(14.dp))
            .padding(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = commandLabel,
                style = MaterialTheme.typography.labelLarge,
                color = OraculoColors.TerminalForeground,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = "exit ${result.exitCode}",
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontFeatureSettings = "tnum",
                ),
                color = if (result.exitCode == 0) OraculoColors.Success else OraculoColors.Error,
            )
            IconButton(
                onClick = onDismiss,
                modifier = Modifier.size(24.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Close,
                    contentDescription = "Fechar saída",
                    tint = OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(16.dp),
                )
            }
        }
        Spacer(modifier = Modifier.height(6.dp))
        SelectionContainer {
            Text(
                text = combined,
                fontFamily = FontFamily.Monospace,
                fontSize = 12.sp,
                lineHeight = 17.sp,
                color = OraculoColors.TerminalForeground.copy(alpha = 0.88f),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(120.dp)
                    .verticalScroll(rememberScrollState()),
            )
        }
    }
}
