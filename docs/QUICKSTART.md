# Run your own auction

From a fresh clone to an open sealed-bid auction with a 2-of-3 escrow. Everything below runs locally. Opening an auction for real bids also needs a synced Zcash node (see [infra/README.md](../infra/README.md)).

## 1. Start the coordinator

Requirements: Rust (via `rust-toolchain.toml`), Docker.

```bash
git clone https://github.com/Udoka-AM/sealForge && cd sealForge
docker compose -f infra/docker-compose.yml up -d postgres
cargo build --release -p sealforge-coordinator -p sealforge-cli
```

```bash
DATABASE_URL=postgres://sealforge:sealforge@localhost:5432/sealforge \
SF_INVITE_CODES=my-invite SF_DATA_KEY="$(openssl rand -hex 32)" \
SF_DEV_CHAIN_HEIGHT=3900000 \
./target/release/sealforge-coordinator
```

`SF_DEV_CHAIN_HEIGHT` stands in for the chain tip until the scanner is connected to a node. Keep `SF_DATA_KEY` safe: it encrypts the escrow viewing key.

## 2. Register as the organiser

```bash
./target/release/sealforge init --invite my-invite --name "My Organisation"
```

This creates `~/.sealforge/organiser.key`, readable only by you.

## 3. Create the auction

Copy `examples/auction.toml`, then set your title, lots, reserve, close height and payout Unified Address:

```bash
./target/release/sealforge auction new my-auction.toml
```

It prints three one-time invite commands. Send each to one trustee **over a private channel**.

## 4. Trustees generate the escrow key (each on their own machine)

```bash
sealforge trustee join <invite-code>
sealforge trustee dkg <AUCTION_CODE> --wait
```

Each trustee ends with a key share on their own disk and a fingerprint like `f698-0330-a8f1-…`. **Compare fingerprints on a call or in person.** If they differ, stop. When the third trustee finishes, the coordinator derives the escrow address.

## 5. Open it

```bash
./target/release/sealforge auction open <AUCTION_CODE>
```

The close height must be at least 144 blocks (about 3 hours at 75 s blocks) after the current height. Bidders now use the web app (`pnpm dev`, then open `/a/<AUCTION_CODE>`).

Optional: `sealforge auction fund-fees <AUCTION_CODE>` prints a payment request for a small fee buffer. Fees are otherwise paid out of the escrow and reduce your payout. The buffer only matters if the auction might end with no winners.

## 6. After close

The coordinator closes the auction automatically once the chain is 10 blocks past the close height. Then:

```bash
./target/release/sealforge auction publish <AUCTION_CODE>
```

Anyone can now audit at `/audit/<AUCTION_CODE>`, or independently from the chain:

```bash
sealforge-audit --published <coordinator>/v1/auctions/<AUCTION_CODE>/result --endpoint <lightwalletd URL>
```

Bidders check their receipts at `/verify`. Refunds go out in batches after random delays, and each batch needs two trustees:

```bash
sealforge trustee sign <AUCTION_CODE> --wait     # or the browser page /trustee/<AUCTION_CODE>
```

After the last refund confirms, the rest of the escrow (winners' payments, minus fees) is paid to your payout address with the same 2-of-3 flow, and the auction is `settled`.

## Or use the browser console

Everything above except the trustees' key generation can be done at `/console`: load `~/.sealforge/organiser.key`, create the auction, open it, publish it, and follow refunds and the payout.

## Try it without a node

To see the whole flow end to end on your machine:

```bash
cargo test -p sealforge-cli --test dkg_e2e        # organiser + 3 trustees + DKG + open
cargo test -p sealforge-coordinator --test close_publish   # close, publish, refunds
```

Both need `DATABASE_URL` pointing at the Postgres from step 1.
