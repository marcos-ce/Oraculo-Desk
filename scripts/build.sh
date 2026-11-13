#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="oraculo-rust-builder:latest"
CARGO_VOL="oraculo-cargo-cache"
TARGET_VOL="oraculo-target-cache"
JOBS="${CARGO_BUILD_JOBS:-8}"

MODE="${1:-release}"

if ! command -v podman >/dev/null 2>&1; then
    echo "Erro: podman não encontrado no PATH." >&2
    exit 1
fi

if ! podman image exists "$IMAGE_NAME" || [[ "${REBUILD_IMAGE:-0}" == "1" ]]; then
    echo "==> Construindo imagem de build isolada ($IMAGE_NAME)..."
    podman build -t "$IMAGE_NAME" -f "$ROOT_DIR/Containerfile" "$ROOT_DIR"
fi

podman volume exists "$CARGO_VOL" >/dev/null 2>&1 || podman volume create "$CARGO_VOL" >/dev/null
podman volume exists "$TARGET_VOL" >/dev/null 2>&1 || podman volume create "$TARGET_VOL" >/dev/null

run_in_container() {
    podman run --rm \
        -e "CARGO_BUILD_JOBS=$JOBS" \
        -v "$ROOT_DIR:/work:Z" \
        -v "$CARGO_VOL:/cargo-cache" \
        -v "$TARGET_VOL:/target-cache" \
        "$IMAGE_NAME" \
        bash -c "$1"
}

case "$MODE" in
    check)
        echo "==> Executando cargo check no container..."
        run_in_container "cargo check --workspace"
        ;;
    test)
        echo "==> Executando cargo test no container..."
        run_in_container "cargo test --workspace"
        ;;
    debug)
        echo "==> Compilando binários (debug) no container..."
        mkdir -p "$ROOT_DIR/dist"
        run_in_container "cargo build --workspace && install -m 755 /target-cache/debug/oraculo-host /target-cache/debug/oraculo-cli /work/dist/"
        echo "==> Binários prontos em ./dist/"
        ;;
    release)
        echo "==> Compilando binários (release) no container..."
        mkdir -p "$ROOT_DIR/dist"
        run_in_container "cargo build --release --workspace && install -m 755 /target-cache/release/oraculo-host /target-cache/release/oraculo-cli /work/dist/"
        echo "==> Binários prontos em ./dist/"
        ;;
    clean-volumes)
        echo "==> Removendo volumes de cache do Podman..."
        podman volume rm -f "$CARGO_VOL" "$TARGET_VOL" || true
        ;;
    *)
        echo "Uso: $0 [check|test|debug|release|clean-volumes]" >&2
        exit 1
        ;;
esac
