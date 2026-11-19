package com.oraculo.app

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.layout.heightIn
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
import androidx.compose.material.icons.automirrored.filled.OpenInNew
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
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.PowerSettingsNew
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
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
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
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
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.ui.graphics.graphicsLayer
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoDeckButton
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.PtyxisPalette
import com.oraculo.app.ui.oraculoPrimaryButtonColors
import com.oraculo.app.ui.oraculoSegmentedButtonColors
import com.oraculo.app.ui.oraculoSegmentedItemShape
import com.oraculo.app.ui.ptyxisPaletteFor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.oraculo_mobile_core.CommandRunResult
import uniffi.oraculo_mobile_core.HostStateObserver
import uniffi.oraculo_mobile_core.InputObserver
import uniffi.oraculo_mobile_core.MobileHostPermissions
import uniffi.oraculo_mobile_core.MobileHostStateWatcher
import uniffi.oraculo_mobile_core.MobileInputSession
import uniffi.oraculo_mobile_core.MobileMouseButton
import uniffi.oraculo_mobile_core.MobileTerminalSession
import uniffi.oraculo_mobile_core.MobileTerminalSessionInfo
import uniffi.oraculo_mobile_core.MobileTunnelSession
import uniffi.oraculo_mobile_core.OraculoException
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.QuickCommandItem
import uniffi.oraculo_mobile_core.SavedHostInfo
import uniffi.oraculo_mobile_core.TerminalColorKind
import uniffi.oraculo_mobile_core.TerminalObserver
import uniffi.oraculo_mobile_core.TerminalScreenSnapshot
import uniffi.oraculo_mobile_core.TerminalSpan
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
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

