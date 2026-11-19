package com.oraculo.app

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.SavedHostInfo
import java.util.concurrent.atomic.AtomicBoolean

internal object OraculoSessionCoordinator {
    val disconnectNonce = MutableStateFlow(0)
    val wolBridgeStoppedNonce = MutableStateFlow(0)
    val networkChangeNonce = MutableStateFlow(0)

    private val stateLock = Any()

    @Volatile
    var activeHostName: String? = null

    @Volatile
    var activeEndpointId: String? = null

    @Volatile
    var activeModeLabel: String = "Touchpad"

    @Volatile
    var activeDisconnectAction: ((String) -> Unit)? = null

    @Volatile
    var wolBridgeActive: Boolean = false

    @Volatile
    var wolBridgeDeviceLabel: String = "Android"

    @Volatile
    var wolBridgeStopAction: (() -> Unit)? = null

    fun notifyNetworkChanged() {
        networkChangeNonce.update { it + 1 }
    }

    fun setSessionContext(
        hostName: String,
        endpointId: String,
        modeLabel: String,
        disconnect: (String) -> Unit,
    ) {
        synchronized(stateLock) {
            activeHostName = hostName
            activeEndpointId = endpointId
            activeModeLabel = modeLabel
            activeDisconnectAction = disconnect
        }
    }

    fun setWolBridgeState(
        context: Context,
        active: Boolean,
        deviceLabel: String,
        stopAction: (() -> Unit)? = null,
    ) {
        val hasHostSession: Boolean
        synchronized(stateLock) {
            wolBridgeActive = active
            if (deviceLabel.isNotBlank()) {
                wolBridgeDeviceLabel = deviceLabel
            }
            wolBridgeStopAction = if (active) stopAction else null
            hasHostSession = activeEndpointId != null
        }
        if (!hasHostSession) {
            if (active) {
                OraculoSessionService.startOrUpdate(
                    context = context,
                    hostName = wolBridgeDeviceLabel,
                    modeLabel = tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active"),
                    isBridge = true,
                )
            } else {
                OraculoSessionService.stop(context)
            }
        }
    }

    fun updateMode(context: Context, host: SavedHostInfo, modeLabel: String) {
        synchronized(stateLock) {
            activeHostName = host.hostName
            activeEndpointId = host.endpointId
            activeModeLabel = modeLabel
        }
        if (OraculoSessionService.isRunning.get()) {
            OraculoSessionService.startOrUpdate(
                context = context,
                hostName = host.hostName,
                modeLabel = modeLabel,
                isBridge = false,
            )
        }
    }

    fun clearSession(context: Context, endpointId: String): Boolean {
        val cleared: Boolean
        val fallbackBridge: Boolean
        val bridgeLabel: String
        synchronized(stateLock) {
            if (activeEndpointId == endpointId) {
                activeHostName = null
                activeEndpointId = null
                activeDisconnectAction = null
                cleared = true
            } else {
                cleared = false
            }
            fallbackBridge = wolBridgeActive
            bridgeLabel = wolBridgeDeviceLabel
        }
        if (cleared) {
            if (fallbackBridge) {
                OraculoSessionService.startOrUpdate(
                    context = context,
                    hostName = bridgeLabel,
                    modeLabel = tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active"),
                    isBridge = true,
                )
            } else {
                OraculoSessionService.stop(context)
            }
        }
        return cleared
    }

    fun refreshNotificationForLanguage(context: Context) {
        if (!OraculoSessionService.isRunning.get()) return
        val hostName: String?
        val modeLabel: String
        val bridgeActive: Boolean
        val bridgeLabel: String
        synchronized(stateLock) {
            hostName = if (activeEndpointId != null) activeHostName else null
            modeLabel = activeModeLabel
            bridgeActive = wolBridgeActive
            bridgeLabel = wolBridgeDeviceLabel
        }
        when {
            hostName != null -> OraculoSessionService.startOrUpdate(
                context = context,
                hostName = hostName,
                modeLabel = modeLabel,
                isBridge = false,
            )
            bridgeActive -> OraculoSessionService.startOrUpdate(
                context = context,
                hostName = bridgeLabel,
                modeLabel = tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active"),
                isBridge = true,
            )
        }
    }

