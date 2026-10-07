/** Assets ShieldRamp accepts in v1 (SOL is a stretch goal). */
export type SourceAsset = "USDC" | "ETH";

/** Chains ShieldRamp accepts in v1. */
export type SourceChain = "ethereum" | "base" | "arbitrum";

/** Steps of a ShieldRamp session, persisted in IndexedDB so it can resume (App Flow S8). */
export type RampStep =
  | "quoted"
  | "deposit_sent"
  | "zec_received"
  | "shielded"
  | "waiting_delay"
  | "bid_sent"
  | "bid_detected"
  | "swap_refunded"
  | "failed";

export interface RampSession {
  /** Local-only random id. Never sent to the sealForge coordinator. */
  id: string;
  auctionCode: string;
  step: RampStep;
  sourceAsset: SourceAsset;
  sourceChain: SourceChain;
  /** Planned bid in zatoshi, after canonical rounding. */
  bidZat: bigint;
  /** Earliest time the bid may be sent (random 20–90 min after shielding). */
  bidNotBefore?: number;
  createdAt: number;
}
