package com.oraculo.app

import android.graphics.Color as AndroidColor
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.content.Context
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.LocalIndication
import androidx.compose.material.icons.filled.BrightnessAuto
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.runtime.DisposableEffect
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.OraculoSquareIconButton
import com.oraculo.app.ui.OraculoTheme
import com.oraculo.app.ui.SleepingCatIllustration
import com.oraculo.app.ui.ThemeMode
import com.oraculo.app.ui.isOraculoDarkTheme
import com.oraculo.app.ui.oraculoPressScale
import com.oraculo.app.ui.rememberPressedInteraction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(AndroidColor.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(AndroidColor.TRANSPARENT),
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
                    onCycleTheme = {
                        val next = themeMode.next()
                        themeMode = next
                        prefs.edit().putString("theme_mode", next.prefValue).apply()
                    },
                    onToggleLang = {
                        AppI18n.toggle(prefs)
                    },
                )
            }
        }
    }
}

@Composable
private fun OraculoApp(
    themeMode: ThemeMode,
    onCycleTheme: () -> Unit,
    onToggleLang: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }

    var client by remember { mutableStateOf<OraculoMobileClient?>(null) }
    var initError by remember { mutableStateOf<String?>(null) }
    var savedHosts by remember { mutableStateOf<List<SavedHostInfo>>(emptyList()) }
    var route by remember { mutableStateOf<AppRoute>(AppRoute.DeviceList) }

    var showManualDialog by remember { mutableStateOf(false) }
    var manualUri by remember { mutableStateOf("") }
    var isPairing by remember { mutableStateOf(false) }
    var hostPendingDelete by remember { mutableStateOf<SavedHostInfo?>(null) }

    LaunchedEffect(Unit) {
        val result = withContext(Dispatchers.IO) {
            runCatching {
                val created = OraculoMobileClient(context.filesDir.absolutePath)
                created to created.listSavedHosts()
            }
        }
        result.onSuccess { (created, hosts) ->
            client = created
            savedHosts = hosts
        }.onFailure { err ->
            initError = formatOraculoError(err)
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
                route = AppRoute.HostControl(pairedHost)
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
                        text = tr("Erro ao iniciar o nó P2P", "Failed to start P2P node"),
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
                }
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    CircularProgressIndicator(
                        color = OraculoColors.Primary,
                        strokeWidth = 2.5.dp,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = tr("Iniciando nó Iroh", "Starting Iroh node"),
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                }
            }
        }
        return
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
                    themeMode = themeMode,
                    onCycleTheme = onCycleTheme,
                    onToggleLang = onToggleLang,
                    snackbarHostState = snackbarHostState,
                    onSelectHost = { host -> route = AppRoute.HostControl(host) },
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
                    route = AppRoute.DeviceList
                }
                HostControlScreen(
                    client = activeClient,
                    host = current.host,
                    onBack = { route = AppRoute.DeviceList },
                )
            }
        }
    }

    if (showManualDialog) {
        AlertDialog(
            onDismissRequest = {
                if (!isPairing) showManualDialog = false
            },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = { Text(tr("Parear via link", "Pair via URI")) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        text = tr(
                            "Cole a URI exibida pelo comando oraculo-host pair:",
                            "Paste the URI printed by oraculo-host pair:",
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                    OutlinedTextField(
                        value = manualUri,
                        onValueChange = { manualUri = it },
                        placeholder = { Text("oraculo://pair?id=...") },
                        singleLine = false,
                        maxLines = 4,
                        shape = RoundedCornerShape(14.dp),
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = OraculoColors.Primary,
                            unfocusedBorderColor = OraculoColors.Outline,
                            focusedTextColor = OraculoColors.OnSurface,
                            unfocusedTextColor = OraculoColors.OnSurface,
                        ),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                Button(
                    onClick = { pairWithUri(manualUri) },
                    enabled = manualUri.trim().startsWith("oraculo://pair?") && !isPairing,
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.Primary,
                        contentColor = OraculoColors.OnPrimary,
                    ),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(if (isPairing) tr("Pareando...", "Pairing...") else tr("Parear", "Pair"))
                }
            },
            dismissButton = {
                TextButton(
                    onClick = { showManualDialog = false },
                    enabled = !isPairing,
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
                }
            },
        )
    }

    val pendingDelete = hostPendingDelete
    if (pendingDelete != null) {
        AlertDialog(
            onDismissRequest = { hostPendingDelete = null },
            containerColor = OraculoColors.Surface,
            titleContentColor = OraculoColors.OnSurface,
            textContentColor = OraculoColors.OnSurface,
            title = { Text(tr("Remover computador?", "Remove computer?")) },
            text = {
                Text(
                    tr(
                        "Remover '${pendingDelete.hostName}' da lista salva? Você poderá parear novamente lendo o QR Code.",
                        "Remove '${pendingDelete.hostName}' from saved hosts? You can pair again later by scanning the QR Code.",
                    )
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        hostPendingDelete = null
                        scope.launch {
                            withContext(Dispatchers.IO) {
                                runCatching { activeClient.removeSavedHost(pendingDelete.endpointId) }
                            }
                            refreshHosts(activeClient)
                        }
                    },
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
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
                    shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                    modifier = Modifier.height(OraculoMetrics.ControlHeightDialog),
                ) {
                    Text(tr("Cancelar", "Cancel"), color = OraculoColors.OnSurfaceMuted)
                }
            },
        )
    }

    if (isPairing) {
        AlertDialog(
            onDismissRequest = {},
            containerColor = OraculoColors.Surface,
            title = { Text(tr("Conectando ao computador", "Connecting to computer"), color = OraculoColors.OnSurface) },
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
                        text = tr("Autenticando chaves P2P...", "Authenticating P2P keys..."),
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
    themeMode: ThemeMode,
    onCycleTheme: () -> Unit,
    onToggleLang: () -> Unit,
    snackbarHostState: SnackbarHostState,
    onSelectHost: (SavedHostInfo) -> Unit,
    onScanQr: () -> Unit,
    onOpenManualPair: () -> Unit,
    onDeleteHost: (SavedHostInfo) -> Unit,
) {
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
                    Text(
                        text = when (hosts.size) {
                            0 -> tr("Nenhum computador pareado", "No paired computers")
                            1 -> tr("1 computador salvo", "1 saved computer")
                            else -> tr("${hosts.size} computadores salvos", "${hosts.size} saved computers")
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                    )
                }

                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    val headerShape = RoundedCornerShape(OraculoMetrics.CornerSmall)
                    val (langSource, langPressed) = rememberPressedInteraction()
                    Box(
                        modifier = Modifier
                            .height(OraculoMetrics.ControlHeightSmall)
                            .oraculoPressScale(langPressed)
                            .clip(headerShape)
                            .background(OraculoColors.Surface)
                            .border(1.dp, OraculoColors.Outline, headerShape)
                            .clickable(
                                interactionSource = langSource,
                                indication = LocalIndication.current,
                                onClick = onToggleLang,
                            )
                            .padding(horizontal = 10.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = AppI18n.lang.badge,
                            style = MaterialTheme.typography.labelLarge,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 12.sp,
                            color = OraculoColors.OnSurface,
                        )
                    }
                    val (themeSource, themePressed) = rememberPressedInteraction()
                    Row(
                        modifier = Modifier
                            .height(OraculoMetrics.ControlHeightSmall)
                            .oraculoPressScale(themePressed)
                            .clip(headerShape)
                            .background(OraculoColors.Surface)
                            .border(1.dp, OraculoColors.Outline, headerShape)
                            .clickable(
                                interactionSource = themeSource,
                                indication = LocalIndication.current,
                                onClick = onCycleTheme,
                            )
                            .padding(horizontal = 11.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                    ) {
                        Icon(
                            imageVector = when (themeMode) {
                                ThemeMode.SYSTEM -> Icons.Default.BrightnessAuto
                                ThemeMode.LIGHT -> Icons.Default.LightMode
                                ThemeMode.DARK -> Icons.Default.DarkMode
                            },
                            contentDescription = tr("Alternar tema", "Toggle theme"),
                            tint = OraculoColors.OnSurfaceMuted,
                            modifier = Modifier.size(16.dp),
                        )
                        Text(
                            text = themeMode.label,
                            style = MaterialTheme.typography.labelLarge,
                            fontSize = 12.sp,
                            color = OraculoColors.OnSurface,
                        )
                    }
                    OraculoSquareIconButton(
                        onClick = onOpenManualPair,
                        containerColor = OraculoColors.Surface,
                        size = OraculoMetrics.ControlHeightSmall,
                    ) {
                        Icon(
                            imageVector = Icons.Default.Link,
                            contentDescription = tr("Colar link de pareamento", "Paste pairing link"),
                            tint = OraculoColors.OnSurfaceMuted,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(22.dp))

            if (hosts.isEmpty()) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState()),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(20.dp))
                            .background(OraculoColors.Surface)
                            .border(1.dp, OraculoColors.Outline, RoundedCornerShape(20.dp))
                            .padding(horizontal = 24.dp, vertical = 24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        SleepingCatIllustration(
                            modifier = Modifier
                                .fillMaxWidth(0.78f)
                                .widthIn(max = 224.dp),
                        )
                        Spacer(modifier = Modifier.height(10.dp))
                        Text(
                            text = tr("Nenhum computador conectado", "No computers connected"),
                            style = MaterialTheme.typography.titleMedium,
                            color = OraculoColors.OnSurface,
                            textAlign = TextAlign.Center,
                        )
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(
                            text = tr(
                                "Rode oraculo-host pair no computador e leia o QR Code.",
                                "Run oraculo-host pair on your computer and scan the QR Code.",
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
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    item {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(22.dp))
                                .background(OraculoColors.Surface)
                                .border(1.dp, OraculoColors.Outline, RoundedCornerShape(22.dp)),
                        ) {
                            hosts.forEachIndexed { index, host ->
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clickable { onSelectHost(host) }
                                        .padding(horizontal = 18.dp, vertical = 16.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .size(9.dp)
                                            .clip(CircleShape)
                                            .background(OraculoColors.Primary)
                                    )
                                    Spacer(modifier = Modifier.width(14.dp))
                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(
                                            text = host.hostName,
                                            style = MaterialTheme.typography.titleMedium,
                                            fontWeight = FontWeight.SemiBold,
                                            color = OraculoColors.OnSurface,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                        Spacer(modifier = Modifier.height(2.dp))
                                        Text(
                                            text = host.endpointId.take(14),
                                            style = MaterialTheme.typography.bodySmall,
                                            fontFamily = FontFamily.Monospace,
                                            color = OraculoColors.OnSurfaceMuted,
                                        )
                                    }
                                    OraculoSquareIconButton(
                                        onClick = { onDeleteHost(host) },
                                        size = OraculoMetrics.ControlHeightSmall,
                                    ) {
                                        Icon(
                                            imageVector = Icons.Default.DeleteOutline,
                                            contentDescription = tr("Remover ${host.hostName}", "Remove ${host.hostName}"),
                                            tint = OraculoColors.OnSurfaceMuted,
                                            modifier = Modifier.size(19.dp),
                                        )
                                    }
                                    Icon(
                                        imageVector = Icons.Default.ChevronRight,
                                        contentDescription = null,
                                        tint = OraculoColors.OnSurfaceMuted,
                                    )
                                }
                                if (index < hosts.lastIndex) {
                                    HorizontalDivider(color = OraculoColors.Outline, thickness = 1.dp)
                                }
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Button(
                    onClick = onScanQr,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = OraculoColors.Primary,
                        contentColor = OraculoColors.OnPrimary,
                    ),
                    shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
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
                        text = tr("Parear novo computador", "Pair new computer"),
                        fontSize = 15.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                }

                if (hosts.isEmpty()) {
                    OutlinedButton(
                        onClick = onOpenManualPair,
                        shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Text(
                            text = tr("Colar link manualmente", "Paste link manually"),
                            fontSize = 15.sp,
                            fontWeight = FontWeight.Medium,
                            color = OraculoColors.OnSurface,
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
    }
}
