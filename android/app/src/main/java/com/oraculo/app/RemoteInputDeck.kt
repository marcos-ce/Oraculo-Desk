package com.oraculo.app

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.oraculoPrimaryButtonColors
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Backspace
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.filled.KeyboardReturn
import androidx.compose.material.icons.automirrored.filled.VolumeDown
import androidx.compose.material.icons.automirrored.filled.VolumeOff
import androidx.compose.material.icons.automirrored.filled.VolumeUp
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateMap
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale
import kotlinx.coroutines.delay
import uniffi.oraculo_mobile_core.MobileInputSession
import uniffi.oraculo_mobile_core.MobileRemoteKey

internal enum class ModifierLatchState { OFF, ARMED, LOCKED }

internal enum class StickyModifier(
    val label: String,
    val compactLabel: String,
    val remoteKey: MobileRemoteKey,
) {
    CTRL("Ctrl", "Ctrl", MobileRemoteKey.CONTROL),
    ALT("Alt", "Alt", MobileRemoteKey.ALT),
    SUPER("Super", "Sup", MobileRemoteKey.SUPER),
    SHIFT("Shift", "Shf", MobileRemoteKey.SHIFT),
}

internal data class ShortcutSpec(
    val label: String,
    val modifiers: List<MobileRemoteKey> = emptyList(),
    val key: MobileRemoteKey? = null,
    val text: String? = null,
    val icon: ImageVector? = null,
    val secondaryIcon: ImageVector? = null,
)

