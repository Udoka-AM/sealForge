# Hosting the testnet site on one server

The hosted testnet site (`sealforge.udokaam.dev`, gRPC-web at `sealforge-lwd.udokaam.dev`) runs on a single Linux server in Docker, using [infra/vps/docker-compose.yml](../infra/vps/docker-compose.yml). It has run there since 2026-11-13, when it moved off a laptop.

| Container | Does |
|---|---|
| `postgres` | Coordinator database. Published on `127.0.0.1:5432` only, for the nightly backup |
| `coordinator` | API, scanner, refund scheduler. Escrow wallet files in a host directory (uid 10001) |
| `web` | The Next.js site. Proxies `/api/coordinator/*` to the coordinator server side |
| `envoy` | gRPC-web proxy to `testnet.zec.rocks:443`, for the browser wallet and in-browser audit |
| `cloudflared` | Cloudflare named-tunnel connector. Dials out, so the site needs no open inbound ports |

Server size: 4 cores, 8 GB RAM. A build needs about 25 GB free (the Rust and Next.js layers); the running stack needs far less.

## Layout on the server

Keep secrets outside the checkout:

```
/root/sealForge/                  git checkout of main
/opt/sealforge/.env               POSTGRES_PASSWORD, SF_COORDINATOR_ENV, SF_LWD_HOST, SF_CLOUDFLARED_DIR  (600)
/opt/sealforge/coordinator.env    SF_DATA_KEY, SF_INVITE_CODES, SF_NETWORK=testnet, SF_ZAINO_URL,
                                  SF_WALLET_DIR=/data/wallets, SF_LISTEN_ADDR=0.0.0.0:8080 ...  (600)
/opt/sealforge/cloudflared/       config.yml (from infra/vps/cloudflared.example.yml) + credentials.json
                                  (600, owned by 65532)
/var/lib/sealforge/wallets/       escrow-<auction id>.sqlite  (700, owned by 10001)
```

`SF_DATA_KEY` also goes in the team password manager. Without it no backup can spend from an escrow.

## Deploy a new version

A deploy restarts the live site. Do it on purpose, not as a side effect.

```bash
cd /root/sealForge && git pull --ff-only
docker compose -f infra/vps/docker-compose.yml --env-file /opt/sealforge/.env build web coordinator
docker compose -f infra/vps/docker-compose.yml --env-file /opt/sealforge/.env up -d
```

Before building, tag the running images so you can go back: `docker tag sealforge-web:local sealforge-web:prev` (same for the coordinator). To roll back, retag `:prev` as `:local` and run `up -d` again. Migrations only add, so an older coordinator still starts on a newer database; check `migrations/` before relying on that.

Old build layers pile up (tens of GB after a few builds). `docker builder prune --filter until=168h` clears them.

## Moving the site from another machine

This is how the site moved off the laptop. Total downtime was about a minute.

1. **Rehearse.** Build and start the stack on the server with no tunnel. Copy a `pg_dump -Fc` of the old database and the wallet directory across, restore, and compare `/v1/auctions/<code>`, `/result`, `/batches` and `/refunds` from both machines byte for byte.
2. **Copy for real.** Stop the old coordinator so nothing changes. Dump and restore again, copy the wallet files again (on macOS: `COPYFILE_DISABLE=1 tar --no-xattrs`, or `._*` files come along), and check row counts and wallet `sha256sum`s on both sides. Copy `SF_DATA_KEY` and `SF_INVITE_CODES` over a pipe, never through chat or a ticket. Start the coordinator.
3. **Switch the tunnel.** Start `cloudflared` on the server and wait for `Registered tunnel connection` twice in its log. For a moment both machines serve the tunnel. Stop the old connector at once: while both run, Cloudflare splits requests between them.
4. **Check.** `/`, an audit page, `/wallet` and `/api/coordinator/v1/chain` return 200, and **Rescan the chain in my browser** on an audit page reports that every published figure matches.
5. Keep the old machine's data, stopped, until the new one has run cleanly for a while. Never start its tunnel again.

## Backups

`/etc/cron.d/sealforge-backup` runs [scripts/backup.sh](../scripts/backup.sh) at 03:15 UTC through a small wrapper that reads its settings from a root-only file, so no secret sits in the crontab. See [infra/README.md](../infra/README.md#backups-task-56) for what is backed up, the restore drill and a real restore.

Set both of these, or a lost server takes its backups with it:

- `BACKUP_REMOTE`: an rsync target on another machine.
- `AGE_RECIPIENT`: an age public key. Keep the private key offline, like `SF_DATA_KEY`.
