import { formatZec } from "@sealforge/bid";
import type { Network } from "./types";

/** Test-network coins are TAZ, so a test auction never reads as real money. */
export function unit(network: Network = "mainnet"): string {
  return network === "mainnet" ? "ZEC" : "TAZ";
}

export function zec(zat: number | bigint, network: Network = "mainnet"): string {
  return `${formatZec(BigInt(zat))} ${unit(network)}`;
}

/** The network a Unified Address belongs to, from its prefix (receipts carry no network field). */
export function networkOfUa(ua: string): Network {
  if (ua.startsWith("utest1")) return "testnet";
  if (ua.startsWith("uregtest1")) return "regtest";
  return "mainnet";
}

export function modeLabel(mode: "uniform_price" | "first_price"): string {
  return mode === "uniform_price" ? "Uniform price" : "First price";
}

export function modeExplainer(mode: "uniform_price" | "first_price", quantity: number): string {
  return mode === "uniform_price"
    ? `The top ${quantity} bids win, and every winner pays the lowest winning bid.`
    : `The top ${quantity} bids win, and each winner pays what they bid.`;
}

/** Target block time: 75 s on mainnet; testnet moved to 25 s with NU7. */
export function blockSeconds(network: Network): number {
  return network === "testnet" ? 25 : 75;
}

/** Rough time for `blocks` blocks, e.g. "~3 h 10 min". */
export function blocksEta(blocks: number, network: Network = "mainnet"): string {
  if (blocks <= 0) return "now";
  const mins = Math.round((blocks * blockSeconds(network)) / 60);
  if (mins < 1) return "under a minute";
  if (mins < 60) return `~${mins} min`;
  const h = Math.floor(mins / 60);
  return mins % 60 === 0 ? `~${h} h` : `~${h} h ${mins % 60} min`;
}
