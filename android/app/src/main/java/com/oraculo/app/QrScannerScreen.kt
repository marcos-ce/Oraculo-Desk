package com.oraculo.app

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
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
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.core.content.ContextCompat
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
import com.oraculo.app.ui.OraculoMetrics
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
    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED
        )
    }
    val defaultHint = tr(
        "Aponte a câmera para o QR Code exibido no terminal do PC",
        "Point the camera at the QR Code displayed on the PC terminal",
    )
    var scanHint by remember {
        mutableStateOf(defaultHint)
    }

    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { granted ->
        hasCameraPermission = granted
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
                        if (text.startsWith("oraculo://pair?")) {
                            onQrScanned(text)
                            true
                        } else {
                            scanHint = tr(
                                "QR Code reconhecido não é do Oráculo (use oraculo://pair?...)",
                                "Scanned QR Code is not an Oráculo link (use oraculo://pair?...)",
                            )
                            false
                        }
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
                        shape = RoundedCornerShape(16.dp),
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
                        shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Icon(
                            imageVector = Icons.Default.EditNote,
                            contentDescription = null,
                        )
                        Spacer(modifier = Modifier.padding(horizontal = 4.dp))
                        Text(tr("Colar link oraculo://pair manualmente", "Paste oraculo://pair link manually"))
                    }
                }
            } else {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(24.dp),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        imageVector = Icons.Default.QrCodeScanner,
                        contentDescription = null,
                        tint = OraculoColors.Primary,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = tr("Permissão de câmera necessária", "Camera permission required"),
                        style = MaterialTheme.typography.titleMedium,
                        color = OraculoColors.OnSurface,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = tr(
                            "Libere a câmera para ler o QR Code gerado por ./dist/oraculo-host serve --pair, ou cole o link manualmente.",
                            "Allow camera access to scan the QR Code generated by oraculo-host serve --pair, or paste the link manually.",
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                        color = OraculoColors.OnSurfaceMuted,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(modifier = Modifier.height(20.dp))
                    Button(
                        onClick = { permissionLauncher.launch(Manifest.permission.CAMERA) },
                        shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = OraculoColors.Primary,
                            contentColor = OraculoColors.OnPrimary,
                        ),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Text(tr("Permitir Câmera", "Allow Camera"))
                    }
                    Spacer(modifier = Modifier.height(10.dp))
                    OutlinedButton(
                        onClick = onOpenManualInput,
                        shape = RoundedCornerShape(OraculoMetrics.CornerLarge),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(OraculoMetrics.ControlHeightCta),
                    ) {
                        Text(tr("Colar link manual", "Paste link manually"))
                    }
                }
            }
        }
    }
}

@Composable
private fun ScannerOverlay(modifier: Modifier = Modifier) {
    val scrimColor = Color(0xFF0C0E12).copy(alpha = 0.68f)
    val bracketColor = OraculoColors.Primary
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

@Composable
private fun CameraQrPreview(
    onDecoded: (String) -> Boolean,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val lifecycleOwner = remember(context) { context.findLifecycleOwner() } ?: return
    val previewView = remember {
        PreviewView(context).apply {
            implementationMode = PreviewView.ImplementationMode.COMPATIBLE
        }
    }

    DisposableEffect(lifecycleOwner, previewView) {
        val executor = Executors.newSingleThreadExecutor()
        val cameraProviderFuture = ProcessCameraProvider.getInstance(context)
        val alreadyHandled = AtomicBoolean(false)
        val reader = MultiFormatReader().apply {
            setHints(
                mapOf(
                    DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
                    DecodeHintType.TRY_HARDER to true,
                )
            )
        }
        var boundProvider: ProcessCameraProvider? = null
        var disposed = false

        cameraProviderFuture.addListener(
            {
                if (disposed) return@addListener
                val provider = runCatching { cameraProviderFuture.get() }.getOrNull() ?: return@addListener
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
                                if (!alreadyHandled.get()) {
                                    val decoded = decodeQrFromImage(reader, imageProxy)
                                    if (decoded != null) {
                                        previewView.post {
                                            if (!alreadyHandled.get() && onDecoded(decoded)) {
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

                runCatching {
                    provider.unbindAll()
                    provider.bindToLifecycle(
                        lifecycleOwner,
                        CameraSelector.DEFAULT_BACK_CAMERA,
                        preview,
                        analysis,
                    )
                }
            },
            ContextCompat.getMainExecutor(context),
        )

        onDispose {
            disposed = true
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

private fun decodeQrFromImage(reader: MultiFormatReader, image: ImageProxy): String? {
    val plane = image.planes.firstOrNull() ?: return null
    val width = image.width
    val height = image.height
    if (width <= 0 || height <= 0) return null

    val buffer = plane.buffer
    buffer.rewind()
    val rowStride = plane.rowStride
    val pixelStride = plane.pixelStride

    val yBytes = ByteArray(width * height)
    if (pixelStride == 1 && rowStride == width) {
        buffer.get(yBytes)
    } else {
        val rowBuffer = ByteArray(rowStride)
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
