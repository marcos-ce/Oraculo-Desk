#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-3389}"
if ! [[ "$PORT" =~ ^[1-9][0-9]*$ ]] || (( 10#$PORT < 1 || 10#$PORT > 65535 )); then
  echo "Invalid TCP port: $PORT" >&2
  exit 2
fi

if ! command -v firewall-cmd >/dev/null 2>&1; then
  echo "firewall-cmd not found; skipping." >&2
  exit 0
fi

if [[ "$(firewall-cmd --state 2>/dev/null || true)" != "running" ]]; then
  echo "firewalld is not running; skipping." >&2
  exit 0
fi

RULE="rule port port=\"${PORT}\" protocol=\"tcp\" reject"
ZONES="$(firewall-cmd --get-active-zones | awk '/^[^[:space:]]/ { zone=$1; next } /^[[:space:]]*interfaces:/ && NF > 1 { print zone }')"

if [[ -z "$ZONES" ]]; then
  firewall-cmd --permanent --add-rich-rule="$RULE"
else
  while IFS= read -r z; do
    [[ -z "$z" ]] && continue
    firewall-cmd --permanent --zone="$z" --add-rich-rule="$RULE"
  done <<< "$ZONES"
fi

firewall-cmd --reload