@Stable
internal class RemoteInputController(
    private val haptic: HapticFeedback,
    private val inputSessionProvider: () -> MobileInputSession?,
    private val onSessionBrokenProvider: () -> (MobileInputSession) -> Unit,
    private val onPublishTelemetryProvider: () -> (String) -> Unit,
) {
    val focusRequester = FocusRequester()
    var remoteField by mutableStateOf(TextFieldValue(""))
        private set
    var syncedText by mutableStateOf("")
        private set
    val modifierStates: SnapshotStateMap<StickyModifier, ModifierLatchState> = mutableStateMapOf(
        StickyModifier.CTRL to ModifierLatchState.OFF,
        StickyModifier.ALT to ModifierLatchState.OFF,
        StickyModifier.SUPER to ModifierLatchState.OFF,
        StickyModifier.SHIFT to ModifierLatchState.OFF,
    )

    val shortcutModifierHeld: Boolean
        get() = (modifierStates[StickyModifier.CTRL] != ModifierLatchState.OFF) ||
            (modifierStates[StickyModifier.ALT] != ModifierLatchState.OFF) ||
            (modifierStates[StickyModifier.SUPER] != ModifierLatchState.OFF)

    val shiftHeld: Boolean
        get() = modifierStates[StickyModifier.SHIFT] != ModifierLatchState.OFF

    fun currentSession(): MobileInputSession? = inputSessionProvider()

    fun sendOnSession(
        target: MobileInputSession? = inputSessionProvider(),
        action: (MobileInputSession) -> Unit,
    ): Boolean {
        val session = target ?: return false
        return runCatching { action(session) }
            .onFailure { onSessionBrokenProvider().invoke(session) }
            .isSuccess
    }

    fun resetState(releaseOnTarget: MobileInputSession? = inputSessionProvider()) {
        releaseAllModifiers(releaseOnTarget)
        clearDraft()
    }

    fun clearDraft() {
        syncedText = ""
        remoteField = TextFieldValue("")
    }

    fun releaseAllModifiers(targetSession: MobileInputSession? = inputSessionProvider()) {
        StickyModifier.entries.forEach { mod ->
            if (modifierStates[mod] != ModifierLatchState.OFF) {
                modifierStates[mod] = ModifierLatchState.OFF
                if (targetSession != null && targetSession.isAlive()) {
                    runCatching { targetSession.pressKey(mod.remoteKey, false) }
                }
            }
        }
    }

    fun disarmArmedModifiers() {
        val session = inputSessionProvider() ?: return
        StickyModifier.entries.forEach { mod ->
            if (modifierStates[mod] == ModifierLatchState.ARMED) {
                modifierStates[mod] = ModifierLatchState.OFF
                sendOnSession(session) { it.pressKey(mod.remoteKey, false) }
            }
        }
    }

    fun onModifierTap(mod: StickyModifier) {
        val session = inputSessionProvider() ?: return
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        val current = modifierStates[mod] ?: ModifierLatchState.OFF

        when (current) {
            ModifierLatchState.OFF -> {
                if (sendOnSession(session) { it.pressKey(mod.remoteKey, true) }) {
                    modifierStates[mod] = ModifierLatchState.ARMED
                    onPublishTelemetryProvider().invoke(tr("${mod.label} armado (1x)", "${mod.label} armed (1x)"))
                }
            }
            ModifierLatchState.ARMED, ModifierLatchState.LOCKED -> {
                modifierStates[mod] = ModifierLatchState.OFF
                sendOnSession(session) { it.pressKey(mod.remoteKey, false) }
                onPublishTelemetryProvider().invoke(tr("${mod.label} solto", "${mod.label} released"))
            }
        }
    }

    fun onModifierLongPress(mod: StickyModifier) {
        val session = inputSessionProvider() ?: return
        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
        val current = modifierStates[mod] ?: ModifierLatchState.OFF

        when (current) {
            ModifierLatchState.OFF -> {
                if (sendOnSession(session) { it.pressKey(mod.remoteKey, true) }) {
                    modifierStates[mod] = ModifierLatchState.LOCKED
                    onPublishTelemetryProvider().invoke(tr("${mod.label} travado", "${mod.label} locked"))
                }
            }
            ModifierLatchState.ARMED -> {
                modifierStates[mod] = ModifierLatchState.LOCKED
                onPublishTelemetryProvider().invoke(tr("${mod.label} travado", "${mod.label} locked"))
            }
            ModifierLatchState.LOCKED -> {
                modifierStates[mod] = ModifierLatchState.OFF
                sendOnSession(session) { it.pressKey(mod.remoteKey, false) }
                onPublishTelemetryProvider().invoke(tr("${mod.label} destravado", "${mod.label} unlocked"))
            }
        }
    }

    private fun isKeyHeld(remoteKey: MobileRemoteKey): Boolean {
        val mod = StickyModifier.entries.firstOrNull { it.remoteKey == remoteKey } ?: return false
        return (modifierStates[mod] ?: ModifierLatchState.OFF) != ModifierLatchState.OFF
    }

    fun executeShortcut(spec: ShortcutSpec) {
        val session = inputSessionProvider() ?: return
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        val tempHeld = spec.modifiers.filter { !isKeyHeld(it) }
        val pressedTemp = mutableListOf<MobileRemoteKey>()
        val sent = sendOnSession(session) { s ->
            try {
                tempHeld.forEach { modKey ->
                    s.pressKey(modKey, true)
                    pressedTemp.add(modKey)
                }
                spec.key?.let { s.sendKey(it) }
                spec.text?.let { s.sendText(it) }
            } finally {
                pressedTemp.asReversed().forEach { modKey ->
                    runCatching { s.pressKey(modKey, false) }
                }
            }
        }
        if (sent) {
            disarmArmedModifiers()
            onPublishTelemetryProvider().invoke(spec.label)
        }
    }

    fun sendBackspace() {
        executeShortcut(ShortcutSpec(tr("Apagar", "Backspace"), key = MobileRemoteKey.BACKSPACE))
    }

    fun sendEnterAndClear() {
        val session = inputSessionProvider() ?: return
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        if (sendOnSession(session) { it.sendKey(MobileRemoteKey.ENTER) }) {
            disarmArmedModifiers()
            clearDraft()
        }
    }

    fun onTextFieldChanged(next: TextFieldValue) {
        val newText = next.text
        val session = inputSessionProvider() ?: return
        if (shortcutModifierHeld) {
            val (deletes, rawInserted) = computeLiveTextDelta(syncedText, newText)
            if (rawInserted.isNotEmpty()) {
                val payload = if (!shiftHeld) rawInserted.lowercase(Locale.ROOT) else rawInserted
                if (payload.isNotEmpty() && sendOnSession(session) { it.sendText(payload) }) {
                    disarmArmedModifiers()
                    onPublishTelemetryProvider().invoke(tr("atalho + $payload", "shortcut + $payload"))
                }
            } else if (deletes > 0) {
                if (sendOnSession(session) { it.sendKey(MobileRemoteKey.BACKSPACE) }) {
                    disarmArmedModifiers()
                }
            }
            remoteField = TextFieldValue(
                text = syncedText,
                selection = TextRange(syncedText.length),
            )
            return
        }

        if (newText != syncedText) {
            val (deletes, inserted) = computeLiveTextDelta(syncedText, newText)
            var okDel = true
            repeat(deletes) {
                if (okDel) okDel = sendOnSession(session) { it.sendKey(MobileRemoteKey.BACKSPACE) }
            }
            if (okDel && inserted.isNotEmpty()) {
                val normalized = inserted.replace("\r\n", "\n").replace('\r', '\n')
                val parts = normalized.split('\n')
                parts.forEachIndexed { idx, segment ->
                    if (segment.isNotEmpty()) {
                        sendOnSession(session) { it.sendText(segment) }
                    }
                    if (idx < parts.lastIndex) {
                        sendOnSession(session) { it.sendKey(MobileRemoteKey.ENTER) }
                    }
                }
            }
            disarmArmedModifiers()
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
    }

    fun onPreviewBackspaceWhenEmpty(): Boolean {
        val session = inputSessionProvider() ?: return false
        if (syncedText.isNotEmpty()) return false
        sendOnSession(session) { it.sendKey(MobileRemoteKey.BACKSPACE) }
        disarmArmedModifiers()
        return true
    }
}

@Composable
internal fun rememberRemoteInputController(
    inputSession: MobileInputSession?,
    onSessionBroken: (MobileInputSession) -> Unit,
    onPublishTelemetry: (String) -> Unit,
): RemoteInputController {
    val haptic = LocalHapticFeedback.current
    val currentSession by rememberUpdatedState(inputSession)
    val currentBroken by rememberUpdatedState(onSessionBroken)
    val currentTelemetry by rememberUpdatedState(onPublishTelemetry)
    return remember(haptic) {
        RemoteInputController(
            haptic = haptic,
            inputSessionProvider = { currentSession },
            onSessionBrokenProvider = { currentBroken },
            onPublishTelemetryProvider = { currentTelemetry },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun RemoteModifierRow(
    controller: RemoteInputController,
    enabled: Boolean,
    compact: Boolean = false,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(if (compact) 5.dp else 6.dp),
    ) {
        val chipHeight = if (compact) {
            OraculoMetrics.ControlHeightCompactChip
        } else {
            OraculoMetrics.ControlHeightSmall
        }
        val shape = RoundedCornerShape(OraculoMetrics.CornerSmall)
        StickyModifier.entries.forEach { mod ->
            val state = controller.modifierStates[mod] ?: ModifierLatchState.OFF
            val bg by animateColorAsState(
                targetValue = when (state) {
                    ModifierLatchState.OFF -> OraculoColors.Surface
                    ModifierLatchState.ARMED -> OraculoColors.SecondaryContainer
                    ModifierLatchState.LOCKED -> OraculoColors.PrimaryAction
                },
                animationSpec = OraculoMotion.microTween(),
                label = "modBg",
            )
            val borderColor by animateColorAsState(
                targetValue = when (state) {
                    ModifierLatchState.OFF -> OraculoColors.Outline
                    ModifierLatchState.ARMED -> OraculoColors.Primary
                    ModifierLatchState.LOCKED -> OraculoColors.Primary
                },
                animationSpec = OraculoMotion.microTween(),
                label = "modBorder",
            )
            val textColor by animateColorAsState(
                targetValue = when (state) {
                    ModifierLatchState.OFF -> OraculoColors.OnSurface
                    ModifierLatchState.ARMED -> OraculoColors.OnSecondaryContainer
                    ModifierLatchState.LOCKED -> OraculoColors.OnPrimaryAction
                },
                animationSpec = OraculoMotion.microTween(),
                label = "modText",
            )
            val baseLabel = if (compact) mod.compactLabel else mod.label
            Surface(
                color = bg,
                shape = shape,
                border = BorderStroke(
                    width = if (state == ModifierLatchState.ARMED) 1.5.dp else 1.dp,
                    color = borderColor,
                ),
                modifier = Modifier
                    .weight(1f)
                    .height(chipHeight)
                    .clip(shape)
                    .combinedClickable(
                        enabled = enabled,
                        onClick = { controller.onModifierTap(mod) },
                        onLongClick = { controller.onModifierLongPress(mod) },
                    ),
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = if (compact) 4.dp else 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.Center,
                ) {
                    Text(
                        text = if (state == ModifierLatchState.ARMED) "$baseLabel•" else baseLabel,
                        style = MaterialTheme.typography.labelMedium,
                        fontSize = if (compact) 11.sp else 12.sp,
                        fontFamily = FontFamily.Monospace,
                        fontWeight = if (state != ModifierLatchState.OFF) FontWeight.Bold else FontWeight.Medium,
                        color = textColor,
                        maxLines = 1,
                        softWrap = false,
                    )
                    if (state == ModifierLatchState.LOCKED) {
                        Spacer(modifier = Modifier.width(2.dp))
                        Icon(
                            imageVector = Icons.Default.Lock,
                            contentDescription = tr("Travada", "Locked"),
                            tint = textColor,
                            modifier = Modifier.size(if (compact) 10.dp else 11.dp),
                        )
                    }
                }
            }
        }

        Surface(
            onClick = { controller.sendBackspace() },
            enabled = enabled,
            color = OraculoColors.Surface,
            shape = shape,
            border = BorderStroke(1.dp, OraculoColors.Outline),
            modifier = Modifier
                .weight(0.68f)
                .height(chipHeight),
        ) {
            Box(
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.Backspace,
                    contentDescription = tr("Apagar (Backspace)", "Backspace"),
                    tint = OraculoColors.OnSurface,
                    modifier = Modifier.size(if (compact) 15.dp else 16.dp),
                )
            }
        }
    }
}

@Composable
internal fun RemoteShortcutStrip(
    controller: RemoteInputController,
    enabled: Boolean,
    compact: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val shortcuts = remember(AppI18n.lang) {
        listOf(
            ShortcutSpec("Super+D", modifiers = listOf(MobileRemoteKey.SUPER), text = "d"),
            ShortcutSpec("Alt+Tab", modifiers = listOf(MobileRemoteKey.ALT), key = MobileRemoteKey.TAB),
            ShortcutSpec("Ctrl+C", modifiers = listOf(MobileRemoteKey.CONTROL), text = "c"),
            ShortcutSpec("Ctrl+V", modifiers = listOf(MobileRemoteKey.CONTROL), text = "v"),
            ShortcutSpec("Ctrl+A", modifiers = listOf(MobileRemoteKey.CONTROL), text = "a"),
            ShortcutSpec("Ctrl+Z", modifiers = listOf(MobileRemoteKey.CONTROL), text = "z"),
            ShortcutSpec("Esc", key = MobileRemoteKey.ESCAPE),
            ShortcutSpec("Tab", key = MobileRemoteKey.TAB),
            ShortcutSpec("Del", key = MobileRemoteKey.DELETE),
            ShortcutSpec("F4", key = MobileRemoteKey.F4),
            ShortcutSpec("F5", key = MobileRemoteKey.F5),
            ShortcutSpec("F11", key = MobileRemoteKey.F11),
            ShortcutSpec(
                tr("Reproduzir ou pausar", "Play or pause"),
                key = MobileRemoteKey.PLAY_PAUSE,
                icon = Icons.Default.PlayArrow,
                secondaryIcon = Icons.Default.Pause,
            ),
            ShortcutSpec(
                tr("Diminuir volume", "Volume down"),
                key = MobileRemoteKey.VOLUME_DOWN,
                icon = Icons.AutoMirrored.Filled.VolumeDown,
            ),
            ShortcutSpec(
                tr("Aumentar volume", "Volume up"),
                key = MobileRemoteKey.VOLUME_UP,
                icon = Icons.AutoMirrored.Filled.VolumeUp,
            ),
            ShortcutSpec(
                tr("Silenciar", "Mute"),
                key = MobileRemoteKey.VOLUME_MUTE,
                icon = Icons.AutoMirrored.Filled.VolumeOff,
            ),
            ShortcutSpec(
                tr("Seta esquerda", "Left arrow"),
                key = MobileRemoteKey.LEFT,
                icon = Icons.AutoMirrored.Filled.KeyboardArrowLeft,
            ),
            ShortcutSpec(
                tr("Seta cima", "Up arrow"),
                key = MobileRemoteKey.UP,
                icon = Icons.Default.KeyboardArrowUp,
            ),
            ShortcutSpec(
                tr("Seta baixo", "Down arrow"),
                key = MobileRemoteKey.DOWN,
                icon = Icons.Default.KeyboardArrowDown,
            ),
            ShortcutSpec(
                tr("Seta direita", "Right arrow"),
                key = MobileRemoteKey.RIGHT,
                icon = Icons.AutoMirrored.Filled.KeyboardArrowRight,
            ),
            ShortcutSpec("Home", key = MobileRemoteKey.HOME),
            ShortcutSpec("End", key = MobileRemoteKey.END),
        )
    }

    val chipHeight = if (compact) {
        OraculoMetrics.ControlHeightCompactChip
    } else {
        OraculoMetrics.ControlHeightSmall
    }
    val shape = RoundedCornerShape(OraculoMetrics.CornerSmall)
    val iconSize = if (compact) 15.dp else 16.dp
    val hPad = if (compact) 9.dp else 11.dp

    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        shortcuts.forEach { item ->
            Surface(
                onClick = { controller.executeShortcut(item) },
                enabled = enabled,
                color = OraculoColors.SurfaceRecessed,
                shape = shape,
                border = BorderStroke(1.dp, OraculoColors.Outline),
                modifier = Modifier.height(chipHeight),
            ) {
                if (item.icon != null) {
                    Row(
                        modifier = Modifier.padding(horizontal = hPad),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(2.dp),
                    ) {
                        Icon(
                            imageVector = item.icon,
                            contentDescription = item.label,
                            tint = OraculoColors.OnSurface,
                            modifier = Modifier.size(iconSize),
                        )
                        if (item.secondaryIcon != null) {
                            Icon(
                                imageVector = item.secondaryIcon,
                                contentDescription = null,
                                tint = OraculoColors.OnSurface,
                                modifier = Modifier.size(if (compact) 13.dp else 14.dp),
                            )
                        }
                    }
                } else {
                    Box(
                        modifier = Modifier.padding(horizontal = hPad),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = item.label,
                            style = MaterialTheme.typography.labelSmall,
                            fontSize = if (compact) 11.sp else 12.sp,
                            fontFamily = FontFamily.Monospace,
                            color = OraculoColors.OnSurface,
                            maxLines = 1,
                            softWrap = false,
                        )
                    }
                }
            }
        }
    }
}

