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