internal fun isPermanentRemoteError(raw: String): Boolean {
    val low = raw.lowercase(java.util.Locale.ROOT)
    return low.contains("bloqueada") ||
        low.contains("locked") ||
        low.contains("desativad") ||
        low.contains("disabled") ||
        low.contains("cancelad") ||
        low.contains("canceled") ||
        low.contains("cancelled") ||
        low.contains("negad") ||
        low.contains("denied") ||
        low.contains("nenhum monitor") ||
        low.contains("no monitor") ||
        low.contains("no active monitor") ||
        low.contains("no display") ||
        low.contains("permiss") ||
        low.contains("permission") ||
        low.contains("revogad") ||
        low.contains("revoked") ||
        low.contains("não pareado") ||
        low.contains("nao pareado") ||
        low.contains("not paired") ||
        low.contains("unpaired") ||
        low.contains("acesso bloqueado") ||
        low.contains("access blocked") ||
        low.contains("[401]") ||
        low.contains("[403]") ||
        low.contains("[410]") ||
        low.contains("[503]") ||
        low.contains("decoder") ||
        low.contains("mediacodec") ||
        low.contains("não encontrada") ||
        low.contains("nao encontrada") ||
        low.contains("not found")
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
    bridges: List<SavedHostInfo> = emptyList(),
    onBack: () -> Unit,
    onRemoteDisconnect: (String) -> Unit = { _ -> onBack() },
) {
    val context = LocalContext.current
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() }
    val scope = rememberCoroutineScope()
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    val focusManager = LocalFocusManager.current
    var selectedTab by remember { mutableStateOf(ControlTab.TOUCHPAD) }
    var visitedTabs by remember(host.endpointId) { mutableStateOf(setOf(ControlTab.TOUCHPAD)) }
    var currentHost by remember(host) { mutableStateOf(host) }
    var hostPermissions by remember(host.endpointId) { mutableStateOf<MobileHostPermissions?>(null) }
    var stateWatchNonce by remember(host.endpointId) { mutableIntStateOf(0) }
    val exitedRemotely = remember(host.endpointId) { AtomicBoolean(false) }
    var consecutivePingFailures by remember(host.endpointId) { mutableIntStateOf(0) }
    var isReconnecting by remember(host.endpointId) { mutableStateOf(false) }
    var reconnectAttempt by remember(host.endpointId) { mutableIntStateOf(0) }
    var connectionRestoredNonce by remember(host.endpointId) { mutableIntStateOf(0) }
    var showWolDialog by remember(host.endpointId) { mutableStateOf(false) }
    var showRdpTunnelDialog by remember(host.endpointId) { mutableStateOf(false) }
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
        host = currentHost,
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

    fun dispatchRemoteDisconnect(message: String) {
        if (exitedRemotely.compareAndSet(false, true)) {
            onRemoteDisconnect(message)
        }
    }

    DisposableEffect(host.endpointId, isAppStarted, stateWatchNonce) {
        if (!isAppStarted || exitedRemotely.get()) {
            return@DisposableEffect onDispose {}
        }
        val active = AtomicBoolean(true)
        val watcherRef = AtomicReference<MobileHostStateWatcher?>(null)
        var reconnectJob: kotlinx.coroutines.Job? = null

        val observer = object : HostStateObserver {
            override fun onState(hostName: String, permissions: MobileHostPermissions) {
                mainHandler.post {
                    if (!active.get() || exitedRemotely.get()) return@post
                    val wasDisconnected = isConnected == false || isReconnecting
                    hostPermissions = permissions
                    isConnected = true
                    isReconnecting = false
                    reconnectAttempt = 0
                    consecutivePingFailures = 0
                    if (hostName.isNotBlank() && hostName != currentHost.hostName) {
                        currentHost = currentHost.copy(hostName = hostName)
                    }
                    if (wasDisconnected) {
                        connectionRestoredNonce += 1
                    }
                }
            }

            override fun onDisconnected(code: UInt, reason: String) {
                mainHandler.post {
                    if (!active.get() || exitedRemotely.get()) return@post
                    when (code) {
                        401u, 403u -> dispatchRemoteDisconnect(
                            tr(
                                "Acesso removido no computador. Pareie novamente.",
                                "Access removed on computer. Please pair again.",
                            ),
                        )
                        410u -> dispatchRemoteDisconnect(
                            tr(
                                "Sessão desconectada no computador.",
                                "Session disconnected on computer.",
                            ),
                        )
                        503u -> dispatchRemoteDisconnect(
                            tr(
                                "O Oráculo foi encerrado no computador.",
                                "Oráculo was closed on computer.",
                            ),
                        )
                        else -> {
                            isConnected = false
                            latencyMs = null
                            isReconnecting = true
                            if (reconnectJob?.isActive == true) return@post
                            val backoffMs = listOf(900L, 2000L, 3500L, 5000L, 7000L)[
                                reconnectAttempt.coerceAtMost(4)
                            ]
                            reconnectJob = scope.launch {
                                delay(backoffMs)
                                if (!active.get() || exitedRemotely.get()) return@launch
                                val probe = withContext(Dispatchers.IO) {
                                    runCatching { client.connectAndVerify(host.endpointId, 9000uL) }
                                }
                                if (!active.get() || exitedRemotely.get()) return@launch
                                probe.onSuccess {
                                    reconnectAttempt = 0
                                    stateWatchNonce += 1
                                }.onFailure { err ->
                                    val raw = formatOraculoError(err)
                                    when {
                                        raw.startsWith("[401]") || raw.startsWith("[403]") ->
                                            dispatchRemoteDisconnect(
                                                tr(
                                                    "Acesso removido no computador. Pareie novamente.",
                                                    "Access removed on computer. Please pair again.",
                                                ),
                                            )
                                        raw.startsWith("[503]") ->
                                            dispatchRemoteDisconnect(
                                                tr(
                                                    "O Oráculo foi encerrado no computador.",
                                                    "Oráculo was closed on computer.",
                                                ),
                                            )
                                        else -> {
                                            reconnectAttempt += 1
                                            stateWatchNonce += 1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        val job = CoroutineScope(Dispatchers.IO).launch {
            val res = runCatching { client.watchHostState(host.endpointId, observer) }
            if (!active.get() || exitedRemotely.get()) {
                res.onSuccess { w ->
                    runCatching {
                        w.closeWatcher()
                        w.close()
                    }
                }
                return@launch
            }
            res.onSuccess { watcher ->
                watcherRef.set(watcher)
            }
            var retryBackoffMs: Long? = null
            withContext(Dispatchers.Main + NonCancellable) {
                if (!active.get() || exitedRemotely.get()) {
                    val toClose = watcherRef.getAndSet(null)
                    if (toClose != null) {
                        CoroutineScope(Dispatchers.IO).launch {
                            runCatching {
                                toClose.closeWatcher()
                                toClose.close()
                            }
                        }
                    }
                    return@withContext
                }
                res.onFailure { err ->
                    val raw = formatOraculoError(err)
                    when {
                        raw.startsWith("[401]") || raw.startsWith("[403]") -> {
                            dispatchRemoteDisconnect(
                                tr(
                                    "Acesso removido no computador. Pareie novamente.",
                                    "Access removed on computer. Please pair again.",
                                ),
                            )
                        }
                        raw.startsWith("[503]") -> {
                            dispatchRemoteDisconnect(
                                tr(
                                    "O Oráculo foi encerrado no computador.",
                                    "Oráculo was closed on computer.",
                                ),
                            )
                        }
                        else -> {
                            isConnected = false
                            latencyMs = null
                            isReconnecting = true
                            retryBackoffMs = listOf(1200L, 2500L, 4000L, 6000L, 8000L)[
                                reconnectAttempt.coerceAtMost(4)
                            ]
                        }
                    }
                }
            }
            val backoffMs = retryBackoffMs
            if (backoffMs != null && active.get() && !exitedRemotely.get()) {
                delay(backoffMs)
                withContext(Dispatchers.Main) {
                    if (active.get() && !exitedRemotely.get()) {
                        reconnectAttempt += 1
                        stateWatchNonce += 1
                    }
                }
            }
        }

        onDispose {
            active.set(false)
            job.cancel()
            reconnectJob?.cancel()
            reconnectJob = null
            val toClose = watcherRef.getAndSet(null)
            if (toClose != null) {
                CoroutineScope(Dispatchers.IO).launch {
                    runCatching {
                        toClose.closeWatcher()
                        toClose.close()
                    }
                }
            }
        }
    }

    suspend fun performPing(showSpinner: Boolean) {
        val gen = ++pingGeneration
        if (showSpinner || isConnected != true) {
            isPinging = true
        }
        try {
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val ping = client.ping(host.endpointId)
                    val synced = runCatching {
                        client.listSavedHosts().firstOrNull { it.endpointId == host.endpointId }
                    }.getOrNull()
                    ping to synced
                }
            }
            if (gen == pingGeneration) {
                result.onSuccess { (ping, synced) ->
                    val wasDisconnected = isConnected == false || isReconnecting
                    isConnected = true
                    isReconnecting = false
                    reconnectAttempt = 0
                    consecutivePingFailures = 0
                    latencyMs = ping.rttMs
                    if (synced != null) {
                        currentHost = synced
                    }
                    if (wasDisconnected) {
                        connectionRestoredNonce += 1
                        stateWatchNonce += 1
                    }
                }.onFailure { err ->
                    isConnected = false
                    latencyMs = null
                    consecutivePingFailures += 1
                    val raw = formatOraculoError(err)
                    when {
                        raw.startsWith("[401]") || raw.startsWith("[403]") -> {
                            dispatchRemoteDisconnect(
                                tr(
                                    "Acesso removido no computador. Pareie novamente.",
                                    "Access removed on computer. Please pair again.",
                                ),
                            )
                        }
                        raw.startsWith("[503]") -> {
                            dispatchRemoteDisconnect(
                                tr(
                                    "O Oráculo foi encerrado no computador.",
                                    "Oráculo was closed on computer.",
                                ),
                            )
                        }
                        else -> {
                            isReconnecting = true
                        }
                    }
                }
            }
        } finally {
            if (gen == pingGeneration) {
                isPinging = false
            }
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

    LaunchedEffect(host.endpointId, isAppStarted) {
        var skipFirst = true
        OraculoSessionCoordinator.networkChangeNonce.collect {
            if (skipFirst) {
                skipFirst = false
                return@collect
            }
            consecutivePingFailures = 0
            reconnectAttempt = 0
            delay(350L)
            if (isAppStarted) {
                if (isConnected != true || isReconnecting) {
                    stateWatchNonce += 1
                }
                if (!isStreamingMode) {
                    performPing(showSpinner = isConnected != true)
                }
            }
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
                .padding(horizontal = 4.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            IconButton(onClick = onBack) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = tr("Voltar", "Back"),
                    tint = OraculoColors.OnSurface,
                )
            }

            val statusSubtitle = when {
                isReconnecting -> tr("Reconectando...", "Reconnecting...")
                isPinging && (isConnected != true || latencyMs == null) -> tr("Testando conexão...", "Checking connection...")
                isConnected == true && latencyMs == 0uL -> "<1 ms · Online"
                isConnected == true && latencyMs != null -> "${latencyMs} ms · Online"
                isConnected == false -> tr("Offline · toque para testar", "Offline · tap to retry")
                else -> "..."
            }
            Column(
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(OraculoMetrics.CornerSmall))
                    .clickable { refreshPing(manual = true) }
                    .padding(horizontal = 6.dp, vertical = 4.dp),
            ) {
                Text(
                    text = currentHost.hostName,
                    style = MaterialTheme.typography.titleLarge,
                    fontSize = 19.sp,
                    color = OraculoColors.OnSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = statusSubtitle,
                    style = TextStyle(
                        fontFamily = FontFamily.Monospace,
                        fontSize = 11.5.sp,
                        fontFeatureSettings = "tnum",
                    ),
                    color = if (isConnected == false) OraculoColors.Error else OraculoColors.OnSurfaceMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            AnimatedVisibility(
                visible = hostPermissions?.allowRdpTunnel != false,
                enter = OraculoMotion.ExpandH,
                exit = OraculoMotion.CollapseH,
            ) {
                IconButton(onClick = { showRdpTunnelDialog = true }) {
                    Icon(
                        imageVector = Icons.Default.Lock,
                        contentDescription = tr("Login remoto (RDP)", "Remote login (RDP)"),
                        tint = OraculoColors.OnSurfaceMuted,
                    )
                }
            }

            AnimatedVisibility(
                visible = isConnected == false,
                enter = OraculoMotion.ExpandH,
                exit = OraculoMotion.CollapseH,
            ) {
                FilledTonalIconButton(
                    onClick = { showWolDialog = true },
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = OraculoColors.SecondaryContainer,
                        contentColor = OraculoColors.OnSecondaryContainer,
                    ),
                ) {
                    Icon(
                        imageVector = Icons.Default.PowerSettingsNew,
                        contentDescription = tr("Ligar computador", "Wake computer"),
                    )
                }
            }

            AnimatedVisibility(
                visible = selectedTab == ControlTab.TOUCHPAD,
                enter = OraculoMotion.ExpandH,
                exit = OraculoMotion.CollapseH,
            ) {
                if (showTouchpadSettings) {
                    FilledTonalIconButton(
                        onClick = { showTouchpadSettings = false },
                        colors = IconButtonDefaults.filledTonalIconButtonColors(
                            containerColor = OraculoColors.SecondaryContainer,
                            contentColor = OraculoColors.OnSecondaryContainer,
                        ),
                    ) {
                        Icon(
                            imageVector = Icons.Default.Tune,
                            contentDescription = tr("Ajustes do touchpad", "Touchpad settings"),
                        )
                    }
                } else {
                    IconButton(onClick = { showTouchpadSettings = true }) {
                        Icon(
                            imageVector = Icons.Default.Tune,
                            contentDescription = tr("Ajustes do touchpad", "Touchpad settings"),
                            tint = OraculoColors.OnSurfaceMuted,
                        )
                    }
                }
            }
        }

        ControlTabRow(
            selected = selectedTab,
            onSelect = { selectedTab = it },
            modifier = Modifier.fillMaxWidth(),
        )

        AnimatedVisibility(
            visible = isReconnecting && !isStreamingMode,
            enter = OraculoMotion.ExpandV,
            exit = OraculoMotion.CollapseV,
        ) {
            Surface(
                color = OraculoColors.SurfaceRecessed,
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 6.dp),
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(start = 12.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(14.dp),
                        strokeWidth = 2.dp,
                        color = OraculoColors.Primary,
                    )
                    Text(
                        text = tr(
                            "Conexão instável · reconectando...",
                            "Connection unstable · reconnecting...",
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurface,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(
                        onClick = {
                            reconnectAttempt = 0
                            consecutivePingFailures = 0
                            stateWatchNonce += 1
                            refreshPing(manual = true)
                        },
                    ) {
                        Text(
                            text = tr("Tentar agora", "Retry now"),
                            style = MaterialTheme.typography.labelMedium,
                            color = OraculoColors.Primary,
                        )
                    }
                }
            }
        }

        Spacer(modifier = Modifier.height(6.dp))

        KeepAliveControlTabs(
            selectedTab = selectedTab,
            visitedTabs = visitedTabs,
            modifier = Modifier.weight(1f),
        ) { tab ->
            when (tab) {
                ControlTab.TOUCHPAD -> TouchpadTab(
                    client = client,
                    host = currentHost,
                    isActive = selectedTab == ControlTab.TOUCHPAD && !isStreamingMode,
                    showKeyboardBar = showRemoteKeyboard,
                    showSettingsBar = showTouchpadSettings,
                    onToggleKeyboardBar = { showRemoteKeyboard = !showRemoteKeyboard },
                    onOpenScreenStream = { isStreamingMode = true },
                    isPaused = isStreamingMode,
                    allowInput = hostPermissions?.allowInput,
                    connectionRestoredNonce = connectionRestoredNonce,
                    modifier = Modifier.fillMaxSize(),
                )
                ControlTab.TERMINAL -> TerminalTab(
                    client = client,
                    host = currentHost,
                    isActive = selectedTab == ControlTab.TERMINAL && !isStreamingMode,
                    allowTerminal = hostPermissions?.allowTerminal,
                    connectionRestoredNonce = connectionRestoredNonce,
                    modifier = Modifier.fillMaxSize(),
                )
                ControlTab.COMMANDS -> CommandsTab(
                    client = client,
                    host = currentHost,
                    allowCommandsRun = hostPermissions?.allowCommandsRun,
                    allowCommandsEdit = hostPermissions?.allowCommandsEdit,
                    modifier = Modifier.fillMaxSize(),
                )
                ControlTab.FILES -> FilesTab(
                    client = client,
                    host = currentHost,
                    isActive = selectedTab == ControlTab.FILES && !isStreamingMode,
                    allowFilesRead = hostPermissions?.allowFilesRead,
                    allowFilesWrite = hostPermissions?.allowFilesWrite,
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
                host = currentHost,
                allowScreen = hostPermissions?.allowScreen,
                allowInput = hostPermissions?.allowInput,
                allowRdpTunnel = hostPermissions?.allowRdpTunnel ?: true,
                connectionRestoredNonce = connectionRestoredNonce,
                onOpenRdpTunnel = { showRdpTunnelDialog = true },
                onExit = { isStreamingMode = false },
            )
        }

        if (showRdpTunnelDialog) {
            RdpTunnelDialog(
                client = client,
                host = currentHost,
                allowRdpTunnel = hostPermissions?.allowRdpTunnel ?: true,
                onDismiss = { showRdpTunnelDialog = false },
            )
        }

        if (showWolDialog) {
            WakeOnLanDialog(
                client = client,
                host = currentHost,
                bridges = bridges,
                onHostUpdated = { updated -> currentHost = updated },
                onHostOnline = { online ->
                    currentHost = online
                    showWolDialog = false
                    refreshPing(manual = true)
                },
                onDismiss = { showWolDialog = false },
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ControlTabRow(
    selected: ControlTab,
    onSelect: (ControlTab) -> Unit,
    modifier: Modifier = Modifier,
) {
    PrimaryTabRow(
        selectedTabIndex = selected.ordinal,
        containerColor = OraculoColors.Background,
        contentColor = OraculoColors.Primary,
        divider = { HorizontalDivider(thickness = 1.dp, color = OraculoColors.Outline) },
        modifier = modifier,
    ) {
        ControlTab.entries.forEach { tab ->
            val isSelected = tab == selected
            Tab(
                selected = isSelected,
                onClick = { onSelect(tab) },
                selectedContentColor = OraculoColors.Primary,
                unselectedContentColor = OraculoColors.OnSurfaceMuted,
                modifier = Modifier.height(54.dp),
            ) {
                Column(
                    modifier = Modifier.padding(horizontal = 4.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    Icon(
                        imageVector = tab.icon,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                    Text(
                        text = tab.label,
                        style = MaterialTheme.typography.labelMedium,
                        fontSize = 11.sp,
                        fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
                        maxLines = 1,
                        softWrap = false,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }

}

@Composable
private fun TouchpadTab(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    isActive: Boolean = true,
    showKeyboardBar: Boolean,
    showSettingsBar: Boolean,
    onToggleKeyboardBar: () -> Unit,
    onOpenScreenStream: () -> Unit,
    isPaused: Boolean = false,
    allowInput: Boolean? = null,
    connectionRestoredNonce: Int = 0,
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
            if (autoRetryAttempts >= 5) {
                inputSession = null
                isConnecting = false
                errorMsg = tr("Touchpad desconectado. Toque para reconectar.", "Touchpad disconnected. Tap to reconnect.")
                keyboardController.releaseAllModifiers(null)
                return
            }
            val backoffSchedule = listOf(350L, 900L, 1800L, 3000L, 4500L)
            pendingReconnectDelayMs = backoffSchedule[autoRetryAttempts.coerceAtMost(backoffSchedule.lastIndex)]
            autoRetryAttempts += 1
        }
        inputSession = null
        errorMsg = null
        connectHint = hint
        keyboardController.releaseAllModifiers(null)
        reconnectNonce += 1
    }
    triggerReconnectRef = ::triggerReconnect

    LaunchedEffect(connectionRestoredNonce, isPaused) {
        if (connectionRestoredNonce == 0 || isPaused || allowInput == false) return@LaunchedEffect
        val current = inputSession
        val currentError = errorMsg
        val blockedByPolicy = currentError != null && isPermanentRemoteError(currentError)
        if (!blockedByPolicy && ((current == null && !isConnecting) || (current != null && !current.isAlive()))) {
            triggerReconnect(
                resetAttempts = true,
                hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
            )
        }
    }

    fun sendOnSession(target: MobileInputSession?, action: (MobileInputSession) -> Unit): Boolean =
        keyboardController.sendOnSession(target, action)

    LaunchedEffect(isActive, isPaused) {
        if (!isActive || isPaused) {
            keyboardController.releaseAllModifiers(inputSession)
        }
    }

    val currentIsPaused by rememberUpdatedState(isPaused)
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() }
    DisposableEffect(lifecycleOwner, host.endpointId) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val lifecycleObserver = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME && !currentIsPaused) {
                val current = inputSession
                if ((current == null && !isConnecting) || (current != null && !current.isAlive())) {
                    triggerReconnect(resetAttempts = true, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
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

    val currentAllowInput by rememberUpdatedState(allowInput)

    LaunchedEffect(allowInput, isPaused) {
        if (allowInput == false) {
            val prev = inputSession
            keyboardController.releaseAllModifiers(prev)
            inputSession = null
            isConnecting = false
            autoRetryAttempts = 0
            pendingReconnectDelayMs = 0L
            errorMsg = tr("Mouse e teclado desativados no computador.", "Mouse and keyboard disabled on computer.")
            if (prev != null) {
                withContext(Dispatchers.IO) {
                    runCatching {
                        prev.closeSession()
                        prev.close()
                    }
                }
            }
        } else if (allowInput == true && !isPaused) {
            val current = inputSession
            if ((current == null && !isConnecting) || (current != null && !current.isAlive())) {
                triggerReconnect(
                    resetAttempts = true,
                    hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."),
                )
            }
        }
    }

    DisposableEffect(host.endpointId, reconnectNonce, isPaused) {
        if (isPaused) {
            resumeAfterPause = true
            isConnecting = false
            errorMsg = null
            return@DisposableEffect onDispose {}
        }
        if (currentAllowInput == false) {
            isConnecting = false
            errorMsg = tr("Mouse e teclado desativados no computador.", "Mouse and keyboard disabled on computer.")
            return@DisposableEffect onDispose {}
        }
        val active = AtomicBoolean(true)
        val sessionRef = AtomicReference<MobileInputSession?>(null)
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
            if (active.get() && isConnecting) {
                connectHint = tr(
                    "Se aparecer um pedido no computador, clique em Compartilhar.",
                    "If prompted on your computer, click Share.",
                )
            }
        }

        val inputObserver = object : InputObserver {
            override fun onSessionClosed(reason: String) {
                mainHandler.post {
                    if (!active.get()) return@post
                    val tracked = sessionRef.get()
                    if (tracked != null && inputSession === tracked) {
                        sessionRef.compareAndSet(tracked, null)
                        if (currentAllowInput == false) {
                            inputSession = null
                            isConnecting = false
                            autoRetryAttempts = 0
                            errorMsg = tr("Mouse e teclado desativados no computador.", "Mouse and keyboard disabled on computer.")
                        } else {
                            triggerReconnect(resetAttempts = false, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
                        }
                    }
                }
            }
        }

        val openJob = CoroutineScope(Dispatchers.IO).launch {
            if (waitBeforeConnectMs > 0L) {
                delay(waitBeforeConnectMs)
            }
            if (!active.get()) return@launch
            val result = runCatching { client.openInput(host.endpointId, inputObserver) }
            if (!active.get()) {
                result.onSuccess { s ->
                    runCatching {
                        s.closeSession()
                        s.close()
                    }
                }
                return@launch
            }
            result.onSuccess { created ->
                sessionRef.set(created)
            }
            withContext(Dispatchers.Main + NonCancellable) {
                if (!active.get()) {
                    val toClose = sessionRef.getAndSet(null)
                    if (toClose != null) {
                        CoroutineScope(Dispatchers.IO).launch {
                            runCatching {
                                toClose.closeSession()
                                toClose.close()
                            }
                        }
                    }
                    return@withContext
                }
                hintJob.cancel()
                result.onSuccess { created ->
                    isConnecting = false
                    pendingReconnectDelayMs = 0L
                    inputSession = created
                }.onFailure { err ->
                    val msg = formatOraculoError(err)
                    if (!isPermanentRemoteError(msg) && autoRetryAttempts < 5) {
                        triggerReconnect(resetAttempts = false, hint = tr("Reconectando touchpad...", "Reconnecting touchpad..."))
                    } else {
                        isConnecting = false
                        errorMsg = msg
                    }
                }
            }
        }

        onDispose {
            active.set(false)
            hintJob.cancel()
            openJob.cancel()
            val toClose = sessionRef.getAndSet(null) ?: inputSession
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
        val speeds = remember { listOf(1.0f to "1.0x", 1.5f to "1.5x", 2.2f to "2.2x", 3.0f to "3.0x") }
        Surface(
            color = OraculoColors.Surface,
            shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
            border = BorderStroke(1.dp, OraculoColors.Outline),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = tr("Velocidade do cursor", "Pointer speed"),
                        style = MaterialTheme.typography.labelMedium,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    FilterChip(
                        selected = naturalScroll,
                        onClick = onToggleNaturalScroll,
                        leadingIcon = if (naturalScroll) {
                            {
                                Icon(
                                    imageVector = Icons.Default.Check,
                                    contentDescription = null,
                                    modifier = Modifier.size(FilterChipDefaults.IconSize),
                                )
                            }
                        } else {
                            null
                        },
                        label = {
                            Text(
                                text = tr("Scroll natural", "Natural scroll"),
                                style = MaterialTheme.typography.labelMedium,
                            )
                        },
                        colors = FilterChipDefaults.filterChipColors(
                            containerColor = OraculoColors.SurfaceRecessed,
                            labelColor = OraculoColors.OnSurfaceMuted,
                            selectedContainerColor = OraculoColors.SecondaryContainer,
                            selectedLabelColor = OraculoColors.OnSecondaryContainer,
                            selectedLeadingIconColor = OraculoColors.OnSecondaryContainer,
                        ),
                        border = FilterChipDefaults.filterChipBorder(
                            enabled = true,
                            selected = naturalScroll,
                            borderColor = OraculoColors.Outline,
                            selectedBorderColor = OraculoColors.SecondaryContainer,
                        ),
                        modifier = Modifier.height(OraculoMetrics.ControlHeightCompactChip),
                    )
                }

                SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                    speeds.forEachIndexed { index, (speed, label) ->
                        val selected = abs(pointerSpeed - speed) < 0.05f
                        SegmentedButton(
                            selected = selected,
                            onClick = { onSelectSpeed(speed) },
                            shape = SegmentedButtonDefaults.itemShape(index = index, count = speeds.size),
                            colors = oraculoSegmentedButtonColors(),
                            icon = {},
                            label = {
                                Text(
                                    text = label,
                                    style = MaterialTheme.typography.labelSmall,
                                    fontFamily = FontFamily.Monospace,
                                    maxLines = 1,
                                )
                            },
                        )
                    }
                }
            }
        }
    }

}

/**
 * Filtro 1-Euro (low-pass adaptativo por velocidade) com acumulador de subpixel para atenuar
 * tremor natural da mão e ruído capacitivo em movimentos lentos sem atrasar arrastes rápidos.
 */
internal class TouchpadNoiseFilter(
    density: Float,
    private val minCutoffHz: Double = 1.6,
    private val beta: Double = 0.032,
    private val dCutoffHz: Double = 1.0,
    deadbandDp: Double = 0.22,
) {
    private val pxPerDp = density.coerceAtLeast(1f).toDouble()
    private val deadbandPx = deadbandDp * pxPerDp
    private var initialized = false
    private var rawX = 0.0
    private var rawY = 0.0
    private var filtX = 0.0
    private var filtY = 0.0
    private var prevRawX = 0.0
    private var prevRawY = 0.0
    private var edx = 0.0
    private var edy = 0.0
    private var emittedX = 0.0
    private var emittedY = 0.0
    private var lastUptimeMs = 0L

    data class FilteredDelta(
        val dx: Double,
        val dy: Double,
        val speedDpPerSec: Double,
    )

    fun reset(uptimeMs: Long) {
        initialized = true
        rawX = 0.0
        rawY = 0.0
        filtX = 0.0
        filtY = 0.0
        prevRawX = 0.0
        prevRawY = 0.0
        edx = 0.0
        edy = 0.0
        emittedX = 0.0
        emittedY = 0.0
        lastUptimeMs = uptimeMs
    }

    fun step(deltaPxX: Float, deltaPxY: Float, uptimeMs: Long): FilteredDelta {
        if (!initialized) reset(uptimeMs)
        rawX += deltaPxX
        rawY += deltaPxY

        val dtSec = ((uptimeMs - lastUptimeMs).coerceIn(1L, 100L)) / 1000.0
        lastUptimeMs = uptimeMs

        val vx = ((rawX - prevRawX) / pxPerDp) / dtSec
        val vy = ((rawY - prevRawY) / pxPerDp) / dtSec
        prevRawX = rawX
        prevRawY = rawY

        val alphaD = smoothingAlpha(dtSec, dCutoffHz)
        edx += alphaD * (vx - edx)
        edy += alphaD * (vy - edy)

        val speedDp = hypot(edx, edy)
        val cutoff = minCutoffHz + beta * speedDp
        val alpha = smoothingAlpha(dtSec, cutoff)
        filtX += alpha * (rawX - filtX)
        filtY += alpha * (rawY - filtY)

        val outDx = filtX - emittedX
        val outDy = filtY - emittedY
        val outDist = hypot(outDx, outDy)

        if (outDist < deadbandPx && speedDp < 18.0) {
            return FilteredDelta(0.0, 0.0, speedDp)
        }

        emittedX = filtX
        emittedY = filtY
        return FilteredDelta(outDx, outDy, speedDp)
    }

    private fun smoothingAlpha(dtSec: Double, cutoffHz: Double): Double {
        val tau = 1.0 / (2.0 * kotlin.math.PI * cutoffHz)
        return 1.0 / (1.0 + tau / dtSec)
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
    val touchDensity = LocalDensity.current.density
    val noiseFilter = remember(touchDensity) { TouchpadNoiseFilter(touchDensity) }
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

    val cornerShape = RoundedCornerShape(
        if (compactMode) OraculoMetrics.CornerLarge else OraculoMetrics.CornerSurface,
    )

    Box(
        modifier = modifier
            .clip(cornerShape)
            .background(OraculoColors.Surface)
            .border(1.dp, OraculoColors.Outline, cornerShape)
            .pointerInput(inputSession, compactMode, touchDensity) {
                val activeSession = inputSession ?: return@pointerInput
                var lastLeftTapUpMs = 0L
                var lastLeftTapPos: Offset? = null
                var consecutiveLeftTaps = 0
                val tapDriftHoldPx = 7.0f * touchDensity
                val rapidTapDriftHoldPx = 8.5f * touchDensity

                awaitEachGesture {
                    val firstDown = awaitFirstDown(requireUnconsumed = false)
                    hasTouched = true
                    touchSpot = firstDown.position
                    touchActive = true
                    noiseFilter.reset(firstDown.uptimeMillis)
                    val gestureStartMs = System.currentTimeMillis()
                    val prevTapPos = lastLeftTapPos
                    val dtSinceTap = gestureStartMs - lastLeftTapUpMs
                    val isRapidFollowUp = prevTapPos != null && dtSinceTap in 20L..380L
                    val effectiveTapHoldPx = if (isRapidFollowUp) rapidTapDriftHoldPx else tapDriftHoldPx
                    var dragCandidate = prevTapPos != null &&
                        dtSinceTap in 25L..260L &&
                        (firstDown.position - prevTapPos).getDistance() <= viewConfiguration.touchSlop * 1.85f
                    var isDragging = false
                    var pointerMoved = false
                    var maxPointers = 1
                    var totalMove = 0f

                    try {
                        while (true) {
                            val event = awaitPointerEvent()
                            val pressed = event.changes.filter { it.pressed }
                            maxPointers = maxOf(maxPointers, pressed.size)
                            if (maxPointers >= 2) {
                                dragCandidate = false
                            }
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
                                    lastLeftTapPos = null
                                    consecutiveLeftTaps = 0
                                    currentDisarmArmed()
                                    currentPublishTelemetry(tr("solto", "released"), 0L)
                                } else if (wentUpCount > 0) {
                                    if (maxPointers >= 3) {
                                        if (totalMove < viewConfiguration.touchSlop * 2.6f && elapsed < 360L) {
                                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                            if (sendOrReport(activeSession) {
                                                    activeSession.clickButton(MobileMouseButton.MIDDLE)
                                                }
                                            ) {
                                                currentDisarmArmed()
                                                currentPublishTelemetry(tr("botão do meio", "middle click"), 0L)
                                            }
                                        }
                                        lastLeftTapUpMs = 0L
                                        lastLeftTapPos = null
                                        consecutiveLeftTaps = 0
                                    } else if (maxPointers == 2) {
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
                                        lastLeftTapPos = null
                                        consecutiveLeftTaps = 0
                                    } else {
                                        val isTap = !pointerMoved && elapsed < 300L
                                        if (isTap) {
                                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                            if (sendOrReport(activeSession) {
                                                    activeSession.clickButton(MobileMouseButton.LEFT)
                                                }
                                            ) {
                                                val nowMs = System.currentTimeMillis()
                                                val multiDt = nowMs - lastLeftTapUpMs
                                                consecutiveLeftTaps = if (multiDt in 20L..380L) {
                                                    consecutiveLeftTaps + 1
                                                } else {
                                                    1
                                                }
                                                currentDisarmArmed()
                                                lastLeftTapUpMs = nowMs
                                                lastLeftTapPos = firstDown.position
                                                val label = if (consecutiveLeftTaps >= 2) {
                                                    tr("${consecutiveLeftTaps}× clique", "${consecutiveLeftTaps}× click")
                                                } else {
                                                    tr("clique", "click")
                                                }
                                                currentPublishTelemetry(label, 0L)
                                            }
                                        } else {
                                            lastLeftTapUpMs = 0L
                                            lastLeftTapPos = null
                                            consecutiveLeftTaps = 0
                                        }
                                    }
                                }
                                break
                            }

                            if (pressed.size == 1 && maxPointers == 1) {
                                val change = pressed[0]
                                val delta = change.positionChange()
                                if (abs(delta.x) > 0.01f || abs(delta.y) > 0.01f) {
                                    val rawDisplacement = (change.position - firstDown.position).getDistance()
                                    val holdElapsed = System.currentTimeMillis() - gestureStartMs

                                    if (dragCandidate && !isDragging) {
                                        if (holdElapsed < 65L && rawDisplacement > viewConfiguration.touchSlop * 1.25f) {
                                            // Movimento rápido logo após um clique é reposicionamento do cursor, não arrasto.
                                            dragCandidate = false
                                        } else if (
                                            (holdElapsed >= 90L && rawDisplacement >= effectiveTapHoldPx) ||
                                            (holdElapsed >= 65L && rawDisplacement >= viewConfiguration.touchSlop * 0.95f)
                                        ) {
                                            isDragging = true
                                            dragCandidate = false
                                            pointerMoved = true
                                            noiseFilter.reset(change.uptimeMillis)
                                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                            sendOrReport(activeSession) {
                                                activeSession.pressButton(MobileMouseButton.LEFT, true)
                                            }
                                        }
                                    }

                                    // Segura micro-deslizamentos iniciais durante toques rápidos para que
                                    // duplo e triplo clique caiam exatamente no mesmo pixel sem arrastar o cursor.
                                    if (!pointerMoved) {
                                        if (
                                            rawDisplacement > effectiveTapHoldPx ||
                                            (holdElapsed >= 160L && rawDisplacement > 2.5f * touchDensity)
                                        ) {
                                            pointerMoved = true
                                            noiseFilter.reset(change.uptimeMillis)
                                        } else {
                                            change.consume()
                                            continue
                                        }
                                    }

                                    val filtered = noiseFilter.step(delta.x, delta.y, change.uptimeMillis)
                                    val filteredStep = hypot(filtered.dx, filtered.dy).toFloat()
                                    totalMove += filteredStep
                                    if (filtered.dx != 0.0 || filtered.dy != 0.0) {
                                        val boost = 1.0 + (filtered.speedDpPerSec / 1150.0).coerceIn(0.0, 0.55)
                                        val sx = filtered.dx * currentPointerSpeed * boost
                                        val sy = filtered.dy * currentPointerSpeed * boost
                                        sendOrReport(activeSession) {
                                            activeSession.movePointer(sx, sy)
                                        }
                                        currentPublishTelemetry(
                                            if (isDragging) tr("arrastando", "dragging") else "dx ${filtered.dx.roundToInt()}  dy ${filtered.dy.roundToInt()}",
                                            55L,
                                        )
                                    }
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
                                val scrollDeadzone = 0.45 * touchDensity
                                if (abs(avgDx) > scrollDeadzone || abs(avgDy) > scrollDeadzone) {
                                    val scrollSign = if (currentNaturalScroll) 1.0 else -1.0
                                    sendOrReport(activeSession) {
                                        activeSession.scrollPointer(
                                            avgDx * 0.42 * scrollSign,
                                            avgDy * 0.42 * scrollSign,
                                        )
                                    }
                                    currentPublishTelemetry(tr("rolar", "scroll"), 55L)
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
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    contentPadding = if (compactMode) PaddingValues(horizontal = 12.dp) else ButtonDefaults.ContentPadding,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.SecondaryContainer,
                        contentColor = OraculoColors.OnSecondaryContainer,
                    ),
                    modifier = Modifier.heightIn(
                        min = if (compactMode) OraculoMetrics.ControlHeightSmall else OraculoMetrics.ControlHeightDialog,
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
                        text = tr("Toque: clicar · 2× arrastar · 2/3 dedos: dir./meio", "Tap: click · 2× drag · 2/3 fingers: right/middle"),
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
                        text = tr("2 dedos: direito ou rolar · 3 dedos: botão do meio", "2 fingers: right-click or scroll · 3 fingers: middle click"),
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
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .padding(overlayPadding)
                    .border(1.dp, OraculoColors.Outline, RoundedCornerShape(OraculoMetrics.CornerMedium)),
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
            color = OraculoColors.SurfaceRecessed.copy(alpha = 0.88f),
            shape = RoundedCornerShape(OraculoMetrics.CornerSmall),
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
    val currentOnPressChanged by rememberUpdatedState(onPressChanged)
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
            isPressed -> OraculoColors.OnSecondaryContainer
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
                    currentOnPressChanged(true)
                    try {
                        while (true) {
                            val event = awaitPointerEvent()
                            if (event.changes.none { it.pressed }) {
                                break
                            }
                        }
                    } finally {
                        isPressed = false
                        currentOnPressChanged(false)
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
                color = glowColor.copy(alpha = 0.06f * touchGlowAlpha),
                radius = radius,
                center = touchSpot,
            )
            drawCircle(
                color = glowColor.copy(alpha = 0.12f * touchGlowAlpha),
                radius = radius * 0.56f,
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

private fun buildTerminalQuickKeys(): List<TerminalQuickKeySpec> = listOf(
    TerminalQuickKeySpec("Esc", byteArrayOf(27)),
    TerminalQuickKeySpec("Tab", byteArrayOf(9)),
    TerminalQuickKeySpec("S-Tab", "\u001b[Z".encodeToByteArray()),
    TerminalQuickKeySpec("Ctrl+C", byteArrayOf(3)),
    TerminalQuickKeySpec("y", "y\r".encodeToByteArray()),
    TerminalQuickKeySpec("n", "n\r".encodeToByteArray()),
    TerminalQuickKeySpec("Enter", byteArrayOf(13)),
    TerminalQuickKeySpec(tr("Cima", "Up"), "\u001b[A".encodeToByteArray(), Icons.Default.KeyboardArrowUp),
    TerminalQuickKeySpec(tr("Baixo", "Down"), "\u001b[B".encodeToByteArray(), Icons.Default.KeyboardArrowDown),
    TerminalQuickKeySpec(tr("Esquerda", "Left"), "\u001b[D".encodeToByteArray(), Icons.AutoMirrored.Filled.KeyboardArrowLeft),
    TerminalQuickKeySpec(tr("Direita", "Right"), "\u001b[C".encodeToByteArray(), Icons.AutoMirrored.Filled.KeyboardArrowRight),
    TerminalQuickKeySpec(tr("Apagar", "Backspace"), byteArrayOf(0x7f), Icons.AutoMirrored.Filled.Backspace),
    TerminalQuickKeySpec("Ctrl+Z", byteArrayOf(26)),
    TerminalQuickKeySpec("Ctrl+D", byteArrayOf(4)),
    TerminalQuickKeySpec("Ctrl+L", byteArrayOf(12), clearsDraft = false),
)

private val DEFAULT_SESSION_LABEL_REGEX = Regex("^Sessão\\s+(\\d+)$")

private fun localizeSessionLabel(raw: String): String {
    if (!AppI18n.isPt) {
        val m = DEFAULT_SESSION_LABEL_REGEX.matchEntire(raw.trim())
        if (m != null) {
            return "Session ${m.groupValues[1]}"
        }
    }
    return raw
}

@Composable
private fun TerminalTab(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    isActive: Boolean,
    allowTerminal: Boolean? = null,
    connectionRestoredNonce: Int = 0,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    @Suppress("DEPRECATION")
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
    val softwareKeyboard = LocalSoftwareKeyboardController.current
    val pendingSnapshot = remember { AtomicReference<TerminalScreenSnapshot?>(null) }
    val renderScheduled = remember { AtomicBoolean(false) }

    var reconnectNonce by remember { mutableIntStateOf(0) }
    var autoRetryAttempts by remember(host.endpointId) { mutableIntStateOf(0) }
    var pendingRetryDelayMs by remember(host.endpointId) { mutableStateOf(0L) }
    var lastTerminalError by remember(host.endpointId) { mutableStateOf<String?>(null) }
    var session by remember { mutableStateOf<MobileTerminalSession?>(null) }
    var sessionsList by remember(host.endpointId) {
        mutableStateOf<List<MobileTerminalSessionInfo>>(emptyList())
    }
    var requestedSessionId by remember(host.endpointId) { mutableStateOf<String?>(null) }
    var requestedNewLabel by remember(host.endpointId) { mutableStateOf<String?>(null) }
    var activeSessionId by remember(host.endpointId) { mutableStateOf<String?>(null) }
    var activeSessionLabel by remember(host.endpointId) { mutableStateOf<String?>(null) }
    var activeSessionPersistent by remember(host.endpointId) { mutableStateOf(false) }
    var showKillConfirm by remember { mutableStateOf(false) }
    var isKillingSession by remember { mutableStateOf(false) }

    var screenSnapshot by remember { mutableStateOf<TerminalScreenSnapshot?>(null) }
    var screenRevision by remember { mutableIntStateOf(0) }
    var fallbackMessage by remember {
        mutableStateOf(tr("Conectando a ${host.hostName}...", "Connecting to ${host.hostName}..."))
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

    fun refreshSessions() {
        scope.launch {
            val fetched = withContext(Dispatchers.IO) {
                runCatching { client.listTerminalSessions(host.endpointId) }.getOrNull()
            }
            if (fetched != null) {
                sessionsList = fetched
            }
        }
    }

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
        if (terminalStatus.startsWith("Terminal (")) {
            terminalStatus = "Terminal (${termCols}×${termRows})"
        }
    }

    val currentAllowTerminal by rememberUpdatedState(allowTerminal)

    LaunchedEffect(allowTerminal) {
        if (allowTerminal == false) {
            val prev = session
            session = null
            sessionsList = emptyList()
            requestedSessionId = null
            activeSessionId = null
            activeSessionLabel = null
            isConnecting = false
            terminalStatus = tr("Desativado no computador", "Disabled on computer")
            fallbackMessage = tr(
                "Terminal desativado em Acesso no computador.",
                "Terminal disabled in Access on computer.",
            )
            if (prev != null) {
                withContext(Dispatchers.IO) {
                    runCatching {
                        prev.closeSession()
                        prev.close()
                    }
                }
            }
        } else if (allowTerminal == true && session == null && !isConnecting) {
            autoRetryAttempts = 0
            pendingRetryDelayMs = 0L
            lastTerminalError = null
            reconnectNonce += 1
        }
    }

    LaunchedEffect(connectionRestoredNonce) {
        if (connectionRestoredNonce == 0 || allowTerminal == false || isKillingSession) return@LaunchedEffect
        val blockedByPolicy = lastTerminalError != null && isPermanentRemoteError(lastTerminalError!!)
        if (!blockedByPolicy && session == null && !isConnecting) {
            autoRetryAttempts = 0
            pendingRetryDelayMs = 0L
            lastTerminalError = null
            reconnectNonce += 1
        }
    }

    DisposableEffect(host.endpointId, reconnectNonce) {
        if (currentAllowTerminal == false) {
            isConnecting = false
            terminalStatus = tr("Desativado no computador", "Disabled on computer")
            fallbackMessage = tr(
                "Terminal desativado em Acesso no computador.",
                "Terminal disabled in Access on computer.",
            )
            return@DisposableEffect onDispose {}
        }
        val active = AtomicBoolean(true)
        val sessionRef = AtomicReference<MobileTerminalSession?>(null)
        val streamEndedOrErrored = AtomicBoolean(false)
        val launchSessionId = requestedSessionId
        val launchNewLabel = requestedNewLabel
        val retryDelayMs = pendingRetryDelayMs
        pendingRetryDelayMs = 0L
        requestedNewLabel = null
        pendingSnapshot.set(null)
        renderScheduled.set(false)
        isConnecting = true
        val keepingSnapshotOnReattach =
            screenSnapshot != null && launchSessionId != null && launchSessionId == activeSessionId
        if (!keepingSnapshotOnReattach) {
            screenSnapshot = null
            screenRevision = 0
        }
        fallbackMessage = tr("Conectando a ${host.hostName}...", "Connecting to ${host.hostName}...")
        terminalStatus = if (keepingSnapshotOnReattach) {
            tr("Reconectando terminal...", "Reconnecting terminal...")
        } else {
            tr("Abrindo terminal...", "Opening terminal...")
        }
        remoteField = TextFieldValue("")
        syncedText = ""

        val job = CoroutineScope(Dispatchers.IO).launch {
            if (retryDelayMs > 0L) {
                delay(retryDelayMs)
                if (!active.get()) return@launch
            }
            val observer = object : TerminalObserver {
                override fun onSessionReady(
                    sessionId: String,
                    label: String,
                    persistent: Boolean,
                    reconnected: Boolean,
                ) {
                    mainHandler.post {
                        if (!active.get()) return@post
                        autoRetryAttempts = 0
                        pendingRetryDelayMs = 0L
                        lastTerminalError = null
                        activeSessionId = sessionId
                        if (persistent) {
                            requestedSessionId = sessionId
                        }
                        activeSessionLabel = label
                        activeSessionPersistent = persistent
                        refreshSessions()
                    }
                }

                override fun onScreenUpdated(snapshot: TerminalScreenSnapshot) {
                    if (!active.get()) return
                    pendingSnapshot.set(snapshot)
                    if (renderScheduled.compareAndSet(false, true)) {
                        mainHandler.post {
                            renderScheduled.set(false)
                            if (!active.get()) return@post
                            val latest = pendingSnapshot.getAndSet(null) ?: return@post
                            screenSnapshot = latest
                            screenRevision += 1
                            if (latest.bellCount > lastBellCount) {
                                lastBellCount = latest.bellCount
                                visualBellActive = true
                            }
                            terminalStatus = "Terminal (${latest.cols}×${latest.rows})"
                            isConnecting = false
                        }
                    }
                }

                override fun onSessionEnded(exitCode: Int) {
                    streamEndedOrErrored.set(true)
                    mainHandler.post {
                        if (active.get()) {
                            session = null
                            if (requestedSessionId == activeSessionId) {
                                requestedSessionId = null
                            }
                            terminalStatus = tr("Encerrado (exit $exitCode)", "Ended (exit $exitCode)")
                            if (screenSnapshot == null) {
                                fallbackMessage = tr(
                                    "Sessão de terminal encerrada (código $exitCode).",
                                    "Terminal session ended (exit code $exitCode).",
                                )
                            }
                            isConnecting = false
                            refreshSessions()
                        }
                    }
                }

                override fun onError(message: String) {
                    streamEndedOrErrored.set(true)
                    mainHandler.post {
                        if (active.get() && !isKillingSession) {
                            session = null
                            lastTerminalError = message
                            val permanent = isPermanentRemoteError(message)
                            if (permanent && requestedSessionId != null) {
                                requestedSessionId = null
                            }
                            if (!permanent && currentAllowTerminal != false && autoRetryAttempts < 5) {
                                val backoffSchedule = listOf(700L, 1500L, 2500L, 4000L, 5000L)
                                pendingRetryDelayMs = backoffSchedule[autoRetryAttempts.coerceAtMost(backoffSchedule.lastIndex)]
                                autoRetryAttempts += 1
                                terminalStatus = tr("Reconectando terminal...", "Reconnecting terminal...")
                                isConnecting = true
                                reconnectNonce += 1
                            } else {
                                terminalStatus = tr("Erro: $message", "Error: $message")
                                if (screenSnapshot == null) {
                                    fallbackMessage = tr(
                                        "Não foi possível conectar ao terminal:\n$message",
                                        "Could not connect to terminal:\n$message",
                                    )
                                }
                                isConnecting = false
                                refreshSessions()
                            }
                        }
                    }
                }
            }

            val result = runCatching {
                client.openTerminalSession(
                    endpointId = host.endpointId,
                    cols = termCols.toUShort(),
                    rows = termRows.toUShort(),
                    persistent = true,
                    sessionId = launchSessionId,
                    label = launchNewLabel,
                    observer = observer,
                )
            }

            if (!active.get() || streamEndedOrErrored.get()) {
                result.onSuccess { created ->
                    runCatching {
                        created.closeSession()
                        created.close()
                    }
                }
                if (!active.get() || result.isSuccess) return@launch
            }

            result.onSuccess { created ->
                sessionRef.set(created)
                withContext(Dispatchers.Main + NonCancellable) {
                    if (active.get() && !streamEndedOrErrored.get()) {
                        session = created
                        if (screenSnapshot == null) {
                            terminalStatus = "Terminal (${termCols}×${termRows})"
                        }
                        isConnecting = false
                    } else {
                        val toClose = sessionRef.getAndSet(null)
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
            }.onFailure { err ->
                val msg = formatOraculoError(err)
                withContext(Dispatchers.Main + NonCancellable) {
                    if (active.get()) {
                        lastTerminalError = msg
                        val permanent = isPermanentRemoteError(msg)
                        if (permanent && requestedSessionId != null) {
                            requestedSessionId = null
                        }
                        if (!permanent && currentAllowTerminal != false && autoRetryAttempts < 5) {
                            val backoffSchedule = listOf(700L, 1500L, 2500L, 4000L, 5000L)
                            pendingRetryDelayMs = backoffSchedule[autoRetryAttempts.coerceAtMost(backoffSchedule.lastIndex)]
                            autoRetryAttempts += 1
                            terminalStatus = tr("Reconectando terminal...", "Reconnecting terminal...")
                            isConnecting = true
                            reconnectNonce += 1
                        } else {
                            isConnecting = false
                            if (!keepingSnapshotOnReattach) {
                                screenSnapshot = null
                                screenRevision = 0
                            }
                            terminalStatus = tr("Falha na conexão", "Connection failed")
                            fallbackMessage = tr(
                                "Não foi possível abrir o terminal:\n$msg",
                                "Could not open terminal:\n$msg",
                            )
                        }
                    }
                }
            }
        }

        onDispose {
            active.set(false)
            pendingSnapshot.set(null)
            renderScheduled.set(false)
            job.cancel()
            remoteField = TextFieldValue("")
            syncedText = ""
            val toClose = sessionRef.getAndSet(null) ?: session
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
                terminalStatus = tr(
                    "Erro ao enviar: ${formatOraculoError(err)}",
                    "Send failed: ${formatOraculoError(err)}",
                )
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
    val windowTitle = remember(activeSessionLabel, screenSnapshot?.title, terminalStatus, session, AppI18n.lang) {
        val snapTitle = screenSnapshot?.title?.takeIf { it.isNotBlank() }
        val base = activeSessionLabel?.takeIf { it.isNotBlank() }?.let(::localizeSessionLabel)
        when {
            base != null && snapTitle != null && snapTitle != base -> "$base · $snapTitle"
            base != null && session == null -> "$base · $terminalStatus"
            base != null -> base
            snapTitle != null -> snapTitle
            else -> terminalStatus
        }
    }
    val displayCols = screenSnapshot?.cols?.toInt() ?: termCols
    val displayRows = screenSnapshot?.rows?.toInt() ?: termRows
    val runningCount = remember(sessionsList) { sessionsList.count { it.running } }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp)
            .padding(bottom = 10.dp, top = 2.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (allowTerminal != false && (sessionsList.isNotEmpty() || (!isConnecting && session == null))) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                sessionsList.forEach { item ->
                    val isSelected = item.id == activeSessionId
                    val displayLabel = localizeSessionLabel(item.label)
                    FilterChip(
                        selected = isSelected,
                        enabled = !isKillingSession,
                        onClick = {
                            if (isKillingSession) return@FilterChip
                            if (item.id != activeSessionId || session == null) {
                                requestedSessionId = item.id
                                requestedNewLabel = null
                                reconnectNonce += 1
                            }
                        },
                        leadingIcon = {
                            Box(
                                modifier = Modifier
                                    .size(7.dp)
                                    .clip(RoundedCornerShape(OraculoMetrics.CornerPill))
                                    .background(
                                        if (item.running) OraculoColors.Success else OraculoColors.OnSurfaceMuted,
                                    ),
                            )
                        },
                        trailingIcon = if (isSelected && activeSessionPersistent) {
                            {
                                Box(
                                    modifier = Modifier
                                        .size(32.dp)
                                        .clip(RoundedCornerShape(OraculoMetrics.CornerPill))
                                        .clickable(enabled = !isKillingSession) {
                                            showKillConfirm = true
                                        },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Icon(
                                        imageVector = Icons.Default.Close,
                                        contentDescription = tr("Encerrar $displayLabel", "End $displayLabel"),
                                        modifier = Modifier.size(16.dp),
                                    )
                                }
                            }
                        } else {
                            null
                        },
                        label = {
                            Text(
                                text = if (item.running) {
                                    displayLabel
                                } else {
                                    "$displayLabel (${tr("fim", "done")})"
                                },
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        },
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        colors = FilterChipDefaults.filterChipColors(
                            containerColor = OraculoColors.Surface,
                            labelColor = OraculoColors.OnSurfaceMuted,
                            selectedContainerColor = OraculoColors.SecondaryContainer,
                            selectedLabelColor = OraculoColors.OnSecondaryContainer,
                            selectedTrailingIconColor = OraculoColors.OnSecondaryContainer,
                        ),
                        border = if (OraculoColors.IsDark && isSelected) {
                            null
                        } else {
                            FilterChipDefaults.filterChipBorder(
                                enabled = !isKillingSession,
                                selected = isSelected,
                                borderColor = OraculoColors.Outline,
                                selectedBorderColor = OraculoColors.Primary,
                            )
                        },
                        modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightSmall),
                    )
                }

                FilterChip(
                    selected = false,
                    enabled = runningCount < 4 && !isKillingSession,
                    onClick = {
                        val nextIdx = sessionsList.size + 1
                        requestedSessionId = null
                        requestedNewLabel = tr("Sessão $nextIdx", "Session $nextIdx")
                        reconnectNonce += 1
                    },
                    leadingIcon = {
                        Icon(
                            imageVector = Icons.Default.Add,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp),
                        )
                    },
                    label = { Text(tr("Nova", "New")) },
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    colors = FilterChipDefaults.filterChipColors(
                        containerColor = OraculoColors.Surface,
                        labelColor = OraculoColors.Primary,
                        iconColor = OraculoColors.Primary,
                    ),
                    border = FilterChipDefaults.filterChipBorder(
                        enabled = runningCount < 4 && !isKillingSession,
                        selected = false,
                        borderColor = OraculoColors.Outline,
                    ),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightSmall),
                )
            }
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .clip(RoundedCornerShape(OraculoMetrics.CornerLarge))
                .background(ptyxis.Background)
                .border(1.dp, terminalBorderColor, RoundedCornerShape(OraculoMetrics.CornerLarge)),
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
                    if (runCatching { clipboard.setText(AnnotatedString(plainCopyText)) }.isSuccess) {
                        copiedRecently = true
                    }
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
                isActive = isActive,
                onGridMeasured = { cols, rows ->
                    termCols = cols
                    termRows = rows
                },
                onTapViewport = {
                    runCatching {
                        inputFocusRequester.requestFocus()
                        softwareKeyboard?.show()
                    }
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
                        if (newText.contains('\n') || newText.contains('\r')) {
                            val tail = newText.replace("\r\n", "\n").replace('\r', '\n').substringAfterLast('\n')
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
                            tr("Digite para enviar ao terminal...", "Type to send to terminal...")
                        } else {
                            tr("Aguardando terminal...", "Waiting for terminal...")
                        },
                    )
                },
                singleLine = true,
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                trailingIcon = if (remoteField.text.isNotEmpty()) {
                    {
                        IconButton(onClick = { clearDraft() }) {
                            Icon(
                                imageVector = Icons.Default.Close,
                                contentDescription = tr("Limpar texto", "Clear text"),
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
                colors = oraculoPrimaryButtonColors(),
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                contentPadding = PaddingValues(horizontal = 16.dp),
                modifier = Modifier
                    .height(OraculoMetrics.ControlHeightInput)
                    .widthIn(min = OraculoMetrics.ControlHeightInput),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.KeyboardReturn,
                    contentDescription = tr("Enviar Enter", "Send Enter"),
                    modifier = Modifier.size(20.dp),
                )
            }
        }
    }

    if (showKillConfirm) {
        val killId = activeSessionId
        val killLabel = activeSessionLabel?.let(::localizeSessionLabel) ?: killId.orEmpty()
        AlertDialog(
            onDismissRequest = { if (!isKillingSession) showKillConfirm = false },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = {
                Text(
                    tr("Encerrar '$killLabel'?", "End '$killLabel'?"),
                )
            },
            text = {
                Text(
                    tr(
                        "Isso finaliza o shell e qualquer processo ou agente em execução nessa sessão no computador.",
                        "This terminates the shell and any running process or agent in this session on your computer.",
                    ),
                    color = OraculoColors.OnSurfaceMuted,
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        if (killId == null || isKillingSession) return@Button
                        isKillingSession = true
                        val prevSession = session
                        session = null
                        activeSessionId = null
                        activeSessionLabel = null
                        requestedSessionId = null
                        scope.launch {
                            val res = withContext(Dispatchers.IO) {
                                runCatching {
                                    prevSession?.closeSession()
                                    prevSession?.close()
                                    client.killTerminalSession(host.endpointId, killId)
                                    client.listTerminalSessions(host.endpointId)
                                }
                            }
                            isKillingSession = false
                            showKillConfirm = false
                            res.onSuccess { remaining ->
                                sessionsList = remaining
                                val nextRunning = remaining.firstOrNull { it.running }
                                if (nextRunning != null) {
                                    requestedSessionId = nextRunning.id
                                    reconnectNonce += 1
                                } else {
                                    screenSnapshot = null
                                    screenRevision = 0
                                    terminalStatus = tr("Sessão encerrada", "Session ended")
                                    fallbackMessage = tr(
                                        "Sessão encerrada no computador. Toque em '+ Nova' para abrir outro shell.",
                                        "Session closed on computer. Tap '+ New' to open another shell.",
                                    )
                                }
                            }.onFailure { err ->
                                terminalStatus = formatOraculoError(err)
                            }
                        }
                    },
                    enabled = !isKillingSession,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.Error,
                        contentColor = OraculoColors.OnError,
                    ),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(if (isKillingSession) tr("Encerrando...", "Ending...") else tr("Encerrar", "End"))
                }
            },
            dismissButton = {
                TextButton(
                    onClick = { showKillConfirm = false },
                    enabled = !isKillingSession,
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
                }
            },
        )
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
    val canDecreaseFont = fontSizeSp > 10f
    val canIncreaseFont = fontSizeSp < 18f

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(ptyxis.HeaderBackground)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(2.dp),
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
            modifier = Modifier.padding(horizontal = 4.dp),
        )
        IconButton(
            onClick = onDecreaseFont,
            enabled = canDecreaseFont,
            modifier = Modifier.size(36.dp),
        ) {
            Icon(
                imageVector = Icons.Default.ZoomOut,
                contentDescription = tr("Diminuir fonte", "Decrease font size"),
                tint = if (canDecreaseFont) ptyxis.ForegroundMuted else ptyxis.ForegroundMuted.copy(alpha = 0.35f),
                modifier = Modifier.size(16.dp),
            )
        }
        IconButton(
            onClick = onIncreaseFont,
            enabled = canIncreaseFont,
            modifier = Modifier.size(36.dp),
        ) {
            Icon(
                imageVector = Icons.Default.ZoomIn,
                contentDescription = tr("Aumentar fonte", "Increase font size"),
                tint = if (canIncreaseFont) ptyxis.ForegroundMuted else ptyxis.ForegroundMuted.copy(alpha = 0.35f),
                modifier = Modifier.size(16.dp),
            )
        }
        IconButton(
            onClick = onCopy,
            modifier = Modifier.size(36.dp),
        ) {
            Icon(
                imageVector = if (copiedRecently) Icons.Default.Check else Icons.Default.ContentCopy,
                contentDescription = tr("Copiar terminal", "Copy terminal"),
                tint = if (copiedRecently) OraculoColors.Primary else ptyxis.ForegroundMuted,
                modifier = Modifier.size(15.dp),
            )
        }
        IconButton(
            onClick = onClearTerminal,
            enabled = canSendClear,
            modifier = Modifier.size(36.dp),
        ) {
            Icon(
                imageVector = Icons.Default.ClearAll,
                contentDescription = tr("Limpar terminal", "Clear terminal"),
                tint = if (canSendClear) ptyxis.ForegroundMuted else ptyxis.ForegroundMuted.copy(alpha = 0.35f),
                modifier = Modifier.size(16.dp),
            )
        }
        IconButton(
            onClick = onReconnect,
            modifier = Modifier.size(36.dp),
        ) {
            Icon(
                imageVector = Icons.Default.Refresh,
                contentDescription = tr("Reconectar terminal", "Reconnect terminal"),
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
    isActive: Boolean,
    onGridMeasured: (cols: Int, rows: Int) -> Unit,
    onTapViewport: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val density = LocalDensity.current
    val textMeasurer = rememberTextMeasurer()
    val currentOnGridMeasured by rememberUpdatedState(onGridMeasured)
    var lastViewportSizePx by remember { mutableStateOf(IntSize.Zero) }
    val cellMetrics = remember(textMeasurer, terminalStyle) {
        val measured = textMeasurer.measure("MMMMMMMMMM", style = terminalStyle)
        val charWidth = (measured.size.width / 10f).coerceAtLeast(1f)
        val lineHeight = measured.size.height.toFloat().coerceAtLeast(1f)
        charWidth to lineHeight
    }
    val padPx = with(density) { 20.dp.toPx() }
    LaunchedEffect(lastViewportSizePx, cellMetrics, padPx) {
        if (lastViewportSizePx.width > 0 && lastViewportSizePx.height > 0) {
            val innerW = (lastViewportSizePx.width - padPx).coerceAtLeast(cellMetrics.first * 36)
            val innerH = (lastViewportSizePx.height - padPx).coerceAtLeast(cellMetrics.second * 10)
            val cols = (innerW / cellMetrics.first).toInt().coerceIn(36, 180)
            val rows = (innerH / cellMetrics.second).toInt().coerceIn(10, 72)
            currentOnGridMeasured(cols, rows)
        }
    }

    val cursorEnabled = (screenSnapshot?.cursorVisible == true) && !isConnecting
    var cursorBlinkOn by remember { mutableStateOf(true) }
    LaunchedEffect(cursorEnabled, isActive, screenRevision) {
        cursorBlinkOn = true
        if (cursorEnabled && isActive) {
            while (true) {
                delay(530L)
                cursorBlinkOn = !cursorBlinkOn
            }
        }
    }

    val renderedBase = remember(screenSnapshot, fallbackMessage, ptyxis) {
        val snap = screenSnapshot
        if (snap != null) {
            buildPtyxisAnnotatedScreen(snap, ptyxis, showCursor = false)
        } else {
            AnnotatedString(fallbackMessage.trimEnd('\n', '\r', ' ').ifEmpty { " " })
        }
    }
    var textLayout by remember { mutableStateOf<TextLayoutResult?>(null) }

    val verticalScroll = rememberScrollState()
    val horizontalScroll = rememberScrollState()
    val autoScrollSlackPx = with(density) { 48.dp.toPx() }
    LaunchedEffect(screenRevision, screenSnapshot?.cursorRow) {
        val wasNearBottom =
            verticalScroll.maxValue == 0 ||
                (verticalScroll.maxValue - verticalScroll.value) <= autoScrollSlackPx
        if (wasNearBottom) {
            withFrameNanos { }
            verticalScroll.scrollTo(verticalScroll.maxValue)
        }
    }

    Box(
        modifier = modifier
            .onSizeChanged { size ->
                lastViewportSizePx = size
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
            Text(
                text = renderedBase,
                style = terminalStyle,
                color = ptyxis.Foreground,
                softWrap = false,
                onTextLayout = { textLayout = it },
                modifier = Modifier.drawWithContent {
                    drawContent()
                    val snap = screenSnapshot
                    val layout = textLayout
                    if (snap != null && layout != null && cursorEnabled && cursorBlinkOn && layout.lineCount > 0) {
                        val row = snap.cursorRow.toInt().coerceIn(0, layout.lineCount - 1)
                        val col = snap.cursorCol.toInt().coerceAtLeast(0)
                        val lineStart = layout.getLineStart(row)
                        val lineEnd = layout.getLineEnd(row, visibleEnd = false)
                        val lineChars = (lineEnd - lineStart).let { raw ->
                            if (raw > 0 && layout.layoutInput.text.text.getOrNull(lineEnd - 1) == '\n') {
                                raw - 1
                            } else {
                                raw
                            }
                        }.coerceAtLeast(0)
                        val top = layout.getLineTop(row)
                        val bottom = layout.getLineBottom(row)
                        val cellW = cellMetrics.first
                        val cellH = (bottom - top).coerceAtLeast(cellMetrics.second)
                        if (col < lineChars) {
                            val box = layout.getBoundingBox(lineStart + col)
                            drawRect(
                                color = ptyxis.Cursor.copy(alpha = 0.45f),
                                topLeft = Offset(box.left, top),
                                size = Size(box.width.coerceAtLeast(cellW), cellH),
                            )
                        } else {
                            val baseLeft = if (lineChars > 0) layout.getLineRight(row) else layout.getLineLeft(row)
                            val extraCols = col - lineChars
                            drawRect(
                                color = ptyxis.Cursor.copy(alpha = 0.85f),
                                topLeft = Offset(baseLeft + extraCols * cellW, top),
                                size = Size(cellW, cellH),
                            )
                        }
                    }
                },
            )
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
    val quickKeys = remember(AppI18n.lang) { buildTerminalQuickKeys() }
    val shape = RoundedCornerShape(OraculoMetrics.CornerSmall)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        quickKeys.forEach { keySpec ->
            Surface(
                onClick = { onKey(keySpec) },
                color = OraculoColors.Surface,
                shape = shape,
                border = BorderStroke(1.dp, OraculoColors.Outline),
                modifier = Modifier.height(OraculoMetrics.ControlHeightSmall),
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
    allowCommandsRun: Boolean? = null,
    allowCommandsEdit: Boolean? = null,
    modifier: Modifier = Modifier,
) {
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val canEditCommands = allowCommandsEdit != false
    val canRunCommands = allowCommandsRun != false
    val loadGen = remember { AtomicLong(0L) }
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
        val gen = loadGen.incrementAndGet()
        scope.launch {
            isLoading = true
            errorMessage = null
            val res = withContext(Dispatchers.IO) {
                runCatching { client.listCommands(host.endpointId) }
            }
            if (gen != loadGen.get()) return@launch
            res.onSuccess { list ->
                commands = list
            }.onFailure { err ->
                errorMessage = formatOraculoError(err)
            }
            isLoading = false
        }
    }

    fun openCreateEditor() {
        if (!canEditCommands) return
        editCommandId = null
        editLabel = ""
        editDescription = ""
        editCommandLine = ""
        editorError = null
        isEditorOpen = true
    }

    LaunchedEffect(allowCommandsRun, allowCommandsEdit) {
        if (allowCommandsRun == true) {
            errorMessage = null
            if (commands.isEmpty() && !isLoading) {
                loadCommands()
            }
        }
        if (allowCommandsEdit == true) {
            editorError = null
        }
    }

    fun openEditEditor(item: QuickCommandItem) {
        if (!canEditCommands) return
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
            try {
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
                    loadGen.incrementAndGet()
                    commands = updated
                    isLoading = false
                    isEditorOpen = false
                    errorMessage = null
                }.onFailure { err ->
                    editorError = formatOraculoError(err)
                }
            } finally {
                isSavingCommand = false
            }
        }
    }

    fun confirmDelete(item: QuickCommandItem) {
        if (deletingCommandId != null) return
        pendingDeleteCommand = null
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        scope.launch {
            deletingCommandId = item.id
            errorMessage = null
            try {
                val res = withContext(Dispatchers.IO) {
                    runCatching { client.deleteCommand(host.endpointId, item.id) }
                }
                res.onSuccess { updated ->
                    loadGen.incrementAndGet()
                    commands = updated
                    isLoading = false
                    if (editCommandId == item.id) {
                        isEditorOpen = false
                    }
                }.onFailure { err ->
                    errorMessage = formatOraculoError(err)
                }
            } finally {
                deletingCommandId = null
            }
        }
    }

    fun executeCommand(item: QuickCommandItem) {
        if (!canRunCommands || runningCommandId != null) return
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        scope.launch {
            runningCommandId = item.id
            errorMessage = null
            try {
                val res = withContext(Dispatchers.IO) {
                    runCatching { client.runCommand(host.endpointId, item.id) }
                }
                res.onSuccess { output ->
                    lastCommandLabel = item.label
                    lastResult = output
                }.onFailure { err ->
                    errorMessage = formatOraculoError(err)
                }
            } finally {
                runningCommandId = null
            }
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
            onDismissRequest = { if (deletingCommandId == null) pendingDeleteCommand = null },
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
                        "\"${target.label}\" será removido deste computador.",
                        "\"${target.label}\" will be removed from this computer.",
                    ),
                    color = OraculoColors.OnSurfaceMuted,
                )
            },
            confirmButton = {
                Button(
                    onClick = { confirmDelete(target) },
                    enabled = deletingCommandId == null,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.Error,
                        contentColor = OraculoColors.OnError,
                    ),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Remover", "Delete"))
                }
            },
            dismissButton = {
                TextButton(
                    onClick = { pendingDeleteCommand = null },
                    enabled = deletingCommandId == null,
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
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
            FilledTonalButton(
                onClick = { openCreateEditor() },
                enabled = canEditCommands,
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                colors = ButtonDefaults.filledTonalButtonColors(
                    containerColor = OraculoColors.SecondaryContainer,
                    contentColor = OraculoColors.OnSecondaryContainer,
                ),
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 0.dp),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightSmall),
            ) {
                Icon(
                    imageVector = Icons.Default.Add,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(modifier = Modifier.width(6.dp))
                Text(
                    text = tr("Novo", "New"),
                    style = MaterialTheme.typography.labelLarge,
                    fontSize = 13.sp,
                )
            }
            IconButton(
                onClick = { loadCommands() },
                modifier = Modifier.size(44.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Refresh,
                    contentDescription = tr("Atualizar lista", "Refresh list"),
                    tint = OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(20.dp),
                )
            }
        }

        if (allowCommandsRun == false) {
            Surface(
                color = OraculoColors.SurfaceRecessed,
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                border = BorderStroke(1.dp, OraculoColors.Outline),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    text = tr(
                        "Execução de comandos desativada nas permissões do computador.",
                        "Running commands is disabled in your computer's permissions.",
                    ),
                    color = OraculoColors.OnSurfaceMuted,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(12.dp),
                )
            }
        }

        if (errorMessage != null) {
            Surface(
                color = OraculoColors.Error.copy(alpha = 0.14f),
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, OraculoColors.Error.copy(alpha = 0.4f), RoundedCornerShape(OraculoMetrics.CornerMedium)),
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
                    text = if (canEditCommands) {
                        tr(
                            "Nenhum comando salvo.\nToque em \"Novo\" para criar o primeiro.",
                            "No saved commands yet.\nTap \"New\" to create one.",
                        )
                    } else {
                        tr(
                            "Nenhum comando salvo.",
                            "No saved commands.",
                        )
                    },
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
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                itemsIndexed(commands, key = { _, item -> item.id }) { index, item ->
                    val itemShape = remember(index, commands.size) {
                        oraculoSegmentedItemShape(index, commands.size)
                    }
                    CommandCardRow(
                        item = item,
                        shape = itemShape,
                        isRunning = runningCommandId == item.id,
                        isDeleting = deletingCommandId == item.id,
                        hasDeleteInFlight = deletingCommandId != null,
                        canRun = canRunCommands && runningCommandId == null,
                        canEdit = canEditCommands,
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
                    commandLabel = lastCommandLabel ?: tr("Saída", "Output"),
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
                    placeholder = { Text(tr("Ex.: Reiniciar áudio", "e.g. Restart audio")) },
                    singleLine = true,
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = commandLine,
                    onValueChange = onCommandLineChange,
                    label = { Text(tr("Comando", "Command")) },
                    placeholder = { Text(tr("Ex.: systemctl --user restart pipewire", "e.g. systemctl --user restart pipewire")) },
                    singleLine = true,
                    textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp),
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = description,
                    onValueChange = onDescriptionChange,
                    label = { Text(tr("Descrição (opcional)", "Description (optional)")) },
                    placeholder = { Text(tr("Para que serve este comando", "What this command does")) },
                    singleLine = true,
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    colors = fieldColors,
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    text = tr(
                        "Aceita argumentos entre aspas.",
                        "Supports quoted arguments.",
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
                colors = oraculoPrimaryButtonColors(),
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                if (isSaving) {
                    CircularProgressIndicator(
                        color = OraculoColors.OnPrimaryAction,
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
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
            }
        },
    )
}

@Composable
private fun CommandCardRow(
    item: QuickCommandItem,
    shape: Shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
    isRunning: Boolean,
    isDeleting: Boolean,
    hasDeleteInFlight: Boolean = false,
    canRun: Boolean,
    canEdit: Boolean = true,
    onExecute: () -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    val titleColor = if (isRunning) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurface
    val subColor = if (isRunning) OraculoColors.OnSecondaryContainer.copy(alpha = 0.82f) else OraculoColors.OnSurfaceMuted
    val cmdLineColor = if (isRunning) OraculoColors.OnSecondaryContainer.copy(alpha = 0.82f) else OraculoColors.OnSurfaceMuted.copy(alpha = 0.85f)
    val canEditRow = canEdit && !isDeleting && !hasDeleteInFlight

    Surface(
        onClick = onExecute,
        enabled = canRun && !isDeleting,
        color = if (isRunning) OraculoColors.SecondaryContainer else OraculoColors.Surface,
        shape = shape,
        border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 14.dp, end = 10.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = item.label,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = titleColor,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (item.description.isNotBlank()) {
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = item.description,
                        style = MaterialTheme.typography.bodySmall,
                        color = subColor,
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
                        color = cmdLineColor,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Spacer(modifier = Modifier.width(6.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                if (canEdit) {
                    IconButton(
                        onClick = onEdit,
                        enabled = canEditRow,
                        modifier = Modifier.size(40.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Default.Edit,
                            contentDescription = tr("Editar ${item.label}", "Edit ${item.label}"),
                            tint = if (canEditRow) subColor else subColor.copy(alpha = 0.38f),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(
                        onClick = onDelete,
                        enabled = canEditRow,
                        modifier = Modifier.size(40.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Default.DeleteOutline,
                            contentDescription = tr("Remover ${item.label}", "Delete ${item.label}"),
                            tint = if (canEditRow) subColor else subColor.copy(alpha = 0.38f),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    Spacer(modifier = Modifier.width(2.dp))
                }
                FilledTonalIconButton(
                    onClick = onExecute,
                    enabled = canRun && !isDeleting && !isRunning,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = OraculoColors.SecondaryContainer,
                        contentColor = OraculoColors.OnSecondaryContainer,
                    ),
                    modifier = Modifier.size(40.dp),
                ) {
                    if (isRunning || isDeleting) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(16.dp),
                            color = OraculoColors.OnSecondaryContainer,
                            strokeWidth = 2.dp,
                        )
                    } else {
                        Icon(
                            imageVector = Icons.Default.PlayArrow,
                            contentDescription = tr("Executar ${item.label}", "Run ${item.label}"),
                            modifier = Modifier.size(20.dp),
                        )
                    }
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
    val darkTermPalette = ptyxisPaletteFor(true)
    val exitErrorColor = ptyxisPaletteFor(false).Ansi16[9]
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
            .clip(RoundedCornerShape(OraculoMetrics.CornerLarge))
            .background(OraculoColors.TerminalBackground)
            .border(1.dp, OraculoColors.TerminalBorder, RoundedCornerShape(OraculoMetrics.CornerLarge))
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
                text = tr("saída ${result.exitCode}", "exit ${result.exitCode}"),
                style = TextStyle(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                    fontFeatureSettings = "tnum",
                ),
                color = if (result.exitCode == 0) darkTermPalette.Ansi16[10] else exitErrorColor,
            )
            IconButton(
                onClick = onDismiss,
                modifier = Modifier.size(36.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Close,
                    contentDescription = tr("Fechar saída", "Close output"),
                    tint = darkTermPalette.ForegroundMuted,
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

@Composable
private fun RdpTunnelDialog(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    allowRdpTunnel: Boolean,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    @Suppress("DEPRECATION")
    val clipboard = LocalClipboardManager.current
    var retryNonce by remember { mutableIntStateOf(0) }
    var tunnelSession by remember { mutableStateOf<MobileTunnelSession?>(null) }
    var isOpening by remember { mutableStateOf(true) }
    var errorMsg by remember { mutableStateOf<String?>(null) }
    var hintMsg by remember { mutableStateOf<String?>(null) }

    DisposableEffect(host.endpointId, allowRdpTunnel, retryNonce) {
        if (!allowRdpTunnel) {
            isOpening = false
            errorMsg = tr(
                "Login remoto (túnel RDP) desativado nas permissões deste computador.",
                "Remote login (RDP tunnel) is turned off on this computer.",
            )
            return@DisposableEffect onDispose {}
        }

        val active = AtomicBoolean(true)
        isOpening = true
        errorMsg = null
        hintMsg = null
        val tunnelRef = AtomicReference<MobileTunnelSession?>(null)

        val job = CoroutineScope(Dispatchers.IO).launch {
            val res = runCatching { client.startRdpTunnel(host.endpointId) }
            if (!active.get()) {
                res.onSuccess { sess ->
                    runCatching {
                        sess.closeTunnel()
                        sess.close()
                    }
                }
                return@launch
            }
            res.onSuccess { sess ->
                tunnelRef.set(sess)
            }
            withContext(Dispatchers.Main + NonCancellable) {
                if (!active.get()) {
                    val toClose = tunnelRef.getAndSet(null)
                    if (toClose != null) {
                        CoroutineScope(Dispatchers.IO).launch {
                            runCatching {
                                toClose.closeTunnel()
                                toClose.close()
                            }
                        }
                    }
                    return@withContext
                }
                isOpening = false
                res.onSuccess { sess ->
                    tunnelSession = sess
                }.onFailure { err ->
                    errorMsg = formatOraculoError(err)
                }
            }
        }

        onDispose {
            active.set(false)
            job.cancel()
            val toClose = tunnelRef.getAndSet(null) ?: tunnelSession
            tunnelSession = null
            if (toClose != null) {
                CoroutineScope(Dispatchers.IO).launch {
                    runCatching {
                        toClose.closeTunnel()
                        toClose.close()
                    }
                }
            }
        }
    }

    LaunchedEffect(tunnelSession) {
        val sess = tunnelSession ?: return@LaunchedEffect
        while (true) {
            delay(1000L)
            val alive = withContext(Dispatchers.IO) {
                runCatching { sess.isAlive() }.getOrDefault(false)
            }
            if (!alive) {
                if (tunnelSession === sess) {
                    tunnelSession = null
                    errorMsg = tr(
                        "Túnel RDP encerrado. Toque em Tentar novamente para reabrir.",
                        "RDP tunnel closed. Tap Try again to reopen.",
                    )
                }
                break
            }
        }
    }

    val activeTunnel = tunnelSession

    AlertDialog(
        onDismissRequest = onDismiss,
        shape = RoundedCornerShape(OraculoMetrics.CornerSurfaceExpanded),
        containerColor = OraculoColors.Surface,
        titleContentColor = OraculoColors.OnSurface,
        textContentColor = OraculoColors.OnSurface,
        title = {
            Text(
                text = tr("Login remoto (${host.hostName})", "Remote login (${host.hostName})"),
                style = MaterialTheme.typography.titleMedium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                if (isOpening) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(18.dp),
                            strokeWidth = 2.dp,
                            color = OraculoColors.Primary,
                        )
                        Text(
                            text = tr("Abrindo túnel criptografado...", "Opening encrypted tunnel..."),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                } else if (errorMsg != null) {
                    Text(
                        text = errorMsg.orEmpty(),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.Error,
                    )
                } else if (activeTunnel != null) {
                    val addr = activeTunnel.localAddress()
                    Text(
                        text = tr(
                            "Conecte no seu app RDP (Windows App ou aFreeRDP) usando este endereço local e a senha do computador:",
                            "Connect in your RDP app (Windows App or aFreeRDP) using this local address and your computer password:",
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    Surface(
                        color = OraculoColors.TerminalBackground,
                        shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                        border = BorderStroke(1.dp, OraculoColors.TerminalBorder),
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            SelectionContainer(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = addr,
                                    style = TextStyle(
                                        fontFamily = FontFamily.Monospace,
                                        fontSize = 14.sp,
                                        fontWeight = FontWeight.Medium,
                                        fontFeatureSettings = "tnum",
                                    ),
                                    color = OraculoColors.TerminalForeground,
                                )
                            }
                            IconButton(
                                onClick = {
                                    if (runCatching { clipboard.setText(AnnotatedString(addr)) }.isSuccess) {
                                        hintMsg = tr("Endereço copiado.", "Address copied.")
                                    }
                                },
                                modifier = Modifier.size(40.dp),
                            ) {
                                Icon(
                                    imageVector = Icons.Default.ContentCopy,
                                    contentDescription = tr("Copiar endereço", "Copy address"),
                                    tint = ptyxisPaletteFor(true).Ansi16[12],
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                        }
                    }
                    if (hintMsg != null) {
                        Text(
                            text = hintMsg.orEmpty(),
                            style = MaterialTheme.typography.labelSmall,
                            color = OraculoColors.Primary,
                        )
                    }
                    Text(
                        text = tr(
                            "Mantenha esta janela aberta enquanto usa o aplicativo RDP para conservar o túnel ativo.",
                            "Keep this dialog open while using your RDP app so the tunnel stays active.",
                        ),
                        style = MaterialTheme.typography.labelSmall,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                }
            }
        },
        confirmButton = {
            if (activeTunnel != null) {
                val addr = activeTunnel.localAddress()
                Button(
                    onClick = {
                        val copied = runCatching { clipboard.setText(AnnotatedString(addr)) }.isSuccess
                        val uri = Uri.parse("rdp://full%20address=s:$addr")
                        val intent = Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        runCatching { context.startActivity(intent) }.onFailure {
                            hintMsg = if (copied) {
                                tr(
                                    "Nenhum aplicativo RDP compatível respondeu ao atalho. Endereço $addr copiado — cole manualmente no Windows App ou aFreeRDP.",
                                    "No compatible RDP app handled the shortcut. $addr copied — paste it manually into Windows App or aFreeRDP.",
                                )
                            } else {
                                tr(
                                    "Nenhum aplicativo RDP compatível respondeu ao atalho. Use o endereço $addr manualmente no Windows App ou aFreeRDP.",
                                    "No compatible RDP app handled the shortcut. Use $addr manually in Windows App or aFreeRDP.",
                                )
                            }
                        }
                    },
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    colors = oraculoPrimaryButtonColors(),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp),
                    )
                    Spacer(modifier = Modifier.width(6.dp))
                    Text(tr("Abrir app RDP", "Open RDP app"))
                }
            } else if (errorMsg != null && allowRdpTunnel) {
                Button(
                    onClick = { retryNonce += 1 },
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    colors = oraculoPrimaryButtonColors(),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Tentar novamente", "Try again"))
                }
            }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                Text(
                    text = if (activeTunnel != null) tr("Encerrar túnel", "Close tunnel") else tr("Fechar", "Close"),
                    color = OraculoColors.OnSurfaceMuted,
                )
            }
        },
    )
}
