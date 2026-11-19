#!/usr/bin/env python3
"""Audita paridade de tokens Material 3, i18n e uso de tokens entre Android e Desktop."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
THEME_KT = ROOT / "android/app/src/main/java/com/oraculo/app/ui/Theme.kt"
DESKTOP_HTML = ROOT / "crates/oraculo-desktop/src/ui.html"
VALUES_COLORS = ROOT / "android/app/src/main/res/values/colors.xml"
VALUES_NIGHT_COLORS = ROOT / "android/app/src/main/res/values-night/colors.xml"
WEBVIEW_RS = ROOT / "crates/oraculo-desktop/src/webview.rs"
ANDROID_APP_DIR = ROOT / "android/app/src/main/java/com/oraculo/app"

SHARED_TOKEN_MAP = [
    ("Background", "--bg"),
    ("Surface", "--surface"),
    ("SurfaceRecessed", "--surface-recessed"),
    ("SurfaceInteractive", "--surface-interactive"),
    ("Primary", "--primary"),
    ("OnPrimary", "--on-primary"),
    ("PrimaryAction", "--primary-action"),
    ("OnPrimaryAction", "--on-primary-action"),
    ("PrimaryActionBorder", "--primary-action-border"),
    ("SecondaryContainer", "--secondary-container"),
    ("OnSecondaryContainer", "--on-secondary-container"),
    ("OnSurface", "--on-surface"),
    ("OnSurfaceMuted", "--on-surface-muted"),
    ("Outline", "--outline"),
    ("Error", "--error"),
    ("OnError", "--on-error"),
    ("Success", "--success"),
]

# Exceções permitidas onde o fundo subjacente é mídia escura (câmera/vídeo/ilustração)
ALLOWED_RAW_COLOR_FILES = {
    "QrScannerScreen.kt",
    "ScreenStreamingScreen.kt",
}

NON_AUTOMIRRORED_DIRECTIONAL = [
    "ArrowBack",
    "Backspace",
    "KeyboardReturn",
    "OpenInNew",
    "KeyboardArrowLeft",
    "KeyboardArrowRight",
    "VolumeDown",
    "VolumeUp",
    "VolumeOff",
    "HelpOutline",
]


def extract_kt_palette(source: str, block_name: str) -> dict[str, str]:
    m = re.search(rf"private val {block_name}\s*=\s*OraculoPalette\((.*?)\n\)", source, re.S)
    if not m:
        raise RuntimeError(f"Bloco {block_name} não encontrado em Theme.kt")
    pairs = re.findall(r"(\w+)\s*=\s*Color\(0xFF([0-9A-Fa-f]{6})\)", m.group(1))
    return {k: f"#{v.upper()}" for k, v in pairs}


def extract_css_palette(source: str, selector_pattern: str) -> dict[str, str]:
    m = re.search(rf"{selector_pattern}\s*\{{(.*?)\}}", source, re.S)
    if not m:
        raise RuntimeError(f"Seletor CSS '{selector_pattern}' não encontrado em ui.html")
    pairs = re.findall(r"(--[a-z0-9-]+)\s*:\s*(#[0-9A-Fa-f]{6})\s*;", m.group(1))
    return {k: v.upper() for k, v in pairs}


def check_palette_parity() -> list[str]:
    errors: list[str] = []
    theme_src = THEME_KT.read_text(encoding="utf-8")
    html_src = DESKTOP_HTML.read_text(encoding="utf-8")

    kt_dark = extract_kt_palette(theme_src, "OraculoDarkPalette")
    kt_light = extract_kt_palette(theme_src, "OraculoLightPalette")
    css_dark = extract_css_palette(html_src, r":root,\s*\[data-theme=\"dark\"\]")
    css_light = extract_css_palette(html_src, r"\[data-theme=\"light\"\]")

    for kt_key, css_key in SHARED_TOKEN_MAP:
        if kt_dark.get(kt_key) != css_dark.get(css_key):
            errors.append(
                f"Divergência Dark {kt_key}/{css_key}: Theme.kt={kt_dark.get(kt_key)} vs ui.html={css_dark.get(css_key)}"
            )
        if kt_light.get(kt_key) != css_light.get(css_key):
            errors.append(
                f"Divergência Light {kt_key}/{css_key}: Theme.kt={kt_light.get(kt_key)} vs ui.html={css_light.get(css_key)}"
            )

    xml_light = VALUES_COLORS.read_text(encoding="utf-8")
    xml_dark = VALUES_NIGHT_COLORS.read_text(encoding="utf-8")
    m_light = re.search(r'name="oraculo_window_bg">(#[0-9A-Fa-f]{6})<', xml_light)
    m_dark = re.search(r'name="oraculo_window_bg">(#[0-9A-Fa-f]{6})<', xml_dark)
    if not m_light or m_light.group(1).upper() != kt_light["Background"]:
        errors.append(f"values/colors.xml oraculo_window_bg != Light Background ({kt_light['Background']})")
    if not m_dark or m_dark.group(1).upper() != kt_dark["Background"]:
        errors.append(f"values-night/colors.xml oraculo_window_bg != Dark Background ({kt_dark['Background']})")

    dark_bg_hex = kt_dark["Background"].lstrip("#")
    r_d, g_d, b_d = (int(dark_bg_hex[i : i + 2], 16) for i in (0, 2, 4))
    wv_src = WEBVIEW_RS.read_text(encoding="utf-8")
    if (
        f"{r_d}.0 / 255.0" not in wv_src
        or f"{g_d}.0 / 255.0" not in wv_src
        or f"{b_d}.0 / 255.0" not in wv_src
    ):
        errors.append(
            f"webview.rs não define cor inicial RGB({r_d}, {g_d}, {b_d}) ({kt_dark['Background']}) para WebKitGTK"
        )

    return errors


def check_desktop_i18n() -> list[str]:
    errors: list[str] = []
    html_src = DESKTOP_HTML.read_text(encoding="utf-8")

    m_i18n = re.search(r"const I18N\s*=\s*\{\s*pt:\s*\{(.*?)\n\s*\},\s*en:\s*\{(.*?)\n\s*\}\s*\};", html_src, re.S)
    if not m_i18n:
        return ["Bloco const I18N = { pt: ..., en: ... } não encontrado em ui.html"]

    pt_keys = set(re.findall(r"(\w+)\s*:", m_i18n.group(1)))
    en_keys = set(re.findall(r"(\w+)\s*:", m_i18n.group(2)))
    if pt_keys != en_keys:
        only_pt = sorted(pt_keys - en_keys)
        only_en = sorted(en_keys - pt_keys)
        if only_pt:
            errors.append(f"Chaves ausentes em I18N.en: {', '.join(only_pt)}")
        if only_en:
            errors.append(f"Chaves ausentes em I18N.pt: {', '.join(only_en)}")

    used_attrs = set(re.findall(r'data-i18n="(\w+)"', html_src))
    used_ph = set(re.findall(r'data-i18n-placeholder="(\w+)"', html_src))
    used_calls = set(re.findall(r'\bt\("(\w+)"\)', html_src))
    for key in sorted(used_attrs | used_ph | used_calls):
        if key not in pt_keys or key not in en_keys:
            errors.append(f"Chave i18n '{key}' usada no HTML/JS mas ausente em I18N")

    return errors


def check_desktop_m3_rules() -> list[str]:
    errors: list[str] = []
    html_src = DESKTOP_HTML.read_text(encoding="utf-8")

    if not re.search(r"grid-template-columns\s*:\s*80px\s+1fr", html_src):
        errors.append("ui.html: .app-shell deve usar Navigation Rail de 80px (grid-template-columns: 80px 1fr)")
    if ":focus-visible" not in html_src:
        errors.append("ui.html: regras :focus-visible obrigatórias ausentes para navegação via Tab")
    if ".m3-menu" not in html_src:
        errors.append("ui.html: componente .m3-menu para dropdown/context menu ausente")
    if "@font-face" not in html_src or "Google Sans" not in html_src:
        errors.append("ui.html: @font-face local para 'Google Sans' ausente")
    m_btn = re.search(r"\.btn\s*\{(.*?)\}", html_src, re.S)
    if not m_btn or "border-radius: 999px" not in m_btn.group(1):
        errors.append("ui.html: .btn deve usar pílula M3 (border-radius: 999px)")

    m_modal = re.search(r"\.modal-card\s*\{(.*?)\}", html_src, re.S)
    if not m_modal:
        errors.append("ui.html: seletor .modal-card ausente")
    else:
        body = m_modal.group(1)
        m_min = re.search(r"min-width\s*:\s*(\d+)px", body)
        m_max = re.search(r"max-width\s*:\s*(\d+)px", body)
        if not m_min or not m_max or int(m_min.group(1)) < 280 or int(m_max.group(1)) > 560:
            errors.append("ui.html: .modal-card deve manter min-width >= 280px e max-width <= 560px")

    return errors


CORNER_TOKEN_BY_DP = {
    "8": "OraculoMetrics.CornerSmall",
    "12": "OraculoMetrics.CornerMedium",
    "16": "OraculoMetrics.CornerLarge",
    "20": "OraculoMetrics.CornerSurface",
    "24": "OraculoMetrics.CornerSurfaceExpanded",
    "999": "OraculoMetrics.CornerPill",
}


def check_android_conventions() -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []

    for kt_file in sorted(ANDROID_APP_DIR.rglob("*.kt")):
        if kt_file.name in {"Theme.kt", "JijiCatIllustration.kt"}:
            continue
        lines = kt_file.read_text(encoding="utf-8").splitlines()
        rel = kt_file.relative_to(ROOT)

        for idx, line in enumerate(lines, 1):
            if "Color(0x" in line:
                msg = f"{rel}:{idx}: literal raw Color(0x...) fora de Theme.kt -> {line.strip()}"
                if kt_file.name in ALLOWED_RAW_COLOR_FILES:
                    warnings.append(f"[exceção mídia] {msg}")
                else:
                    errors.append(msg)

            for icon_name in NON_AUTOMIRRORED_DIRECTIONAL:
                if re.search(rf"Icons\.(Filled|Default|Outlined)\.{icon_name}\b", line):
                    errors.append(f"{rel}:{idx}: use Icons.AutoMirrored.* para '{icon_name}'")

            m_shape = re.search(r"RoundedCornerShape\(\s*(\d+(?:\.\d+)?)\.dp\s*\)", line)
            if m_shape:
                dp_val = m_shape.group(1)
                if dp_val == "999":
                    continue
                if dp_val in CORNER_TOKEN_BY_DP:
                    warnings.append(
                        f"[token ausente] {rel}:{idx}: RoundedCornerShape({dp_val}.dp) — use {CORNER_TOKEN_BY_DP[dp_val]}"
                    )
                else:
                    warnings.append(
                        f"[literal fora da escala] {rel}:{idx}: RoundedCornerShape({dp_val}.dp) — alinhe a OraculoMetrics.Corner*"
                    )

    return errors, warnings


def main() -> int:
    parser = argparse.ArgumentParser(description="Verifica aderência ao padrão Oráculo Material 3.")
    parser.add_argument("--strict", action="store_true", help="Falha também em warnings de literais dp/exceções.")
    args = parser.parse_args()

    errors: list[str] = []
    warnings: list[str] = []

    errors.extend(check_palette_parity())
    errors.extend(check_desktop_i18n())
    errors.extend(check_desktop_m3_rules())
    kt_errors, kt_warnings = check_android_conventions()
    errors.extend(kt_errors)
    warnings.extend(kt_warnings)

    print("=== Oráculo M3 Design Audit ===")
    if errors:
        print(f"\nERROS ({len(errors)}):")
        for e in errors:
            print(f"  ✗ {e}")
    else:
        print("\n✓ Paridade de paleta (17 tokens Dark/Light), startup backgrounds, M3 Desktop (80px rail/foco/modal 280-560px), i18n e AutoMirrored OK.")

    if warnings:
        print(f"\nAVISOS / DERIVA HISTÓRICA ({len(warnings)}):")
        for w in warnings:
            print(f"  ! {w}")

    if errors or (args.strict and warnings):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
