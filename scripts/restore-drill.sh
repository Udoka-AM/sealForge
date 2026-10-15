#!/usr/bin/env bash
# Restore drill (task 5.6): restore a coordinator dump into a scratch database
# and check it is usable. Never touches the live database.
#
#   scripts/restore-drill.sh <dump> [scratch-db-name]
#
# Needs SOURCE_URL (the database the dump came from, for row-count
# comparison; optional), ADMIN_URL (a connection that may create databases)
# and SF_DATA_KEY. Prints a report; exits non-zero if a check fails.
set -euo pipefail

dump=${1:?usage: restore-drill.sh <dump> [scratch-db]}
scratch=${2:-sealforge_restore_drill}
: "${ADMIN_URL:?set ADMIN_URL, e.g. postgres://sealforge:sealforge@localhost:5432/postgres}"
: "${SF_DATA_KEY:?set SF_DATA_KEY (from the password manager)}"
PSQL=${PSQL:-psql}
PG_RESTORE=${PG_RESTORE:-pg_restore}
scratch_url="${ADMIN_URL%/*}/$scratch"

start=$(date +%s)
$PSQL "$ADMIN_URL" -qc "DROP DATABASE IF EXISTS $scratch" -c "CREATE DATABASE $scratch"
$PG_RESTORE --no-owner --exit-on-error -d "$scratch_url" < "$dump"
elapsed=$(( $(date +%s) - start ))

q() { $PSQL "$1" -tAc "$2"; }
fail=0
check() { if [[ "$2" == "$3" ]]; then echo "ok    $1 ($2)"; else echo "FAIL  $1: got $2, want $3"; fail=1; fi; }

echo "Restored $dump into $scratch in ${elapsed}s"
check "migrations applied" "$(q "$scratch_url" "SELECT count(*) FROM _sqlx_migrations WHERE success")" \
  "$(ls "$(dirname "$0")/../migrations"/*.sql | wc -l | tr -d ' ')"
for table in organisers auctions trustees bids clearing_results refund_notes tx_batches signature_shares; do
  restored=$(q "$scratch_url" "SELECT count(*) FROM $table")
  if [[ -n "${SOURCE_URL:-}" ]]; then
    check "$table rows" "$restored" "$(q "$SOURCE_URL" "SELECT count(*) FROM $table")"
  else
    echo "info  $table rows: $restored"
  fi
done
# The one thing the chain can't give back: every escrow's viewing key must decrypt.
escrows=$(q "$scratch_url" "SELECT count(*) FROM auctions WHERE escrow_ufvk_enc IS NOT NULL")
decrypted=$(q "$scratch_url" "SELECT count(*) FROM auctions WHERE escrow_ufvk_enc IS NOT NULL \
  AND pgp_sym_decrypt(escrow_ufvk_enc, '$SF_DATA_KEY') LIKE 'uview%'")
check "escrow UFVKs decrypt with SF_DATA_KEY" "$decrypted" "$escrows"
refund_addrs=$(q "$scratch_url" "SELECT count(*) FROM bids WHERE refund_ua_enc IS NOT NULL")
refund_ok=$(q "$scratch_url" "SELECT count(*) FROM bids WHERE refund_ua_enc IS NOT NULL \
  AND pgp_sym_decrypt(refund_ua_enc, '$SF_DATA_KEY') LIKE 'u%'")
check "refund addresses decrypt" "$refund_ok" "$refund_addrs"

exit $fail
