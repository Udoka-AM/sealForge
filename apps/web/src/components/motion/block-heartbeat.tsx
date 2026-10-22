"use client";

import { useChainTip } from "@/lib/chain-tip";
import { CountUp } from "./count-up";

/** The chain, visibly ticking: the current block, with a single pulse when a new one lands. */
export function BlockHeartbeat() {
  const { tip, changedAt } = useChainTip();
  if (tip === null) return null;
  return (
    <span
      className="hidden items-center gap-2 rounded-[var(--radius-chip)] border border-border px-2.5 py-1 text-[12px] text-muted md:inline-flex"
      title="The chain tip the auction server sees"
    >
      <span className="relative flex size-2" aria-hidden="true">
        {changedAt ? (
          <span key={changedAt} className="heartbeat absolute inset-0 rounded-full bg-success" />
        ) : null}
        <span className="relative size-2 rounded-full bg-success" />
      </span>
      <span>
        Block <CountUp value={tip} duration={0.6} fromZero={false} className="text-text" />
      </span>
    </span>
  );
}
