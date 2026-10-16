# Infra

One VM per environment (staging = NU7 testnet, production = mainnet). See [docs/02-TRD.md §2.3 and §2.9](../docs/02-TRD.md).

| Service | Profile | Purpose |
|---|---|---|
| `postgres` | default | Coordinator database |
| `zebra` | `node` | Full node |
| `zaino` | `node` | Compact-block gRPC indexer (image pinned in task 1.3) |
| `envoy` | `node` | gRPC-web proxy for WebZjs |
| `coordinator` | `app` | sealForge API, scanner, refund scheduler |
| `caddy` | `app` | TLS termination, IP-free access logs |

## Local development

```bash
cp .env.example .env
docker compose -f infra/docker-compose.yml up -d postgres
cargo run -p sealforge-coordinator
psql "$DATABASE_URL" -f seed/dev.sql
```

## Temporary hosting from a laptop

Until a server exists, `scripts/host-local.sh start` runs the public site from this machine, against a public lightwallet server, so no node has to sync:

| Piece | Where |
|---|---|
| Coordinator | `127.0.0.1:8090`, local only (the website proxies to it) |
| Website | a public `https://…trycloudflare.com` URL (Cloudflare quick tunnel: no account, no router ports) |
| gRPC-web for the browser audit | a second tunnel → Envoy (`infra/envoy-tls.yaml`) → `zec.rocks:443` over TLS |
| State | `~/.sealforge-host/<network>/` (0700). `host.env` holds `SF_DATA_KEY`: back it up |

`scripts/host-local.sh status` shows what's running and the URLs; `stop` stops everything.

**Fixed hostnames.** With a Cloudflare named tunnel (`cloudflared tunnel login`, then `cloudflared tunnel create sealforge` and `cloudflared tunnel route dns sealforge <host>` for the site and the gRPC-web host), put `SF_HOST_TUNNEL`, `SF_HOST_WEB_HOST` and `SF_HOST_LWD_HOST` in `~/.sealforge-host/<network>/tunnel.env`. The script then serves both through that tunnel. The live instance uses `sealforge.udokaam.dev` and `sealforge-lwd.udokaam.dev`.

**Test-network wallet.** `cargo run --release -p sealforge-chain --features dev-wallet --example dev_wallet -- testnet <wallet.sqlite> <seed-file> address|balance|send|pay-many|bid|reveal` is a small wallet for faucet TAZ and demo bids. `bid` builds the same memo and receipt as the browser (sealed when given a price) with the wallet's own address for refunds, so run one wallet per simulated bidder; `pay-many` funds them in one transaction. It refuses mainnet.

**An auction in an hour (test networks).** Mainnet keeps 144-block windows and hours-long refund delays. On testnet (25-second blocks) put `SF_MIN_WINDOW_BLOCKS=30` and `SF_REFUND_DELAY_SECS=20,120` in `~/.sealforge-host/testnet/demo.env`: the coordinator then accepts 12-minute bidding and reveal windows and sends refunds within two minutes of publishing. Both settings are refused on mainnet.

Limits of laptop hosting: the URLs change on every start, nothing is served while the machine sleeps (the script runs `caffeinate`, but a closed lid on battery still sleeps), and `zec.rocks` is trusted for liveness only. It can't move funds, and a hidden payment fails the audit against any other server. Check a lightwallet server first with `cargo run --release -p sealforge-chain --example probe_lightwalletd -- https://zec.rocks:443` (mainnet is on Ironwood, branch id `37a5165b`).

## Deploy (task 6.2)

```bash
docker compose -f infra/docker-compose.yml --profile node --profile app pull
docker compose -f infra/docker-compose.yml --profile node --profile app up -d
```

**Rollback:** set `SF_IMAGE_TAG` to the previous tag and re-run `up -d`. Pause refunds with `SF_REFUNDS_PAUSED=true`.

## Backups (task 5.6)

**What matters most:** each escrow's UFVK. It is stored only in Postgres, encrypted with `SF_DATA_KEY`, and the trustees' FROST shares can't spend without it. Losing both the database and every backup, or losing `SF_DATA_KEY`, leaves escrowed bids unrefundable. Everything else can be rebuilt: payments and memos by rescanning with the UFVK, and published results from the chain plus the published bid set.

| What | How | Where | Kept |
|---|---|---|---|
| Postgres | `scripts/backup.sh` nightly at 03:15 UTC (`pg_dump -Fc`) | VM `/var/backups/sealforge`, plus the Storage Box via `BACKUP_REMOTE` | 14 days |
| Escrow wallet files (`SF_WALLET_DIR`) | Same script, tarball | Same | 14 days |
| `SF_DATA_KEY` | Never in a backup. Team password manager, plus a paper copy with the organiser | Offline | Forever |
| Backup encryption | `AGE_RECIPIENT` (age public key). The private key is held like `SF_DATA_KEY` | Offline | Forever |

```bash
# /etc/cron.d/sealforge-backup
15 3 * * * sealforge DATABASE_URL=... BACKUP_REMOTE=u123@u123.your-storagebox.de:sealforge AGE_RECIPIENT=age1... /opt/sealforge/scripts/backup.sh >> /var/log/sealforge-backup.log 2>&1
```

The RPO is 24 h. Funds never depend on data newer than the last backup: a lost day of DB rows is recovered by the scanner (payments) and by trustees re-signing any batch that wasn't confirmed.

### Restore drill (monthly, and before mainnet)

```bash
age -d -i key.txt sealforge-<stamp>.dump.age > /tmp/drill.dump
ADMIN_URL=postgres://sealforge:...@localhost:5432/postgres \
SOURCE_URL=$DATABASE_URL SF_DATA_KEY=... \
  scripts/restore-drill.sh /tmp/drill.dump
```

The drill restores into a scratch database, `sealforge_restore_drill`, and never into the live one. It checks that every migration is present, that row counts match the source, and that **every escrow UFVK and refund address decrypts** with `SF_DATA_KEY`. Then boot a coordinator on the scratch DB with `SF_REFUNDS_PAUSED=true` and `SF_DEV_CHAIN_HEIGHT` set (so it doesn't scan or spend), and compare a published result with production. Record the run in `docs/evidence/backup-drill.md`.

### Real restore

1. Stop the coordinator: `docker compose ... stop coordinator`.
2. Restore the latest dump into a fresh `sealforge` database with `pg_restore --no-owner`, and restore the wallet tarball into `SF_WALLET_DIR`.
3. Start with `SF_REFUNDS_PAUSED=true`. Check `/readyz` and let the scanner catch up (`scanner_lag_blocks` ≤ 4).
4. Batches left `awaiting_signatures` restart their signing round on their own. Batches marked `broadcast` are confirmed by the scanner once mined.
5. Unpause refunds.
