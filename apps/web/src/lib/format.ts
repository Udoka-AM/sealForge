import { formatZec } from "@sealforge/bid";

export function zec(zat: number | bigint): string {
  return `${formatZec(BigInt(zat))} ZEC`;
}

export function modeLabel(mode: "uniform_price" | "first_price"): string {
  return mode === "uniform_price" ? "Uniform price" : "First price";
}

export function modeExplainer(mode: "uniform_price" | "first_price", quantity: number): string {
  return mode === "uniform_price"
    ? `The top ${quantity} bids win, and every winner pays the lowest winning bid.`
    : `The top ${quantity} bids win, and each winner pays what they bid.`;
}

/** Zcash targets 75-second blocks. */
export const BLOCK_SECS = 75;

/** Rough time for `blocks` blocks, e.g. "~3 h 10 min". */
export function blocksEta(blocks: number): string {
  if (blocks <= 0) return "now";
  const mins = Math.round((blocks * BLOCK_SECS) / 60);
  if (mins < 60) return `~${mins} min`;
  const h = Math.floor(mins / 60);
  return mins % 60 === 0 ? `~${h} h` : `~${h} h ${mins % 60} min`;
}
