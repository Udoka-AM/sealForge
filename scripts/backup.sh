#!/usr/bin/env bash
# Nightly coordinator backup (task 5.6). Run from cron on the VM, e.g.
#   15 3 * * * /opt/sealforge/scripts/backup.sh >> /var/log/sealforge-backup.log 2>&1
#
# Writes BACKUP_DIR/sealforge-<UTC timestamp>.dump (pg_dump custom format) and
# a tarball of the escrow wallet files, encrypts both to AGE_RECIPIENT when
# set, copies them to BACKUP_REMOTE (rsync target, e.g. a Hetzner Storage Box)
# when set, and keeps RETENTION_DAYS of local copies.
#
# The dump holds escrow UFVKs encrypted with SF_DATA_KEY. That key is NOT in
# the dump: keep it in the team password manager and offline. Without it a
# restore can't spend from any escrow.
set -euo pipefail

: "${DATABASE_URL:?set DATABASE_URL}"
BACKUP_DIR=${BACKUP_DIR:-/var/backups/sealforge}
WALLET_DIR=${SF_WALLET_DIR:-data/wallets}
RETENTION_DAYS=${RETENTION_DAYS:-14}
# Streams to stdout, so a wrapper like `docker exec -i <container> pg_dump` works too.
PG_DUMP=${PG_DUMP:-pg_dump}

mkdir -p "$BACKUP_DIR"
chmod 700 "$BACKUP_DIR"
stamp=$(date -u +%Y%m%dT%H%MZ)
dump="$BACKUP_DIR/sealforge-$stamp.dump"
wallets="$BACKUP_DIR/sealforge-wallets-$stamp.tar.gz"

$PG_DUMP --format=custom --no-owner "$DATABASE_URL" > "$dump"
if [[ -d "$WALLET_DIR" ]]; then
  tar -czf "$wallets" -C "$(dirname "$WALLET_DIR")" "$(basename "$WALLET_DIR")"
fi

files=("$dump")
[[ -f "$wallets" ]] && files+=("$wallets")
if [[ -n "${AGE_RECIPIENT:-}" ]]; then
  for f in "${files[@]}"; do
    age -r "$AGE_RECIPIENT" -o "$f.age" "$f" && rm -f "$f"
  done
  files=("${files[@]/%/.age}")
fi
chmod 600 "${files[@]}"

if [[ -n "${BACKUP_REMOTE:-}" ]]; then
  rsync -a "${files[@]}" "$BACKUP_REMOTE/"
fi

find "$BACKUP_DIR" -name 'sealforge-*' -mtime "+$RETENTION_DAYS" -delete
echo "backup ok: ${files[*]}"
