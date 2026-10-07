/**
 * Linkability Score (PRD F9, implementation plan task 4.2).
 *
 * Shown before the user signs. It estimates how easily a public observer could
 * link the source-chain deposit to the shielded bid. It is an honest estimate of
 * residual risk, never a guarantee.
 */

export type LinkabilityLevel = "low" | "medium" | "high";

export interface ScoreInputs {
  /** How many Intents → ZEC swaps of a similar size happened in the last 24 h. */
  similarAmountSwaps24h: number;
  /** Planned delay between ZEC arriving and the bid being sent, in minutes. */
  delayMinutes: number;
  /** Number of transparent hops on the path (e.g. a transparent Intents ZEC leg). */
  transparentHops: number;
  /** True when the bid amount is a canonical denomination (1/2/5 × 10^k). */
  canonicalAmount: boolean;
}

export interface ScoreFactor {
  id: "amount_uniqueness" | "time_gap" | "transparent_hops" | "denomination_fit";
  /** 0 (no risk) to 1 (fully linkable). */
  risk: number;
  /** Plain-language explanation shown in the score card. */
  explanation: string;
}

export interface LinkabilityScore {
  level: LinkabilityLevel;
  /** 0–100, higher means more linkable. */
  value: number;
  factors: ScoreFactor[];
}

const WEIGHTS: Record<ScoreFactor["id"], number> = {
  amount_uniqueness: 0.35,
  time_gap: 0.3,
  transparent_hops: 0.2,
  denomination_fit: 0.15,
};

/**
 * First-pass scoring model. Task 4.2 calibrates thresholds against real
 * Intents flow data and the linkability harness (task 4.3).
 */
export function scoreLinkability(input: ScoreInputs): LinkabilityScore {
  const factors: ScoreFactor[] = [
    {
      id: "amount_uniqueness",
      risk: 1 / Math.max(1, input.similarAmountSwaps24h),
      explanation: `${input.similarAmountSwaps24h} similar-sized swaps in the last 24 h hide yours among them.`,
    },
    {
      id: "time_gap",
      risk: Math.max(0, 1 - input.delayMinutes / 360),
      explanation: `Your bid waits ${input.delayMinutes} min after your ZEC arrives.`,
    },
    {
      id: "transparent_hops",
      risk: Math.min(1, input.transparentHops * 0.5),
      explanation:
        input.transparentHops === 0
          ? "No transparent hops on the path."
          : `${input.transparentHops} transparent hop(s) are publicly visible.`,
    },
    {
      id: "denomination_fit",
      risk: input.canonicalAmount ? 0 : 1,
      explanation: input.canonicalAmount
        ? "Your amount is a common round denomination."
        : "Your amount is unusual, which makes it easier to spot.",
    },
  ];

  const value = Math.round(100 * factors.reduce((sum, f) => sum + WEIGHTS[f.id] * f.risk, 0));
  const level: LinkabilityLevel = value < 30 ? "low" : value < 60 ? "medium" : "high";
  return { level, value, factors };
}
