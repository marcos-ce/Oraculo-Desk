package com.oraculo.app

import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.Process
import android.util.Log
import android.view.Surface
import uniffi.oraculo_mobile_core.ScreenStreamInfo
import uniffi.oraculo_mobile_core.ScreenVideoFrame

/**
 * Decodificador H.264 Annex-B em hardware via [MediaCodec] em modo assíncrono (`Callback`)
 * sobre [HandlerThread] dedicada, renderizando cada frame diretamente no [Surface] assim que
 * a decodificação termina — sem esperar pela chegada do próximo frame na rede.
 */
class ScreenVideoDecoder(
    private val onVideoSizeChanged: (width: Int, height: Int) -> Unit = { _, _ -> },
    private val onRequestKeyframe: () -> Unit = {},
    private val onDecoderError: (message: String) -> Unit = {},
) {
    private val lock = Any()
    private val codecThread = HandlerThread("OraculoVideoCodec", Process.THREAD_PRIORITY_DISPLAY).apply {
        start()
    }
    private val codecHandler = Handler(codecThread.looper)

    private val availableInputs = ArrayDeque<Int>()
    private val pendingFrames = ArrayDeque<ScreenVideoFrame>()

    private var surface: Surface? = null
    private var codec: MediaCodec? = null
    private var codecGeneration: Long = 0L
    private var streamWidth: Int = 0
    private var streamHeight: Int = 0
    private var configuredWidth: Int = 0
    private var configuredHeight: Int = 0
    private var awaitingKeyframe: Boolean = true
    private var released: Boolean = false

    fun attachSurface(newSurface: Surface) {
        var staleCodec: MediaCodec? = null
        var shouldRequestIdr = false
        synchronized(lock) {
            if (released) return
            if (surface === newSurface && codec != null) return
            staleCodec = releaseCodecStateLocked()
            surface = newSurface
            awaitingKeyframe = true
            shouldRequestIdr = streamWidth > 0 && streamHeight > 0
        }
        stopAndReleaseCodec(staleCodec)
        if (shouldRequestIdr) {
            onRequestKeyframe()
        }
    }

    fun detachSurface(oldSurface: Surface? = null) {
        var staleCodec: MediaCodec? = null
        synchronized(lock) {
            if (oldSurface != null && surface != null && surface !== oldSurface) {
                return
            }
            surface = null
            awaitingKeyframe = true
            staleCodec = releaseCodecStateLocked()
        }
        stopAndReleaseCodec(staleCodec)
    }

    fun onStreamReady(info: ScreenStreamInfo) {
        val nextW = info.width.toInt().coerceIn(160, 3840)
        val nextH = info.height.toInt().coerceIn(64, 2160)
        var sizeChanged = false
        var staleCodec: MediaCodec? = null

        synchronized(lock) {
            if (released) return
            sizeChanged = streamWidth != nextW || streamHeight != nextH
            if (configuredWidth != nextW || configuredHeight != nextH) {
                staleCodec = releaseCodecStateLocked()
            }
            streamWidth = nextW
            streamHeight = nextH
            awaitingKeyframe = true
        }

        stopAndReleaseCodec(staleCodec)
        if (sizeChanged) {
            onVideoSizeChanged(nextW, nextH)
        }
    }

    fun submitFrame(frame: ScreenVideoFrame) {
        var staleToStop: MediaCodec? = null
        var staleBeforeConfigure: MediaCodec? = null
        var needKeyframeRequest = false
        var errorMessage: String? = null

        var targetW = 0
        var targetH = 0
        var targetSurface: Surface? = null
        var needsConfigure = false

        synchronized(lock) {
            if (released) return
            val s = surface ?: return
            if (streamWidth <= 0 || streamHeight <= 0) return

            if (awaitingKeyframe && !frame.keyframe) {
                return
            }

            if (codec == null || configuredWidth != streamWidth || configuredHeight != streamHeight) {
                if (!frame.keyframe) {
                    awaitingKeyframe = true
                    return
                }
                staleBeforeConfigure = releaseCodecStateLocked()
                targetW = streamWidth
                targetH = streamHeight
                targetSurface = s
                needsConfigure = true
            }
        }

        // Para qualquer instância anterior fora do lock antes de alocar um novo MediaCodec.
        stopAndReleaseCodec(staleBeforeConfigure)

        if (needsConfigure) {
            val expectedSurface = targetSurface ?: return
            synchronized(lock) {
                if (released || surface !== expectedSurface || streamWidth != targetW || streamHeight != targetH) {
                    return
                }
                if (codec == null) {
                    try {
                        configureCodecLocked(targetW, targetH, expectedSurface)
                    } catch (e: Exception) {
                        Log.w(TAG, "Falha ao configurar MediaCodec H.264: ${e.message}", e)
                        staleToStop = releaseCodecStateLocked()
                        awaitingKeyframe = true
                        errorMessage = "Decodificador H.264 indisponível: ${e.message ?: "erro de codec"}"
                    }
                }
            }
            stopAndReleaseCodec(staleToStop)
            if (errorMessage != null) {
                onDecoderError(errorMessage!!)
                return
            }
        }

        staleToStop = null
        synchronized(lock) {
            if (released || surface == null || codec == null) return

            if (frame.keyframe) {
                // Novo keyframe substitui qualquer fila antiga imediatamente para zerar backlog.
                pendingFrames.clear()
                awaitingKeyframe = false
                pendingFrames.addLast(frame)
            } else {
                if (awaitingKeyframe) return
                if (pendingFrames.size >= MAX_PENDING_INPUT_FRAMES) {
                    // Evita acumular atraso no decodificador descartando até o próximo IDR limpo.
                    pendingFrames.clear()
                    awaitingKeyframe = true
                    needKeyframeRequest = true
                    return@synchronized
                }
                pendingFrames.addLast(frame)
            }

            val feedOutcome = feedAvailableLocked()
            if (feedOutcome.requestKeyframe) needKeyframeRequest = true
            if (feedOutcome.errorMessage != null) errorMessage = feedOutcome.errorMessage
            staleToStop = feedOutcome.staleCodec
        }

        stopAndReleaseCodec(staleToStop)
        val err = errorMessage
        if (err != null) {
            onDecoderError(err)
        } else if (needKeyframeRequest) {
            onRequestKeyframe()
        }
    }

    fun release(surfaceToRelease: Surface? = null) {
        var staleCodec: MediaCodec? = null
        synchronized(lock) {
            if (released) return
            released = true
            surface = null
            staleCodec = releaseCodecStateLocked()
        }
        val posted = codecHandler.post {
            try {
                stopAndReleaseCodec(staleCodec)
            } finally {
                runCatching { surfaceToRelease?.release() }
                codecThread.quitSafely()
            }
        }
        if (!posted) {
            try {
                stopAndReleaseCodec(staleCodec)
            } finally {
                runCatching { surfaceToRelease?.release() }
                codecThread.quitSafely()
            }
        }
    }

    private data class FeedOutcome(
        val staleCodec: MediaCodec? = null,
        val requestKeyframe: Boolean = false,
        val errorMessage: String? = null,
    )

    private fun feedAvailableLocked(): FeedOutcome {
        val activeCodec = codec ?: return FeedOutcome()
        while (availableInputs.isNotEmpty() && pendingFrames.isNotEmpty()) {
            val inputIdx = availableInputs.removeFirst()
            val frame = pendingFrames.first()
            try {
                val inputBuffer = activeCodec.getInputBuffer(inputIdx)
                if (inputBuffer == null) {
                    activeCodec.queueInputBuffer(inputIdx, 0, 0, 0L, 0)
                    continue
                }
                inputBuffer.clear()
                if (frame.data.size > inputBuffer.remaining()) {
                    Log.w(
                        TAG,
                        "Frame H.264 (${frame.data.size} bytes) excede buffer (${inputBuffer.remaining()})",
                    )
                    activeCodec.queueInputBuffer(inputIdx, 0, 0, 0L, 0)
                    pendingFrames.clear()
                    awaitingKeyframe = true
                    return FeedOutcome(requestKeyframe = true)
                }
                inputBuffer.put(frame.data)
                val flags = if (frame.keyframe) MediaCodec.BUFFER_FLAG_KEY_FRAME else 0
                val ptsUs = frame.ptsMs.toLong().coerceAtLeast(0L) * 1000L
                activeCodec.queueInputBuffer(inputIdx, 0, frame.data.size, ptsUs, flags)
                pendingFrames.removeFirst()
            } catch (e: Exception) {
                Log.w(TAG, "Erro ao enfileirar entrada no MediaCodec; resetando para próximo keyframe", e)
                val stale = releaseCodecStateLocked()
                awaitingKeyframe = true
                val fatal = if (e is MediaCodec.CodecException && !e.isRecoverable && !e.isTransient) {
                    "Falha irrecuperável no decodificador H.264: ${e.diagnosticInfo}"
                } else {
                    null
                }
                return FeedOutcome(staleCodec = stale, requestKeyframe = fatal == null, errorMessage = fatal)
            }
        }
        return FeedOutcome()
    }

    private fun createAvcFormat(width: Int, height: Int, operatingRate: Int?): MediaFormat =
        MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, width, height).apply {
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, MAX_INPUT_BUFFER_BYTES)
            setInteger(MediaFormat.KEY_PRIORITY, 0)
            if (operatingRate != null) {
                setInteger(MediaFormat.KEY_OPERATING_RATE, operatingRate)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
            }
        }

    private fun configureCodecLocked(width: Int, height: Int, targetSurface: Surface) {
        val baseFormat = createAvcFormat(width, height, operatingRate = null)
        val tunedFormat = createAvcFormat(width, height, operatingRate = 120)

        codecGeneration += 1L
        val generation = codecGeneration
        val newCodec = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        try {
            val callback = createCallback(generation)
            newCodec.setCallback(callback, codecHandler)
            try {
                newCodec.configure(tunedFormat, targetSurface, null, 0)
            } catch (_: Exception) {
                newCodec.reset()
                newCodec.setCallback(callback, codecHandler)
                newCodec.configure(baseFormat, targetSurface, null, 0)
            }
            newCodec.start()
        } catch (e: Exception) {
            runCatching { newCodec.release() }
            throw e
        }

        codec = newCodec
        configuredWidth = width
        configuredHeight = height
    }

    private fun createCallback(generation: Long): MediaCodec.Callback = object : MediaCodec.Callback() {
        override fun onInputBufferAvailable(mc: MediaCodec, index: Int) {
            var staleToStop: MediaCodec? = null
            var needKeyframe = false
            var errorMessage: String? = null

            synchronized(lock) {
                if (released || codec !== mc || codecGeneration != generation) return
                availableInputs.addLast(index)
                val outcome = feedAvailableLocked()
                staleToStop = outcome.staleCodec
                needKeyframe = outcome.requestKeyframe
                errorMessage = outcome.errorMessage
            }

            stopAndReleaseCodec(staleToStop)
            val err = errorMessage
            if (err != null) {
                onDecoderError(err)
            } else if (needKeyframe) {
                onRequestKeyframe()
            }
        }

        override fun onOutputBufferAvailable(mc: MediaCodec, index: Int, info: MediaCodec.BufferInfo) {
            synchronized(lock) {
                if (released || codec !== mc || codecGeneration != generation) return
                val isConfig = (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG) != 0
                val shouldRender = !isConfig && info.size > 0 && surface != null
                runCatching { mc.releaseOutputBuffer(index, shouldRender) }
            }
        }

        override fun onOutputFormatChanged(mc: MediaCodec, format: MediaFormat) {
            var detectedSize: Pair<Int, Int>? = null
            synchronized(lock) {
                if (released || codec !== mc || codecGeneration != generation) return
                detectedSize = extractFormatDimensions(format)
            }
            detectedSize?.let { (w, h) ->
                onVideoSizeChanged(w, h)
            }
        }

        override fun onError(mc: MediaCodec, e: MediaCodec.CodecException) {
            var stale: MediaCodec? = null
            var fatalMsg: String? = null
            var needKeyframe = false

            synchronized(lock) {
                if (released || codec !== mc || codecGeneration != generation) return
                Log.w(TAG, "Erro assíncrono no MediaCodec; resetando para próximo keyframe", e)
                stale = releaseCodecStateLocked()
                awaitingKeyframe = true
                if (!e.isRecoverable && !e.isTransient) {
                    fatalMsg = "Falha irrecuperável no decodificador H.264: ${e.diagnosticInfo}"
                } else {
                    needKeyframe = true
                }
            }

            stopAndReleaseCodec(stale)
            if (fatalMsg != null) {
                onDecoderError(fatalMsg!!)
            } else if (needKeyframe) {
                onRequestKeyframe()
            }
        }
    }

    private fun extractFormatDimensions(format: MediaFormat): Pair<Int, Int>? {
        val rawW = if (format.containsKey(MediaFormat.KEY_WIDTH)) format.getInteger(MediaFormat.KEY_WIDTH) else return null
        val rawH = if (format.containsKey(MediaFormat.KEY_HEIGHT)) format.getInteger(MediaFormat.KEY_HEIGHT) else return null
        val croppedW = if (format.containsKey("crop-left") && format.containsKey("crop-right")) {
            format.getInteger("crop-right") - format.getInteger("crop-left") + 1
        } else {
            rawW
        }
        val croppedH = if (format.containsKey("crop-top") && format.containsKey("crop-bottom")) {
            format.getInteger("crop-bottom") - format.getInteger("crop-top") + 1
        } else {
            rawH
        }
        return if (croppedW > 0 && croppedH > 0) croppedW to croppedH else null
    }

    private fun releaseCodecStateLocked(): MediaCodec? {
        val current = codec
        codec = null
        codecGeneration += 1L
        configuredWidth = 0
        configuredHeight = 0
        availableInputs.clear()
        pendingFrames.clear()
        return current
    }

    private fun stopAndReleaseCodec(target: MediaCodec?) {
        if (target == null) return
        runCatching { target.stop() }
        runCatching { target.release() }
    }

    companion object {
        private const val TAG = "OraculoScreenDecoder"
        private const val MAX_INPUT_BUFFER_BYTES = 2_097_152
        private const val MAX_PENDING_INPUT_FRAMES = 8
    }
}
