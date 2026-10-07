# Linkability test harness (task 4.3)

Runs the same bid set through a **naive** refund flow (one exact refund per bidder, immediately) and the **sealForge** flow (canonical denominations, random delays, cross-bidder batching). A heuristic attacker then tries to link refunds to bids by amount and timing.

Target (PRD AC13): naive ≈ 100% linkable, sealForge < 5%. Output chart: `docs/evidence/linkability.svg`.

Not built yet.
