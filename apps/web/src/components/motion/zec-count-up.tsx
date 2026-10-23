"use client";

import { formatZec } from "@sealforge/bid";
import { unit } from "@/lib/format";
import type { Network } from "@/lib/types";
import { CountUp } from "./count-up";

/** A ZEC/TAZ amount (zatoshi in) that counts up into place. */
export function ZecCountUp({
  zat,
  network,
  className = "",
}: {
  zat: number;
  network: Network;
  className?: string;
}) {
  return (
    <span className={className}>
      <CountUp value={zat} format={(n) => formatZec(BigInt(Math.round(n)))} duration={1.1} />{" "}
      <span className="text-muted">{unit(network)}</span>
    </span>
  );
}
