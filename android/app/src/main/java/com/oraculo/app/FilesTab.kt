package com.oraculo.app

import android.Manifest
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.provider.OpenableColumns
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.InsertDriveFile
import androidx.compose.material.icons.automirrored.filled.OpenInNew
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.AudioFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.Upload
import androidx.compose.material.icons.filled.VideoFile
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.OraculoMotion
import com.oraculo.app.ui.oraculoPrimaryButtonColors
import com.oraculo.app.ui.oraculoSegmentedItemShape
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.oraculo_mobile_core.FileTransferObserver
import uniffi.oraculo_mobile_core.OraculoMobileClient
import uniffi.oraculo_mobile_core.RemoteDirectoryListing
import uniffi.oraculo_mobile_core.RemoteFileEntry
import uniffi.oraculo_mobile_core.SavedHostInfo
import java.io.File
import java.io.FileOutputStream
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.atomic.AtomicLong

private data class TransferProgressState(
    val title: String,
    val transferredBytes: Long,
    val totalBytes: Long,
    val isUpload: Boolean,
)

private sealed interface FilePreviewState {
    data class TextPreview(
        val entry: RemoteFileEntry,
        val content: String,
        val cachedFile: File,
    ) : FilePreviewState

    data class ImagePreview(
        val entry: RemoteFileEntry,
        val bitmap: Bitmap,
        val cachedFile: File,
    ) : FilePreviewState
}

