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
import kotlinx.coroutines.flow.MutableStateFlow
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.SavedHostInfo
import java.util.concurrent.atomic.AtomicBoolean

internal object OraculoSessionCoordinator {
    val disconnectNonce = MutableStateFlow(0)

    @Volatile
    var activeHostName: String? = null

    @Volatile
    var activeEndpointId: String? = null

    @Volatile
    var activeModeLabel: String = "Touchpad"

    @Volatile
    var activeDisconnectAction: ((String) -> Unit)? = null

    fun setSessionContext(hostName: String, endpointId: String, modeLabel: String, disconnect: (String) -> Unit) {
        activeHostName = hostName
        activeEndpointId = endpointId
        activeModeLabel = modeLabel
        activeDisconnectAction = disconnect
    }

    fun updateMode(context: Context, host: SavedHostInfo, modeLabel: String) {
        activeHostName = host.hostName
        activeEndpointId = host.endpointId
        activeModeLabel = modeLabel
        if (OraculoSessionService.isRunning.get()) {
            OraculoSessionService.startOrUpdate(context, host.hostName, modeLabel)
        }
    }

    fun clearSession(endpointId: String): Boolean {
        if (activeEndpointId == endpointId) {
            activeHostName = null
            activeEndpointId = null
            activeDisconnectAction = null
            return true
        }
        return false
    }

    fun disconnectNow() {
        val endpointId = activeEndpointId
        val disconnect = activeDisconnectAction
        if (endpointId != null && disconnect != null) {
            runCatching { disconnect(endpointId) }
        }
        disconnectNonce.value += 1
    }
}

class SessionDisconnectReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent?) {
        OraculoSessionService.stop(context)
        val pendingResult = goAsync()
        Thread {
            try {
                OraculoSessionCoordinator.disconnectNow()
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
        promoteForeground(
            buildNotification(
                hostName = OraculoSessionCoordinator.activeHostName ?: tr("Computador", "Computer"),
                modeLabel = OraculoSessionCoordinator.activeModeLabel,
            )
        )
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        ensureChannel()
        val hostName = intent?.getStringExtra(EXTRA_HOST_NAME)
            ?.takeIf { it.isNotBlank() }
            ?: OraculoSessionCoordinator.activeHostName
            ?: tr("Computador", "Computer")
        val modeLabel = intent?.getStringExtra(EXTRA_MODE_LABEL)
            ?.takeIf { it.isNotBlank() }
            ?: OraculoSessionCoordinator.activeModeLabel

        promoteForeground(buildNotification(hostName = hostName, modeLabel = modeLabel))

        if (intent?.action == ACTION_STOP) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return START_NOT_STICKY
        }

        isRunning.set(true)
        return START_NOT_STICKY
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        isRunning.set(false)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
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
            stopSelf()
        }
    }

    private fun ensureChannel() {
        val manager = getSystemService(NotificationManager::class.java) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            tr("Sessão ativa", "Active session"),
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = tr(
                "Indica quando o Oráculo está conectado a um computador.",
                "Indicates when Oráculo is connected to a computer.",
            )
            setShowBadge(false)
            enableVibration(false)
            enableLights(false)
        }
        manager.createNotificationChannel(channel)
    }

    private fun buildNotification(hostName: String, modeLabel: String): Notification {
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

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_oraculo)
            .setContentTitle(tr("Conectado a $hostName", "Connected to $hostName"))
            .setContentText(tr("Sessão aberta · $modeLabel", "Active session · $modeLabel"))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setContentIntent(openPendingIntent)
            .addAction(0, tr("Encerrar sessão", "Disconnect"), disconnectPendingIntent)
            .build()
    }

    companion object {
        private const val TAG = "OraculoSessionSvc"
        private const val CHANNEL_ID = "oraculo_active_session"
        private const val NOTIFICATION_ID = 4107
        private const val EXTRA_HOST_NAME = "host_name"
        private const val EXTRA_MODE_LABEL = "mode_label"
        private const val ACTION_STOP = "com.oraculo.app.action.STOP_SESSION_SERVICE"

        internal val isRunning = AtomicBoolean(false)

        fun canPostNotifications(context: Context): Boolean {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return true
            return ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
        }

        fun startOrUpdate(context: Context, hostName: String, modeLabel: String) {
            if (!canPostNotifications(context)) return
            val appContext = context.applicationContext
            val intent = Intent(appContext, OraculoSessionService::class.java).apply {
                putExtra(EXTRA_HOST_NAME, hostName)
                putExtra(EXTRA_MODE_LABEL, modeLabel)
            }
            try {
                ContextCompat.startForegroundService(appContext, intent)
            } catch (err: Exception) {
                Log.w(TAG, "Não foi possível iniciar notificação persistente: ${err.message}")
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

    LaunchedEffect(host.endpointId, host.hostName, modeLabel, canNotify) {
        OraculoSessionCoordinator.activeModeLabel = modeLabel
        if (canNotify) {
            OraculoSessionService.startOrUpdate(
                context = context,
                hostName = host.hostName,
                modeLabel = modeLabel,
            )
        }
    }

    DisposableEffect(host.endpointId) {
        onDispose {
            if (OraculoSessionCoordinator.clearSession(host.endpointId)) {
                OraculoSessionService.stop(context)
            }
        }
    }
}
