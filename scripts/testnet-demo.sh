#!/usr/bin/env bash
# A whole auction on testnet, from one machine, with simulated bidders and
# trustees: create, DKG, open, sealed bids, reveals, close, publish, refunds,
# payout. For demos and for checking a deployment end to end.
#
#   scripts/testnet-demo.sh <plan.conf>          run it all (resumable: re-run after a stop)
#   scripts/testnet-demo.sh <plan.conf> status   where it is
#
# One machine holding all three trustee keys defeats the 2-of-3 threshold, so
# this is for test networks only. Each simulated bidder has its own wallet
# (and so its own refund address), funded from the plan's funder wallet in a
# single transaction.
#
# plan.conf (bash) sets:
#   STATE=~/.sealforge-host/testnet/genesis     where keys, wallets and receipts go
#   COORDINATOR=http://127.0.0.1:8090
#   FUNDER_DB=… FUNDER_SEED=…                   a dev_wallet with TAZ
#   ORGANISER_HOME=…                            an organiser keystore already registered (sealforge init)
#   TITLE= DESCRIPTION= QUANTITY= RESERVE_ZAT= MODE=uniform_price
#   SEALED=1                                    commit-reveal (prices hidden until the reveal window)
#   BID_BLOCKS= REVEAL_BLOCKS=                  window lengths (25-second blocks on testnet)
#   TRUSTEES=(Greg Harry Dmitry)
#   BIDDERS=("deposit_zat price_zat reveal|skip" …)   in the order they bid
#   FEE_BUFFER_ZAT=100000
set -euo pipefail
cd "$(dirname "$0")/.."

PLAN=${1:?usage: scripts/testnet-demo.sh <plan.conf> [status]}
# shellcheck disable=SC1090
source "$PLAN"
: "${STATE:?}" "${COORDINATOR:?}" "${FUNDER_DB:?}" "${FUNDER_SEED:?}" "${ORGANISER_HOME:?}"
MODE=${MODE:-uniform_price} SEALED=${SEALED:-1} FEE_BUFFER_ZAT=${FEE_BUFFER_ZAT:-100000}
REVEAL_ZAT=10000
# Margin per bidder for the deposit's fee, the reveal and its fee.
BIDDER_MARGIN=40000

SF=./target/release/sealforge
WALLET=./target/release/examples/dev_wallet
export SF_COORDINATOR_URL=$COORDINATOR
mkdir -p "$STATE/log"
chmod 700 "$STATE"

