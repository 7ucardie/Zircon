#!/usr/bin/env bash
# Run on an OVHcloud VPS with Docker Compose and rust/ at /srv/zircon/rust.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export ZIRCON_ASSETS_DIR=${ZIRCON_ASSETS_DIR:-/srv/zircon/assets}
export ZIRCON_DATA_DIR=${ZIRCON_DATA_DIR:-/srv/zircon/data}
export ZIRCON_UID=$(id -u)
export ZIRCON_GID=$(id -g)
compose=(docker compose -f "$here/compose.yml")

case ${1:-} in
  up)
    test -d "$ZIRCON_ASSETS_DIR/Client/Map"
    test -f "$ZIRCON_ASSETS_DIR/Database/System.db"
    mkdir -p "$ZIRCON_DATA_DIR"
    "${compose[@]}" up -d --build
    ;;
  start|stop|down|ps|logs)
    "${compose[@]}" "$1" "${@:2}"
    ;;
  restore-latest)
    if [[ ${FENCED_OLD_PRIMARY:-} != yes ]]; then
      echo 'Refusing promotion: power off the old primary in OVHcloud, then set FENCED_OLD_PRIMARY=yes.' >&2
      exit 1
    fi
    snapshot=/srv/zircon/backups/latest
    test -d "$snapshot" || { echo 'No complete standby snapshot found.' >&2; exit 1; }
    "${compose[@]}" down
    if [[ -e "$ZIRCON_DATA_DIR" ]]; then
      mv -- "$ZIRCON_DATA_DIR" "$ZIRCON_DATA_DIR.before-promotion-$(date -u +%Y%m%dT%H%M%SZ)"
    fi
    mkdir -p "$ZIRCON_DATA_DIR"
    cp -a "$snapshot/." "$ZIRCON_DATA_DIR/"
    "$here/node.sh" up
    echo 'Server running. Move the OVHcloud Additional IP to this VPS before reconnecting clients.'
    ;;
  *)
    echo 'Usage: node.sh {up|start|stop|down|ps|logs|restore-latest}' >&2
    exit 2
    ;;
esac