    fun disconnectNow(context: Context) {
        val endpointId: String?
        val disconnect: ((String) -> Unit)?
        val bridgeActive: Boolean
        val bridgeLabel: String
        val bridgeStop: (() -> Unit)?
        synchronized(stateLock) {
            endpointId = activeEndpointId
            disconnect = activeDisconnectAction
            bridgeActive = wolBridgeActive
            bridgeLabel = wolBridgeDeviceLabel
            bridgeStop = wolBridgeStopAction
            if (endpointId != null) {
                activeHostName = null
                activeEndpointId = null
                activeDisconnectAction = null
            } else if (bridgeActive) {
                wolBridgeActive = false
                wolBridgeStopAction = null
            }
        }

        if (endpointId != null) {
            if (disconnect != null) {
                runCatching { disconnect(endpointId) }
            }
            disconnectNonce.update { it + 1 }
            if (bridgeActive) {
                OraculoSessionService.startOrUpdate(
                    context = context,
                    hostName = bridgeLabel,
                    modeLabel = tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active"),
                    isBridge = true,
                )
            } else {
                OraculoSessionService.stop(context)
            }
            return
        }

        if (bridgeActive) {
            if (bridgeStop != null) {
                runCatching { bridgeStop() }
            } else {
                runCatching {
                    OraculoApplication.currentClientOrNull()
                        ?.setWolBridgeEnabled(false, bridgeLabel)
                }
            }
            runCatching {
                context.applicationContext
                    .getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE)
                    .edit()
                    .putBoolean("wol_bridge_enabled", false)
                    .apply()
            }
            wolBridgeStoppedNonce.update { it + 1 }
        }
        OraculoSessionService.stop(context)
    }
}

class SessionDisconnectReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent?) {
        val appContext = context.applicationContext
        val pendingResult = goAsync()
        Thread {
            try {
                OraculoSessionCoordinator.disconnectNow(appContext)
            } finally {
                pendingResult.finish()
            }
        }.start()
    }
}

