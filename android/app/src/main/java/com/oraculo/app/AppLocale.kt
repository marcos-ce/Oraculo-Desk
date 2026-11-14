package com.oraculo.app

import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.Locale

enum class AppLang(val prefValue: String, val badge: String) {
    PT("pt", "PT"),
    EN("en", "EN");

    fun next(): AppLang = when (this) {
        PT -> EN
        EN -> PT
    }

    companion object {
        fun defaultForDevice(): AppLang {
            val lang = Locale.getDefault().language.lowercase(Locale.ROOT)
            return if (lang.startsWith("pt")) PT else EN
        }

        fun fromPref(raw: String?): AppLang =
            entries.firstOrNull { it.prefValue == raw } ?: defaultForDevice()
    }
}

enum class AppLangMode(
    val prefValue: String,
    private val ptLabel: String,
    private val enLabel: String,
) {
    SYSTEM("system", "Auto", "Auto"),
    PT("pt", "Português", "Português"),
    EN("en", "English", "English");

    val label: String
        get() = tr(ptLabel, enLabel)

    companion object {
        fun fromPref(raw: String?): AppLangMode =
            entries.firstOrNull { it.prefValue == raw } ?: SYSTEM
    }
}

object AppI18n {
    private const val PREF_KEY_LANG = "app_lang"

    var mode: AppLangMode by mutableStateOf(AppLangMode.SYSTEM)
        private set

    var lang: AppLang by mutableStateOf(AppLang.defaultForDevice())
        private set

    val isPt: Boolean
        get() = lang == AppLang.PT

    fun init(prefs: SharedPreferences) {
        val savedMode = AppLangMode.fromPref(prefs.getString(PREF_KEY_LANG, null))
        mode = savedMode
        lang = resolveLang(savedMode)
    }

    fun setMode(prefs: SharedPreferences, nextMode: AppLangMode) {
        mode = nextMode
        lang = resolveLang(nextMode)
        prefs.edit().putString(PREF_KEY_LANG, nextMode.prefValue).apply()
    }

    private fun resolveLang(mode: AppLangMode): AppLang = when (mode) {
        AppLangMode.SYSTEM -> AppLang.defaultForDevice()
        AppLangMode.PT -> AppLang.PT
        AppLangMode.EN -> AppLang.EN
    }
}

internal fun tr(pt: String, en: String): String =
    if (AppI18n.isPt) pt else en
