"use client";

import { m } from "motion/react";
import { useChainTip } from "@/lib/chain-tip";
import { blocksEta } from "@/lib/format";
import type { AuctionView } from "@/lib/types";
import { auctionWindow } from "./block-clock";
import { DURATION, EASE_OUT } from "./motion-provider";

/** A thin bar for an auction card: how much of the window has run, and what's left. */
export function WindowBar({
  auction,
  initialTip,
}: {
  auction: AuctionView;
  initialTip: number | null;
}) {
  const { tip } = useChainTip(initialTip);
  const w = auctionWindow(auction, tip);
  const label =
    w.phase === "bidding"
      ? `Closes in ${blocksEta(w.blocksLeft, auction.network)}`
      : w.phase === "reveal"
        ? `Reveal window: ${blocksEta(w.blocksLeft, auction.network)} left`
        : w.phase === "pending"
          ? "Opening soon"
          : "Closed";
  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-3 text-[13px]">
        <span className={w.phase === "reveal" ? "text-accent" : "text-text"}>{label}</span>
        {w.blocksLeft > 0 ? (
          <span className="num text-muted">{w.blocksLeft.toLocaleString("en-US")} blocks</span>
        ) : null}
      </div>
      <div className="h-1 overflow-hidden rounded-full bg-border">
        <m.div
          className="h-full origin-left rounded-full"
          style={{ background: w.phase === "reveal" ? "var(--accent)" : "var(--primary)" }}
          initial={{ scaleX: 0 }}
          animate={{ scaleX: w.progress }}
          transition={{ duration: DURATION.slow * 2, ease: EASE_OUT }}
        />
      </div>
    </div>
  );
}
