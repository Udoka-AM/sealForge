# Deploying sealForge on a VPS

This runbook moves the public site, today served from a laptop, to a single Linux VPS (written for Contabo, but any Ubuntu 24.04 server works). The public URLs stay the same: Cloudflare keeps serving `sealforge.udokaam.dev` and `sealforge-lwd.udokaam.dev` through a Cloudflare Tunnel, which now points at the VPS instead of the laptop.

**Who does what.** The **owner** (Udoka) hands over data and secrets and controls Cloudflare. The **operator** (the collaborator with the VPS) prepares the server and runs the deploy. Each step says who.

## 1. What runs on the server

```
Internet ──► Cloudflare ──(tunnel, outbound only)──► VPS
                                                      │
   sealforge.udokaam.dev ──────────► web (Next.js)   127.0.0.1:3200 ─┐
                                                                     │ /api/coordinator proxy
                                      coordinator    127.0.0.1:8090 ◄┘
                                        │  └─ PostgreSQL 127.0.0.1:5432 (Docker)
                                        └─ testnet.zec.rocks:443 (public lightwallet server)
   sealforge-lwd.udokaam.dev ───────► Envoy (gRPC-web) 127.0.0.1:8082 ──► testnet.zec.rocks:443
```

| Service | What it is | How it runs |
|---|---|---|
| `sealforge-coordinator` | Rust API, escrow scanner, closer, refund and payout worker | systemd, built from source |
| `sealforge-web` | Next.js site: auctions, bid flow, wallet, audit, console | systemd, `next start` |
| PostgreSQL 16 | The coordinator's database | Docker (`infra/docker-compose.yml`) |
| Envoy | gRPC-web proxy for the browser wallet and the browser audit | Docker |
| `cloudflared` | Cloudflare Tunnel connector: no inbound ports needed | systemd |

**No Zcash node is needed.** The coordinator and the browser sync from the public lightwallet server `testnet.zec.rocks`. It is trusted for liveness only: it can't move funds, and a hidden payment fails the public audit against any other server. Running your own Zebra and Zaino is optional (the `node` profile in `infra/docker-compose.yml`) and needs about 300 GB more disk.

**Keys never go on the server.** Trustee FROST shares stay with each trustee, and the organiser's key stays in the organiser's browser or CLI. The server holds only the coordinator database, the escrow wallet files, and `SF_DATA_KEY`, which decrypts the escrows' viewing keys stored in the database.

## 2. Requirements

### The VPS

| | Minimum | Why |
|---|---|---|
| OS | Ubuntu 24.04 LTS, 64-bit | Commands below assume it |
| CPU | 4 vCPU | Building the coordinator in release mode |
| RAM | 8 GB, plus 4 GB swap | The Rust release build peaks around 5 GB |
| Disk | 50 GB SSD/NVMe | Rust toolchain and build cache (about 15 GB), Node modules, Docker images, database, backups |
| Network | Outbound HTTPS | Cloudflare Tunnel and the lightwallet server. **No inbound ports except SSH** |

A Contabo "Cloud VPS" with 4 vCPU and 8 GB RAM or more is enough. Pick a region close to most users; the tunnel adds little latency.

### Access

- SSH access for the operator, by key (no passwords).
- The operator doesn't need a Cloudflare account. The owner either hands over the existing tunnel credentials (section 9, option A) or creates a tunnel token (option B).

### The hand-over (owner → operator)

