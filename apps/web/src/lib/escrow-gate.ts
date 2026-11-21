import type { AuctionView, EscrowCheck } from "./types";

/** The state of an in-browser escrow check. */
export type EscrowCheckState =
  | { status: "checking" }
  | { status: "done"; check: EscrowCheck }
  | { status: "error"; message: string };

/**
 * The only way a payment, reveal or QR code gets an escrow address (security
 * review F14). It fails closed: the address comes back only when every trustee
 * signed exactly the address on the page, and, for a bid already prepared,
 * only if it is the address saved on the receipt when that bid was checked.
 * Checking, an error, "unattested" and "failed" all block.
 */
export type EscrowGate =
  | { ok: true; ua: string; state: EscrowCheckState }
  | { ok: false; reason: string; state: EscrowCheckState };

export function escrowGate(
  state: EscrowCheckState,
  auction: AuctionView | null,
  receipt?: { escrow_ua?: string },
): EscrowGate {
  const shown = auction?.escrow_ua ?? null;
  if (!shown) return { ok: false, reason: "This auction has no escrow address yet.", state };
  if (state.status === "checking")
    return { ok: false, reason: "Checking the trustees\u2019 signatures\u2026", state };
  if (state.status === "error")
    return {
      ok: false,
      reason: "The trustees\u2019 signatures couldn\u2019t be checked, so paying is blocked.",
      state,
    };
  if (state.check.verdict !== "verified" || state.check.escrow_ua !== shown)
    return {
      ok: false,
      reason:
        "The trustees\u2019 signatures don\u2019t confirm this address. Don\u2019t send funds to it.",
      state,
    };
  if (receipt) {
    if (!receipt.escrow_ua)
      return {
        ok: false,
        reason:
          "This receipt predates escrow checks. Open the auction page to verify the escrow address before paying.",
        state,
      };
    if (receipt.escrow_ua !== shown)
      return {
        ok: false,
        reason:
          "The escrow address changed since you prepared this bid. Don\u2019t send funds to it.",
        state,
      };
  }
  return { ok: true, ua: shown, state };
}
