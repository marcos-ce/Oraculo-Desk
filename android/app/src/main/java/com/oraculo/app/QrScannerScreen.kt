package com.oraculo.app

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import android.util.Size as AndroidSize
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.EditNote
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.LuminanceSource
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.GlobalHistogramBinarizer
import com.google.zxing.common.HybridBinarizer
import com.oraculo.app.ui.OraculoColors
import com.oraculo.app.ui.OraculoDarkOverlayPalette
import com.oraculo.app.ui.OraculoMetrics
import com.oraculo.app.ui.oraculoPrimaryButtonColors
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QrScannerScreen(
    onQrScanned: (String) -> Unit,
    onOpenManualInput: () -> Unit,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() }
    val activity = remember(context) { context as? Activity }
    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED
        )
    }
    var permanentlyDenied by remember { mutableStateOf(false) }
    val defaultHint = tr(
        "Aponte a câmera para o QR Code do computador ou da ponte",
        "Point the camera at your computer or bridge QR Code",
    )
    var scanHint by remember(AppI18n.lang) {
        mutableStateOf(defaultHint)
    }

    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { granted ->
        hasCameraPermission = granted
        permanentlyDenied = !granted && activity != null &&
            !ActivityCompat.shouldShowRequestPermissionRationale(activity, Manifest.permission.CAMERA)
    }

    DisposableEffect(lifecycleOwner) {
        val owner = lifecycleOwner ?: return@DisposableEffect onDispose {}
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                val grantedNow = ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                    PackageManager.PERMISSION_GRANTED
                hasCameraPermission = grantedNow
                if (grantedNow) {
                    permanentlyDenied = false
                }
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose { owner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(Unit) {
        if (!hasCameraPermission) {
            permissionLauncher.launch(Manifest.permission.CAMERA)
        }
    }

    Scaffold(
        containerColor = OraculoColors.Background,
        topBar = {
            TopAppBar(
                title = { Text(tr("Escanear QR Code", "Scan QR Code")) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = tr("Voltar", "Back"),
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = OraculoColors.Background,
                    titleContentColor = OraculoColors.OnSurface,
                    navigationIconContentColor = OraculoColors.OnSurface,
                ),
            )
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            if (hasCameraPermission) {
                CameraQrPreview(
                    onDecoded = { text ->
                        val normalized = text.trim()
                        if (
                            normalized.startsWith("oraculo://pair?", ignoreCase = true) ||
                            normalized.startsWith("oraculo://mcu-bridge?", ignoreCase = true)
                        ) {
                            onQrScanned(normalized)
                            true
                        } else {
                            scanHint = tr(
                                "Esse QR Code não é do Oráculo.",
                                "This QR Code isn't from Oráculo.",
                            )
                            false
                        }
                    },
                    onCameraError = { errMsg ->
                        scanHint = errMsg
                    },
                    modifier = Modifier.fillMaxSize(),
                )

                ScannerOverlay(modifier = Modifier.fillMaxSize())

                Column(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Surface(
                        color = OraculoColors.Background.copy(alpha = 0.88f),
                        shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
                    ) {
                        Text(
                            text = scanHint,
                            style = MaterialTheme.typography.bodyMedium,
                            color = OraculoColors.OnSurface,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                        )
                    }

                    Button(
                        onClick = onOpenManualInput,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OraculoColors.SecondaryContainer,
                            contentColor = OraculoColors.OnSecondaryContainer,
                        ),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = OraculoMetrics.ControlHeightCta),
                    ) {
                        Icon(
                            imageVector = Icons.Default.EditNote,
                            contentDescription = null,
                        )
                        Spacer(modifier = Modifier.padding(horizontal = 4.dp))
                        Text(tr("Digitar código manualmente", "Enter code manually"))
                    }
                }
            } else {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(24.dp),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        imageVector = Icons.Default.QrCodeScanner,
                        contentDescription = null,
                        tint = OraculoColors.Primary,
                        modifier = Modifier.size(48.dp),
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = tr("Permissão de câmera necessária", "Camera permission required"),
                        style = MaterialTheme.typography.titleMedium,
                        color = OraculoColors.OnSurface,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = if (permanentlyDenied) {
                            tr(
                                "O acesso à câmera foi negado. Abra as configurações do app para liberar a câmera ou digite o código de 8 caracteres.",
                                "Camera access was denied. Open app settings to enable the camera, or enter the 8-character code.",
                            )
                        } else {
                            tr(
                                "Libere a câmera para ler o QR Code do computador ou da ponte, ou digite o código de 8 caracteres.",
                                "Allow camera access to scan a computer or bridge QR Code, or enter the 8-character code.",
                            )
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(modifier = Modifier.height(20.dp))
                    Button(
                        onClick = {
                            if (permanentlyDenied) {
                                runCatching {
                                    context.startActivity(
                                        Intent(
                                            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                                            Uri.fromParts("package", context.packageName, null),
                                        ),
                                    )
                                }
                            } else {
                                permissionLauncher.launch(Manifest.permission.CAMERA)
                            }
                        },
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        colors = oraculoPrimaryButtonColors(),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = OraculoMetrics.ControlHeightCta),
                    ) {
                        Text(
                            if (permanentlyDenied) {
                                tr("Abrir configurações", "Open settings")
                            } else {
                                tr("Permitir câmera", "Allow camera")
                            },
                        )
                    }
                    Spacer(modifier = Modifier.height(10.dp))
                    OutlinedButton(
                        onClick = onOpenManualInput,
                        border = BorderStroke(1.dp, OraculoColors.Outline),
                        shape = RoundedCornerShape(OraculoMetrics.CornerPill),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = OraculoMetrics.ControlHeightCta),
                    ) {
                        Text(tr("Digitar código", "Enter code"))
                    }
                }
            }
        }
    }
}

