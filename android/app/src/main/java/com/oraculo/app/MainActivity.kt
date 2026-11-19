package com.oraculo.app

import android.Manifest
import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color as AndroidColor
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Build
import android.os.Bundle
import android.os.CancellationSignal
import android.os.SystemClock
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
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
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.DesktopWindows
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material.icons.filled.Router
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.OraculoTheme
import com.oraculo.app.ui.JijiCatIllustration
import com.oraculo.app.ui.ThemeMode
import com.oraculo.app.ui.isOraculoDarkTheme
import com.oraculo.app.ui.oraculoPrimaryButtonColors
import com.oraculo.app.ui.oraculoSegmentedButtonColors
import com.oraculo.app.ui.oraculoSegmentedItemShape
import com.oraculo.app.ui.oraculoSwitchColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.oraculo_mobile_core.DeviceRole
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.SavedHostInfo

private sealed interface AppRoute {
    data object DeviceList : AppRoute
    data object QrScanner : AppRoute
    data class HostControl(val host: SavedHostInfo) : AppRoute
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val prefs = getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE)
        AppI18n.init(prefs)
        val initialThemeMode = ThemeMode.fromPref(prefs.getString("theme_mode", null))
        val systemIsDark =
            (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        val initialDark = when (initialThemeMode) {
            ThemeMode.SYSTEM -> systemIsDark
            ThemeMode.LIGHT -> false
            ThemeMode.DARK -> true
        }
        enableEdgeToEdge(
            statusBarStyle = if (initialDark) {
                SystemBarStyle.dark(AndroidColor.TRANSPARENT)
            } else {
                SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
            },
            navigationBarStyle = if (initialDark) {
                SystemBarStyle.dark(AndroidColor.TRANSPARENT)
            } else {
                SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
            },
        )
        setContent {
            var themeMode by remember { mutableStateOf(initialThemeMode) }
            val darkTheme = isOraculoDarkTheme(themeMode)
            DisposableEffect(darkTheme) {
                enableEdgeToEdge(
                    statusBarStyle = if (darkTheme) {
                        SystemBarStyle.dark(AndroidColor.TRANSPARENT)
                    } else {
                        SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
                    },
                    navigationBarStyle = if (darkTheme) {
                        SystemBarStyle.dark(AndroidColor.TRANSPARENT)
                    } else {
                        SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
                    },
                )
                onDispose {}
            }
            OraculoTheme(themeMode = themeMode) {
                OraculoApp(
                    themeMode = themeMode,
                    onSelectTheme = { next ->
                        themeMode = next
                        prefs.edit().putString("theme_mode", next.prefValue).apply()
                    },
                    onSelectLang = { next ->
                        AppI18n.setMode(prefs, next)
                        OraculoApplication.syncLanguage(AppI18n.lang)
                        OraculoSessionCoordinator.refreshNotificationForLanguage(this@MainActivity)
                    },
                )
            }
        }
    }
}

