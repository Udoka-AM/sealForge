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

## Deploy (task 6.2)

```bash
docker compose -f infra/docker-compose.yml --profile node --profile app pull
docker compose -f infra/docker-compose.yml --profile node --profile app up -d
```

**Rollback:** set `SF_IMAGE_TAG` to the previous tag and re-run `up -d`. Pause refunds with `SF_REFUNDS_PAUSED=true`.

## Backups (task 5.6)

TODO: nightly `pg_dump` + WAL archiving to a Hetzner Storage Box (14-day retention), plus the restore drill runbook.