@Composable
private fun ScannerOverlay(modifier: Modifier = Modifier) {
    val scrimColor = OraculoDarkOverlayPalette.TerminalBackground.copy(alpha = 0.72f)
    val bracketColor = OraculoDarkOverlayPalette.Primary
    Canvas(
        modifier = modifier.graphicsLayer {
            compositingStrategy = CompositingStrategy.Offscreen
        }
    ) {
        val cutoutSize = size.minDimension * 0.64f
        val left = (size.width - cutoutSize) / 2f
        val top = (size.height - cutoutSize) / 2.2f
        val right = left + cutoutSize
        val bottom = top + cutoutSize
        val radius = 22.dp.toPx()
        val arm = 26.dp.toPx()

        drawRect(color = scrimColor)
        drawRoundRect(
            color = Color.Transparent,
            topLeft = Offset(left, top),
            size = Size(cutoutSize, cutoutSize),
            cornerRadius = CornerRadius(radius, radius),
            blendMode = BlendMode.Clear,
        )

        val brackets = Path().apply {
            // Top-left
            moveTo(left, top + arm)
            lineTo(left, top + radius)
            quadraticTo(left, top, left + radius, top)
            lineTo(left + arm, top)
            // Top-right
            moveTo(right - arm, top)
            lineTo(right - radius, top)
            quadraticTo(right, top, right, top + radius)
            lineTo(right, top + arm)
            // Bottom-left
            moveTo(left, bottom - arm)
            lineTo(left, bottom - radius)
            quadraticTo(left, bottom, left + radius, bottom)
            lineTo(left + arm, bottom)
            // Bottom-right
            moveTo(right - arm, bottom)
            lineTo(right - radius, bottom)
            quadraticTo(right, bottom, right, bottom - radius)
            lineTo(right, bottom - arm)
        }
        drawPath(
            path = brackets,
            color = bracketColor,
            style = Stroke(width = 3.5.dp.toPx(), cap = StrokeCap.Round),
        )
    }
}

private class QrDecodeScratch {
    var yBytes: ByteArray = ByteArray(0)
    var rowBuffer: ByteArray = ByteArray(0)
}

