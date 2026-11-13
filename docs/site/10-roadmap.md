# 10. Roadmap and mainnet gates

Phases in order, each with an exit criterion. No dates: none are committed in the repository. Mainnet has not been run.

| # | Phase | Exit criterion | Status |
|---|---|---|---|
| 1 | **Docs and reviewer feedback** | These pages published; the questions in [11](11-open-questions.md) answered or explicitly deferred by reviewers; each answer recorded in the [changelog](appendix-changelog.md) | In progress |
| 2 | **External review and the mainnet gate checklist** | Every item in the [mainnet gate checklist](05-security.md#mainnet-gate-checklist) done. That includes trustee-side PCZT verification (D1), an external review of the escrow signing path and DKG relay, clean reproducible WASM builds, and FROST on an upstream-published ciphersuite | `Not built` |
| 3 | **Mainnet with caps and independent trustees** | One auction settled on mainnet under a published per-auction cap, with three independent trustee operators, and its result independently re-audited by a third party with `sealforge-audit` | `Not built` |
| 4 | **First-seller pilots** | One real seller in one vertical ([9](09-product.md#verticals)) runs a settled mainnet auction | `Not built` |
| 5 | **ShieldRamp** | Quote, swap leg and delayed bid built and run on mainnet, with the Linkability Score shown before signing (today only the score is `Tested`) | `Specified` |
| 6 | **Protocol fee and white label** | Take-rate output in the payout sweep, included in the published result and checked by the auditor; branded audit page | `Specified` |

The phase 2 checklist lives in one place: [5 Security §Mainnet gate checklist](05-security.md#mainnet-gate-checklist).
