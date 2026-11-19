package com.oraculo.app

import android.os.SystemClock
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.oraculoPrimaryButtonColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.SavedHostInfo

private const val BOOT_WAIT_TOTAL_SEC = 55

@Composable
internal fun WakeOnLanDialog(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    bridges: List<SavedHostInfo> = emptyList(),
    onHostUpdated: (SavedHostInfo) -> Unit,
    onHostOnline: (SavedHostInfo) -> Unit,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val currentOnHostUpdated by rememberUpdatedState(onHostUpdated)
    val currentOnHostOnline by rememberUpdatedState(onHostOnline)
    var currentHost by remember(host.endpointId) { mutableStateOf(host) }
    var isEditing by remember(host.endpointId) { mutableStateOf(host.wolMac.isNullOrBlank()) }

    var macInput by remember(currentHost.wolMac) { mutableStateOf(currentHost.wolMac.orEmpty()) }
    var targetInput by remember(currentHost.wolTargetOverride) {
        mutableStateOf(currentHost.wolTargetOverride.orEmpty())
    }
    var portInput by remember(currentHost.wolPort) {
        mutableStateOf(currentHost.wolPort.toInt().toString())
    }
    var showAdvanced by remember(host.endpointId) {
        mutableStateOf(!host.wolTargetOverride.isNullOrBlank() || host.wolPort.toInt() != 9)
    }

    var isBusy by remember { mutableStateOf(false) }
    var isWaitingBoot by remember { mutableStateOf(false) }
    var bootWaitGeneration by remember { mutableIntStateOf(0) }
    var elapsedSec by remember { mutableIntStateOf(0) }
    var relayedVia by remember { mutableStateOf<List<String>>(emptyList()) }
    var errorText by remember { mutableStateOf<String?>(null) }
    var statusText by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(isWaitingBoot, bootWaitGeneration) {
        if (!isWaitingBoot) return@LaunchedEffect
        val myGen = bootWaitGeneration
        val startedMs = SystemClock.elapsedRealtime()
        elapsedSec = 0

        val tickerJob = launch {
            while (isWaitingBoot && myGen == bootWaitGeneration) {
                delay(1000L)
                val secs = ((SystemClock.elapsedRealtime() - startedMs) / 1000L).toInt()
                elapsedSec = secs.coerceAtMost(BOOT_WAIT_TOTAL_SEC)
                if (secs >= BOOT_WAIT_TOTAL_SEC) break
            }
        }

        delay(1500L)
        while (myGen == bootWaitGeneration && isWaitingBoot) {
            val nowElapsed = ((SystemClock.elapsedRealtime() - startedMs) / 1000L).toInt()
            if (nowElapsed >= BOOT_WAIT_TOTAL_SEC) break

            val onlineHost = withContext(Dispatchers.IO) {
                if (client.probeHostOnline(currentHost.endpointId, 8000uL)) {
                    runCatching {
                        client.listSavedHosts().firstOrNull { it.endpointId == currentHost.endpointId }
                    }.getOrNull() ?: currentHost
                } else {
                    null
                }
            }
            if (onlineHost != null && myGen == bootWaitGeneration) {
                tickerJob.cancel()
                isWaitingBoot = false
                currentHost = onlineHost
                currentOnHostUpdated(onlineHost)
                currentOnHostOnline(onlineHost)
                return@LaunchedEffect
            }
            delay(900L)
        }

        tickerJob.cancel()
        if (myGen == bootWaitGeneration && isWaitingBoot) {
            isWaitingBoot = false
            statusText = tr(
                "Sinal enviado, mas o computador ainda não respondeu. No Wi-Fi funciona em suspensão; se estiver desligado, requer cabo de rede e o serviço ativo ao iniciar.",
                "Signal sent, but the computer hasn't responded yet. Wi-Fi wakes from Sleep; full power-off requires wired Ethernet and launch at startup.",
            )
        }
    }

    fun triggerWake(targetHost: SavedHostInfo = currentHost) {
        if (isBusy) return
        val wasWaiting = isWaitingBoot
        scope.launch {
            isBusy = true
            errorText = null
            statusText = null
            val result = withContext(Dispatchers.IO) {
                runCatching { client.wakeHost(targetHost.endpointId) }
            }
            isBusy = false
            result.onSuccess { dispatch ->
                relayedVia = dispatch.relayedVia
                bootWaitGeneration++
                isWaitingBoot = true
            }.onFailure { err ->
                if (!wasWaiting) {
                    isWaitingBoot = false
                }
                errorText = formatOraculoError(err)
            }
        }
    }

    fun saveConfiguration(wakeAfterSave: Boolean) {
        if (isBusy) return
        val parsedPort = portInput.trim().toIntOrNull() ?: 9
        if (parsedPort !in 1..65535) {
            errorText = tr(
                "Informe uma porta UDP válida (1 a 65535).",
                "Enter a valid UDP port (1 to 65535).",
            )
            return
        }
        scope.launch {
            isBusy = true
            errorText = null
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    client.updateHostWol(
                        currentHost.endpointId,
                        macInput.trim().ifEmpty { null },
                        targetInput.trim().ifEmpty { null },
                        parsedPort.toUShort(),
                    )
                }
            }
            isBusy = false
            result.onSuccess { updated ->
                currentHost = updated
                onHostUpdated(updated)
                if (updated.wolMac.isNullOrBlank()) {
                    onDismiss()
                } else {
                    isEditing = false
                    if (wakeAfterSave) {
                        triggerWake(updated)
                    }
                }
            }.onFailure { err ->
                errorText = formatOraculoError(err)
            }
        }
    }

    AlertDialog(
        onDismissRequest = {
            if (!isBusy) {
                isWaitingBoot = false
                onDismiss()
            }
        },
        containerColor = OraculoColors.Surface,
        titleContentColor = OraculoColors.OnSurface,
        textContentColor = OraculoColors.OnSurface,
        title = {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.PowerSettingsNew,
                    contentDescription = null,
                    tint = OraculoColors.Primary,
                    modifier = Modifier.size(22.dp),
                )
                Text(
                    text = if (isEditing) {
                        tr("Configurar Wake-on-LAN", "Configure Wake-on-LAN")
                    } else {
                        tr("Ligar ${currentHost.hostName}", "Wake ${currentHost.hostName}")
                    },
                    style = MaterialTheme.typography.titleLarge,
                    fontSize = 19.sp,
                )
            }
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (isEditing) {
                    if (currentHost.wolMac.isNullOrBlank()) {
                        Text(
                            text = tr(
                                "Conecte-se uma vez com o computador ligado para preencher o MAC automaticamente, ou informe abaixo:",
                                "Connect once while your computer is on to fill in the MAC automatically, or enter it below:",
                            ),
                            style = MaterialTheme.typography.bodySmall,
                            color = OraculoColors.OnSurfaceMuted,
                        )
                    }

                    OutlinedTextField(
                        value = macInput,
                        onValueChange = {
                            macInput = it.uppercase()
                            errorText = null
                        },
                        label = { Text(tr("Endereço MAC da placa", "NIC MAC address")) },
                        placeholder = { Text("AA:BB:CC:DD:EE:FF") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(
                            capitalization = KeyboardCapitalization.Characters,
                            keyboardType = KeyboardType.Ascii,
                        ),
                        textStyle = MaterialTheme.typography.bodyMedium.copy(
                            fontFamily = FontFamily.Monospace,
                        ),
                        shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = OraculoColors.Primary,
                            unfocusedBorderColor = OraculoColors.Outline,
                            focusedTextColor = OraculoColors.OnSurface,
                            unfocusedTextColor = OraculoColors.OnSurface,
                        ),
                        modifier = Modifier.fillMaxWidth(),
                    )

                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                            .clickable { showAdvanced = !showAdvanced }
                            .padding(horizontal = 4.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(
                            text = tr(
                                "Opções avançadas (DDNS e porta)",
                                "Advanced options (DDNS & port)",
                            ),
                            style = MaterialTheme.typography.labelLarge,
                            color = OraculoColors.Primary,
                        )
                        Icon(
                            imageVector = if (showAdvanced) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                            contentDescription = null,
                            tint = OraculoColors.Primary,
                        )
                    }

                    AnimatedVisibility(
                        visible = showAdvanced,
                        enter = OraculoMotion.ExpandV,
                        exit = OraculoMotion.CollapseV,
                    ) {
                        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            OutlinedTextField(
                                value = targetInput,
                                onValueChange = {
                                    targetInput = it
                                    errorText = null
                                },
                                label = { Text(tr("IP / DDNS externo (opcional)", "Custom IP / DDNS (optional)")) },
                                placeholder = {
                                    Text(currentHost.wolBroadcast ?: tr("meu-pc.duckdns.org", "my-pc.duckdns.org"))
                                },
                                singleLine = true,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                                textStyle = MaterialTheme.typography.bodyMedium.copy(
                                    fontFamily = FontFamily.Monospace,
                                ),
                                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                                colors = OutlinedTextFieldDefaults.colors(
                                    focusedBorderColor = OraculoColors.Primary,
                                    unfocusedBorderColor = OraculoColors.Outline,
                                    focusedTextColor = OraculoColors.OnSurface,
                                    unfocusedTextColor = OraculoColors.OnSurface,
                                ),
                                modifier = Modifier.fillMaxWidth(),
                            )

                            OutlinedTextField(
                                value = portInput,
                                onValueChange = { next ->
                                    if (next.length <= 5 && next.all { it.isDigit() }) {
                                        portInput = next
                                        errorText = null
                                    }
                                },
                                label = { Text(tr("Porta UDP", "UDP Port")) },
                                placeholder = { Text("9") },
                                singleLine = true,
                                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                                textStyle = MaterialTheme.typography.bodyMedium.copy(
                                    fontFamily = FontFamily.Monospace,
                                ),
                                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                                colors = OutlinedTextFieldDefaults.colors(
                                    focusedBorderColor = OraculoColors.Primary,
                                    unfocusedBorderColor = OraculoColors.Outline,
                                    focusedTextColor = OraculoColors.OnSurface,
                                    unfocusedTextColor = OraculoColors.OnSurface,
                                ),
                                modifier = Modifier.fillMaxWidth(),
                            )

                            Text(
                                text = tr(
                                    "Na mesma rede Wi-Fi, deixe o IP/DDNS em branco. No Linux, ative o Wake-on-LAN nas configurações do Oráculo no computador ou rode 'oraculo-host wol --enable'.",
                                    "On the same Wi-Fi, leave IP/DDNS blank. On Linux, enable Wake-on-LAN in Oráculo on your computer or run 'oraculo-host wol --enable'.",
                                ),
                                style = MaterialTheme.typography.bodySmall,
                                color = OraculoColors.OnSurfaceMuted,
                            )
                        }
                    }
                } else {
                    val infoShape = RoundedCornerShape(OraculoMetrics.CornerMedium)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(infoShape)
                            .background(OraculoColors.SurfaceRecessed)
                            .let { mod ->
                                if (OraculoColors.IsDark) mod else mod.border(1.dp, OraculoColors.Outline, infoShape)
                            }
                            .padding(start = 14.dp, end = 6.dp, top = 10.dp, bottom = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            val extraCount = currentHost.wolExtraMacs.size
                            val extraSuffix = if (extraCount > 0) " (+$extraCount)" else ""
                            Text(
                                text = "${currentHost.wolMac.orEmpty()}$extraSuffix",
                                style = MaterialTheme.typography.bodyMedium,
                                fontFamily = FontFamily.Monospace,
                                fontWeight = FontWeight.SemiBold,
                                color = OraculoColors.OnSurface,
                            )
                            Spacer(modifier = Modifier.height(2.dp))
                            val reachSummary = if (bridges.isNotEmpty()) {
                                val shown = bridges.take(2).joinToString(", ") { it.hostName }
                                val more = if (bridges.size > 2) " (+${bridges.size - 2})" else ""
                                if (bridges.size == 1) {
                                    tr(
                                        "Via Wi-Fi local + 1 ponte: $shown",
                                        "Via local Wi-Fi + 1 bridge: $shown",
                                    )
                                } else {
                                    tr(
                                        "Via Wi-Fi local + ${bridges.size} pontes: $shown$more",
                                        "Via local Wi-Fi + ${bridges.size} bridges: $shown$more",
                                    )
                                }
                            } else if (!currentHost.wolTargetOverride.isNullOrBlank()) {
                                "${currentHost.wolTargetOverride}:${currentHost.wolPort} + Wi-Fi"
                            } else {
                                tr(
                                    "Via Wi-Fi local · adicione uma ponte para ligar fora de casa",
                                    "Via local Wi-Fi · add a bridge to wake away from home",
                                )
                            }
                            Text(
                                text = reachSummary,
                                style = MaterialTheme.typography.bodySmall,
                                color = OraculoColors.OnSurfaceMuted,
                            )
                        }
                        IconButton(
                            onClick = {
                                isWaitingBoot = false
                                errorText = null
                                statusText = null
                                isEditing = true
                            },
                            enabled = !isBusy,
                        ) {
                            Icon(
                                imageVector = Icons.Default.Edit,
                                contentDescription = tr("Editar Wake-on-LAN", "Edit Wake-on-LAN"),
                                tint = OraculoColors.OnSurfaceMuted,
                                modifier = Modifier.size(19.dp),
                            )
                        }
                    }

                    if (isWaitingBoot) {
                        Surface(
                            color = OraculoColors.SecondaryContainer,
                            shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                            border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                CircularProgressIndicator(
                                    color = OraculoColors.OnSecondaryContainer,
                                    modifier = Modifier.size(20.dp),
                                    strokeWidth = 2.2.dp,
                                )
                                Column(modifier = Modifier.weight(1f)) {
                                    val headline = if (relayedVia.isNotEmpty()) {
                                        val relayedNames = relayedVia.joinToString(", ")
                                        tr(
                                            "Sinal enviado via ponte ($relayedNames)!",
                                            "Signal relayed via bridge ($relayedNames)!",
                                        )
                                    } else {
                                        tr(
                                            "Sinal enviado! Aguardando inicialização...",
                                            "Signal sent! Waiting for boot...",
                                        )
                                    }
                                    Text(
                                        text = headline,
                                        style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.Medium,
                                        color = OraculoColors.OnSecondaryContainer,
                                    )
                                    Text(
                                        text = tr(
                                            "Conectará automaticamente (${elapsedSec}s / ${BOOT_WAIT_TOTAL_SEC}s)",
                                            "Will connect automatically (${elapsedSec}s / ${BOOT_WAIT_TOTAL_SEC}s)",
                                        ),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = OraculoColors.OnSecondaryContainer.copy(alpha = 0.82f),
                                    )
                                }
                            }
                        }
                    } else if (statusText != null) {
                        Text(
                            text = statusText.orEmpty(),
                            style = MaterialTheme.typography.bodySmall,
                            color = OraculoColors.OnSurfaceMuted,
                        )
                    } else {
                        Text(
                            text = tr(
                                "Envia o sinal de ativação pela rede local, por DDNS opcional e pelas pontes Oráculo disponíveis.",
                                "Sends the wake signal over the local network, optional DDNS, and available Oráculo bridges.",
                            ),
                            style = MaterialTheme.typography.bodySmall,
                            color = OraculoColors.OnSurfaceMuted,
                        )
                    }
                }

                if (errorText != null) {
                    Text(
                        text = errorText.orEmpty(),
                        style = MaterialTheme.typography.bodySmall,
                        color = OraculoColors.Error,
                    )
                }
            }
        },
        confirmButton = {
            if (isEditing) {
                val hasMac = macInput.trim().isNotEmpty()
                if (hasMac) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilledTonalButton(
                            onClick = { saveConfiguration(wakeAfterSave = false) },
                            enabled = !isBusy,
                            shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                            contentPadding = PaddingValues(horizontal = 14.dp, vertical = 8.dp),
                            modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                        ) {
                            Text(
                                text = tr("Salvar", "Save"),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Button(
                            onClick = { saveConfiguration(wakeAfterSave = true) },
                            enabled = !isBusy,
                            colors = oraculoPrimaryButtonColors(),
                            shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                            contentPadding = PaddingValues(horizontal = 14.dp, vertical = 8.dp),
                            modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                        ) {
                            Text(
                                text = tr("Salvar e ligar", "Save & wake"),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                } else {
                    Button(
                        onClick = { saveConfiguration(wakeAfterSave = false) },
                        enabled = !isBusy && !currentHost.wolMac.isNullOrBlank(),
                        colors = oraculoPrimaryButtonColors(),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                        modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                    ) {
                        Text(tr("Limpar", "Clear"))
                    }
                }
            } else {
                Button(
                    onClick = { triggerWake() },
                    enabled = !isBusy,
                    colors = oraculoPrimaryButtonColors(),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Icon(
                        imageVector = Icons.Default.PowerSettingsNew,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(modifier = Modifier.width(6.dp))
                    Text(
                        text = if (isWaitingBoot) {
                            tr("Reenviar sinal", "Resend signal")
                        } else {
                            tr("Ligar computador", "Wake computer")
                        },
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        },
        dismissButton = {
            TextButton(
                onClick = {
                    if (isEditing && !currentHost.wolMac.isNullOrBlank()) {
                        macInput = currentHost.wolMac.orEmpty()
                        targetInput = currentHost.wolTargetOverride.orEmpty()
                        portInput = currentHost.wolPort.toInt().toString()
                        errorText = null
                        isEditing = false
                    } else {
                        isWaitingBoot = false
                        onDismiss()
                    }
                },
                enabled = !isBusy,
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                Text(
                    text = if (isWaitingBoot) tr("Fechar", "Close") else tr("Cancelar", "Cancel"),
                    color = OraculoColors.OnSurfaceMuted,
                )
            }
        },
    )
}
