import { describe, expect, it } from "vitest";
import { verifyRound2Inputs } from "./sfsig";

// From sealforge_api::auth::sign_round2_inputs with the Ed25519 seed [7; 32],
// digest [0x11; 32], auction ABCDEFGHJK, slot 2: the browser must accept the
// statements the CLI makes, and the other way round.
const PUBKEY = "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";
const SIGNATURE =
  "ce8bf103162857f43deea10114f69c45f722997c51061568944483d1773bedd44464f011eef1efa3e21dbf9a732b8227a0142818153231783a76e91b4b1fe60b";
const BATCH = "5f0c2a4e-0000-4000-8000-000000000001";
const DIGEST = "11".repeat(32);

describe("round 2 inputs statements", () => {
  it("verifies a statement signed by the Rust code", async () => {
    expect(await verifyRound2Inputs(PUBKEY, SIGNATURE, "ABCDEFGHJK", BATCH, 2, DIGEST)).toBe(true);
    // Upper-case hex in the digest is the same digest.
    expect(
      await verifyRound2Inputs(PUBKEY, SIGNATURE, "ABCDEFGHJK", BATCH, 2, DIGEST.toUpperCase()),
    ).toBe(true);
  });

  it("rejects it for another slot, batch, auction or digest", async () => {
    expect(await verifyRound2Inputs(PUBKEY, SIGNATURE, "ABCDEFGHJK", BATCH, 1, DIGEST)).toBe(false);
    expect(
      await verifyRound2Inputs(
        PUBKEY,
        SIGNATURE,
        "ABCDEFGHJK",
        `${BATCH.slice(0, -1)}2`,
        2,
        DIGEST,
      ),
    ).toBe(false);
    expect(await verifyRound2Inputs(PUBKEY, SIGNATURE, "ZZZZZZZZZZ", BATCH, 2, DIGEST)).toBe(false);
    expect(
      await verifyRound2Inputs(PUBKEY, SIGNATURE, "ABCDEFGHJK", BATCH, 2, "12".repeat(32)),
    ).toBe(false);
  });

  it("rejects malformed keys and signatures", async () => {
    expect(await verifyRound2Inputs("00", SIGNATURE, "ABCDEFGHJK", BATCH, 2, DIGEST)).toBe(false);
    expect(await verifyRound2Inputs(PUBKEY, "zz", "ABCDEFGHJK", BATCH, 2, DIGEST)).toBe(false);
  });
});