class OraculoSessionService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        isRunning.set(true)
        ensureChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        ensureChannel()
        val isBridge = intent?.getBooleanExtra(EXTRA_IS_BRIDGE, false)
            ?: (OraculoSessionCoordinator.activeEndpointId == null && OraculoSessionCoordinator.wolBridgeActive)
        val hostName = intent?.getStringExtra(EXTRA_HOST_NAME)
            ?.takeIf { it.isNotBlank() }
            ?: if (isBridge) {
                OraculoSessionCoordinator.wolBridgeDeviceLabel
            } else {
                OraculoSessionCoordinator.activeHostName ?: tr("Computador", "Computer")
            }
        val modeLabel = intent?.getStringExtra(EXTRA_MODE_LABEL)
            ?.takeIf { it.isNotBlank() }
            ?: if (isBridge) {
                tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active")
            } else {
                OraculoSessionCoordinator.activeModeLabel
            }

        promoteForeground(
            buildNotification(
                hostName = hostName,
                modeLabel = modeLabel,
                isBridge = isBridge,
            ),
        )
        isRunning.set(true)
        return START_NOT_STICKY
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        Log.w(TAG, "Serviço em primeiro plano atingiu o tempo limite (type=$fgsType); encerrando.")
        isRunning.set(false)
        runCatching { ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE) }
        stopSelf(startId)
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        if (OraculoSessionCoordinator.wolBridgeActive) {
            if (OraculoSessionCoordinator.activeEndpointId != null) {
                val ep = OraculoSessionCoordinator.activeEndpointId
                if (ep != null) {
                    OraculoSessionCoordinator.clearSession(this, ep)
                }
            } else {
                ensureChannel()
                promoteForeground(
                    buildNotification(
                        hostName = OraculoSessionCoordinator.wolBridgeDeviceLabel,
                        modeLabel = tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active"),
                        isBridge = true,
                    ),
                )
            }
        } else {
            isRunning.set(false)
            runCatching { ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE) }
            stopSelf()
        }
        super.onTaskRemoved(rootIntent)
    }

    override fun onDestroy() {
        isRunning.set(false)
        super.onDestroy()
    }

    private fun promoteForeground(notification: Notification) {
        try {
            val fgsType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            } else {
                0
            }
            ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, fgsType)
        } catch (err: Exception) {
            Log.w(TAG, "Falha ao promover serviço em primeiro plano: ${err.message}")
            isRunning.set(false)
            stopSelf()
        }
    }

    private fun ensureChannel() {
        val manager = getSystemService(NotificationManager::class.java) ?: return
        val channel = NotificationChannel(
            CHANNEL_ID,
            tr("Sessão ativa", "Active session"),
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = tr(
                "Indica quando o Oráculo está conectado a um computador ou operando como ponte Wake-on-LAN.",
                "Indicates when Oráculo is connected to a computer or running as a Wake-on-LAN bridge.",
            )
            setShowBadge(false)
            enableVibration(false)
            enableLights(false)
        }
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(
        hostName: String,
        modeLabel: String,
        isBridge: Boolean,
    ): Notification {
        val openAppIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val openPendingIntent = PendingIntent.getActivity(
            this,
            0,
            openAppIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val disconnectIntent = Intent(this, SessionDisconnectReceiver::class.java)
        val disconnectPendingIntent = PendingIntent.getBroadcast(
            this,
            1,
            disconnectIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val title = if (isBridge) {
            tr("Ponte Wake-on-LAN ativa", "Wake-on-LAN Bridge active")
        } else {
            tr("Conectado a $hostName", "Connected to $hostName")
        }
        val body = if (isBridge) {
            tr("Aguardando pedidos na rede local · $hostName", "Listening on local network · $hostName")
        } else {
            tr("Sessão aberta · $modeLabel", "Active session · $modeLabel")
        }
        val actionLabel = if (isBridge) {
            tr("Desativar ponte", "Turn off bridge")
        } else {
            tr("Encerrar sessão", "Disconnect")
        }

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_oraculo)
            .setContentTitle(title)
            .setContentText(body)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setContentIntent(openPendingIntent)
            .addAction(0, actionLabel, disconnectPendingIntent)
            .build()
    }

    companion object {
        private const val TAG = "OraculoSessionSvc"
        private const val CHANNEL_ID = "oraculo_active_session"
        private const val NOTIFICATION_ID = 4107
        private const val EXTRA_HOST_NAME = "host_name"
        private const val EXTRA_MODE_LABEL = "mode_label"
        private const val EXTRA_IS_BRIDGE = "is_bridge"

        internal val isRunning = AtomicBoolean(false)

        fun canPostNotifications(context: Context): Boolean {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return true
            return ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
        }

        fun startOrUpdate(
            context: Context,
            hostName: String,
            modeLabel: String,
            isBridge: Boolean = false,
        ) {
            val appContext = context.applicationContext
            val intent = Intent(appContext, OraculoSessionService::class.java).apply {
                putExtra(EXTRA_HOST_NAME, hostName)
                putExtra(EXTRA_MODE_LABEL, modeLabel)
                putExtra(EXTRA_IS_BRIDGE, isBridge)
            }
            try {
                ContextCompat.startForegroundService(appContext, intent)
            } catch (err: Exception) {
                Log.w(TAG, "Não foi possível iniciar serviço em primeiro plano: ${err.message}")
            }
        }

        fun stop(context: Context) {
            val appContext = context.applicationContext
            isRunning.set(false)
            runCatching {
                appContext.stopService(Intent(appContext, OraculoSessionService::class.java))
            }
        }
    }
}

@Composable
internal fun HostSessionNotificationEffect(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    modeLabel: String,
    onDisconnectRequested: () -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = context as? LifecycleOwner
    val currentOnDisconnect by rememberUpdatedState(onDisconnectRequested)
    var canNotify by remember(host.endpointId) {
        mutableStateOf(OraculoSessionService.canPostNotifications(context))
    }
    var permissionRequested by remember(host.endpointId) { mutableStateOf(false) }

    val notificationPermissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission(),
    ) { granted ->
        canNotify = granted || OraculoSessionService.canPostNotifications(context)
    }

    DisposableEffect(lifecycleOwner, host.endpointId) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                canNotify = OraculoSessionService.canPostNotifications(context)
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose { owner.lifecycle.removeObserver(observer) }
    }

    val initialDisconnectNonce = remember(host.endpointId) {
        OraculoSessionCoordinator.disconnectNonce.value
    }
    val disconnectNonce by OraculoSessionCoordinator.disconnectNonce.collectAsState()

    LaunchedEffect(disconnectNonce, initialDisconnectNonce) {
        if (disconnectNonce > initialDisconnectNonce) {
            currentOnDisconnect()
        }
    }

    LaunchedEffect(host.endpointId) {
        OraculoSessionCoordinator.setSessionContext(
            hostName = host.hostName,
            endpointId = host.endpointId,
            modeLabel = modeLabel,
            disconnect = { endpointId -> runCatching { client.disconnect(endpointId) } },
        )
        if (!canNotify && !permissionRequested && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            permissionRequested = true
            notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    val currentLang = AppI18n.lang
    LaunchedEffect(host.endpointId, host.hostName, modeLabel, canNotify, currentLang) {
        OraculoSessionCoordinator.activeModeLabel = modeLabel
        OraculoSessionService.startOrUpdate(
            context = context,
            hostName = host.hostName,
            modeLabel = modeLabel,
            isBridge = false,
        )
    }

    DisposableEffect(host.endpointId) {
        onDispose {
            OraculoSessionCoordinator.clearSession(context, host.endpointId)
        }
    }
}
