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

## Trustee backups (no coordinator)

**Date:** 2026-11-21, local regtest, auction `KBANH7TJZ2` (settled; keystores from before viewing keys were kept).

| Step | Command | Result |
|---|---|---|
| Back up | `sealforge trustee backup KBANH7TJZ2 > t1.txt` (trustees 1 and 3) | The older keystores fetched the escrow viewing key from the coordinator once, checked it against the attested address, and kept it (`escrow-view.json`, 0600). Each string is 236 characters, `sfbackup1…` |
| Check | `sealforge trustee check-backup KBANH7TJZ2 t1.txt` | `Backup OK: trustee 1 of a 2-of-3 escrow`, with the escrow address bidders were shown. Trustee 3's string on trustee 1's machine: refused (another trustee's share). One edited character: checksum error |
| Recover | `sealforge escrow recover-check t1.txt t3.txt`, no coordinator, no keystore | Trustees 1 and 3 signed a random spend authorization together and it verified under the escrow's `ak`. The printed escrow address equals the public page's |

What this proves: two trustees' strings hold everything a spend needs. **Not yet drilled:** building and broadcasting an actual sweep from those strings without the coordinator (scanning with the viewing key, building the PCZT, two-party signing). That tooling is the next step; until then the coordinator builds every spend.

Run this drill on each mainnet auction after key generation: every trustee runs `check-backup` on the string it stored offline, and two of them run `recover-check` together.
