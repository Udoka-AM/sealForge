/** Integer zatoshi amounts. 1 ZEC = 100,000,000 zat. Never use floats for money. */

export const COIN = 100_000_000n;
export const MAX_MONEY = 21_000_000n * COIN;

/** Smallest canonical denomination: 0.0001 ZEC (matches sealforge-core). */
export const MIN_DENOMINATION = 10_000n;

/** Format zatoshi as a decimal ZEC string without trailing zeros ("0.42", "1"). */
export function formatZec(zat: bigint): string {
  const whole = zat / COIN;
  const frac = zat % COIN;
  if (frac === 0n) return whole.toString();
  return `${whole}.${frac.toString().padStart(8, "0").replace(/0+$/, "")}`;
}

/** Parse a decimal ZEC string ("1.5", ".25") into zatoshi. Returns null if invalid. */
export function parseZec(input: string): bigint | null {
  const value = input.trim();
  const match = /^(\d*)(?:\.(\d{0,8}))?$/.exec(value);
  if (!match || value === "" || value === ".") return null;
  const whole = BigInt(match[1] || "0");
  const frac = BigInt((match[2] ?? "").padEnd(8, "0") || "0");
  const zat = whole * COIN + frac;
  return zat <= MAX_MONEY ? zat : null;
}

/** All canonical denominations ({1, 2, 5} × 10^k zat, 0.0001 to 5,000 ZEC), ascending. */
export function canonicalDenominations(): bigint[] {
  const out: bigint[] = [];
  for (let base = MIN_DENOMINATION; base <= 5_000n * COIN; base *= 10n) {
    for (const m of [1n, 2n, 5n]) {
      if (base * m <= 5_000n * COIN) out.push(base * m);
    }
  }
  return out;
}

export function isCanonical(zat: bigint): boolean {
  return canonicalDenominations().includes(zat);
}

/**
 * Canonical amounts just below and above `zat`. Rounding a bid to a common
 * amount makes it blend in with other bids and refunds (PRD Q7).
 */
export function roundingOptions(zat: bigint): { lower: bigint | null; upper: bigint | null } {
  const all = canonicalDenominations();
  const lower = [...all].reverse().find((d) => d <= zat) ?? null;
  const upper = all.find((d) => d >= zat) ?? null;
  return { lower, upper };
}