| Item | Where it is today (owner's Mac) | Secret? | Needed for |
|---|---|---|---|
| Database dump | Docker Postgres, database `sealforge_testnet` | **Yes**: holds encrypted viewing keys and refund addresses | Keeping the existing auctions, results and audit pages |
| Escrow wallet files | `~/.sealforge-host/testnet/wallets/` | Yes | The coordinator's escrow scanning |
| `SF_DATA_KEY`, `SF_INVITE_CODES` | `~/.sealforge-host/testnet/host.env` | **Yes, critical** | Decrypting escrow keys; organiser sign-up |
| Test-network settings | `~/.sealforge-host/testnet/demo.env` | No | Short testnet windows (testnet only) |
| Tunnel credentials | `~/.cloudflared/<tunnel-id>.json` and the tunnel name in `tunnel.env` | **Yes** | Serving the existing hostnames (option A) |

**Never send secrets by chat, email or a Git issue.** Use an end-to-end encrypted channel (a shared password-manager item, or [`age`](https://github.com/FiloSottile/age)-encrypted files over `scp`). `SF_DATA_KEY` also stays in the owner's password manager: if it's lost, no escrow can be spent from again.

## 3. Prepare the server (operator)

Log in as root once, create a deploy user, and lock SSH down.

```bash
adduser --disabled-password --gecos "" sealforge
usermod -aG sudo sealforge
mkdir -p /home/sealforge/.ssh && cp ~/.ssh/authorized_keys /home/sealforge/.ssh/
chown -R sealforge:sealforge /home/sealforge/.ssh && chmod 700 /home/sealforge/.ssh
passwd -d sealforge && echo "sealforge ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/sealforge
```

In `/etc/ssh/sshd_config` set `PasswordAuthentication no` and `PermitRootLogin no`, then `systemctl restart ssh`. Log back in as `sealforge` before closing the root session.

**Firewall: SSH only.** The tunnel is outbound, so 80 and 443 stay closed.

```bash
sudo ufw default deny incoming && sudo ufw default allow outgoing
sudo ufw allow OpenSSH && sudo ufw enable
```

**Swap** (for the Rust build):

```bash
sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

**Packages:** build tools, Docker, Node 22 with pnpm, Rust, and `cloudflared`.

```bash
sudo apt-get update && sudo apt-get install -y build-essential pkg-config clang libsqlite3-dev protobuf-compiler git curl ca-certificates gnupg postgresql-client unattended-upgrades

# Docker
curl -fsSL https://get.docker.com | sudo sh && sudo usermod -aG docker sealforge

# Node 22 and pnpm (the repo pins pnpm 9.15 via packageManager)
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt-get install -y nodejs
sudo corepack enable

# Rust (rust-toolchain.toml pins 1.99.0; rustup installs it on first build)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal

# cloudflared
curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/cloudflared.list
sudo apt-get update && sudo apt-get install -y cloudflared
```

Log out and back in so the `docker` group and `~/.cargo/bin` are picked up.

## 4. Get the code and build it (operator)

```bash
sudo mkdir -p /opt/sealforge /var/lib/sealforge/wallets /etc/sealforge /var/backups/sealforge
sudo chown -R sealforge:sealforge /opt/sealforge /var/lib/sealforge /var/backups/sealforge
sudo chmod 700 /var/lib/sealforge /etc/sealforge /var/backups/sealforge

git clone https://github.com/Udoka-AM/sealForge /opt/sealforge && cd /opt/sealforge
cargo build --release -p sealforge-coordinator -p sealforge-cli   # 10–20 min the first time
pnpm install --frozen-lockfile
pnpm --filter @sealforge/web build
```

The browser WebAssembly bundles (wallet, auditor, scanner, signer) are committed under `apps/web/public/`, so no WASM build is needed.

## 5. Database (operator)

Start PostgreSQL with a strong password. The compose file binds it to `127.0.0.1` only.

```bash
cd /opt/sealforge
echo "POSTGRES_PASSWORD=$(openssl rand -hex 24)" | sudo tee /etc/sealforge/postgres.env >/dev/null
sudo chmod 600 /etc/sealforge/postgres.env
sudo sh -c 'set -a; . /etc/sealforge/postgres.env; docker compose -f /opt/sealforge/infra/docker-compose.yml up -d postgres'
docker exec sealforge-postgres-1 createdb -U sealforge sealforge_testnet
```

Make it start again after a reboot:

```bash
docker update --restart unless-stopped sealforge-postgres-1
```

## 6. Bring the existing data over (owner, then operator)

Skip this section only if you want a fresh, empty instance. The existing auctions, results and audit links (`KE5DFF4PDA`, `K7GHHBC4GP`) then stop working.

**Rehearse first to keep downtime short.** Do a first pass with a dump taken while the Mac is still serving (skip the `stop`), and finish sections 7–8 against it. Nothing public changes until section 9. For the real cutover, repeat this section with a fresh dump: empty the database first (`docker exec sealforge-postgres-1 dropdb -U sealforge sealforge_testnet && docker exec sealforge-postgres-1 createdb -U sealforge sealforge_testnet`), then go straight on to section 9. Downtime is then a few minutes.

**Owner, on the Mac.** Freeze the old instance first, so nothing changes after the dump.

```bash
cd ~/zecathon/sealForge && scripts/host-local.sh stop     # stops the coordinator, web, tunnel and Envoy
docker exec sealforge-postgres-1 pg_dump -U sealforge -Fc --no-owner sealforge_testnet > sealforge_testnet.dump
tar -czf sealforge-wallets.tar.gz -C ~/.sealforge-host/testnet wallets
```

Send `sealforge_testnet.dump`, `sealforge-wallets.tar.gz` and the values of `SF_DATA_KEY` and `SF_INVITE_CODES` over an encrypted channel (section 2). The site is down from here until section 9 is finished, so do sections 7–9 promptly.

**Operator, on the VPS:**

```bash
docker exec -i sealforge-postgres-1 pg_restore -U sealforge -d sealforge_testnet --no-owner < sealforge_testnet.dump
tar -xzf sealforge-wallets.tar.gz -C /var/lib/sealforge        # → /var/lib/sealforge/wallets
chmod 700 /var/lib/sealforge/wallets
shred -u sealforge_testnet.dump sealforge-wallets.tar.gz        # once the service is verified
```

## 7. Configuration (operator)

`/etc/sealforge/coordinator.env` (mode 600). Fill in the two secrets from the owner and the Postgres password from `/etc/sealforge/postgres.env`.

```ini
DATABASE_URL=postgres://sealforge:<POSTGRES_PASSWORD>@127.0.0.1:5432/sealforge_testnet
SF_LISTEN_ADDR=127.0.0.1:8090
SF_NETWORK=testnet
SF_ZAINO_URL=https://testnet.zec.rocks:443
SF_WALLET_DIR=/var/lib/sealforge/wallets
SF_DATA_KEY=<from the owner>
SF_INVITE_CODES=<from the owner>
# Testnet only: short auction windows and quick refunds (refused on mainnet).
SF_MIN_WINDOW_BLOCKS=24
SF_REFUND_DELAY_SECS=20,120
RUST_LOG=info
```

`/etc/sealforge/web.env` (mode 644, no secrets):

```ini
COORDINATOR_URL=http://127.0.0.1:8090
LIGHTWALLETD_GRPCWEB=https://sealforge-lwd.udokaam.dev
NEXT_PUBLIC_DEFAULT_NETWORK=testnet
NODE_ENV=production
# pnpm comes from Corepack: never stop to ask about downloading it under systemd.
COREPACK_ENABLE_DOWNLOAD_PROMPT=0
```

```bash
sudo chmod 600 /etc/sealforge/coordinator.env && sudo chown sealforge:sealforge /etc/sealforge/*.env
```

Without `LIGHTWALLETD_GRPCWEB` the site still works, but the wallet page says the wallet is unavailable and the bid flow offers QR payment only.

## 8. Services (operator)

**Envoy** (gRPC-web → `testnet.zec.rocks`):

```bash
sed "s|UPSTREAM_HOST|testnet.zec.rocks|g" /opt/sealforge/infra/envoy-tls.yaml | sudo tee /etc/sealforge/envoy.yaml >/dev/null
docker run -d --name sealforge-envoy --restart unless-stopped -p 127.0.0.1:8082:8081 \
  -v /etc/sealforge/envoy.yaml:/etc/envoy/envoy.yaml:ro envoyproxy/envoy:v1.31-latest
```

**Coordinator:** `/etc/systemd/system/sealforge-coordinator.service`

```ini
[Unit]
Description=sealForge coordinator
After=network-online.target docker.service
Wants=network-online.target

[Service]
User=sealforge
EnvironmentFile=/etc/sealforge/coordinator.env
ExecStart=/opt/sealforge/target/release/sealforge-coordinator
Restart=always
RestartSec=5
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

**Website:** `/etc/systemd/system/sealforge-web.service`

```ini
[Unit]
Description=sealForge website
After=network-online.target sealforge-coordinator.service

[Service]
User=sealforge
WorkingDirectory=/opt/sealforge
EnvironmentFile=/etc/sealforge/web.env
ExecStart=/usr/bin/pnpm --filter @sealforge/web exec next start -p 3200 -H 127.0.0.1
Restart=always
RestartSec=5
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now sealforge-coordinator sealforge-web
curl -s http://127.0.0.1:8090/readyz && echo      # the coordinator is up and its database is reachable
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:3200/   # 200
```

Logs: `journalctl -u sealforge-coordinator -f` and `journalctl -u sealforge-web -f`. The coordinator applies any pending database migrations on start.

## 9. Point Cloudflare at the VPS

### Option A: move the existing tunnel (fewest changes)

**Owner:** send the tunnel name (from `~/.sealforge-host/testnet/tunnel.env`) and its credentials file `~/.cloudflared/<tunnel-id>.json` over an encrypted channel.

**Operator:**

```bash
sudo mkdir -p /etc/cloudflared && sudo cp <tunnel-id>.json /etc/cloudflared/ && sudo chmod 600 /etc/cloudflared/<tunnel-id>.json
sudo tee /etc/cloudflared/config.yml >/dev/null <<'EOF'
tunnel: <tunnel-id>
credentials-file: /etc/cloudflared/<tunnel-id>.json
ingress:
  - hostname: sealforge.udokaam.dev
    service: http://127.0.0.1:3200
  - hostname: sealforge-lwd.udokaam.dev
    service: http://127.0.0.1:8082
  - service: http_status:404
EOF
sudo cloudflared service install && sudo systemctl enable --now cloudflared
```

The DNS records already point at this tunnel, so traffic reaches the VPS as soon as its connector is up. **Make sure the Mac's connector is stopped** (section 6 did this): two connectors on one tunnel split traffic between the two machines.

### Option B: a new tunnel managed from the dashboard

**Owner:**
1. In Cloudflare Zero Trust → Networks → Tunnels, create a tunnel and copy its connector token.
2. Add public hostnames: `sealforge.udokaam.dev` → `http://127.0.0.1:3200` and `sealforge-lwd.udokaam.dev` → `http://127.0.0.1:8082`. Remove the old tunnel's DNS records first if the dashboard says they exist.

**Operator:** `sudo cloudflared service install <token>`.

## 10. Verify (operator, then owner)

```bash
for p in / /a/KE5DFF4PDA /a/K7GHHBC4GP /audit/K7GHHBC4GP /wallet /me /verify /how-it-works /console /api/wallet/config; do
  printf "%s %s\n" "$(curl -s -o /dev/null -w '%{http_code}' https://sealforge.udokaam.dev$p)" "$p"; done
```

All should be `200`. Then in a browser:

1. **Home:** the curtain, the hero and the live status bar with a block height.
2. **Audit:** open an auction's audit and run "Rescan the chain in my browser". Every figure should match (this exercises `sealforge-lwd` through Envoy).
3. **Wallet:** the onboarding shows Create, Link and Use your own wallet, not "unavailable".
4. **Settled auctions:** the result panels of `KE5DFF4PDA` and `K7GHHBC4GP` show their published figures (this confirms the database came over).

Once it's all good, the owner can turn off the Mac hosting for good (`scripts/host-local.sh stop`).

## 11. Deploying updates (operator)

```bash
cd /opt/sealforge && git pull --ff-only
cargo build --release -p sealforge-coordinator
pnpm install --frozen-lockfile && pnpm --filter @sealforge/web build
sudo systemctl restart sealforge-coordinator sealforge-web
```

`next build` replaces the files the running site serves from, so **restart `sealforge-web` straight after the build.** Until the restart, some pages return errors. Never build in `/opt/sealforge` and leave the old process running.

**Rollback:** `git checkout <previous commit>`, rebuild, restart.

## 12. Backups (operator)

The database holds each escrow's viewing key, encrypted with `SF_DATA_KEY`. Back it up nightly with `scripts/backup.sh` (details and the restore drill: [infra/README.md](../infra/README.md#backups-task-56)).

```bash
# /etc/cron.d/sealforge-backup
15 3 * * * sealforge . /etc/sealforge/coordinator.env; DATABASE_URL="$DATABASE_URL" PG_DUMP="docker exec -i sealforge-postgres-1 pg_dump -U sealforge" SF_WALLET_DIR=/var/lib/sealforge/wallets BACKUP_DIR=/var/backups/sealforge /opt/sealforge/scripts/backup.sh >> /var/log/sealforge-backup.log 2>&1
```

Add `AGE_RECIPIENT=age1…` to encrypt backups, and `BACKUP_REMOTE=user@host:path` to copy them off the server. Contabo's own snapshots are a useful extra, not a replacement. `SF_DATA_KEY` is never in a backup: it lives in the owner's password manager.

## 13. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `502` or Cloudflare error page | `sealforge-web` down, or the tunnel points at the wrong port | `systemctl status sealforge-web cloudflared`; check `config.yml` |
| Pages load, auction data missing, "coordinator unavailable" | Coordinator down or the database unreachable | `journalctl -u sealforge-coordinator`; `curl 127.0.0.1:8090/readyz`; check `DATABASE_URL` |
| `500` on some pages right after an update | Build finished, web not restarted | `sudo systemctl restart sealforge-web` |
| Wallet page says "unavailable" | `LIGHTWALLETD_GRPCWEB` missing in `web.env` | Set it, restart `sealforge-web` |
| Browser audit or wallet can't sync | Envoy down, or `testnet.zec.rocks` unreachable | `docker ps`, `docker logs sealforge-envoy`; `curl -sI https://sealforge-lwd.udokaam.dev` |
| Cargo build killed | Out of memory | Check swap is on (`swapon --show`); build with `-j 2` |
| Coordinator fails to decrypt escrows | Wrong `SF_DATA_KEY` | Use the exact value from the owner's `host.env` |
| Traffic flips between old and new data | The Mac's tunnel connector is still running | Stop it on the Mac |

## 14. Moving to mainnet later

Same layout, with these changes:
- `SF_NETWORK=mainnet` and `NEXT_PUBLIC_DEFAULT_NETWORK=mainnet`.
- A new database (`sealforge_mainnet`) and a **new `SF_DATA_KEY`**.
- `SF_ZAINO_URL=https://zec.rocks:443`, with Envoy pointed at `zec.rocks`.
- No `SF_MIN_WINDOW_BLOCKS` or `SF_REFUND_DELAY_SECS`: they're refused on mainnet.
- Before real funds: per-auction caps, independent trustees, an external review of the escrow signing path, and preferably your own Zebra and Zaino (the `node` profile, about 300 GB more disk). See [docs/THREAT-MODEL.md](THREAT-MODEL.md).