@Composable
private fun OraculoApp(
    themeMode: ThemeMode,
    onSelectTheme: (ThemeMode) -> Unit,
    onSelectLang: (AppLangMode) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }

    var client by remember { mutableStateOf<OraculoMobileClient?>(OraculoApplication.currentClientOrNull()) }
    var initError by remember { mutableStateOf<String?>(null) }
    var initAttempt by remember { mutableIntStateOf(0) }
    var savedHosts by remember { mutableStateOf<List<SavedHostInfo>>(emptyList()) }
    var route by remember { mutableStateOf<AppRoute>(AppRoute.DeviceList) }

    var showManualDialog by remember { mutableStateOf(false) }
    var manualUri by remember { mutableStateOf("") }
    var isPairing by remember { mutableStateOf(false) }
    var connectingHostId by remember { mutableStateOf<String?>(null) }
    var hostPendingDelete by remember { mutableStateOf<SavedHostInfo?>(null) }
    var hostPendingWol by remember { mutableStateOf<SavedHostInfo?>(null) }
    var bridgeDetailTarget by remember { mutableStateOf<SavedHostInfo?>(null) }

    val prefs = remember(context) {
        context.getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE)
    }
    var wolBridgeEnabled by remember {
        mutableStateOf(prefs.getBoolean("wol_bridge_enabled", false))
    }
    var wolBridgeBusy by remember { mutableStateOf(false) }
    var issuedBridgeCode by remember { mutableStateOf<String?>(null) }
    var appLockEnabled by remember {
        mutableStateOf(prefs.getBoolean("app_lock_enabled", true))
    }
    val keyguardManager = remember(context) {
        context.getSystemService(Context.KEYGUARD_SERVICE) as? KeyguardManager
    }
    var deviceHasScreenLock by remember(keyguardManager) {
        mutableStateOf(keyguardManager?.isDeviceSecure == true)
    }
    var sessionUnlocked by remember { mutableStateOf(false) }
    var backgroundedAtMs by remember { mutableStateOf(0L) }
    var authInProgress by remember { mutableStateOf(false) }
    var activeAuthCancel by remember { mutableStateOf<CancellationSignal?>(null) }
    var pendingCredentialSuccess by remember { mutableStateOf<(() -> Unit)?>(null) }
    var pendingCredentialCancel by remember { mutableStateOf<(() -> Unit)?>(null) }

    val notificationPermissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission(),
    ) { _ ->
        OraculoSessionCoordinator.refreshNotificationForLanguage(context)
    }

    fun requestNotificationPermissionIfNeeded() {
        if (
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            !OraculoSessionService.canPostNotifications(context)
        ) {
            runCatching {
                notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }

    val initialBridgeStoppedNonce = remember { OraculoSessionCoordinator.wolBridgeStoppedNonce.value }
    val bridgeStoppedNonce by OraculoSessionCoordinator.wolBridgeStoppedNonce.collectAsState()
    LaunchedEffect(bridgeStoppedNonce, initialBridgeStoppedNonce) {
        if (bridgeStoppedNonce > initialBridgeStoppedNonce) {
            wolBridgeEnabled = false
            issuedBridgeCode = null
        }
    }

    DisposableEffect(Unit) {
        onDispose {
            activeAuthCancel?.cancel()
            activeAuthCancel = null
        }
    }

    val confirmCredentialLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        authInProgress = false
        val action = pendingCredentialSuccess
        val cancelAction = pendingCredentialCancel
        pendingCredentialSuccess = null
        pendingCredentialCancel = null
        if (result.resultCode == Activity.RESULT_OK) {
            sessionUnlocked = true
            action?.invoke()
        } else {
            cancelAction?.invoke()
        }
    }

    fun runWithDeviceAuth(
        title: String,
        subtitle: String,
        forcePrompt: Boolean = false,
        onDismissedOrError: () -> Unit = {},
        onAuthorized: () -> Unit,
    ) {
        val isSecureNow = keyguardManager?.isDeviceSecure == true
        deviceHasScreenLock = isSecureNow
        if (!appLockEnabled || !isSecureNow) {
            sessionUnlocked = true
            onAuthorized()
            return
        }
        if (!forcePrompt && sessionUnlocked) {
            onAuthorized()
            return
        }
        if (authInProgress) return

        val activity = context as? ComponentActivity
        if (activity == null) {
            onDismissedOrError()
            return
        }

        val launchLegacyConfirm: () -> Unit = {
            @Suppress("DEPRECATION")
            val intent: Intent? = keyguardManager?.createConfirmDeviceCredentialIntent(title, subtitle)
            if (intent != null) {
                authInProgress = true
                pendingCredentialSuccess = onAuthorized
                pendingCredentialCancel = onDismissedOrError
                runCatching { confirmCredentialLauncher.launch(intent) }.onFailure {
                    authInProgress = false
                    pendingCredentialSuccess = null
                    pendingCredentialCancel = null
                    onDismissedOrError()
                }
            } else {
                sessionUnlocked = true
                onAuthorized()
            }
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val bioMgr = activity.getSystemService(BiometricManager::class.java)
            val authenticators =
                BiometricManager.Authenticators.BIOMETRIC_WEAK or
                    BiometricManager.Authenticators.DEVICE_CREDENTIAL
            if (bioMgr?.canAuthenticate(authenticators) == BiometricManager.BIOMETRIC_SUCCESS) {
                val cancelSignal = CancellationSignal()
                activeAuthCancel = cancelSignal
                authInProgress = true
                try {
                    val prompt = BiometricPrompt.Builder(activity)
                        .setTitle(title)
                        .setSubtitle(subtitle)
                        .setAllowedAuthenticators(authenticators)
                        .build()
                    prompt.authenticate(
                        cancelSignal,
                        activity.mainExecutor,
                        object : BiometricPrompt.AuthenticationCallback() {
                            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult?) {
                                activeAuthCancel = null
                                authInProgress = false
                                sessionUnlocked = true
                                onAuthorized()
                            }

                            override fun onAuthenticationError(errorCode: Int, errString: CharSequence?) {
                                activeAuthCancel = null
                                authInProgress = false
                                onDismissedOrError()
                                if (
                                    errorCode != BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED &&
                                    errorCode != BiometricPrompt.BIOMETRIC_ERROR_CANCELED &&
                                    !errString.isNullOrBlank()
                                ) {
                                    scope.launch {
                                        snackbarHostState.showSnackbar(errString.toString())
                                    }
                                }
                            }
                        },
                    )
                    return
                } catch (_: Exception) {
                    activeAuthCancel = null
                    authInProgress = false
                }
            }
        }
        launchLegacyConfirm()
    }

    val lifecycleOwner = context as? LifecycleOwner
    DisposableEffect(client, lifecycleOwner) {
        val activeClient = client ?: return@DisposableEffect onDispose {}
        val connectivityManager = context.getSystemService(ConnectivityManager::class.java)
            ?: return@DisposableEffect onDispose {}

        fun transportMask(caps: NetworkCapabilities?): Int {
            if (caps == null) return 0
            var mask = 0
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) mask = mask or 1
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR)) mask = mask or 2
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)) mask = mask or 4
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)) mask = mask or 8
            return mask
        }

        val netLock = Any()
        val initialNetwork = connectivityManager.activeNetwork
        var lastNetworkHandle: Long? = initialNetwork?.networkHandle
        var lastTransports: Int? = initialNetwork?.let {
            transportMask(connectivityManager.getNetworkCapabilities(it))
        }
        var sawInitialCallback = false

        fun notifyChange(handle: Long?, transports: Int?) {
            lastNetworkHandle = handle
            lastTransports = transports
            runCatching { activeClient.notifyNetworkChange() }
            OraculoSessionCoordinator.notifyNetworkChanged()
        }

        val networkCallback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                val handle = network.networkHandle
                val transports = transportMask(connectivityManager.getNetworkCapabilities(network))
                synchronized(netLock) {
                    if (!sawInitialCallback) {
                        sawInitialCallback = true
                        if (
                            lastNetworkHandle != null &&
                            handle == lastNetworkHandle &&
                            (lastTransports == null || transports == 0 || transports == lastTransports)
                        ) {
                            return
                        }
                    }
                    if (handle != lastNetworkHandle || (transports != 0 && transports != lastTransports)) {
                        notifyChange(handle, transports)
                    }
                }
            }

            override fun onCapabilitiesChanged(network: Network, networkCapabilities: NetworkCapabilities) {
                val handle = network.networkHandle
                val transports = transportMask(networkCapabilities)
                synchronized(netLock) {
                    if (
                        handle == lastNetworkHandle &&
                        transports != 0 &&
                        transports != lastTransports
                    ) {
                        notifyChange(handle, transports)
                    }
                }
            }

            override fun onLost(network: Network) {
                val handle = network.networkHandle
                synchronized(netLock) {
                    if (handle == lastNetworkHandle) {
                        lastNetworkHandle = null
                        lastTransports = null
                    }
                }
            }
        }

        val registered = runCatching {
            connectivityManager.registerDefaultNetworkCallback(networkCallback)
        }.isSuccess

        val resumeObserver = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_START) {
                val current = connectivityManager.activeNetwork ?: return@LifecycleEventObserver
                val curHandle = current.networkHandle
                val curTransports = transportMask(connectivityManager.getNetworkCapabilities(current))
                synchronized(netLock) {
                    if (
                        curHandle != lastNetworkHandle ||
                        (curTransports != 0 && curTransports != lastTransports)
                    ) {
                        notifyChange(curHandle, curTransports)
                    }
                }
            }
        }
        lifecycleOwner?.lifecycle?.addObserver(resumeObserver)

        onDispose {
            lifecycleOwner?.lifecycle?.removeObserver(resumeObserver)
            if (registered) {
                runCatching { connectivityManager.unregisterNetworkCallback(networkCallback) }
            }
        }
    }

    DisposableEffect(lifecycleOwner, appLockEnabled, route) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_STOP -> {
                    if (!authInProgress) {
                        backgroundedAtMs = SystemClock.elapsedRealtime()
                    }
                }
                Lifecycle.Event.ON_START -> {
                    deviceHasScreenLock = keyguardManager?.isDeviceSecure == true
                    val awaySince = backgroundedAtMs
                    backgroundedAtMs = 0L
                    if (
                        !authInProgress &&
                        appLockEnabled &&
                        deviceHasScreenLock &&
                        awaySince > 0L &&
                        (SystemClock.elapsedRealtime() - awaySince) >= 45_000L
                    ) {
                        sessionUnlocked = false
                    }
                }
                Lifecycle.Event.ON_RESUME -> {
                    deviceHasScreenLock = keyguardManager?.isDeviceSecure == true
                }
                else -> Unit
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose {
            owner.lifecycle.removeObserver(observer)
        }
    }

    LaunchedEffect(initAttempt) {
        val deviceLabel = Build.MODEL.takeIf { it.isNotBlank() } ?: "Android"
        val wantBridge = prefs.getBoolean("wol_bridge_enabled", false)
        val result = withContext(Dispatchers.IO) {
            runCatching {
                val created = OraculoApplication.getOrCreateClient(context)
                created.warmUp()
                if (wantBridge && !created.isWolBridgeEnabled()) {
                    runCatching { created.setWolBridgeEnabled(true, deviceLabel) }
                }
                Triple(created, created.listSavedHosts(), created.isWolBridgeEnabled())
            }
        }
        result.onSuccess { (created, hosts, bridgeRunning) ->
            initError = null
            client = created
            savedHosts = hosts
            wolBridgeEnabled = bridgeRunning
            if (bridgeRunning) {
                OraculoSessionCoordinator.setWolBridgeState(
                    context = context,
                    active = true,
                    deviceLabel = deviceLabel,
                    stopAction = { runCatching { created.setWolBridgeEnabled(false, deviceLabel) } },
                )
            } else {
                OraculoSessionCoordinator.setWolBridgeState(
                    context = context,
                    active = false,
                    deviceLabel = deviceLabel,
                )
            }
        }.onFailure { err ->
            initError = formatOraculoError(err)
        }
    }

    fun toggleWolBridge(enabled: Boolean) {
        val active = client ?: return
        if (wolBridgeBusy) return
        val deviceLabel = Build.MODEL.takeIf { it.isNotBlank() } ?: "Android"
        val previous = wolBridgeEnabled
        wolBridgeBusy = true
        wolBridgeEnabled = enabled
        prefs.edit().putBoolean("wol_bridge_enabled", enabled).apply()
        if (enabled) {
            requestNotificationPermissionIfNeeded()
        }
        scope.launch {
            val res = withContext(Dispatchers.IO) {
                runCatching { active.setWolBridgeEnabled(enabled, deviceLabel) }
            }
            wolBridgeBusy = false
            res.onSuccess {
                if (enabled) {
                    OraculoSessionCoordinator.setWolBridgeState(
                        context = context,
                        active = true,
                        deviceLabel = deviceLabel,
                        stopAction = { runCatching { active.setWolBridgeEnabled(false, deviceLabel) } },
                    )
                } else {
                    issuedBridgeCode = null
                    OraculoSessionCoordinator.setWolBridgeState(
                        context = context,
                        active = false,
                        deviceLabel = deviceLabel,
                    )
                }
            }.onFailure { err ->
                wolBridgeEnabled = previous
                prefs.edit().putBoolean("wol_bridge_enabled", previous).apply()
                snackbarHostState.showSnackbar(formatOraculoError(err))
            }
        }
    }

    fun generateWolBridgeCode() {
        val active = client ?: return
        if (wolBridgeBusy) return
        val deviceLabel = Build.MODEL.takeIf { it.isNotBlank() } ?: "Android"
        requestNotificationPermissionIfNeeded()
        wolBridgeBusy = true
        scope.launch {
            val res = withContext(Dispatchers.IO) {
                runCatching { active.issueWolBridgeCode(deviceLabel) }
            }
            wolBridgeBusy = false
            res.onSuccess { code ->
                wolBridgeEnabled = true
                prefs.edit().putBoolean("wol_bridge_enabled", true).apply()
                OraculoSessionCoordinator.setWolBridgeState(
                    context = context,
                    active = true,
                    deviceLabel = deviceLabel,
                    stopAction = { runCatching { active.setWolBridgeEnabled(false, deviceLabel) } },
                )
                issuedBridgeCode = code
            }.onFailure { err ->
                snackbarHostState.showSnackbar(formatOraculoError(err))
            }
        }
    }

    fun refreshHosts(activeClient: OraculoMobileClient) {
        scope.launch {
            val hosts = withContext(Dispatchers.IO) {
                runCatching { activeClient.listSavedHosts() }.getOrDefault(emptyList())
            }
            savedHosts = hosts
        }
    }

    fun pairWithUri(rawUri: String) {
        val activeClient = client ?: return
        val trimmed = rawUri.trim()
        if (trimmed.isEmpty() || isPairing) return

        scope.launch {
            isPairing = true
            val deviceName = Build.MODEL.takeIf { it.isNotBlank() } ?: "Android"
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val host = activeClient.pairByUri(trimmed, deviceName)
                    val list = activeClient.listSavedHosts()
                    host to list
                }
            }
            isPairing = false
            result.onSuccess { (pairedHost, hosts) ->
                savedHosts = hosts
                showManualDialog = false
                manualUri = ""
                if (pairedHost.role == DeviceRole.WOL_BRIDGE) {
                    route = AppRoute.DeviceList
                    snackbarHostState.showSnackbar(
                        tr(
                            "Ponte adicionada para ligar seus computadores remotamente.",
                            "Bridge added to wake your computers remotely.",
                        ),
                    )
                } else {
                    sessionUnlocked = true
                    route = AppRoute.HostControl(pairedHost)
                }
            }.onFailure { err ->
                route = AppRoute.DeviceList
                snackbarHostState.showSnackbar(
                    tr(
                        "Falha no pareamento: ${formatOraculoError(err)}",
                        "Pairing failed: ${formatOraculoError(err)}",
                    ),
                )
            }
        }
    }

    val activeClient = client
    if (activeClient == null) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(OraculoColors.Background)
                .statusBarsPadding()
                .navigationBarsPadding()
                .padding(24.dp),
            contentAlignment = Alignment.Center,
        ) {
            if (initError != null) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        text = tr("Não foi possível iniciar a conexão", "Could not start connection"),
                        style = MaterialTheme.typography.titleMedium,
                        color = OraculoColors.Error,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = initError.orEmpty(),
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(modifier = Modifier.height(20.dp))
                    Button(
                        onClick = {
                            initError = null
                            initAttempt += 1
                        },
                        colors = oraculoPrimaryButtonColors(),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                    ) {
                        Text(tr("Tentar novamente", "Try again"))
                    }
                }
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    CircularProgressIndicator(
                        color = OraculoColors.Primary,
                        strokeWidth = 2.5.dp,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = tr("Preparando conexão...", "Preparing connection..."),
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                }
            }
        }
        return
    }

    fun leaveHostControl(hostId: String, remoteReason: String? = null) {
        route = AppRoute.DeviceList
        val fallbackHosts = savedHosts
        scope.launch {
            val hosts = withContext(Dispatchers.IO) {
                runCatching { activeClient.disconnectHost(hostId) }
                runCatching { activeClient.listSavedHosts() }.getOrDefault(fallbackHosts)
            }
            savedHosts = hosts
            if (!remoteReason.isNullOrBlank()) {
                snackbarHostState.showSnackbar(remoteReason)
            }
        }
    }

    fun openHostIfReachable(host: SavedHostInfo) {
        if (host.role == DeviceRole.WOL_BRIDGE) {
            bridgeDetailTarget = host
            return
        }
        if (connectingHostId != null) return
        runWithDeviceAuth(
            title = tr("Confirmar identidade", "Verify your identity"),
            subtitle = tr(
                "Desbloqueie para conectar a ${host.hostName}",
                "Unlock to connect to ${host.hostName}",
            ),
        ) {
            if (connectingHostId != null) return@runWithDeviceAuth
            connectingHostId = host.endpointId
            scope.launch {
                val res = withContext(Dispatchers.IO) {
                    runCatching {
                        val ping = activeClient.connectAndVerify(host.endpointId, 14000uL)
                        val updatedList = activeClient.listSavedHosts()
                        val synced = updatedList.firstOrNull { it.endpointId == host.endpointId } ?: host
                        Triple(ping, synced, updatedList)
                    }
                }
                connectingHostId = null
                res.onSuccess { (ping, synced, updatedList) ->
                    savedHosts = updatedList
                    if (ping.role == DeviceRole.WOL_BRIDGE || synced.role == DeviceRole.WOL_BRIDGE) {
                        bridgeDetailTarget = synced
                        snackbarHostState.showSnackbar(
                            tr(
                                "${synced.hostName} foi identificado como Ponte Wake-on-LAN.",
                                "${synced.hostName} was identified as a Wake-on-LAN bridge.",
                            ),
                        )
                    } else {
                        route = AppRoute.HostControl(synced)
                    }
                }.onFailure { err ->
                    val rawMsg = formatOraculoError(err)
                    val isRevoked = rawMsg.startsWith("[401]") || rawMsg.startsWith("[403]")
                    val cleanMsg = rawMsg.replace(Regex("^\\[\\d+]\\s*"), "")
                    if (isRevoked) {
                        snackbarHostState.showSnackbar(
                            tr(
                                "Acesso removido no computador. Pareie novamente.",
                                "Access removed on computer. Please pair again.",
                            ),
                        )
                    } else if (!host.wolMac.isNullOrBlank()) {
                        val snackRes = snackbarHostState.showSnackbar(
                            message = tr(
                                "${host.hostName} desligado ou desconectado.",
                                "${host.hostName} is offline or disconnected.",
                            ),
                            actionLabel = tr("Ligar", "Wake"),
                        )
                        if (snackRes == SnackbarResult.ActionPerformed) {
                            hostPendingWol = host
                        }
                    } else {
                        snackbarHostState.showSnackbar(
                            if (cleanMsg.isNotBlank()) {
                                cleanMsg
                            } else {
                                tr(
                                    "${host.hostName} indisponível. Abra o Oráculo no computador para conectar.",
                                    "${host.hostName} unavailable. Open Oráculo on your computer to connect.",
                                )
                            },
                        )
                    }
                }
            }
        }
    }

    AnimatedContent(
        targetState = route,
        transitionSpec = {
            val forward = initialState is AppRoute.DeviceList && targetState !is AppRoute.DeviceList
            if (forward) {
                (
                    slideInHorizontally(tween(OraculoMotion.RouteEnterMs, easing = OraculoMotion.Easing)) { it / 7 } +
                        fadeIn(tween(OraculoMotion.PanelEnterMs, easing = OraculoMotion.Easing))
                    ) togetherWith (
                    slideOutHorizontally(tween(OraculoMotion.RouteExitMs, easing = OraculoMotion.Easing)) { -it / 10 } +
                        fadeOut(tween(OraculoMotion.PanelExitMs, easing = OraculoMotion.Easing))
                    )
            } else {
                (
                    slideInHorizontally(tween(OraculoMotion.RouteEnterMs, easing = OraculoMotion.Easing)) { -it / 8 } +
                        fadeIn(tween(OraculoMotion.PanelEnterMs, easing = OraculoMotion.Easing))
                    ) togetherWith (
                    slideOutHorizontally(tween(OraculoMotion.RouteExitMs, easing = OraculoMotion.Easing)) { it / 8 } +
                        fadeOut(tween(OraculoMotion.PanelExitMs, easing = OraculoMotion.Easing))
                    )
            }
        },
        contentKey = { state ->
            when (state) {
                AppRoute.DeviceList -> "list"
                AppRoute.QrScanner -> "qr"
                is AppRoute.HostControl -> "host_${state.host.endpointId}"
            }
        },
        label = "appRoute",
    ) { current ->
        when (current) {
            is AppRoute.DeviceList -> {
                DeviceListScreen(
                    hosts = savedHosts,
                    connectingHostId = connectingHostId,
                    themeMode = themeMode,
                    wolBridgeEnabled = wolBridgeEnabled,
                    appLockEnabled = appLockEnabled,
                    deviceHasScreenLock = deviceHasScreenLock,
                    onSelectTheme = onSelectTheme,
                    onSelectLang = onSelectLang,
                    onToggleWolBridge = ::toggleWolBridge,
                    onToggleAppLock = { wantEnabled ->
                        if (wantEnabled) {
                            appLockEnabled = true
                            prefs.edit().putBoolean("app_lock_enabled", true).apply()
                        } else {
                            runWithDeviceAuth(
                                title = tr("Desativar bloqueio", "Disable app lock"),
                                subtitle = tr(
                                    "Confirme sua identidade para desativar a proteção",
                                    "Confirm your identity to disable protection",
                                ),
                                forcePrompt = true,
                            ) {
                                appLockEnabled = false
                                prefs.edit().putBoolean("app_lock_enabled", false).apply()
                            }
                        }
                    },
                    onIssueBridgeCode = ::generateWolBridgeCode,
                    snackbarHostState = snackbarHostState,
                    onSelectHost = ::openHostIfReachable,
                    onInspectBridge = { bridge -> bridgeDetailTarget = bridge },
                    onWakeHost = { host ->
                        runWithDeviceAuth(
                            title = tr("Confirmar identidade", "Verify your identity"),
                            subtitle = tr(
                                "Desbloqueie para ligar ${host.hostName}",
                                "Unlock to wake ${host.hostName}",
                            ),
                        ) {
                            hostPendingWol = host
                        }
                    },
                    onScanQr = { route = AppRoute.QrScanner },
                    onOpenManualPair = { showManualDialog = true },
                    onDeleteHost = { host -> hostPendingDelete = host },
                )
            }

            is AppRoute.QrScanner -> {
                BackHandler {
                    route = AppRoute.DeviceList
                }
                QrScannerScreen(
                    onQrScanned = { scanned -> pairWithUri(scanned) },
                    onOpenManualInput = {
                        route = AppRoute.DeviceList
                        showManualDialog = true
                    },
                    onBack = { route = AppRoute.DeviceList },
                )
            }

            is AppRoute.HostControl -> {
                BackHandler {
                    leaveHostControl(current.host.endpointId, null)
                }
                val isSessionLocked = appLockEnabled && deviceHasScreenLock && !sessionUnlocked
                Box(modifier = Modifier.fillMaxSize()) {
                    HostControlScreen(
                        client = activeClient,
                        host = current.host,
                        bridges = savedHosts.filter { it.role == DeviceRole.WOL_BRIDGE },
                        onBack = {
                            leaveHostControl(current.host.endpointId, null)
                        },
                        onRemoteDisconnect = { reason ->
                            leaveHostControl(current.host.endpointId, reason)
                        },
                    )
                    if (isSessionLocked) {
                        LaunchedEffect(current.host.endpointId, isSessionLocked) {
                            runWithDeviceAuth(
                                title = tr("Sessão bloqueada", "Session locked"),
                                subtitle = tr(
                                    "Desbloqueie para retomar ${current.host.hostName}",
                                    "Unlock to resume ${current.host.hostName}",
                                ),
                            ) {}
                        }
                        OraculoSessionLockOverlay(
                            hostName = current.host.hostName,
                            isAuthenticating = authInProgress,
                            onUnlock = {
                                runWithDeviceAuth(
                                    title = tr("Sessão bloqueada", "Session locked"),
                                    subtitle = tr(
                                        "Desbloqueie para retomar ${current.host.hostName}",
                                        "Unlock to resume ${current.host.hostName}",
                                    ),
                                    forcePrompt = true,
                                ) {}
                            },
                            onLeave = { leaveHostControl(current.host.endpointId, null) },
                        )
                    }
                }
            }
        }
    }

    if (showManualDialog) {
        val trimmedInput = manualUri.trim()
        val isUriInput = trimmedInput.startsWith("oraculo://", ignoreCase = true)
        val cleanCodeChars = trimmedInput.filter { it != '-' && !it.isWhitespace() }.uppercase()
        val hasInvalidCodeChar =
            !isUriInput &&
                cleanCodeChars.isNotEmpty() &&
                cleanCodeChars.any { it !in "23456789ABCDEFGHJKLMNPQRSTUVWXYZ" }
        val isShortCodeValid =
            cleanCodeChars.length == 8 && !hasInvalidCodeChar

        AlertDialog(
            onDismissRequest = {
                if (!isPairing) showManualDialog = false
            },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = { Text(tr("Parear com código", "Pair with code")) },
            text = {
                Column(
                    modifier = Modifier.verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = tr(
                            "Digite o código de 8 caracteres mostrado no computador/ponte, ou cole um link oraculo://:",
                            "Enter the 8-character code shown on your computer/bridge, or paste an oraculo:// link:",
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    OutlinedTextField(
                        value = manualUri,
                        onValueChange = { next ->
                            manualUri = if (
                                next.trimStart().startsWith("oraculo:", ignoreCase = true) ||
                                    next.contains("://")
                            ) {
                                next
                            } else {
                                val deletedHyphen =
                                    manualUri.length == next.length + 1 &&
                                        manualUri.getOrNull(4) == '-' &&
                                        next.length >= 4 &&
                                        next.getOrNull(4) != '-' &&
                                        manualUri.removeRange(4, 5) == next
                                val base = if (deletedHyphen) {
                                    next.removeRange(3, 4)
                                } else {
                                    next
                                }
                                val alnum = base
                                    .filter { it.isLetterOrDigit() }
                                    .uppercase()
                                    .take(8)
                                if (alnum.length > 4) {
                                    "${alnum.substring(0, 4)}-${alnum.substring(4)}"
                                } else {
                                    alnum
                                }
                            }
                        },
                        isError = hasInvalidCodeChar,
                        supportingText = if (hasInvalidCodeChar) {
                            {
                                Text(
                                    text = tr(
                                        "Códigos não usam 0, 1, I ou O (evita confusão visual).",
                                        "Codes never use 0, 1, I, or O (avoids lookalikes).",
                                    ),
                                    color = OraculoColors.Error,
                                )
                            }
                        } else {
                            null
                        },
                        placeholder = { Text("K7P4-M9X2") },
                        singleLine = !isUriInput,
                        maxLines = if (isUriInput) 4 else 1,
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Characters,
                            keyboardType = KeyboardType.Ascii,
                        ),
                        textStyle = MaterialTheme.typography.titleMedium.copy(
                            fontFamily = FontFamily.Monospace,
                            fontFeatureSettings = "tnum",
                            letterSpacing = if (isUriInput) 0.sp else 1.5.sp,
                        ),
                        shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = OraculoColors.Primary,
                            unfocusedBorderColor = OraculoColors.Outline,
                            errorBorderColor = OraculoColors.Error,
                            focusedTextColor = OraculoColors.OnSurface,
                            unfocusedTextColor = OraculoColors.OnSurface,
                        ),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                val isSupportedUri =
                    trimmedInput.startsWith("oraculo://pair?", ignoreCase = true) ||
                        trimmedInput.startsWith("oraculo://mcu-bridge?", ignoreCase = true)
                val pairEnabled = (isSupportedUri || isShortCodeValid) && !isPairing
                Button(
                    onClick = { pairWithUri(manualUri) },
                    enabled = pairEnabled,
                    colors = oraculoPrimaryButtonColors(),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(if (isPairing) tr("Pareando...", "Pairing...") else tr("Parear", "Pair"))
                }
            },
            dismissButton = {
                TextButton(
                    onClick = { showManualDialog = false },
                    enabled = !isPairing,
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
                }
            },
        )
    }

    val pendingDelete = hostPendingDelete
    if (pendingDelete != null) {
        val isBridge = pendingDelete.role == DeviceRole.WOL_BRIDGE
        AlertDialog(
            onDismissRequest = { hostPendingDelete = null },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = {
                Text(
                    if (isBridge) {
                        tr("Remover ponte?", "Remove bridge?")
                    } else {
                        tr("Remover computador?", "Remove computer?")
                    },
                )
            },
            text = {
                Text(
                    tr(
                        "Remover '${pendingDelete.hostName}'? Você poderá parear de novo quando quiser.",
                        "Remove '${pendingDelete.hostName}'? You can pair again anytime.",
                    ),
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        hostPendingDelete = null
                        if (bridgeDetailTarget?.endpointId == pendingDelete.endpointId) {
                            bridgeDetailTarget = null
                        }
                        scope.launch {
                            withContext(Dispatchers.IO) {
                                runCatching { activeClient.removeSavedHost(pendingDelete.endpointId) }
                            }
                            refreshHosts(activeClient)
                        }
                    },
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.Error,
                        contentColor = OraculoColors.OnError,
                    ),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Remover", "Remove"))
                }
            },
            dismissButton = {
                TextButton(
                    onClick = { hostPendingDelete = null },
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
                }
            },
        )
    }

    val bridgeTarget = bridgeDetailTarget
    if (bridgeTarget != null) {
        val isMcuBridge = bridgeTarget.endpointId.startsWith("mcu:")
        val displayBridgeId = if (isMcuBridge) {
            bridgeTarget.endpointId.removePrefix("mcu:")
        } else {
            bridgeTarget.endpointId
        }
        var testingBridge by remember(bridgeTarget.endpointId) { mutableStateOf(false) }
        var bridgeTestMsg by remember(bridgeTarget.endpointId) { mutableStateOf<String?>(null) }
        var bridgeTestOk by remember(bridgeTarget.endpointId) { mutableStateOf<Boolean?>(null) }

        AlertDialog(
            onDismissRequest = { if (!testingBridge) bridgeDetailTarget = null },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = {
                Text(
                    text = bridgeTarget.hostName,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            },
            text = {
                Column(
                    modifier = Modifier.verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = if (isMcuBridge) {
                            tr(
                                "Ponte física (Cardputer / ESP32) via MQTT TLS para acordar seus computadores pela internet.",
                                "Hardware bridge (Cardputer / ESP32) over MQTT TLS used to wake your computers over the internet.",
                            )
                        } else {
                            tr(
                                "Ponte de rede dedicada para acordar seus computadores pela internet.",
                                "Dedicated network bridge used to wake your computers over the internet.",
                            )
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    if (isMcuBridge && !bridgeTarget.relayUrl.isNullOrBlank()) {
                        Text(
                            text = tr("Broker MQTT TLS", "MQTT TLS Broker"),
                            style = MaterialTheme.typography.labelMedium,
                            color = OraculoColors.OnSurfaceMuted,
                        )
                        Text(
                            text = bridgeTarget.relayUrl.orEmpty(),
                            style = MaterialTheme.typography.bodySmall,
                            fontFamily = FontFamily.Monospace,
                            color = OraculoColors.OnSurface,
                        )
                    }
                    Text(
                        text = if (isMcuBridge) {
                            tr("Identificador da ponte (Bridge ID)", "Bridge identifier (Bridge ID)")
                        } else {
                            tr("Identificador (EndpointId)", "Identifier (EndpointId)")
                        },
                        style = MaterialTheme.typography.labelMedium,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    Surface(
                        color = OraculoColors.SurfaceRecessed,
                        shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                        border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        SelectionContainer {
                            Text(
                                text = displayBridgeId,
                                style = MaterialTheme.typography.bodySmall,
                                fontFamily = FontFamily.Monospace,
                                color = OraculoColors.OnSurface,
                                modifier = Modifier.padding(12.dp),
                            )
                        }
                    }
                    if (bridgeTestMsg != null) {
                        Text(
                            text = bridgeTestMsg.orEmpty(),
                            style = MaterialTheme.typography.bodySmall,
                            color = when (bridgeTestOk) {
                                true -> OraculoColors.Success
                                false -> OraculoColors.Error
                                null -> OraculoColors.OnSurfaceMuted
                            },
                        )
                    }
                }
            },
            confirmButton = {
                Button(
                    onClick = {
                        if (testingBridge) return@Button
                        testingBridge = true
                        bridgeTestOk = null
                        bridgeTestMsg = tr("Testando conexão...", "Testing connection...")
                        scope.launch {
                            val res = withContext(Dispatchers.IO) {
                                runCatching { activeClient.ping(bridgeTarget.endpointId) }
                            }
                            testingBridge = false
                            res.onSuccess { ping ->
                                bridgeTestOk = true
                                bridgeTestMsg = tr(
                                    "Ponte online (${ping.rttMs} ms) e pronta para ligar seus computadores.",
                                    "Bridge online (${ping.rttMs} ms) and ready to wake your computers.",
                                )
                                refreshHosts(activeClient)
                            }.onFailure { err ->
                                bridgeTestOk = false
                                bridgeTestMsg = tr(
                                    "Ponte offline ou inacessível: ${formatOraculoError(err)}",
                                    "Bridge offline or unreachable: ${formatOraculoError(err)}",
                                )
                            }
                        }
                    },
                    enabled = !testingBridge,
                    colors = oraculoPrimaryButtonColors(),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(
                        if (testingBridge) {
                            tr("Testando...", "Testing...")
                        } else {
                            tr("Testar conexão", "Test connection")
                        },
                    )
                }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(
                        onClick = { hostPendingDelete = bridgeTarget },
                        enabled = !testingBridge,
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                    ) {
                        Text(tr("Remover", "Remove"), color = OraculoColors.Error)
                    }
                    TextButton(
                        onClick = { bridgeDetailTarget = null },
                        enabled = !testingBridge,
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                    ) {
                        Text(tr("Fechar", "Close"), color = OraculoColors.OnSurfaceMuted)
                    }
                }
            },
        )
    }

    val pendingWol = hostPendingWol
    if (pendingWol != null) {
        WakeOnLanDialog(
            client = activeClient,
            host = pendingWol,
            bridges = savedHosts.filter { it.role == DeviceRole.WOL_BRIDGE },
            onHostUpdated = { updated ->
                hostPendingWol = updated
                refreshHosts(activeClient)
            },
            onHostOnline = { onlineHost ->
                hostPendingWol = null
                refreshHosts(activeClient)
                route = AppRoute.HostControl(onlineHost)
            },
            onDismiss = { hostPendingWol = null },
        )
    }

    val bridgeCode = issuedBridgeCode
    if (bridgeCode != null) {
        AlertDialog(
            onDismissRequest = { issuedBridgeCode = null },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = { Text(tr("Código da Ponte Wake-on-LAN", "Wake-on-LAN Bridge Code")) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        text = tr(
                            "Mantenha este aparelho no Wi-Fi de casa e digite este código no celular principal em \"Código\" ou no menu \"Parear com código\" (válido por 3 min):",
                            "Keep this device on your home Wi-Fi and enter this code on your main phone under \"Code\" or \"Pair with code\" in the menu (valid for 3 min):",
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    Surface(
                        color = OraculoColors.SecondaryContainer,
                        shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                        border = BorderStroke(1.dp, OraculoColors.Outline),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            text = bridgeCode,
                            style = MaterialTheme.typography.headlineSmall,
                            fontFamily = FontFamily.Monospace,
                            fontWeight = FontWeight.Bold,
                            letterSpacing = 2.sp,
                            color = OraculoColors.OnSecondaryContainer,
                            textAlign = TextAlign.Center,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 16.dp),
                        )
                    }
                }
            },
            confirmButton = {
                Button(
                    onClick = { issuedBridgeCode = null },
                    colors = oraculoPrimaryButtonColors(),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Pronto", "Done"))
                }
            },
        )
    }

    if (isPairing) {
        AlertDialog(
            onDismissRequest = {},
            containerColor = OraculoColors.Surface,
            title = { Text(tr("Conectando ao dispositivo", "Connecting to device"), color = OraculoColors.OnSurface) },
            text = {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    CircularProgressIndicator(
                        color = OraculoColors.Primary,
                        modifier = Modifier.size(26.dp),
                        strokeWidth = 2.5.dp,
                    )
                    Text(
                        text = tr("Verificando acesso...", "Verifying access..."),
                        color = OraculoColors.OnSurfaceMuted,
                    )
                }
            },
            confirmButton = {},
        )
    }
}

