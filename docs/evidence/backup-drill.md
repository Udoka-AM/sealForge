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

What this proves: two trustees' strings hold everything a spend needs. The sweep itself is drilled below.

Run this drill on each mainnet auction after key generation: every trustee runs `check-backup` on the string it stored offline, and two of them run `recover-check` together.

## Sweep from two backups, coordinator stopped (#64)

**Date:** 2026-11-21, local regtest, auction `G2ME977STY` after its refunds, with the coordinator process stopped (`/readyz` unreachable). The escrow still held the winning bid's 0.1998 ZEC.

| Step | Command | Result |
|---|---|---|
| Backups | `sealforge trustee backup G2ME977STY` for trustees 1 and 2, coordinator down | 236-character strings from each keystore; birthday height 1984 recorded |
| 1. Start (trustee 1) | `sealforge escrow recover-start --backup t1.txt --lightwalletd http://127.0.0.1:8137 --to <agreed address>` | Rebuilt the escrow wallet from the backup's viewing key, scanned from height 1984 in under 3 s, found 1 note, built and proved a sweep of 0.1997 ZEC (fee 0.0001), committed nonces (`nonces.secret.json`, 0600), wrote `request.json` |
| 2. Co-sign (trustee 2) | `sealforge escrow recover-cosign --backup t2.txt --request request.json --to <agreed address>` | Refused with another destination typed ("the request sweeps to …, not the address you agreed"), and with trustee 1's own backup ("you are trustee 1"). Without `--approve` it showed the decoded transaction: one payment to the agreed address, the spend, the fee, the expiry. With `--approve` it signed and wrote `reply.json` |
| 3. Finish (trustee 1) | `sealforge escrow recover-finish --backup t1.txt --reply reply.json --lightwalletd …` | Signed, aggregated, checked each signature under the escrow's key, broadcast `4d66c55a…8bf1`; mined at height 2074. A second run was refused: the nonces had been deleted before signing |
| Empty | `recover-start` again from trustee 2's backup | "escrow holds 0 zat: nothing left to sweep" |

The sweep sends everything to one agreed address (for example a new escrow, or each bidder's refund in a later step). Refunding bidders note by note straight from backups is not built.
