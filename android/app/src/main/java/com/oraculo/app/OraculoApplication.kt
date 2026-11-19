package com.oraculo.app

import android.app.Application
import android.content.Context
import uniffi.oraculo_mobile_core.OraculoMobileClient

class OraculoApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        val prefs = getSharedPreferences("oraculo_prefs", Context.MODE_PRIVATE)
        AppI18n.init(prefs)
    }

    companion object {
        private val clientLock = Any()

        @Volatile
        private var sharedClient: OraculoMobileClient? = null

        fun getOrCreateClient(context: Context): OraculoMobileClient {
            sharedClient?.let { existing ->
                runCatching { existing.setLanguage(AppI18n.lang.prefValue) }
                return existing
            }
            return synchronized(clientLock) {
                sharedClient?.let { existing ->
                    runCatching { existing.setLanguage(AppI18n.lang.prefValue) }
                    return@synchronized existing
                }
                val created = OraculoMobileClient(context.applicationContext.filesDir.absolutePath)
                runCatching { created.setLanguage(AppI18n.lang.prefValue) }
                sharedClient = created
                created
            }
        }

        fun currentClientOrNull(): OraculoMobileClient? = sharedClient

        fun syncLanguage(lang: AppLang) {
            val existing = sharedClient ?: return
            runCatching { existing.setLanguage(lang.prefValue) }
        }
    }
}
