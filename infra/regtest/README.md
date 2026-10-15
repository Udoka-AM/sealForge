# Local regtest chain

A private Zcash chain on your machine: Zebra in regtest mode with every network upgrade through Ironwood (NU6.3) active from block 1, and no proof of work. Blocks are mined on demand. Use it to develop and test the scanner, escrow spends and refunds without a testnet node.

```bash
scripts/regtest.sh up <miner-unified-address>   # start Zebra
scripts/regtest.sh mine 101                     # mine blocks (coinbase matures after 100)
scripts/regtest.sh height
scripts/regtest.sh down                         # stop (add --wipe to delete the chain)
```
