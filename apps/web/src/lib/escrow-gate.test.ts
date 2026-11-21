import { describe, expect, it } from "vitest";
import { type EscrowCheckState, escrowGate } from "./escrow-gate";
import type { AuctionView, EscrowCheck } from "./types";

const UA = "utest1escrow";

const auction = (escrow_ua: string | null = UA) =>
  ({ code: "7K3QX9MZ2P", status: "open", escrow_ua }) as unknown as AuctionView;

const done = (
  verdict: EscrowCheck["verdict"],
  escrow_ua: string | null = UA,
): EscrowCheckState => ({
  status: "done",
  check: { verdict, escrow_ua, signed: 3, total: 3, trustees: [] },
});

describe("escrowGate (security review F14)", () => {
  it("gives the address only when every trustee signed exactly that address", () => {
    const gate = escrowGate(done("verified"), auction());
    expect(gate).toMatchObject({ ok: true, ua: UA });
  });

  it("blocks while checking, on an error, and on anything but verified", () => {
    expect(escrowGate({ status: "checking" }, auction()).ok).toBe(false);
    expect(escrowGate({ status: "error", message: "bad JSON" }, auction()).ok).toBe(false);
    expect(escrowGate(done("unattested"), auction()).ok).toBe(false);
    expect(escrowGate(done("failed"), auction()).ok).toBe(false);
  });

  it("blocks when the check covered a different address than the one shown", () => {
    expect(escrowGate(done("verified", "utest1other"), auction()).ok).toBe(false);
  });

  it("blocks when there is no address", () => {
    expect(escrowGate(done("verified"), auction(null)).ok).toBe(false);
    expect(escrowGate(done("verified"), null).ok).toBe(false);
  });

  it("a prepared bid pays only the address saved on its receipt", () => {
    expect(escrowGate(done("verified"), auction(), { escrow_ua: UA }).ok).toBe(true);
    // The address on the page changed after the bid was prepared, and the new
    // one even carries valid signatures: still blocked.
    const swapped = escrowGate(done("verified", "utest1swapped"), auction("utest1swapped"), {
      escrow_ua: UA,
    });
    expect(swapped.ok).toBe(false);
    // A receipt from before addresses were saved can't pay from here.
    expect(escrowGate(done("verified"), auction(), {}).ok).toBe(false);
  });
});
