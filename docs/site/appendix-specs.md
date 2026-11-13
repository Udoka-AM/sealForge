# Appendix: ZIP and spec index

Every specification these pages cite, and what for. Titles and statuses as listed in the [ZIP index](https://zips.z.cash/) on 2026-11-13. Statuses change; check the index.

| Spec | Title (status) | Used for | Cited in |
|---|---|---|---|
| [ZIP 32](https://zips.z.cash/zip-0032) | Shielded Hierarchical Deterministic Wallets (Final) | Bidder wallet keys from a seed, account 0 | [3 §Bidder wallet keys](03-zcash-integration.md#bidder-wallet-keys) |
| [ZIP 302](https://zips.z.cash/zip-0302) | Standardized Memo Field Format (Draft) | 512-byte memo field, text memos | [2.3](02-protocol/wire-formats.md), [3 §Memos](03-zcash-integration.md#memos) |
| [ZIP 307](https://zips.z.cash/zip-0307) | Light Client Protocol for Payment Detection (Draft) | Compact-block scanning, then full-transaction decryption | [3 §Light client protocol](03-zcash-integration.md#light-client-protocol) |
| [ZIP 312](https://zips.z.cash/zip-0312) | FROST for Spend Authorization Multisignatures (Draft) | Rerandomized FROST for the spend authorization key. **Not** cited for `nk`/`rivk` generation, which it doesn't specify | [3 §Escrow key construction](03-zcash-integration.md#escrow-key-construction), [§Threshold signing](03-zcash-integration.md#threshold-signing) |
| [ZIP 316](https://zips.z.cash/zip-0316) | Unified Addresses and Unified Viewing Keys | Receiver types, UFVK, UIVK | [3 §UA, UFVK, UIVK](03-zcash-integration.md#unified-addresses-ufvk-uivk) |
| [ZIP 317](https://zips.z.cash/zip-0317) | Proportional Transfer Fee Mechanism | Escrow and wallet fees (`StandardFeeRule::Zip317`) | [3 §Fees](03-zcash-integration.md#fees), [9 §Unit economics](09-product.md#unit-economics) |
| [ZIP 318](https://zips.z.cash/zip-0318) | Orchard to Ironwood Migration (Draft) | Only as a comparison: its denominations share the {1, 2, 5} × 10^k shape. sealForge does **not** implement it | [2.7](02-protocol/refunds.md#mechanism) |
| [ZIP 321](https://zips.z.cash/zip-0321) | Payment Request URIs (Active) | One URI per bid, base64url memo | [2.3](02-protocol/wire-formats.md#zip-321-payment-uri) |
| [ZIP 374](https://zips.z.cash/zip-0374) | Partially Created Zcash Transaction Format (Draft) | Escrow spend roles | [3 §PCZT](03-zcash-integration.md#pczt) |
| [ZIP 258](https://zips.z.cash/zip-0258) | Deployment of the NU6.3 Network Upgrade (Final) | Ironwood | [3 §Shielded pool](03-zcash-integration.md#shielded-pool-ironwood) |
| [ZIP 259](https://zips.z.cash/zip-0259) | Deployment of the NU7 Network Upgrade (Proposed) | Testnet network | [3 §Network](03-zcash-integration.md#network-and-consensus) |
| [ZIP 326](https://zips.z.cash/zip-0326) | NU6.3 Consequences for Wallets (Draft) | Wallet behaviour around Ironwood | [3 §Shielded pool](03-zcash-integration.md#shielded-pool-ironwood) |
| [ZIP 2005](https://zips.z.cash/zip-2005) | Ironwood Quantum Recoverability (Final) | Cited by `orchard 0.16.0` for `rcm_v3`. Not restated | [2.4](02-protocol/commitments.md#note-level-randomness-is-not-ours) |
| [ZIP 218](https://zips.z.cash/zip-0218) | 25-second Block Target Spacing (Proposed) | Context for the 25 s testnet spacing in the evidence | [3 §Network](03-zcash-integration.md#network-and-consensus) |
| [Zcash Protocol Specification](https://zips.z.cash/protocol/protocol.pdf) | | Orchard keys, note commitments (§4.7.3 "Sending Notes (Orchard)" in the NU5 edition, as cited by `orchard`), RedDSA/RedPallas, note encryption. Handled by `orchard 0.16.0`, `zcash_note_encryption 0.5.1`, `reddsa 0.6.1`; not restated | [2.4](02-protocol/commitments.md), [3](03-zcash-integration.md) |
| [RFC 9591](https://www.rfc-editor.org/rfc/rfc9591) | The Flexible Round-Optimized Schnorr Threshold (FROST) Protocol for Two-Round Schnorr Signatures | FROST | [3 §Threshold signing](03-zcash-integration.md#threshold-signing) |
| [RFC 7693](https://www.rfc-editor.org/rfc/rfc7693) | The BLAKE2 Cryptographic Hash and Message Authentication Code | All sealForge hashes | [2.4](02-protocol/commitments.md#hash-function) |
| [RFC 4648 §5](https://www.rfc-editor.org/rfc/rfc4648#section-5) | Base 64 Encoding with URL and Filename Safe Alphabet | ZIP 321 memo encoding | [2.3](02-protocol/wire-formats.md#zip-321-payment-uri) |
| [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) | Key words for use in RFCs | MUST/SHOULD | [2.1](02-protocol/roles.md) |

Section numbers inside the protocol specification change between editions. Where we needed one, we used the one the upstream crate cites, with its edition.