@Composable
private fun DeviceListScreen(
    hosts: List<SavedHostInfo>,
    connectingHostId: String?,
    themeMode: ThemeMode,
    wolBridgeEnabled: Boolean,
    appLockEnabled: Boolean,
    deviceHasScreenLock: Boolean,
    onSelectTheme: (ThemeMode) -> Unit,
    onSelectLang: (AppLangMode) -> Unit,
    onToggleWolBridge: (Boolean) -> Unit,
    onToggleAppLock: (Boolean) -> Unit,
    onIssueBridgeCode: () -> Unit,
    snackbarHostState: SnackbarHostState,
    onSelectHost: (SavedHostInfo) -> Unit,
    onInspectBridge: (SavedHostInfo) -> Unit,
    onWakeHost: (SavedHostInfo) -> Unit,
    onScanQr: () -> Unit,
    onOpenManualPair: () -> Unit,
    onDeleteHost: (SavedHostInfo) -> Unit,
) {
    var showMenuSheet by remember { mutableStateOf(false) }
    val computers = remember(hosts) { hosts.filter { it.role == DeviceRole.COMPUTER } }
    val bridges = remember(hosts) { hosts.filter { it.role == DeviceRole.WOL_BRIDGE } }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(OraculoColors.Background)
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 20.dp),
        ) {
            Spacer(modifier = Modifier.height(14.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.Top,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Oráculo",
                        style = MaterialTheme.typography.displaySmall,
                        color = OraculoColors.OnSurface,
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    val summaryLabel = when {
                        computers.isEmpty() && bridges.isEmpty() ->
                            tr("Nenhum computador salvo", "No saved computers")
                        computers.isEmpty() && bridges.isNotEmpty() -> when (bridges.size) {
                            1 -> tr("1 ponte de rede ativa", "1 active network bridge")
                            else -> tr("${bridges.size} pontes de rede ativas", "${bridges.size} active network bridges")
                        }
                        bridges.isEmpty() -> when (computers.size) {
                            1 -> tr("1 computador salvo", "1 saved computer")
                            else -> tr("${computers.size} computadores salvos", "${computers.size} saved computers")
                        }
                        else -> {
                            val pcPart = if (computers.size == 1) {
                                tr("1 computador", "1 computer")
                            } else {
                                tr("${computers.size} computadores", "${computers.size} computers")
                            }
                            val bridgePart = if (bridges.size == 1) {
                                tr("1 ponte", "1 bridge")
                            } else {
                                tr("${bridges.size} pontes", "${bridges.size} bridges")
                            }
                            "$pcPart · $bridgePart"
                        }
                    }
                    Text(
                        text = summaryLabel,
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                }

                IconButton(onClick = { showMenuSheet = true }) {
                    Icon(
                        imageVector = Icons.Default.Menu,
                        contentDescription = tr("Abrir menu", "Open menu"),
                        tint = OraculoColors.OnSurface,
                    )
                }
            }

            if (wolBridgeEnabled) {
                Spacer(modifier = Modifier.height(14.dp))
                Surface(
                    color = OraculoColors.SecondaryContainer,
                    contentColor = OraculoColors.OnSecondaryContainer,
                    shape = RoundedCornerShape(OraculoMetrics.CornerSurface),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 14.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Default.Router,
                                contentDescription = null,
                                tint = OraculoColors.OnSecondaryContainer,
                                modifier = Modifier.size(22.dp),
                            )
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = tr(
                                        "Modo ponte ativo neste aparelho",
                                        "Bridge mode active on this device",
                                    ),
                                    style = MaterialTheme.typography.titleSmall,
                                    color = OraculoColors.OnSecondaryContainer,
                                )
                                Text(
                                    text = tr(
                                        "Pronto no Wi-Fi local para ligar seus computadores",
                                        "Ready on local Wi-Fi to wake your computers",
                                    ),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = OraculoColors.OnSecondaryContainer.copy(alpha = 0.85f),
                                )
                            }
                        }
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Button(
                                onClick = onIssueBridgeCode,
                                colors = oraculoPrimaryButtonColors(),
                                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
                                modifier = Modifier
                                    .weight(1f)
                                    .heightIn(min = OraculoMetrics.ControlHeightSmall),
                            ) {
                                Text(
                                    text = tr("Código", "Code"),
                                    fontSize = 13.sp,
                                    fontWeight = FontWeight.Medium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            OutlinedButton(
                                onClick = { onToggleWolBridge(false) },
                                border = BorderStroke(
                                    1.dp,
                                    OraculoColors.OnSecondaryContainer.copy(alpha = 0.35f),
                                ),
                                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
                                modifier = Modifier
                                    .weight(1f)
                                    .heightIn(min = OraculoMetrics.ControlHeightSmall),
                            ) {
                                Text(
                                    text = tr("Desativar", "Turn off"),
                                    fontSize = 13.sp,
                                    fontWeight = FontWeight.Medium,
                                    color = OraculoColors.OnSecondaryContainer,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            if (computers.isEmpty() && bridges.isEmpty()) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState()),
                    contentAlignment = Alignment.Center,
                ) {
                    val cardShape = RoundedCornerShape(OraculoMetrics.CornerSurfaceExpanded)
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(cardShape)
                            .background(OraculoColors.Surface)
                            .let { mod ->
                                if (OraculoColors.IsDark) mod else mod.border(1.dp, OraculoColors.Outline, cardShape)
                            }
                            .padding(horizontal = 24.dp, vertical = 32.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        JijiCatIllustration(
                            modifier = Modifier
                                .fillMaxWidth(0.88f)
                                .widthIn(max = 264.dp),
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = tr("Nenhum computador salvo", "No saved computers"),
                            style = MaterialTheme.typography.titleMedium,
                            color = OraculoColors.OnSurface,
                            textAlign = TextAlign.Center,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = tr(
                                "Abra o Oráculo no computador (ou rode oraculo-host serve --pair) e leia o QR Code.",
                                "Open Oráculo on your computer (or run oraculo-host serve --pair) and scan the QR Code.",
                            ),
                            style = MaterialTheme.typography.bodyMedium,
                            color = OraculoColors.OnSurfaceMuted,
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            } else {
                LazyColumn(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    if (computers.isNotEmpty() && bridges.isNotEmpty()) {
                        item(key = "header_computers") {
                            Text(
                                text = tr("Computadores", "Computers"),
                                style = MaterialTheme.typography.labelLarge,
                                color = OraculoColors.OnSurfaceMuted,
                                modifier = Modifier.padding(start = 4.dp, bottom = 5.dp),
                            )
                        }
                    }

                    if (computers.isEmpty() && bridges.isNotEmpty()) {
                        item(key = "empty_computers_hint") {
                            Surface(
                                color = OraculoColors.Surface,
                                shape = RoundedCornerShape(OraculoMetrics.CornerSurface),
                                border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(bottom = 8.dp),
                            ) {
                                Text(
                                    text = tr(
                                        "Nenhum computador salvo ainda. Pareie seu computador abaixo para ligar ou controlar à distância.",
                                        "No saved computers yet. Pair your computer below to wake or control it remotely.",
                                    ),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = OraculoColors.OnSurfaceMuted,
                                    modifier = Modifier.padding(16.dp),
                                )
                            }
                        }
                    }

                    itemsIndexed(computers, key = { _, host -> "pc_${host.endpointId}" }) { index, host ->
                        val isThisConnecting = connectingHostId == host.endpointId
                        val itemShape = remember(index, computers.size) {
                            oraculoSegmentedItemShape(index, computers.size)
                        }
                        Surface(
                            onClick = { onSelectHost(host) },
                            enabled = connectingHostId == null,
                            color = OraculoColors.Surface,
                            shape = itemShape,
                            border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 16.dp, vertical = 14.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Box(
                                    modifier = Modifier
                                        .size(40.dp)
                                        .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                                        .background(OraculoColors.SecondaryContainer),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Icon(
                                        imageVector = Icons.Default.DesktopWindows,
                                        contentDescription = null,
                                        tint = OraculoColors.OnSecondaryContainer,
                                        modifier = Modifier.size(20.dp),
                                    )
                                }
                                Spacer(modifier = Modifier.width(14.dp))
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = host.hostName,
                                        style = MaterialTheme.typography.titleMedium,
                                        color = OraculoColors.OnSurface,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                    Spacer(modifier = Modifier.height(2.dp))
                                    Text(
                                        text = if (!host.wolMac.isNullOrBlank()) {
                                            "${host.endpointId.take(12)} · WoL"
                                        } else {
                                            host.endpointId.take(14)
                                        },
                                        style = MaterialTheme.typography.bodySmall,
                                        fontFamily = FontFamily.Monospace,
                                        color = OraculoColors.OnSurfaceMuted,
                                    )
                                }
                                IconButton(onClick = { onWakeHost(host) }) {
                                    Icon(
                                        imageVector = Icons.Default.PowerSettingsNew,
                                        contentDescription = tr("Ligar ${host.hostName}", "Wake ${host.hostName}"),
                                        tint = if (!host.wolMac.isNullOrBlank()) {
                                            OraculoColors.Primary
                                        } else {
                                            OraculoColors.OnSurfaceMuted
                                        },
                                    )
                                }
                                IconButton(onClick = { onDeleteHost(host) }) {
                                    Icon(
                                        imageVector = Icons.Default.DeleteOutline,
                                        contentDescription = tr("Remover ${host.hostName}", "Remove ${host.hostName}"),
                                        tint = OraculoColors.OnSurfaceMuted,
                                    )
                                }
                                if (isThisConnecting) {
                                    CircularProgressIndicator(
                                        modifier = Modifier.size(18.dp),
                                        strokeWidth = 2.dp,
                                        color = OraculoColors.Primary,
                                    )
                                } else {
                                    Icon(
                                        imageVector = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                                        contentDescription = null,
                                        tint = OraculoColors.OnSurfaceMuted,
                                    )
                                }
                            }
                        }
                    }

                    if (bridges.isNotEmpty()) {
                        item(key = "header_bridges") {
                            Text(
                                text = tr("Pontes de rede", "Network bridges"),
                                style = MaterialTheme.typography.labelLarge,
                                color = OraculoColors.OnSurfaceMuted,
                                modifier = Modifier.padding(
                                    start = 4.dp,
                                    top = if (computers.isNotEmpty()) 14.dp else 4.dp,
                                    bottom = 5.dp,
                                ),
                            )
                        }

                        itemsIndexed(bridges, key = { _, bridge -> "br_${bridge.endpointId}" }) { index, bridge ->
                            val itemShape = remember(index, bridges.size) {
                                oraculoSegmentedItemShape(index, bridges.size)
                            }
                            Surface(
                                onClick = { onInspectBridge(bridge) },
                                enabled = connectingHostId == null,
                                color = OraculoColors.Surface,
                                shape = itemShape,
                                border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = 16.dp, vertical = 14.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(40.dp)
                                            .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                                            .background(OraculoColors.SurfaceInteractive),
                                        contentAlignment = Alignment.Center,
                                    ) {
                                        Icon(
                                            imageVector = Icons.Default.Router,
                                            contentDescription = null,
                                            tint = OraculoColors.OnSurface,
                                            modifier = Modifier.size(20.dp),
                                        )
                                    }
                                    Spacer(modifier = Modifier.width(14.dp))
                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(
                                            text = bridge.hostName,
                                            style = MaterialTheme.typography.titleMedium,
                                            color = OraculoColors.OnSurface,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                        Spacer(modifier = Modifier.height(2.dp))
                                        val isMcu = bridge.endpointId.startsWith("mcu:")
                                        val bridgeSubtitle = if (isMcu) {
                                            val broker = bridge.relayUrl?.takeIf { it.isNotBlank() }
                                                ?: bridge.endpointId.removePrefix("mcu:")
                                            "MQTT TLS · $broker"
                                        } else {
                                            tr(
                                                "Ponte WoL · ${bridge.endpointId.take(12)}",
                                                "WoL bridge · ${bridge.endpointId.take(12)}",
                                            )
                                        }
                                        Text(
                                            text = bridgeSubtitle,
                                            style = MaterialTheme.typography.bodySmall,
                                            fontFamily = FontFamily.Monospace,
                                            color = OraculoColors.OnSurfaceMuted,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                    }
                                    IconButton(onClick = { onDeleteHost(bridge) }) {
                                        Icon(
                                            imageVector = Icons.Default.DeleteOutline,
                                            contentDescription = tr("Remover ${bridge.hostName}", "Remove ${bridge.hostName}"),
                                            tint = OraculoColors.OnSurfaceMuted,
                                        )
                                    }
                                    Icon(
                                        imageVector = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                                        contentDescription = null,
                                        tint = OraculoColors.OnSurfaceMuted,
                                    )
                                }
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(14.dp))

            if (hosts.isEmpty()) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Button(
                        onClick = onScanQr,
                        colors = oraculoPrimaryButtonColors(),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Icon(
                            imageVector = Icons.Default.QrCodeScanner,
                            contentDescription = null,
                            modifier = Modifier.size(20.dp),
                        )
                        Spacer(modifier = Modifier.width(10.dp))
                        Text(
                            text = tr("Parear dispositivo", "Pair device"),
                            fontSize = 15.sp,
                            fontWeight = FontWeight.Medium,
                        )
                    }

                    OutlinedButton(
                        onClick = onOpenManualPair,
                        border = BorderStroke(1.dp, OraculoColors.Outline),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Text(
                            text = tr("Digitar código de pareamento", "Enter pairing code"),
                            fontSize = 15.sp,
                            fontWeight = FontWeight.Medium,
                            color = OraculoColors.OnSurface,
                        )
                    }
                }
            } else {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Button(
                        onClick = onScanQr,
                        colors = oraculoPrimaryButtonColors(),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier
                            .weight(1.35f)
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Icon(
                            imageVector = Icons.Default.QrCodeScanner,
                            contentDescription = null,
                            modifier = Modifier.size(19.dp),
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = tr("Parear dispositivo", "Pair device"),
                            fontSize = 14.sp,
                            fontWeight = FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }

                    OutlinedButton(
                        onClick = onOpenManualPair,
                        border = BorderStroke(1.dp, OraculoColors.Outline),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier
                            .weight(1f)
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Icon(
                            imageVector = Icons.Default.Link,
                            contentDescription = null,
                            tint = OraculoColors.OnSurface,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(modifier = Modifier.width(6.dp))
                        Text(
                            text = tr("Código", "Code"),
                            fontSize = 14.sp,
                            fontWeight = FontWeight.Medium,
                            color = OraculoColors.OnSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }

        SnackbarHost(
            hostState = snackbarHostState,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 86.dp),
        )

        if (showMenuSheet) {
            OraculoSettingsSheet(
                themeMode = themeMode,
                langMode = AppI18n.mode,
                wolBridgeEnabled = wolBridgeEnabled,
                appLockEnabled = appLockEnabled,
                deviceHasScreenLock = deviceHasScreenLock,
                onSelectTheme = onSelectTheme,
                onSelectLang = onSelectLang,
                onToggleWolBridge = onToggleWolBridge,
                onToggleAppLock = onToggleAppLock,
                onIssueBridgeCode = {
                    showMenuSheet = false
                    onIssueBridgeCode()
                },
                onOpenManualPair = {
                    showMenuSheet = false
                    onOpenManualPair()
                },
                onDismiss = { showMenuSheet = false },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun OraculoSettingsSheet(
    themeMode: ThemeMode,
    langMode: AppLangMode,
    wolBridgeEnabled: Boolean,
    appLockEnabled: Boolean,
    deviceHasScreenLock: Boolean,
    onSelectTheme: (ThemeMode) -> Unit,
    onSelectLang: (AppLangMode) -> Unit,
    onToggleWolBridge: (Boolean) -> Unit,
    onToggleAppLock: (Boolean) -> Unit,
    onIssueBridgeCode: () -> Unit,
    onOpenManualPair: () -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val context = LocalContext.current
    val versionName = remember(context) { readAppVersionName(context) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = OraculoColors.Surface,
        contentColor = OraculoColors.OnSurface,
        shape = RoundedCornerShape(
            topStart = OraculoMetrics.CornerSurfaceExpanded,
            topEnd = OraculoMetrics.CornerSurfaceExpanded,
        ),
        dragHandle = {
            BottomSheetDefaults.DragHandle(color = OraculoColors.Outline)
        },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = tr("Configurações", "Settings"),
                style = MaterialTheme.typography.titleLarge,
                color = OraculoColors.OnSurface,
            )

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = tr("Aparência", "Appearance"),
                    style = MaterialTheme.typography.labelLarge,
                    color = OraculoColors.OnSurfaceMuted,
                )
                OraculoSegmentedSelector(
                    items = ThemeMode.entries,
                    selected = themeMode,
                    labelOf = { it.label },
                    onSelect = onSelectTheme,
                )
            }

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = tr("Idioma", "Language"),
                    style = MaterialTheme.typography.labelLarge,
                    color = OraculoColors.OnSurfaceMuted,
                )
                OraculoSegmentedSelector(
                    items = AppLangMode.entries,
                    selected = langMode,
                    labelOf = { it.label },
                    onSelect = onSelectLang,
                )
            }

            HorizontalDivider(thickness = 1.dp, color = OraculoColors.Outline)

            ListItem(
                headlineContent = {
                    Text(
                        text = tr("Bloqueio por biometria ou PIN", "Biometric or PIN lock"),
                        style = MaterialTheme.typography.titleMedium,
                        fontSize = 15.sp,
                    )
                },
                supportingContent = {
                    Text(
                        text = if (deviceHasScreenLock) {
                            tr(
                                "Exige desbloqueio do aparelho ao abrir um computador salvo",
                                "Requires device unlock before opening a saved computer",
                            )
                        } else {
                            tr(
                                "Ative um PIN ou biometria nas configurações do Android para usar",
                                "Set a PIN or biometric lock in Android settings to enable",
                            )
                        },
                        style = MaterialTheme.typography.bodySmall,
                    )
                },
                leadingContent = {
                    Icon(
                        imageVector = Icons.Default.Fingerprint,
                        contentDescription = null,
                        tint = if (appLockEnabled && deviceHasScreenLock) OraculoColors.Primary else OraculoColors.OnSurfaceMuted,
                    )
                },
                trailingContent = {
                    Switch(
                        checked = appLockEnabled && deviceHasScreenLock,
                        enabled = deviceHasScreenLock,
                        onCheckedChange = onToggleAppLock,
                        colors = oraculoSwitchColors(),
                    )
                },
                colors = ListItemDefaults.colors(
                    containerColor = Color.Transparent,
                    headlineColor = OraculoColors.OnSurface,
                    supportingColor = OraculoColors.OnSurfaceMuted,
                ),
            )

            ListItem(
                headlineContent = {
                    Text(
                        text = tr("Modo Ponte Wake-on-LAN", "Wake-on-LAN Bridge Mode"),
                        style = MaterialTheme.typography.titleMedium,
                        fontSize = 15.sp,
                    )
                },
                supportingContent = {
                    Text(
                        text = tr(
                            "Mantém este aparelho no Wi-Fi de casa para ligar seus computadores à distância",
                            "Keeps this device on home Wi-Fi to wake your computers remotely",
                        ),
                        style = MaterialTheme.typography.bodySmall,
                    )
                },
                leadingContent = {
                    Icon(
                        imageVector = Icons.Default.PowerSettingsNew,
                        contentDescription = null,
                        tint = if (wolBridgeEnabled) OraculoColors.Primary else OraculoColors.OnSurfaceMuted,
                    )
                },
                trailingContent = {
                    Switch(
                        checked = wolBridgeEnabled,
                        onCheckedChange = onToggleWolBridge,
                        colors = oraculoSwitchColors(),
                    )
                },
                colors = ListItemDefaults.colors(
                    containerColor = Color.Transparent,
                    headlineColor = OraculoColors.OnSurface,
                    supportingColor = OraculoColors.OnSurfaceMuted,
                ),
            )

            if (wolBridgeEnabled) {
                ListItem(
                    headlineContent = {
                        Text(
                            text = tr("Gerar código desta ponte", "Generate bridge pairing code"),
                            style = MaterialTheme.typography.bodyLarge,
                            fontSize = 15.sp,
                        )
                    },
                    supportingContent = {
                        Text(
                            text = tr(
                                "Autoriza seu celular principal a usar esta ponte",
                                "Authorize your main phone to use this bridge",
                            ),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    },
                    leadingContent = {
                        Icon(
                            imageVector = Icons.Default.Router,
                            contentDescription = null,
                            tint = OraculoColors.Primary,
                        )
                    },
                    trailingContent = {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                            contentDescription = null,
                        )
                    },
                    colors = ListItemDefaults.colors(
                        containerColor = Color.Transparent,
                        headlineColor = OraculoColors.Primary,
                        supportingColor = OraculoColors.OnSurfaceMuted,
                        trailingIconColor = OraculoColors.Primary,
                    ),
                    modifier = Modifier
                        .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                        .clickable(onClick = onIssueBridgeCode),
                )
            }

            ListItem(
                headlineContent = {
                    Text(
                        text = tr("Parear com código", "Pair with code"),
                        style = MaterialTheme.typography.titleMedium,
                        fontSize = 15.sp,
                    )
                },
                supportingContent = {
                    Text(
                        text = tr("Ex.: K7P4-M9X2", "e.g. K7P4-M9X2"),
                        style = MaterialTheme.typography.bodySmall,
                        fontFamily = FontFamily.Monospace,
                    )
                },
                leadingContent = {
                    Icon(
                        imageVector = Icons.Default.Link,
                        contentDescription = null,
                        tint = OraculoColors.Primary,
                    )
                },
                trailingContent = {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.KeyboardArrowRight,
                        contentDescription = null,
                    )
                },
                colors = ListItemDefaults.colors(
                    containerColor = Color.Transparent,
                    headlineColor = OraculoColors.OnSurface,
                    supportingColor = OraculoColors.OnSurfaceMuted,
                    trailingIconColor = OraculoColors.OnSurfaceMuted,
                ),
                modifier = Modifier
                    .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                    .clickable(onClick = onOpenManualPair),
            )

            ListItem(
                headlineContent = {
                    Text(
                        text = tr("Versão do app", "App version"),
                        style = MaterialTheme.typography.bodyLarge,
                        fontSize = 15.sp,
                    )
                },
                leadingContent = {
                    Icon(
                        imageVector = Icons.Outlined.Info,
                        contentDescription = null,
                        tint = OraculoColors.OnSurfaceMuted,
                    )
                },
                trailingContent = {
                    Text(
                        text = "Oráculo v$versionName",
                        style = MaterialTheme.typography.bodyMedium,
                        fontFamily = FontFamily.Monospace,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                },
                colors = ListItemDefaults.colors(
                    containerColor = Color.Transparent,
                    headlineColor = OraculoColors.OnSurface,
                ),
            )
        }
    }
}

@Composable
private fun OraculoSessionLockOverlay(
    hostName: String,
    isAuthenticating: Boolean,
    onUnlock: () -> Unit,
    onLeave: () -> Unit,
) {
    Surface(
        modifier = Modifier
            .fillMaxSize()
            .background(OraculoColors.Background)
            .clickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
            ) {}
            .statusBarsPadding()
            .navigationBarsPadding(),
        color = OraculoColors.Background,
        contentColor = OraculoColors.OnSurface,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .clip(RoundedCornerShape(OraculoMetrics.CornerSurface))
                    .background(OraculoColors.SecondaryContainer),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = Icons.Default.Lock,
                    contentDescription = null,
                    tint = OraculoColors.OnSecondaryContainer,
                    modifier = Modifier.size(30.dp),
                )
            }
            Spacer(modifier = Modifier.height(20.dp))
            Text(
                text = tr("Sessão bloqueada", "Session locked"),
                style = MaterialTheme.typography.titleLarge,
                color = OraculoColors.OnSurface,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = tr(
                    "Confirme sua biometria ou PIN para continuar controlando $hostName.",
                    "Confirm your biometric or PIN lock to resume controlling $hostName.",
                ),
                style = MaterialTheme.typography.bodyMedium,
                color = OraculoColors.OnSurfaceMuted,
                textAlign = TextAlign.Center,
            )
            Spacer(modifier = Modifier.height(24.dp))
            Button(
                onClick = onUnlock,
                enabled = !isAuthenticating,
                colors = oraculoPrimaryButtonColors(),
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightCta),
            ) {
                if (isAuthenticating) {
                    CircularProgressIndicator(
                        color = OraculoColors.OnPrimaryAction,
                        strokeWidth = 2.dp,
                        modifier = Modifier.size(18.dp),
                    )
                } else {
                    Icon(
                        imageVector = Icons.Default.Fingerprint,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                }
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = if (isAuthenticating) {
                        tr("Aguardando desbloqueio...", "Waiting for unlock...")
                    } else {
                        tr("Desbloquear", "Unlock")
                    },
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Medium,
                )
            }
            Spacer(modifier = Modifier.height(10.dp))
            TextButton(onClick = onLeave) {
                Text(
                    text = tr("Voltar aos computadores", "Back to computers"),
                    color = OraculoColors.OnSurfaceMuted,
                )
            }
        }
    }
}

@Composable
private fun <T> OraculoSegmentedSelector(
    items: List<T>,
    selected: T,
    labelOf: (T) -> String,
    onSelect: (T) -> Unit,
) {
    SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
        items.forEachIndexed { index, item ->
            val isSelected = item == selected
            SegmentedButton(
                selected = isSelected,
                onClick = { onSelect(item) },
                shape = SegmentedButtonDefaults.itemShape(index = index, count = items.size),
                colors = oraculoSegmentedButtonColors(),
                icon = { SegmentedButtonDefaults.Icon(active = isSelected) },
                label = {
                    Text(
                        text = labelOf(item),
                        style = MaterialTheme.typography.labelMedium,
                        maxLines = 1,
                        softWrap = false,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
            )
        }
    }
}

private fun readAppVersionName(context: Context): String = runCatching {
    val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        context.packageManager.getPackageInfo(
            context.packageName,
            PackageManager.PackageInfoFlags.of(0),
        )
    } else {
        @Suppress("DEPRECATION")
        context.packageManager.getPackageInfo(context.packageName, 0)
    }
    info.versionName?.takeIf { it.isNotBlank() } ?: "0.2.0-beta.1"
}.getOrDefault("0.2.0-beta.1")