log() { printf '%s  %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$STATE/log/demo.log"; }
tip() { curl -sf "$COORDINATOR/readyz" | python3 -c 'import sys,json;print(json.load(sys.stdin)["chain_tip"])'; }
field() { # field <json-key> from the organiser's view of the auction
  $SF --home "$ORGANISER_HOME" auction show "$(cat "$STATE/code")" |
    python3 -c "import sys,json;v=json.load(sys.stdin).get('$1');print('' if v is None else v)"
}
status() { field status 2>/dev/null || echo unknown; } # unknown while the coordinator restarts
thome() { echo "$STATE/trustee-$(echo "$1" | tr '[:upper:]' '[:lower:]')"; }
wait_height() { local h=$1; until [[ $(tip) -ge $h ]]; do sleep 10; done; }
wait_status() { until [[ " $* " == *" $(status) "* ]]; do sleep 10; done; }
wallet() { # retries only when it couldn't reach the server (nothing was sent)
  local out try
  for try in 1 2 3 4 5 6; do
    if out=$($WALLET testnet "$STATE/$1.sqlite" "$STATE/$1.seed" "${@:2}" 2>&1); then
      echo "$out"
      return
    fi
    grep -q "connecting to" <<< "$out" || { echo "$out" >&2; return 1; }
    sleep $((try * 5))
  done
  echo "$out" >&2
  return 1
}
wait_funds() { # wait_funds <wallet> <zat>: until the wallet can spend that much (funding confirmed)
  until [[ $(wallet "$1" balance | awk '{print $1}') -ge $2 ]]; do sleep 15; done
}

build() {
  cargo build --release -q -p sealforge-cli
  cargo build --release -q -p sealforge-chain --features dev-wallet --example dev_wallet
}

setup_wallets() {
  local i total=0
  for i in "${!BIDDERS[@]}"; do
    [[ -f "$STATE/bidder-$i.addr" ]] || wallet "bidder-$i" address > "$STATE/bidder-$i.addr"
  done
  [[ -f "$STATE/payout.addr" ]] || wallet payout address > "$STATE/payout.addr"
  [[ -f "$STATE/funded" ]] && return
  # One transaction pays every bidder its deposit plus a margin.
  local outputs=()
  for i in "${!BIDDERS[@]}"; do
    read -r deposit _ _ <<< "${BIDDERS[$i]}"
    outputs+=("$((deposit + BIDDER_MARGIN))" "$(cat "$STATE/bidder-$i.addr")")
    total=$((total + deposit + BIDDER_MARGIN))
  done
  log "funding ${#BIDDERS[@]} bidder wallets: $total zat"
  $WALLET testnet "$FUNDER_DB" "$FUNDER_SEED" pay-many "${outputs[@]}" | tee -a "$STATE/log/demo.log"
  touch "$STATE/funded"
}

create() {
  [[ -f "$STATE/code" ]] && return
  local t close reveal
  t=$(tip)
  # DKG takes a couple of blocks; open must still leave BID_BLOCKS of bidding.
  close=$((t + BID_BLOCKS + 6))
  reveal=$((close + REVEAL_BLOCKS))
  {
    printf 'title = "%s"\ndescription = "%s"\nnetwork = "testnet"\n' "$TITLE" "$DESCRIPTION"
    printf 'mode = "%s"\nquantity = %s\nreserve_zat = %s\nclose_height = %s\n' "$MODE" "$QUANTITY" "$RESERVE_ZAT" "$close"
    [[ $SEALED == 1 ]] && printf 'bid_mode = "commit_reveal"\nreveal_close_height = %s\n' "$reveal"
    printf 'payout_ua = "%s"\n' "$(cat "$STATE/payout.addr")"
    for i in "${!TRUSTEES[@]}"; do
      printf '\n[[trustees]]\nslot = %s\nlabel = "%s"\n' "$((i + 1))" "${TRUSTEES[$i]}"
    done
  } > "$STATE/auction.toml"
  $SF --home "$ORGANISER_HOME" auction new "$STATE/auction.toml" > "$STATE/new.out"
  awk '/Created draft auction/ {print $NF}' "$STATE/new.out" > "$STATE/code"
  log "created $(cat "$STATE/code"): bidding until $close, reveals until $reveal (tip $t)"
}

dkg() {
  local code i
  code=$(cat "$STATE/code")
  for i in "${!TRUSTEES[@]}"; do
    local home
    home=$(thome "${TRUSTEES[$i]}")
    if [[ ! -d "$home" ]]; then
      local invite
      invite=$(awk -v s="slot $((i + 1))" '$0 ~ s {print $NF}' "$STATE/new.out")
      $SF --home "$home" trustee join "$invite" >> "$STATE/log/trustees.log" &
    fi
  done
  wait
  for i in "${!TRUSTEES[@]}"; do
    $SF --home "$(thome "${TRUSTEES[$i]}")" trustee dkg "$code" --wait >> "$STATE/log/trustees.log" &
  done
  wait
  log "DKG done, status $(status)"
}

open() {
  local code
  code=$(cat "$STATE/code")
  [[ $(status) == ready ]] && $SF --home "$ORGANISER_HOME" auction open "$code" > /dev/null
  log "open at $(tip): escrow $(field escrow_ua | cut -c1-24)…"
  if [[ ! -s "$STATE/fee-buffer" ]]; then
    local out escrow
    escrow=$(field escrow_ua)
    # The funder's change from funding the bidders may still be confirming.
    until out=$($WALLET testnet "$FUNDER_DB" "$FUNDER_SEED" send "$escrow" "$FEE_BUFFER_ZAT" "SF1FEE:$code" 2>&1); do
      grep -qE "Insufficient balance|connecting to" <<< "$out" || { echo "$out" >&2; return 1; }
      sleep 20
    done
    echo "$out" > "$STATE/fee-buffer"
    log "fee buffer sent: $FEE_BUFFER_ZAT zat"
  fi
}

bids() {
  local code escrow close gap i
  code=$(cat "$STATE/code")
  escrow=$(field escrow_ua)
  close=$(field close_height)
  # Spread the bids over the first two thirds of the bidding window.
  gap=$(((close - $(tip)) * 25 * 2 / 3 / ${#BIDDERS[@]}))
  for i in "${!BIDDERS[@]}"; do
    [[ -f "$STATE/bidder-$i.receipt.json" ]] && continue
    read -r deposit price _ <<< "${BIDDERS[$i]}"
    wait_funds "bidder-$i" $((deposit + 20000))
    [[ $(tip) -lt $close ]] || { log "bidding closed before bidder $i"; break; }
    if [[ $SEALED == 1 ]]; then
      wallet "bidder-$i" bid "$STATE/bidder-$i.receipt.json" "$code" "$escrow" "$deposit" "$price" >> "$STATE/log/demo.log"
      log "bidder $i: sealed bid, deposit $deposit zat"
    else
      wallet "bidder-$i" bid "$STATE/bidder-$i.receipt.json" "$code" "$escrow" "$deposit" >> "$STATE/log/demo.log"
      log "bidder $i: bid $deposit zat"
    fi
    sleep "$gap"
  done
}

reveals() {
  [[ $SEALED == 1 ]] || return 0
  local escrow close i
  escrow=$(field escrow_ua)
  close=$(field close_height)
  log "waiting for bidding to close at $close"
  wait_height $((close + 1))
  for i in "${!BIDDERS[@]}"; do
    read -r _ price reveal <<< "${BIDDERS[$i]}"
    [[ -f "$STATE/bidder-$i.revealed" ]] && continue
    if [[ $reveal == skip ]]; then
      log "bidder $i: does not reveal (deposit comes back in full)"
      touch "$STATE/bidder-$i.revealed"
      continue
    fi
    wait_funds "bidder-$i" $((REVEAL_ZAT + 10000))
    wallet "bidder-$i" reveal "$STATE/bidder-$i.receipt.json" "$escrow" >> "$STATE/log/demo.log"
    touch "$STATE/bidder-$i.revealed"
    log "bidder $i: revealed $price zat"
  done
}

publish() {
  local code
  code=$(cat "$STATE/code")
  log "waiting for the close (10 confirmations past the last window)"
  wait_status closed published refunding settled
  [[ $(status) == closed ]] && $SF --home "$ORGANISER_HOME" auction publish "$code" | tee -a "$STATE/log/demo.log"
  curl -sf "$COORDINATOR/v1/auctions/$code/result" > "$STATE/result.json"
  log "published: $(python3 -c 'import json,sys;r=json.load(open(sys.argv[1]));print({k:r.get(k) for k in ("clearing_price_zat","total_volume_zat","winner_count","valid_bid_count")})' "$STATE/result.json")"
}

sign_until_settled() {
  local code i
  code=$(cat "$STATE/code")
  log "trustees co-sign refunds and the payout as they come up"
  until [[ $(status) == settled ]]; do
    for i in "${!TRUSTEES[@]}"; do
      $SF --home "$(thome "${TRUSTEES[$i]}")" trustee sign "$code" >> "$STATE/log/trustees.log" 2>&1 &
    done
    wait
    sleep 15
  done
  log "settled at $(tip)"
}

show_status() {
  [[ -f "$STATE/code" ]] || { echo "not created yet"; return; }
  echo "auction $(cat "$STATE/code"): $(status) (tip $(tip))"
  curl -sf "$COORDINATOR/v1/auctions/$(cat "$STATE/code")/refunds" && echo
  tail -5 "$STATE/log/demo.log"
}

case "${2:-run}" in
  status) show_status ;;
  run)
    build
    setup_wallets
    create
    [[ $(status) == draft || $(status) == dkg ]] && dkg
    open
    bids
    reveals
    publish
    sign_until_settled
    ;;
  *) echo "usage: scripts/testnet-demo.sh <plan.conf> [status]" >&2; exit 1 ;;
esac
