#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="oraculo-android-builder:latest"
CARGO_VOL="oraculo-cargo-cache"
TARGET_VOL="oraculo-target-cache"
GRADLE_VOL="oraculo-gradle-cache"
JOBS="${CARGO_BUILD_JOBS:-8}"

MODE="${1:-debug}"

if ! command -v podman >/dev/null 2>&1; then
    echo "Erro: podman não encontrado no PATH." >&2
    exit 1
fi

if ! podman image exists "$IMAGE_NAME" || [[ "${REBUILD_IMAGE:-0}" == "1" ]]; then
    echo "==> Construindo imagem Android isolada ($IMAGE_NAME)..."
    podman build -t "$IMAGE_NAME" -f "$ROOT_DIR/Containerfile.android" "$ROOT_DIR"
fi

podman volume exists "$CARGO_VOL" >/dev/null 2>&1 || podman volume create "$CARGO_VOL" >/dev/null
podman volume exists "$TARGET_VOL" >/dev/null 2>&1 || podman volume create "$TARGET_VOL" >/dev/null
podman volume exists "$GRADLE_VOL" >/dev/null 2>&1 || podman volume create "$GRADLE_VOL" >/dev/null

run_in_container() {
    podman run --rm \
        -e "CARGO_BUILD_JOBS=$JOBS" \
        -e "GRADLE_OPTS=-Xmx2g -Dorg.gradle.daemon=false" \
        -e "ANDROID_USER_HOME=/gradle-cache/.android" \
        -v "$ROOT_DIR:/work:Z" \
        -v "$CARGO_VOL:/cargo-cache" \
        -v "$TARGET_VOL:/target-cache" \
        -v "$GRADLE_VOL:/gradle-cache" \
        "$IMAGE_NAME" \
        bash -c "mkdir -p /gradle-cache/.android && rm -rf /root/.android && ln -s /gradle-cache/.android /root/.android && $1"
}

build_rust_and_bindings() {
    local profile_flag="--release"
    local profile_dir="release"
    if [[ "${RUST_DEBUG:-0}" == "1" ]]; then
        profile_flag=""
        profile_dir="debug"
    fi

    echo "==> Compilando oraculo-mobile-core (arm64-v8a) e gerando bindings Kotlin..."
    run_in_container "
        set -euo pipefail
        mkdir -p /work/android/app/src/main/jniLibs /work/android/app/src/main/java
        cargo ndk -t arm64-v8a -o /work/android/app/src/main/jniLibs build -p oraculo-mobile-core ${profile_flag}
        # Remove cdylibs extras emitidas por iroh/iroh-relay (já ligadas estaticamente em liboraculo_mobile_core.so)
        rm -f /work/android/app/src/main/jniLibs/arm64-v8a/libiroh.so /work/android/app/src/main/jniLibs/arm64-v8a/libiroh_relay.so
        cargo build -p oraculo-mobile-core --features cli --bin uniffi-bindgen ${profile_flag}
        /target-cache/${profile_dir}/uniffi-bindgen generate \
            --library /target-cache/aarch64-linux-android/${profile_dir}/liboraculo_mobile_core.so \
            --language kotlin \
            --out-dir /work/android/app/src/main/java
        \"\${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip\" --strip-unneeded /work/android/app/src/main/jniLibs/arm64-v8a/liboraculo_mobile_core.so
    "
}

case "$MODE" in
    image)
        echo "==> Imagem $IMAGE_NAME pronta."
        ;;
    bindings)
        build_rust_and_bindings debug
        ;;
    debug)
        build_rust_and_bindings debug
        mkdir -p "$ROOT_DIR/dist"
        echo "==> Empacotando APK debug via Gradle no container..."
        run_in_container "
            set -euo pipefail
            cd /work/android
            gradle --no-daemon assembleDebug
            cp /work/android/app/build/outputs/apk/debug/app-debug.apk /work/dist/oraculo-debug.apk
        "
        echo "==> APK pronto em ./dist/oraculo-debug.apk"
        ;;
    release)
        build_rust_and_bindings release
        mkdir -p "$ROOT_DIR/dist"
        echo "==> Empacotando APK release (unsigned) via Gradle no container..."
        run_in_container "
            set -euo pipefail
            cd /work/android
            gradle --no-daemon assembleRelease
            cp /work/android/app/build/outputs/apk/release/app-release-unsigned.apk /work/dist/oraculo-release-unsigned.apk
        "
        echo "==> APK pronto em ./dist/oraculo-release-unsigned.apk"
        ;;
    *)
        echo "Uso: $0 [image|bindings|debug|release]" >&2
        exit 1
        ;;
esac
