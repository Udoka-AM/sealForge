# Backup and restore drill (task 5.6)

**Date:** 2026-10-15, local machine, against the regtest coordinator database (`sealforge_e2e`, two settled auctions with real on-chain escrows).

**Backup:** `scripts/backup.sh` produced a 157 KB custom-format dump and a 333 KB wallet tarball, both mode 0600 in a 0700 directory.

**Restore:** `scripts/restore-drill.sh` restored into `sealforge_restore_drill` in 1 s:

```text
ok    migrations applied (6)
ok    organisers rows (1)
ok    auctions rows (2)
ok    trustees rows (6)
ok    bids rows (9)
ok    clearing_results rows (2)
ok    refund_notes rows (14)
ok    tx_batches rows (4)
ok    signature_shares rows (10)
ok    escrow UFVKs decrypt with SF_DATA_KEY (2)
ok    refund addresses decrypt (8)
```

**Service check:** a coordinator booted on the restored database (refunds paused, no scanning) served `GET /v1/auctions/NKZNE8FM69/result` with the same clearing price (0.12 ZEC), winner count (2) and Merkle root (`a8325e35…`) as the live one.

**Not yet drilled:** encryption with `age` and the copy to the Storage Box. Both need the production VM (task 1.3) and will be re-run there before mainnet.
