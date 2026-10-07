import { describe, expect, it } from "vitest";
import { scoreLinkability } from "./score";

describe("scoreLinkability", () => {
  it("rates a long delay, canonical amount and busy flow as low", () => {
    const score = scoreLinkability({
      similarAmountSwaps24h: 50,
      delayMinutes: 360,
      transparentHops: 0,
      canonicalAmount: true,
    });
    expect(score.level).toBe("low");
  });

  it("rates an immediate, unusual, transparent path as high", () => {
    const score = scoreLinkability({
      similarAmountSwaps24h: 1,
      delayMinutes: 0,
      transparentHops: 2,
      canonicalAmount: false,
    });
    expect(score.level).toBe("high");
    expect(score.value).toBe(100);
  });
});
