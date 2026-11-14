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

object AppI18n {
    private const val PREF_KEY_LANG = "app_lang"

    var lang: AppLang by mutableStateOf(AppLang.defaultForDevice())
        private set

    val isPt: Boolean
        get() = lang == AppLang.PT

    fun init(prefs: SharedPreferences) {
        lang = AppLang.fromPref(prefs.getString(PREF_KEY_LANG, null))
    }

    fun toggle(prefs: SharedPreferences): AppLang {
        val next = lang.next()
        lang = next
        prefs.edit().putString(PREF_KEY_LANG, next.prefValue).apply()
        return next
    }
}

internal fun tr(pt: String, en: String): String =
    if (AppI18n.isPt) pt else en