@Composable
internal fun RemoteKeyboardBar(
    visible: Boolean,
    controller: RemoteInputController,
    enabled: Boolean,
    compact: Boolean = false,
    autoFocusOnVisible: Boolean = true,
    modifier: Modifier = Modifier,
) {
    val focusManager = LocalFocusManager.current
    val softwareKeyboard = LocalSoftwareKeyboardController.current
    var wasVisible by remember { mutableStateOf(visible) }

    LaunchedEffect(visible) {
        if (visible && autoFocusOnVisible) {
            wasVisible = true
            delay(50)
            runCatching {
                controller.focusRequester.requestFocus()
                softwareKeyboard?.show()
            }
        } else if (visible) {
            wasVisible = true
        } else if (wasVisible) {
            wasVisible = false
            focusManager.clearFocus(force = true)
            softwareKeyboard?.hide()
        }
    }

    AnimatedVisibility(
        visible = visible,
        enter = OraculoMotion.ExpandV,
        exit = OraculoMotion.CollapseV,
        modifier = modifier,
    ) {
        val actionHeight = if (compact) {
            OraculoMetrics.ControlHeightInputCompact
        } else {
            OraculoMetrics.ControlHeightInput
        }
        val placeholderLabel = if (controller.shortcutModifierHeld) {
            tr("Atalho ativo: digite a tecla (ex.: d, c, v)...", "Shortcut active: type a key (e.g. d, c, v)...")
        } else if (compact) {
            tr("Digitar no computador...", "Type on computer...")
        } else {
            tr("Digite para enviar ao computador...", "Type to send to computer...")
        }
        val fieldShape = RoundedCornerShape(OraculoMetrics.CornerMedium)

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(if (compact) 6.dp else 8.dp),
        ) {
            if (compact) {
                var isFocused by remember { mutableStateOf(false) }
                val compactBorder by animateColorAsState(
                    targetValue = if (isFocused) OraculoColors.Primary else OraculoColors.Outline,
                    animationSpec = OraculoMotion.microTween(),
                    label = "compactInputBorder",
                )
                Row(
                    modifier = Modifier
                        .weight(1f)
                        .height(actionHeight)
                        .clip(fieldShape)
                        .background(OraculoColors.Surface)
                        .border(if (isFocused) 1.5.dp else 1.dp, compactBorder, fieldShape)
                        .padding(start = 12.dp, end = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(
                        modifier = Modifier.weight(1f),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        if (controller.remoteField.text.isEmpty()) {
                            Text(
                                text = placeholderLabel,
                                style = TextStyle(fontSize = 13.sp),
                                color = OraculoColors.OnSurfaceMuted,
                                maxLines = 1,
                            )
                        }
                        BasicTextField(
                            value = controller.remoteField,
                            onValueChange = { controller.onTextFieldChanged(it) },
                            singleLine = true,
                            textStyle = TextStyle(
                                fontSize = 13.sp,
                                color = OraculoColors.OnSurface,
                            ),
                            cursorBrush = SolidColor(OraculoColors.Primary),
                            keyboardOptions = KeyboardOptions(
                                capitalization = KeyboardCapitalization.None,
                                autoCorrectEnabled = false,
                                imeAction = ImeAction.Send,
                            ),
                            keyboardActions = KeyboardActions(onSend = { controller.sendEnterAndClear() }),
                            modifier = Modifier
                                .fillMaxWidth()
                                .focusRequester(controller.focusRequester)
                                .onFocusChanged { isFocused = it.isFocused }
                                .onPreviewKeyEvent { keyEvent ->
                                    if (keyEvent.type == KeyEventType.KeyDown && keyEvent.key == Key.Backspace) {
                                        controller.onPreviewBackspaceWhenEmpty()
                                    } else {
                                        false
                                    }
                                },
                        )
                    }
                    if (controller.remoteField.text.isNotEmpty()) {
                        IconButton(
                            onClick = { controller.clearDraft() },
                            modifier = Modifier.size(32.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Default.Close,
                                contentDescription = tr("Limpar texto", "Clear text"),
                                tint = OraculoColors.OnSurfaceMuted,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            } else {
                OutlinedTextField(
                    value = controller.remoteField,
                    onValueChange = { controller.onTextFieldChanged(it) },
                    placeholder = {
                        Text(
                            text = placeholderLabel,
                            style = TextStyle(fontSize = 14.sp),
                            maxLines = 1,
                        )
                    },
                    textStyle = TextStyle(fontSize = 14.sp),
                    singleLine = true,
                    trailingIcon = if (controller.remoteField.text.isNotEmpty()) {
                        {
                            IconButton(onClick = { controller.clearDraft() }) {
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
                    shape = fieldShape,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.None,
                        autoCorrectEnabled = false,
                        imeAction = ImeAction.Send,
                    ),
                    keyboardActions = KeyboardActions(onSend = { controller.sendEnterAndClear() }),
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
                        .focusRequester(controller.focusRequester)
                        .onPreviewKeyEvent { keyEvent ->
                            if (keyEvent.type == KeyEventType.KeyDown && keyEvent.key == Key.Backspace) {
                                controller.onPreviewBackspaceWhenEmpty()
                            } else {
                                false
                            }
                        },
                )
            }

            Button(
                onClick = { controller.sendEnterAndClear() },
                enabled = enabled,
                colors = oraculoPrimaryButtonColors(),
                shape = fieldShape,
                contentPadding = PaddingValues(horizontal = if (compact) 14.dp else 16.dp),
                modifier = Modifier
                    .height(actionHeight)
                    .widthIn(min = actionHeight),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.KeyboardReturn,
                    contentDescription = tr("Enviar Enter", "Send Enter"),
                    modifier = Modifier.size(if (compact) 18.dp else 20.dp),
                )
            }
        }
    }
}

internal fun applyLiveDraftWindow(next: TextFieldValue): Pair<String, TextFieldValue> {
    val text = next.text
    if (text.length <= 240) {
        return text to next
    }
    var cut = text.length - 120
    if (cut in 1 until text.length && Character.isLowSurrogate(text[cut])) {
        cut++
    }
    val trimmed = text.substring(cut)
    val shiftedStart = (next.selection.start - cut).coerceIn(0, trimmed.length)
    val shiftedEnd = (next.selection.end - cut).coerceIn(0, trimmed.length)
    return trimmed to TextFieldValue(
        text = trimmed,
        selection = TextRange(shiftedStart, shiftedEnd),
    )
}

internal fun computeLiveTextDelta(previous: String, current: String): Pair<Int, String> {
    val maxPrefix = minOf(previous.length, current.length)
    var prefix = 0
    while (prefix < maxPrefix && previous[prefix] == current[prefix]) {
        prefix++
    }
    if (prefix > 0 && (
            (prefix < previous.length && Character.isLowSurrogate(previous[prefix])) ||
                (prefix < current.length && Character.isLowSurrogate(current[prefix]))
            )
    ) {
        prefix--
    }
    val deletes = previous.codePointCount(prefix, previous.length)
    val inserted = current.substring(prefix)
    return deletes to inserted
}