@Composable
internal fun FilesTab(
    client: OraculoMobileClient,
    host: SavedHostInfo,
    isActive: Boolean,
    allowFilesRead: Boolean? = null,
    allowFilesWrite: Boolean? = null,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    val loadGen = remember { AtomicLong(0L) }
    val transferGen = remember { AtomicLong(0L) }

    var currentPath by remember { mutableStateOf("") }
    var listing by remember { mutableStateOf<RemoteDirectoryListing?>(null) }
    var isLoading by remember { mutableStateOf(true) }
    var showHidden by remember { mutableStateOf(false) }
    var isSearchOpen by remember { mutableStateOf(false) }
    var searchQuery by remember { mutableStateOf("") }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var statusMessage by remember { mutableStateOf<String?>(null) }
    var transferState by remember { mutableStateOf<TransferProgressState?>(null) }
    var selectedEntry by remember { mutableStateOf<RemoteFileEntry?>(null) }
    var previewState by remember { mutableStateOf<FilePreviewState?>(null) }
    var pendingLegacyStorageAction by remember { mutableStateOf<(() -> Unit)?>(null) }

    val legacyStoragePermissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission(),
    ) { granted ->
        val action = pendingLegacyStorageAction
        pendingLegacyStorageAction = null
        if (granted) {
            action?.invoke()
        } else {
            errorMessage = tr(
                "Permissão de armazenamento necessária para salvar em Downloads neste Android.",
                "Storage permission is required to save to Downloads on this Android version.",
            )
        }
    }

    fun runWithDownloadStoragePermission(action: () -> Unit) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            action()
            return
        }
        val granted = ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.WRITE_EXTERNAL_STORAGE,
        ) == PackageManager.PERMISSION_GRANTED
        if (granted) {
            action()
        } else {
            pendingLegacyStorageAction = action
            runCatching {
                legacyStoragePermissionLauncher.launch(Manifest.permission.WRITE_EXTERNAL_STORAGE)
            }.onFailure { err ->
                pendingLegacyStorageAction = null
                errorMessage = formatOraculoError(err)
            }
        }
    }

    fun beginTransfer(label: String, isUpload: Boolean): Long {
        val gen = transferGen.incrementAndGet()
        mainHandler.removeCallbacksAndMessages(null)
        transferState = TransferProgressState(
            title = label,
            transferredBytes = 0L,
            totalBytes = 0L,
            isUpload = isUpload,
        )
        return gen
    }

    fun finishTransfer(gen: Long) {
        transferGen.compareAndSet(gen, gen + 1L)
        mainHandler.removeCallbacksAndMessages(null)
        transferState = null
    }

    DisposableEffect(host.endpointId) {
        onDispose {
            transferGen.incrementAndGet()
            mainHandler.removeCallbacksAndMessages(null)
            val appCtx = context.applicationContext
            Thread { cleanupOldCachedFiles(appCtx, maxAgeMs = 5L * 60L * 1000L) }.start()
        }
    }

    fun loadDirectory(relPath: String, hidden: Boolean = showHidden) {
        if (allowFilesRead == false) {
            isLoading = false
            errorMessage = tr(
                "Leitura de arquivos desativada nas permissões do computador.",
                "File browsing is disabled in your computer's permissions.",
            )
            return
        }
        val myGen = loadGen.incrementAndGet()
        scope.launch {
            isLoading = true
            errorMessage = null
            val res = withContext(Dispatchers.IO) {
                runCatching { client.listFiles(host.endpointId, relPath, hidden) }
            }
            if (loadGen.get() != myGen) return@launch
            res.onSuccess { data ->
                listing = data
                currentPath = data.currentPath
            }.onFailure { err ->
                errorMessage = formatOraculoError(err)
            }
            isLoading = false
        }
    }

    BackHandler(enabled = isActive && selectedEntry == null && previewState == null && currentPath.isNotEmpty()) {
        val parent = listing?.parentPath ?: ""
        loadDirectory(parent)
    }

    LaunchedEffect(host.endpointId) {
        withContext(Dispatchers.IO) {
            cleanupOldCachedFiles(context, maxAgeMs = 0L)
        }
        loadDirectory("")
    }

    LaunchedEffect(allowFilesRead, allowFilesWrite) {
        if (allowFilesRead == false) {
            isLoading = false
            selectedEntry = null
            errorMessage = tr(
                "Leitura de arquivos desativada nas permissões do computador.",
                "File browsing is disabled in your computer's permissions.",
            )
        } else if (allowFilesRead == true && (listing == null || errorMessage != null) && !isLoading) {
            loadDirectory(currentPath)
        } else if (allowFilesWrite == true && errorMessage != null) {
            errorMessage = null
        }
    }

    fun buildObserver(gen: Long, label: String, isUpload: Boolean): FileTransferObserver =
        object : FileTransferObserver {
            override fun onProgress(transferred: ULong, total: ULong) {
                mainHandler.post {
                    if (transferGen.get() != gen) return@post
                    transferState = TransferProgressState(
                        title = label,
                        transferredBytes = transferred.toLong(),
                        totalBytes = total.toLong(),
                        isUpload = isUpload,
                    )
                }
            }
        }

    suspend fun fetchToCache(entry: RemoteFileEntry, gen: Long, label: String): Result<File> =
        withContext(Dispatchers.IO) {
            runCatching {
                val cacheDir = File(context.cacheDir, "oraculo_files")
                cacheDir.mkdirs()
                val safeName = sanitizeDownloadFileName(entry.name)
                val dest = File(cacheDir, safeName)
                client.downloadFile(
                    endpointId = host.endpointId,
                    remoteRelPath = entry.relPath,
                    destLocalPath = dest.absolutePath,
                    observer = buildObserver(gen, label, isUpload = false),
                )
                dest
            }
        }

    fun startDownloadToDevice(entry: RemoteFileEntry) {
        if (transferState != null || allowFilesRead == false) return
        selectedEntry = null
        runWithDownloadStoragePermission {
            if (transferState != null) return@runWithDownloadStoragePermission
            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
            val label = tr("Baixando ${entry.name}", "Downloading ${entry.name}")
            val gen = beginTransfer(label, isUpload = false)
            scope.launch {
                try {
                    errorMessage = null
                    statusMessage = null
                    val cachedRes = fetchToCache(entry, gen, label)
                    cachedRes.onSuccess { cached ->
                        val saveRes = withContext(Dispatchers.IO) {
                            runCatching {
                                try {
                                    saveFileToDownloads(context, cached, entry.name, entry.mimeHint)
                                } finally {
                                    cached.delete()
                                }
                            }
                        }
                        saveRes.onSuccess { savedLabel ->
                            statusMessage = tr("Salvo em $savedLabel", "Saved to $savedLabel")
                        }.onFailure { err ->
                            errorMessage = tr(
                                "Falha ao salvar em Downloads: ${formatOraculoError(err)}",
                                "Failed to save to Downloads: ${formatOraculoError(err)}",
                            )
                        }
                    }.onFailure { err ->
                        errorMessage = formatOraculoError(err)
                    }
                } finally {
                    finishTransfer(gen)
                }
            }
        }
    }

    fun startOpenOrShare(entry: RemoteFileEntry, shareOnly: Boolean) {
        if (transferState != null || allowFilesRead == false) return
        selectedEntry = null
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        val label = tr("Baixando ${entry.name}", "Downloading ${entry.name}")
        val gen = beginTransfer(label, isUpload = false)
        scope.launch {
            try {
                errorMessage = null
                statusMessage = null
                val cachedRes = fetchToCache(entry, gen, label)
                cachedRes.onSuccess { cached ->
                    launchExternalFileIntent(context, cached, entry.mimeHint, shareOnly)
                        .onFailure { err ->
                            errorMessage = formatOraculoError(err)
                        }
                }.onFailure { err ->
                    errorMessage = formatOraculoError(err)
                }
            } finally {
                finishTransfer(gen)
            }
        }
    }

    fun startInAppPreview(entry: RemoteFileEntry) {
        if (transferState != null || allowFilesRead == false) return
        selectedEntry = null
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        if (!isImageEntry(entry) && entry.size > 8UL * 1024UL * 1024UL) {
            errorMessage = tr(
                "Arquivo muito grande para prévia de texto (${formatFileSize(entry.size.toLong())}). Use Baixar para Downloads.",
                "File too large for text preview (${formatFileSize(entry.size.toLong())}). Use Save to Downloads.",
            )
            return
        }
        val label = tr("Baixando ${entry.name}", "Downloading ${entry.name}")
        val gen = beginTransfer(label, isUpload = false)
        scope.launch {
            try {
                errorMessage = null
                statusMessage = null
                val cachedRes = fetchToCache(entry, gen, label)
                cachedRes.onSuccess { cached ->
                    val decoded = withContext(Dispatchers.IO) {
                        runCatching {
                            if (isImageEntry(entry)) {
                                val bmp = decodeScaledBitmap(cached, maxSidePx = 2048)
                                    ?: error(tr("Formato de imagem não suportado pelo Android", "Image format not supported by Android"))
                                FilePreviewState.ImagePreview(entry, bmp, cached)
                            } else {
                                val maxPreviewBytes = 512 * 1024
                                val bytes = cached.inputStream().use { input ->
                                    val buf = ByteArray(maxPreviewBytes + 1)
                                    var offset = 0
                                    while (offset < buf.size) {
                                        val read = input.read(buf, offset, buf.size - offset)
                                        if (read <= 0) break
                                        offset += read
                                    }
                                    buf.copyOf(offset)
                                }
                                val text = if (bytes.size > maxPreviewBytes) {
                                    String(bytes, 0, maxPreviewBytes, Charsets.UTF_8) +
                                        tr("\n\n[... prévia truncada em 512 KB ...]", "\n\n[... preview truncated at 512 KB ...]")
                                } else {
                                    String(bytes, Charsets.UTF_8)
                                }
                                FilePreviewState.TextPreview(entry, text, cached)
                            }
                        }
                    }
                    decoded.onSuccess { preview ->
                        previewState = preview
                    }.onFailure { err ->
                        runCatching { cached.delete() }
                        errorMessage = tr(
                            "Não foi possível abrir prévia: ${formatOraculoError(err)}",
                            "Could not open preview: ${formatOraculoError(err)}",
                        )
                    }
                }.onFailure { err ->
                    errorMessage = formatOraculoError(err)
                }
            } finally {
                finishTransfer(gen)
            }
        }
    }

    val uploadLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenMultipleDocuments(),
    ) { uris: List<Uri> ->
        if (uris.isEmpty() || transferState != null || allowFilesWrite == false) {
            return@rememberLauncherForActivityResult
        }
        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        val targetDir = currentPath
        val initLabel = tr("Preparando envio...", "Preparing upload...")
        val gen = beginTransfer(initLabel, isUpload = true)
        scope.launch {
            var uploadedCount = 0
            var lastSavedRel = ""
            try {
                errorMessage = null
                statusMessage = null

                for ((index, uri) in uris.withIndex()) {
                    val uploadRes = withContext(Dispatchers.IO) {
                        var tempFile: File? = null
                        try {
                            val displayName = resolveDocumentDisplayName(context, uri)
                            val prefix = if (uris.size > 1) "[${index + 1}/${uris.size}] " else ""
                            val stepLabel = tr("${prefix}Enviando $displayName", "${prefix}Uploading $displayName")
                            mainHandler.post {
                                if (transferGen.get() == gen) {
                                    transferState = TransferProgressState(
                                        title = stepLabel,
                                        transferredBytes = 0L,
                                        totalBytes = 0L,
                                        isUpload = true,
                                    )
                                }
                            }
                            tempFile = File(
                                context.cacheDir,
                                "oraculo_upload_${System.nanoTime()}.tmp",
                            )
                            context.contentResolver.openInputStream(uri)?.use { input ->
                                FileOutputStream(tempFile).use { output ->
                                    input.copyTo(output, bufferSize = 64 * 1024)
                                }
                            } ?: error(tr("Não foi possível ler o arquivo selecionado", "Could not read selected file"))

                            val res = client.uploadFile(
                                endpointId = host.endpointId,
                                sourceLocalPath = tempFile.absolutePath,
                                remoteDirRelPath = targetDir,
                                fileName = displayName,
                                overwrite = false,
                                observer = buildObserver(gen, stepLabel, isUpload = true),
                            )
                            Result.success(res)
                        } catch (err: Throwable) {
                            Result.failure(err)
                        } finally {
                            tempFile?.delete()
                        }
                    }

                    uploadRes.onSuccess { res ->
                        uploadedCount += 1
                        lastSavedRel = res.remoteRelPath
                    }.onFailure { err ->
                        errorMessage = formatOraculoError(err)
                    }
                    if (uploadRes.isFailure) break
                }
            } finally {
                finishTransfer(gen)
            }

            if (uploadedCount > 0) {
                statusMessage = if (uploadedCount == 1) {
                    tr("Enviado para ~/$lastSavedRel", "Uploaded to ~/$lastSavedRel")
                } else {
                    val destLabel = if (targetDir.isEmpty()) "~" else "~/$targetDir"
                    tr("$uploadedCount arquivos enviados para $destLabel", "$uploadedCount files uploaded to $destLabel")
                }
                loadDirectory(targetDir)
            }
        }
    }

    val visibleEntries = remember(listing, searchQuery) {
        val raw = listing?.entries.orEmpty()
        val q = searchQuery.trim()
        if (q.isEmpty()) raw else raw.filter { it.name.contains(q, ignoreCase = true) }
    }

    selectedEntry?.let { entryDialog ->
        FileEntryActionDialog(
            entry = entryDialog,
            onDismiss = { selectedEntry = null },
            onPreview = { startInAppPreview(entryDialog) },
            onDownload = { startDownloadToDevice(entryDialog) },
            onOpen = { startOpenOrShare(entryDialog, shareOnly = false) },
            onShare = { startOpenOrShare(entryDialog, shareOnly = true) },
        )
    }

    previewState?.let { activePreview ->
        FilePreviewDialog(
            previewState = activePreview,
            onDismiss = {
                val toDelete = when (activePreview) {
                    is FilePreviewState.ImagePreview -> activePreview.cachedFile
                    is FilePreviewState.TextPreview -> activePreview.cachedFile
                }
                previewState = null
                scope.launch(Dispatchers.IO) {
                    runCatching { toDelete.delete() }
                }
            },
            onDownload = { cachedFile, previewEntry ->
                runWithDownloadStoragePermission {
                    previewState = null
                    scope.launch {
                        val saveRes = withContext(Dispatchers.IO) {
                            runCatching {
                                try {
                                    saveFileToDownloads(
                                        context,
                                        cachedFile,
                                        previewEntry.name,
                                        previewEntry.mimeHint,
                                    )
                                } finally {
                                    cachedFile.delete()
                                }
                            }
                        }
                        saveRes.onSuccess { savedLabel ->
                            statusMessage = tr("Salvo em $savedLabel", "Saved to $savedLabel")
                        }.onFailure { err ->
                            errorMessage = formatOraculoError(err)
                        }
                    }
                }
            },
        )
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 14.dp)
            .padding(bottom = 10.dp, top = 2.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // Barra superior da aba: voltar pasta, título/sandbox e ações principais
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            IconButton(
                onClick = {
                    val parent = listing?.parentPath ?: return@IconButton
                    loadDirectory(parent)
                },
                enabled = listing?.parentPath != null && !isLoading && allowFilesRead != false,
                modifier = Modifier.size(44.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.ArrowUpward,
                    contentDescription = tr("Pasta acima", "Parent folder"),
                    tint = if (listing?.parentPath != null && allowFilesRead != false) {
                        OraculoColors.OnSurface
                    } else {
                        OraculoColors.OnSurfaceMuted.copy(alpha = 0.4f)
                    },
                    modifier = Modifier.size(20.dp),
                )
            }

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = if (currentPath.isEmpty()) tr("Pasta pessoal (~)", "Home folder (~)") else currentPath.substringAfterLast('/'),
                    style = MaterialTheme.typography.titleMedium,
                    color = OraculoColors.OnSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = if (currentPath.isEmpty()) (listing?.homeLabel ?: "~") else "~/$currentPath",
                    style = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 11.sp),
                    color = OraculoColors.OnSurfaceMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            val uploadEnabled = transferState == null && allowFilesWrite != false
            Button(
                onClick = {
                    runCatching {
                        uploadLauncher.launch(arrayOf("*/*"))
                    }.onFailure { err ->
                        errorMessage = tr(
                            "Não foi possível abrir o seletor de arquivos: ${formatOraculoError(err)}",
                            "Could not open file picker: ${formatOraculoError(err)}",
                        )
                    }
                },
                enabled = uploadEnabled,
                colors = oraculoPrimaryButtonColors(),
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightSmall),
            ) {
                Icon(
                    imageVector = Icons.Default.Upload,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(modifier = Modifier.width(6.dp))
                Text(
                    text = tr("Enviar", "Upload"),
                    style = MaterialTheme.typography.labelLarge,
                    fontSize = 13.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }

        // Atalhos rápidos + ferramentas compactas (busca, ocultos, atualizar)
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            val quickPaths = listing?.quickPaths.orEmpty()
            Row(
                modifier = Modifier
                    .weight(1f)
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                quickPaths.forEach { quick ->
                    val selected = currentPath == quick.relPath
                    val chipLabel = if (quick.relPath.isEmpty()) {
                        tr("Início (~)", "Home (~)")
                    } else {
                        quick.name
                    }
                    FilterChip(
                        selected = selected,
                        onClick = { loadDirectory(quick.relPath) },
                        enabled = !isLoading && allowFilesRead != false,
                        leadingIcon = if (quick.relPath.isEmpty()) {
                            {
                                Icon(
                                    imageVector = Icons.Default.Home,
                                    contentDescription = null,
                                    modifier = Modifier.size(FilterChipDefaults.IconSize),
                                )
                            }
                        } else {
                            null
                        },
                        label = {
                            Text(
                                text = chipLabel,
                                maxLines = 1,
                            )
                        },
                        colors = FilterChipDefaults.filterChipColors(
                            containerColor = OraculoColors.Surface,
                            labelColor = OraculoColors.OnSurface,
                            selectedContainerColor = OraculoColors.SecondaryContainer,
                            selectedLabelColor = OraculoColors.OnSecondaryContainer,
                            selectedLeadingIconColor = OraculoColors.OnSecondaryContainer,
                        ),
                        border = FilterChipDefaults.filterChipBorder(
                            enabled = !isLoading && allowFilesRead != false,
                            selected = selected,
                            borderColor = OraculoColors.Outline,
                            selectedBorderColor = OraculoColors.SecondaryContainer,
                        ),
                        modifier = Modifier.height(OraculoMetrics.ControlHeightCompactChip),
                    )
                }
            }

            val searchActive = isSearchOpen || searchQuery.isNotEmpty()
            if (searchActive) {
                FilledTonalIconButton(
                    onClick = {
                        isSearchOpen = false
                        searchQuery = ""
                    },
                    enabled = allowFilesRead != false,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = OraculoColors.SecondaryContainer,
                        contentColor = OraculoColors.OnSecondaryContainer,
                    ),
                    modifier = Modifier.size(40.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.Search,
                        contentDescription = tr("Filtrar arquivos", "Filter files"),
                        modifier = Modifier.size(19.dp),
                    )
                }
            } else {
                IconButton(
                    onClick = { isSearchOpen = true },
                    enabled = allowFilesRead != false,
                    modifier = Modifier.size(40.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.Search,
                        contentDescription = tr("Filtrar arquivos", "Filter files"),
                        tint = OraculoColors.OnSurfaceMuted,
                        modifier = Modifier.size(19.dp),
                    )
                }
            }

            if (showHidden) {
                FilledTonalIconButton(
                    onClick = {
                        showHidden = false
                        loadDirectory(currentPath, hidden = false)
                    },
                    enabled = allowFilesRead != false,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(
                        containerColor = OraculoColors.SecondaryContainer,
                        contentColor = OraculoColors.OnSecondaryContainer,
                    ),
                    modifier = Modifier.size(40.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.Visibility,
                        contentDescription = tr("Ocultar arquivos ocultos", "Hide hidden files"),
                        modifier = Modifier.size(19.dp),
                    )
                }
            } else {
                IconButton(
                    onClick = {
                        showHidden = true
                        loadDirectory(currentPath, hidden = true)
                    },
                    enabled = allowFilesRead != false,
                    modifier = Modifier.size(40.dp),
                ) {
                    Icon(
                        imageVector = Icons.Default.VisibilityOff,
                        contentDescription = tr("Mostrar arquivos ocultos", "Show hidden files"),
                        tint = OraculoColors.OnSurfaceMuted,
                        modifier = Modifier.size(19.dp),
                    )
                }
            }

            IconButton(
                onClick = { loadDirectory(currentPath) },
                enabled = !isLoading && allowFilesRead != false,
                modifier = Modifier.size(40.dp),
            ) {
                Icon(
                    imageVector = Icons.Default.Refresh,
                    contentDescription = tr("Atualizar pasta", "Refresh folder"),
                    tint = OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(19.dp),
                )
            }
        }

        // Busca local recolhível para liberar altura da lista
        AnimatedVisibility(
            visible = isSearchOpen || searchQuery.isNotEmpty(),
            enter = OraculoMotion.ExpandV,
            exit = OraculoMotion.CollapseV,
        ) {
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                placeholder = { Text(tr("Filtrar nesta pasta...", "Filter in this folder..."), fontSize = 13.sp) },
                singleLine = true,
                leadingIcon = {
                    Icon(
                        imageVector = Icons.Default.Search,
                        contentDescription = null,
                        tint = OraculoColors.OnSurfaceMuted,
                        modifier = Modifier.size(17.dp),
                    )
                },
                trailingIcon = {
                    IconButton(
                        onClick = {
                            searchQuery = ""
                            isSearchOpen = false
                        },
                        modifier = Modifier.size(40.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Default.Close,
                            contentDescription = tr("Fechar busca", "Close search"),
                            tint = OraculoColors.OnSurfaceMuted,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                },
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = OraculoColors.Primary,
                    unfocusedBorderColor = OraculoColors.Outline,
                    focusedContainerColor = OraculoColors.Surface,
                    unfocusedContainerColor = OraculoColors.Surface,
                    focusedTextColor = OraculoColors.OnSurface,
                    unfocusedTextColor = OraculoColors.OnSurface,
                ),
                modifier = Modifier.fillMaxWidth(),
            )
        }

        AnimatedVisibility(
            visible = transferState != null,
            enter = OraculoMotion.ExpandV,
            exit = OraculoMotion.CollapseV,
        ) {
            val active = transferState
            if (active != null) {
                val fraction = if (active.totalBytes > 0L) {
                    (active.transferredBytes.toFloat() / active.totalBytes.toFloat()).coerceIn(0f, 1f)
                } else {
                    0f
                }
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                        .background(OraculoColors.Surface)
                        .border(1.dp, OraculoColors.Primary.copy(alpha = 0.45f), RoundedCornerShape(OraculoMetrics.CornerMedium))
                        .padding(horizontal = 12.dp, vertical = 10.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = active.title,
                            style = MaterialTheme.typography.labelLarge,
                            color = OraculoColors.OnSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = if (active.totalBytes > 0L) {
                                "${formatFileSize(active.transferredBytes)} / ${formatFileSize(active.totalBytes)}"
                            } else {
                                formatFileSize(active.transferredBytes)
                            },
                            style = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 11.sp),
                            color = OraculoColors.Primary,
                        )
                    }
                    if (active.totalBytes > 0L) {
                        LinearProgressIndicator(
                            progress = { fraction },
                            color = OraculoColors.Primary,
                            trackColor = OraculoColors.SurfaceRecessed,
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(5.dp)
                                .clip(RoundedCornerShape(999.dp)),
                        )
                    } else {
                        LinearProgressIndicator(
                            color = OraculoColors.Primary,
                            trackColor = OraculoColors.SurfaceRecessed,
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(5.dp)
                                .clip(RoundedCornerShape(999.dp)),
                        )
                    }
                }
            }
        }

        if (statusMessage != null) {
            Surface(
                color = OraculoColors.SecondaryContainer,
                shape = RoundedCornerShape(OraculoMetrics.CornerMedium),
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, OraculoColors.Primary.copy(alpha = 0.4f), RoundedCornerShape(OraculoMetrics.CornerMedium)),
            ) {
                Row(
                    modifier = Modifier.padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(
                        text = statusMessage.orEmpty(),
                        color = OraculoColors.OnSecondaryContainer,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = { statusMessage = null }, modifier = Modifier.size(36.dp)) {
                        Icon(
                            imageVector = Icons.Default.Close,
                            contentDescription = tr("Fechar aviso", "Dismiss notice"),
                            tint = OraculoColors.OnSecondaryContainer,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }
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

        if (isLoading && listing == null) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator(color = OraculoColors.Primary)
            }
        } else if (visibleEntries.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = when {
                        allowFilesRead == false -> tr(
                            "Navegação de arquivos desativada no computador.",
                            "File browsing is disabled on your computer.",
                        )
                        searchQuery.isBlank() -> if (allowFilesWrite == false) {
                            tr("Pasta vazia.", "Empty folder.")
                        } else {
                            tr(
                                "Pasta vazia.\nToque em \"Enviar\" acima para mandar arquivos do celular.",
                                "Empty folder.\nTap \"Upload\" above to send files from your phone.",
                            )
                        }
                        else -> tr(
                            "Nenhum arquivo corresponde a \"$searchQuery\".",
                            "No files match \"$searchQuery\".",
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
                itemsIndexed(visibleEntries, key = { _, entry -> entry.relPath }) { index, entry ->
                    val itemShape = remember(index, visibleEntries.size) {
                        oraculoSegmentedItemShape(index, visibleEntries.size)
                    }
                    FileEntryRow(
                        entry = entry,
                        shape = itemShape,
                        onClick = {
                            if (allowFilesRead == false) return@FileEntryRow
                            if (entry.isDir) {
                                searchQuery = ""
                                loadDirectory(entry.relPath)
                            } else {
                                selectedEntry = entry
                            }
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun FileEntryActionDialog(
    entry: RemoteFileEntry,
    onDismiss: () -> Unit,
    onPreview: () -> Unit,
    onDownload: () -> Unit,
    onOpen: () -> Unit,
    onShare: () -> Unit,
) {
    val canPreviewImage = isImageEntry(entry) && entry.size.toLong() <= 15L * 1024L * 1024L
    val canPreviewText = isTextEntry(entry) && entry.size.toLong() <= 512L * 1024L

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = OraculoColors.Surface,
        title = {
            Text(
                text = entry.name,
                color = OraculoColors.OnSurface,
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
                    text = "~/${entry.relPath}",
                    style = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp),
                    color = OraculoColors.OnSurfaceMuted,
                )
                Text(
                    text = "${formatFileSize(entry.size.toLong())} • ${formatUnixDate(entry.modifiedUnix.toLong())}",
                    style = MaterialTheme.typography.bodySmall,
                    color = OraculoColors.OnSurfaceMuted,
                )

                if (canPreviewImage || canPreviewText) {
                    Button(
                        onClick = onPreview,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OraculoColors.SecondaryContainer,
                            contentColor = OraculoColors.OnSecondaryContainer,
                        ),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 10.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = OraculoMetrics.ControlHeightDialog),
                    ) {
                        Icon(
                            imageVector = if (canPreviewImage) Icons.Default.Image else Icons.Default.Description,
                            contentDescription = null,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = if (canPreviewImage) {
                                tr("Visualizar imagem", "Preview image")
                            } else {
                                tr("Visualizar texto", "Preview text")
                            },
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }

                Button(
                    onClick = onDownload,
                    colors = oraculoPrimaryButtonColors(),
                    shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 10.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(min = OraculoMetrics.ControlHeightDialog),
                ) {
                    Icon(
                        imageVector = Icons.Default.Download,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = tr("Baixar para Downloads", "Save to Downloads"),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedButton(
                        onClick = onOpen,
                        border = BorderStroke(1.dp, OraculoColors.Outline),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                        modifier = Modifier
                            .weight(1f)
                            .heightIn(min = OraculoMetrics.ControlHeightDialog),
                    ) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.OpenInNew,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(modifier = Modifier.width(5.dp))
                        Text(
                            text = tr("Abrir", "Open"),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    OutlinedButton(
                        onClick = onShare,
                        border = BorderStroke(1.dp, OraculoColors.Outline),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 8.dp),
                        modifier = Modifier
                            .weight(1f)
                            .heightIn(min = OraculoMetrics.ControlHeightDialog),
                    ) {
                        Icon(
                            imageVector = Icons.Default.Share,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(modifier = Modifier.width(5.dp))
                        Text(
                            text = tr("Compartilhar", "Share"),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = onDismiss,
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                Text(tr("Fechar", "Close"), color = OraculoColors.OnSurfaceMuted)
            }
        },
    )
}

@Composable
private fun FilePreviewDialog(
    previewState: FilePreviewState,
    onDismiss: () -> Unit,
    onDownload: (File, RemoteFileEntry) -> Unit,
) {
    val previewEntry = when (previewState) {
        is FilePreviewState.ImagePreview -> previewState.entry
        is FilePreviewState.TextPreview -> previewState.entry
    }
    val cachedFile = when (previewState) {
        is FilePreviewState.ImagePreview -> previewState.cachedFile
        is FilePreviewState.TextPreview -> previewState.cachedFile
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = OraculoColors.Surface,
        title = {
            Text(
                text = previewEntry.name,
                color = OraculoColors.OnSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        },
        text = {
            when (previewState) {
                is FilePreviewState.ImagePreview -> {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 180.dp, max = 380.dp)
                            .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                            .background(OraculoColors.TerminalBackground),
                        contentAlignment = Alignment.Center,
                    ) {
                        Image(
                            bitmap = previewState.bitmap.asImageBitmap(),
                            contentDescription = previewEntry.name,
                            contentScale = ContentScale.Fit,
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }
                is FilePreviewState.TextPreview -> {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 160.dp, max = 380.dp)
                            .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                            .background(OraculoColors.TerminalBackground)
                            .border(1.dp, OraculoColors.TerminalBorder, RoundedCornerShape(OraculoMetrics.CornerMedium))
                            .padding(10.dp)
                            .verticalScroll(rememberScrollState())
                            .horizontalScroll(rememberScrollState()),
                    ) {
                        SelectionContainer {
                            Text(
                                text = previewState.content.ifEmpty { tr("(arquivo vazio)", "(empty file)") },
                                style = TextStyle(
                                    fontFamily = FontFamily.Monospace,
                                    fontSize = 11.5.sp,
                                    lineHeight = 16.sp,
                                ),
                                color = OraculoColors.TerminalForeground,
                                softWrap = false,
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            Button(
                onClick = { onDownload(cachedFile, previewEntry) },
                colors = oraculoPrimaryButtonColors(),
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                Text(tr("Baixar", "Download"))
            }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                modifier = Modifier.heightIn(min = OraculoMetrics.ControlHeightDialog),
            ) {
                Text(tr("Fechar", "Close"), color = OraculoColors.OnSurfaceMuted)
            }
        },
    )
}

@Composable
private fun FileEntryRow(
    entry: RemoteFileEntry,
    shape: Shape,
    onClick: () -> Unit,
) {
    val icon = remember(entry.isDir, entry.mimeHint, entry.name) {
        iconForFileEntry(entry)
    }
    Surface(
        onClick = onClick,
        color = OraculoColors.Surface,
        shape = shape,
        border = if (OraculoColors.IsDark) null else BorderStroke(1.dp, OraculoColors.Outline),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(11.dp),
        ) {
            Box(
                modifier = Modifier
                    .size(38.dp)
                    .clip(RoundedCornerShape(OraculoMetrics.CornerMedium))
                    .background(
                        if (entry.isDir) OraculoColors.SecondaryContainer else OraculoColors.SurfaceRecessed,
                    ),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = if (entry.isDir) OraculoColors.OnSecondaryContainer else OraculoColors.OnSurfaceMuted,
                    modifier = Modifier.size(20.dp),
                )
            }

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = entry.name,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = if (entry.isDir) FontWeight.SemiBold else FontWeight.Medium,
                    color = OraculoColors.OnSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(modifier = Modifier.height(2.dp))
                val subtitle = if (entry.isDir) {
                    "${tr("Pasta", "Folder")} • ${formatUnixDate(entry.modifiedUnix.toLong())}"
                } else {
                    "${formatFileSize(entry.size.toLong())} • ${formatUnixDate(entry.modifiedUnix.toLong())}"
                }
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = OraculoColors.OnSurfaceMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            if (!entry.isDir) {
                val ext = entry.name.substringAfterLast('.', "").uppercase(Locale.ROOT)
                if (ext.isNotEmpty() && ext.length <= 5 && entry.name.contains('.')) {
                    Text(
                        text = ext,
                        style = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 10.sp),
                        color = OraculoColors.OnSurfaceMuted,
                        modifier = Modifier
                            .clip(RoundedCornerShape(OraculoMetrics.CornerSmall))
                            .background(OraculoColors.SurfaceRecessed)
                            .padding(horizontal = 6.dp, vertical = 3.dp),
                    )
                }
            }
        }
    }
}

private fun cleanupOldCachedFiles(context: Context, maxAgeMs: Long) {
    runCatching {
        val dir = File(context.cacheDir, "oraculo_files")
        if (!dir.isDirectory) return
        val cutoff = System.currentTimeMillis() - maxAgeMs
        dir.listFiles()?.forEach { f ->
            if (f.isFile && (maxAgeMs <= 0L || f.lastModified() <= cutoff)) {
                runCatching { f.delete() }
            }
        }
    }
}

private fun iconForFileEntry(entry: RemoteFileEntry): ImageVector {
    if (entry.isDir) return Icons.Default.Folder
    val mime = entry.mimeHint
    return when {
        mime.startsWith("image/") -> Icons.Default.Image
        mime.startsWith("video/") -> Icons.Default.VideoFile
        mime.startsWith("audio/") -> Icons.Default.AudioFile
        mime == "application/json" || mime == "application/xml" || entry.name.endsWith(".rs") ||
            entry.name.endsWith(".kt") || entry.name.endsWith(".py") || entry.name.endsWith(".sh") ||
            entry.name.endsWith(".toml") || entry.name.endsWith(".yaml") || entry.name.endsWith(".yml") ->
            Icons.Default.Code
        mime.startsWith("text/") -> Icons.Default.Description
        else -> Icons.AutoMirrored.Filled.InsertDriveFile
    }
}

private fun isImageEntry(entry: RemoteFileEntry): Boolean =
    entry.mimeHint == "image/png" ||
        entry.mimeHint == "image/jpeg" ||
        entry.mimeHint == "image/webp" ||
        entry.mimeHint == "image/gif" ||
        entry.mimeHint == "image/bmp"

private fun isTextEntry(entry: RemoteFileEntry): Boolean =
    entry.mimeHint.startsWith("text/") ||
        entry.mimeHint == "application/json" ||
        entry.mimeHint == "application/xml"

private fun decodeScaledBitmap(file: File, maxSidePx: Int): Bitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(file.absolutePath, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null

    var sampleSize = 1
    var w = bounds.outWidth
    var h = bounds.outHeight
    while (w > maxSidePx || h > maxSidePx) {
        w /= 2
        h /= 2
        sampleSize *= 2
    }
    val opts = BitmapFactory.Options().apply { inSampleSize = sampleSize }
    return BitmapFactory.decodeFile(file.absolutePath, opts)
}

private fun resolveDocumentDisplayName(context: Context, uri: Uri): String {
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor ->
            if (cursor.moveToFirst()) {
                val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (idx >= 0) {
                    val value = cursor.getString(idx)?.trim()
                    if (!value.isNullOrEmpty()) return value
                }
            }
        }
    return uri.lastPathSegment?.substringAfterLast('/')?.ifBlank { null }
        ?: "${tr("arquivo", "file")}_${System.currentTimeMillis()}"
}

private fun sanitizeDownloadFileName(raw: String): String {
    val cleaned = raw
        .replace('\u0000', '_')
        .replace('/', '_')
        .replace('\\', '_')
        .trim()
    return if (cleaned.isEmpty() || cleaned == "." || cleaned == "..") {
        tr("arquivo", "file")
    } else {
        cleaned
    }
}

private fun saveFileToDownloads(
    context: Context,
    sourceFile: File,
    fileName: String,
    mimeHint: String,
): String {
    val safeName = sanitizeDownloadFileName(fileName)
    val cleanMime = mimeHint.ifBlank { "application/octet-stream" }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        val resolver = context.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, safeName)
            put(MediaStore.Downloads.MIME_TYPE, cleanMime)
            put(MediaStore.Downloads.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/Oraculo")
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: error(tr("Falha ao criar entrada em Downloads", "Failed to create entry in Downloads"))
        try {
            resolver.openOutputStream(uri)?.use { out ->
                sourceFile.inputStream().use { input ->
                    input.copyTo(out, bufferSize = 64 * 1024)
                }
            } ?: error(tr("Falha ao abrir destino em Downloads", "Failed to open destination stream in Downloads"))

            values.clear()
            values.put(MediaStore.Downloads.IS_PENDING, 0)
            resolver.update(uri, values, null, null)
            return "Downloads/Oraculo/$safeName"
        } catch (err: Throwable) {
            runCatching { resolver.delete(uri, null, null) }
            throw err
        }
    }

    @Suppress("DEPRECATION")
    val downloadsDir = File(
        Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS),
        "Oraculo",
    )
    downloadsDir.mkdirs()
    var target = File(downloadsDir, safeName)
    if (target.exists()) {
        val stem = safeName.substringBeforeLast('.', safeName)
        val ext = safeName.substringAfterLast('.', "")
        var idx = 1
        while (target.exists()) {
            val nextName = if (ext.isNotEmpty() && safeName.contains('.')) {
                "$stem ($idx).$ext"
            } else {
                "$stem ($idx)"
            }
            target = File(downloadsDir, nextName)
            idx++
        }
    }
    sourceFile.inputStream().use { input ->
        FileOutputStream(target).use { out ->
            input.copyTo(out, bufferSize = 64 * 1024)
        }
    }
    return "Downloads/Oraculo/${target.name}"
}

private fun launchExternalFileIntent(
    context: Context,
    cachedFile: File,
    mimeHint: String,
    shareOnly: Boolean,
): Result<Unit> = runCatching {
    val uri = FileProvider.getUriForFile(
        context,
        "${context.packageName}.fileprovider",
        cachedFile,
    )
    val effectiveMime = mimeHint.ifBlank { "*/*" }
    val intent = if (shareOnly) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = effectiveMime
            putExtra(Intent.EXTRA_STREAM, uri)
            clipData = ClipData.newRawUri("", uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        Intent.createChooser(send, tr("Compartilhar arquivo", "Share file")).apply {
            clipData = ClipData.newRawUri("", uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
    } else {
        Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, effectiveMime)
            clipData = ClipData.newRawUri("", uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
    }
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        if (!shareOnly) {
            val fallback = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "*/*")
                clipData = ClipData.newRawUri("", uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            val chooser = Intent.createChooser(fallback, tr("Abrir arquivo com", "Open file with")).apply {
                clipData = ClipData.newRawUri("", uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            context.startActivity(chooser)
        } else {
            error(tr("Nenhum aplicativo disponível para compartilhar este arquivo", "No app available to share this file"))
        }
    }
}

private fun formatFileSize(bytes: Long): String {
    if (bytes < 1024L) return "$bytes B"
    val kb = bytes / 1024.0
    if (kb < 1024.0) return String.format(Locale.ROOT, "%.1f KB", kb)
    val mb = kb / 1024.0
    if (mb < 1024.0) return String.format(Locale.ROOT, "%.1f MB", mb)
    val gb = mb / 1024.0
    return String.format(Locale.ROOT, "%.2f GB", gb)
}

private fun formatUnixDate(epochSeconds: Long): String {
    if (epochSeconds <= 0L) return "--"
    val isPt = AppI18n.isPt
    val pattern = if (isPt) "dd/MM HH:mm" else "MMM d, HH:mm"
    val locale = if (isPt) Locale.forLanguageTag("pt-BR") else Locale.US
    return SimpleDateFormat(pattern, locale).format(Date(epochSeconds * 1000L))
}