@Composable
private fun CameraQrPreview(
    onDecoded: (String) -> Boolean,
    onCameraError: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() } ?: return
    val currentOnDecoded by rememberUpdatedState(onDecoded)
    val currentOnCameraError by rememberUpdatedState(onCameraError)
    val previewView = remember {
        PreviewView(context).apply {
            implementationMode = PreviewView.ImplementationMode.COMPATIBLE
        }
    }

    DisposableEffect(lifecycleOwner, previewView) {
        val executor = Executors.newSingleThreadExecutor()
        val cameraProviderFuture = ProcessCameraProvider.getInstance(context)
        val alreadyHandled = AtomicBoolean(false)
        val scratch = QrDecodeScratch()
        val reader = MultiFormatReader().apply {
            setHints(
                mapOf(
                    DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
                    DecodeHintType.TRY_HARDER to true,
                )
            )
        }
        var boundProvider: ProcessCameraProvider? = null
        var boundAnalysis: ImageAnalysis? = null
        var disposed = false

        cameraProviderFuture.addListener(
            {
                if (disposed) return@addListener
                val provider = runCatching { cameraProviderFuture.get() }.getOrElse {
                    currentOnCameraError(
                        tr(
                            "Não foi possível iniciar a câmera. Digite o código manualmente.",
                            "Could not start the camera. Enter the code manually.",
                        ),
                    )
                    return@addListener
                }
                boundProvider = provider

                val preview = Preview.Builder().build().also {
                    it.setSurfaceProvider(previewView.surfaceProvider)
                }

                val resolutionSelector = ResolutionSelector.Builder()
                    .setResolutionStrategy(
                        ResolutionStrategy(
                            AndroidSize(1280, 720),
                            ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER,
                        )
                    )
                    .build()

                val analysis = ImageAnalysis.Builder()
                    .setResolutionSelector(resolutionSelector)
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                    .also { imageAnalysis ->
                        imageAnalysis.setAnalyzer(executor) { imageProxy ->
                            try {
                                if (!disposed && !alreadyHandled.get()) {
                                    val decoded = decodeQrFromImage(reader, imageProxy, scratch)
                                    if (decoded != null) {
                                        previewView.post {
                                            if (disposed) return@post
                                            if (!alreadyHandled.get() && currentOnDecoded(decoded)) {
                                                alreadyHandled.set(true)
                                            }
                                        }
                                    }
                                }
                            } finally {
                                imageProxy.close()
                            }
                        }
                    }
                boundAnalysis = analysis

                runCatching {
                    provider.unbindAll()
                    provider.bindToLifecycle(
                        lifecycleOwner,
                        CameraSelector.DEFAULT_BACK_CAMERA,
                        preview,
                        analysis,
                    )
                }.onFailure {
                    currentOnCameraError(
                        tr(
                            "Câmera traseira indisponível neste aparelho. Digite o código manualmente.",
                            "Rear camera unavailable on this device. Enter the code manually.",
                        ),
                    )
                }
            },
            ContextCompat.getMainExecutor(context),
        )

        onDispose {
            disposed = true
            runCatching { boundAnalysis?.clearAnalyzer() }
            runCatching { boundProvider?.unbindAll() }
            executor.shutdown()
        }
    }

    AndroidView(
        factory = { previewView },
        modifier = modifier.background(Color.Black),
    )
}

private fun tryDecodeLuminance(reader: MultiFormatReader, source: LuminanceSource): String? {
    val candidates = listOf(
        BinaryBitmap(HybridBinarizer(source)),
        BinaryBitmap(HybridBinarizer(source.invert())),
        BinaryBitmap(GlobalHistogramBinarizer(source)),
        BinaryBitmap(GlobalHistogramBinarizer(source.invert())),
    )
    for (bitmap in candidates) {
        try {
            return reader.decodeWithState(bitmap).text
        } catch (_: NotFoundException) {
            // tenta próxima estratégia de binarização/inversão
        } finally {
            reader.reset()
        }
    }
    return null
}

private fun decodeQrFromImage(
    reader: MultiFormatReader,
    image: ImageProxy,
    scratch: QrDecodeScratch,
): String? {
    val plane = image.planes.firstOrNull() ?: return null
    val width = image.width
    val height = image.height
    if (width <= 0 || height <= 0) return null

    val buffer = plane.buffer
    buffer.rewind()
    val rowStride = plane.rowStride
    val pixelStride = plane.pixelStride
    val needed = width * height
    if (needed <= 0) return null

    if (scratch.yBytes.size != needed) {
        scratch.yBytes = ByteArray(needed)
    }
    val yBytes = scratch.yBytes
    if (pixelStride == 1 && rowStride == width) {
        val toRead = minOf(needed, buffer.remaining())
        if (toRead < needed) {
            yBytes.fill(0)
        }
        buffer.get(yBytes, 0, toRead)
    } else {
        if (scratch.rowBuffer.size < rowStride) {
            scratch.rowBuffer = ByteArray(rowStride)
        }
        val rowBuffer = scratch.rowBuffer
        for (row in 0 until height) {
            val remaining = buffer.remaining()
            if (remaining <= 0) break
            val toRead = minOf(rowStride, remaining)
            buffer.get(rowBuffer, 0, toRead)
            val outOffset = row * width
            if (pixelStride == 1) {
                System.arraycopy(rowBuffer, 0, yBytes, outOffset, minOf(width, toRead))
            } else {
                for (col in 0 until width) {
                    val srcIndex = col * pixelStride
                    if (srcIndex < toRead) {
                        yBytes[outOffset + col] = rowBuffer[srcIndex]
                    }
                }
            }
        }
    }

    val cropSide = (minOf(width, height) * 0.72f).toInt().coerceAtLeast(1)
    val cropLeft = (width - cropSide) / 2
    val cropTop = (height - cropSide) / 2
    val centeredSource = PlanarYUVLuminanceSource(
        yBytes,
        width,
        height,
        cropLeft,
        cropTop,
        cropSide,
        cropSide,
        false,
    )
    tryDecodeLuminance(reader, centeredSource)?.let { return it }

    val fullSource = PlanarYUVLuminanceSource(
        yBytes,
        width,
        height,
        0,
        0,
        width,
        height,
        false,
    )
    return tryDecodeLuminance(reader, fullSource)
}
