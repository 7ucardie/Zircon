#!/usr/bin/env bash
# Run from an admin machine: checkpoint the stopped primary to two standby VPSs.
set -euo pipefail

if [[ $# != 3 ]]; then
  echo 'Usage: checkpoint.sh PRIMARY_SSH STANDBY1_SSH STANDBY2_SSH' >&2
  exit 2
fi
for host in "$@"; do
  [[ $host =~ ^[a-zA-Z0-9_.@-]+$ ]] || { echo "Invalid SSH host: $host" >&2; exit 2; }
done
[[ $1 != "$2" && $1 != "$3" && $2 != "$3" ]] || { echo 'Hosts must be distinct.' >&2; exit 2; }

umask 077
stage=$(mktemp -d)
primary_stopped=no
cleanup() {
  if [[ $primary_stopped == yes ]]; then
    ssh "$1" /srv/zircon/rust/deploy/ovh/node.sh start || true
  fi
  rm -rf -- "$stage"
}
trap 'cleanup "$1"' EXIT

ssh "$1" /srv/zircon/rust/deploy/ovh/node.sh stop
primary_stopped=yes
rsync -a -- "$1:/srv/zircon/data/" "$stage/data/"
ssh "$1" /srv/zircon/rust/deploy/ovh/node.sh start
primary_stopped=no

stamp=$(date -u +%Y%m%dT%H%M%SZ)
for standby in "$2" "$3"; do
  ssh "$standby" /srv/zircon/rust/deploy/ovh/node.sh down
  ssh "$standby" "mkdir -p /srv/zircon/backups/$stamp"
  rsync -a -- "$stage/data/" "$standby:/srv/zircon/backups/$stamp/"
  ssh "$standby" "ln -sfn $stamp /srv/zircon/backups/latest"
done
echo "Checkpoint $stamp copied to both standbys."
