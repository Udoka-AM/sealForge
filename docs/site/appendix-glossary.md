# Appendix: glossary

| Term | Meaning here |
|---|---|
| **α (alpha)** | The spend authorization randomizer for one Orchard/Ironwood action. Fixed in the PCZT by the IO finalizer. Trustees sign with their shares randomized by α |
| **`ak`, `nk`, `rivk`** | The three components of an Orchard full viewing key. `ak` is the spend validating key (here the FROST group key); `nk` and `rivk` are viewing components (here sampled by the coordinator) |
| **`rk`** | The randomized spend validating key, `rk = ak + [α]G`. A spend signature must verify under it |
| **Canonical denomination** | A refund note value of {1, 2, 5} × 10^k zatoshi between 0.0001 and 5,000 ZEC ([2.7](02-protocol/refunds.md)) |
| **Clearing price** | In uniform-price mode, the amount every winner pays: the lowest winning bid ([2.6](02-protocol/clearing.md)) |
| **Commitment** | `BLAKE2b` of the auction code, refund address, salt and (sealed-reveal) price, carried in the bid memo ([2.4](02-protocol/commitments.md)) |
| **Coordinator** | The `sealforge-coordinator` service: API, scanner, closer, refund planner, PCZT builder. Holds the escrow UFVK, never a key share |
| **Deposit mode** | The bid amount is the payment amount. Simple, but the coordinator sees amounts as they arrive |
| **DKG** | Distributed key generation. The three FROST rounds the trustees run to create the escrow's group key without any party holding the whole key |
| **Dust** | The part of a refund below 0.0001 ZEC. Not sent; it leaves with the payout |
| **Escrow** | The per-auction Unified Address (Orchard receiver only) whose spend key is FROST 2-of-3 |
| **Fee buffer** | The organiser's optional `SF1FEE` deposit that pays escrow fees. The unused part returns with the payout |
| **FROST** | Flexible Round-Optimized Schnorr Threshold signatures ([RFC 9591](https://www.rfc-editor.org/rfc/rfc9591)). Here rerandomized, over RedPallas ([ZIP 312](https://zips.z.cash/zip-0312)) |
| **Included bid** | A `valid` or `under_reserve` bid. It goes into the Merkle tree |
| **Ironwood** | The shielded value pool introduced with NU6.3 ([ZIP 258](https://zips.z.cash/zip-0258)), using Orchard keys and receivers |
| **Orchard receiver** | The Orchard component of a Unified Address. Required for every escrow, refund and payout address here |
| **PCZT** | Partially Created Zcash Transaction ([ZIP 374](https://zips.z.cash/zip-0374)): a transaction passed between roles (creator, IO finalizer, prover, signer, extractor) |
| **Published result** | `GET /v1/auctions/{code}/result`: rules, scan range, UIVK, figures, Merkle root, per-bid outcomes ([2.3](02-protocol/wire-formats.md#published-result)) |
| **Receipt** | The bidder's JSON record (salt included), kept in the browser ([2.3](02-protocol/wire-formats.md#receipt-json)) |
| **Sealed-reveal mode** | Commit-reveal. The deposit is a budget and its memo commits to a hidden price, revealed by an `SF1R` payment after the bidding close |
| **Trustee** | One of three holders of a FROST key share for an auction's escrow |
| **UA / UFVK / UIVK** | Unified Address / Unified Full Viewing Key / Unified Incoming Viewing Key ([ZIP 316](https://zips.z.cash/zip-0316)). The UIVK is published at publish; the UFVK stays private |
| **Zatoshi (zat)** | 10^−8 ZEC. All amounts in code are integer zatoshi |
