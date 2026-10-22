"use client";

import { m } from "motion/react";
import { useChainTip } from "@/lib/chain-tip";
import type { AuctionStatus } from "@/lib/types";
import { auctionWindow } from "./block-clock";
import { DURATION, EASE_OUT } from "./motion-provider";

/** Bidding → (Reveal) → Result → Refunds → Settled, with the live step marked. */
export function PhaseTrack({
  auction,
  initialTip,
}: {
  auction: {
    status: AuctionStatus;
    open_height: number | null;
    close_height: number;
    reveal_close_height: number | null;
    bid_mode: "deposit" | "commit_reveal";
  };
  initialTip: number | null;
}) {
  const { tip } = useChainTip(initialTip);
  const sealed = auction.bid_mode === "commit_reveal";
  const steps = ["Bidding", ...(sealed ? ["Reveal"] : []), "Result", "Refunds", "Settled"];
  const w = auctionWindow(auction, tip);
  let current = 0;
  switch (auction.status) {
    case "open":
      current = w.phase === "reveal" ? 1 : w.phase === "closed" ? (sealed ? 2 : 1) : 0;
      break;
    case "closed":
      current = sealed ? 2 : 1;
      break;
    case "published":
    case "refunding":
      current = sealed ? 3 : 2;
      break;
    case "settled":
      current = steps.length;
      break;
    default:
      current = 0;
  }
  return (
    <ol
      className="grid gap-2"
      style={{ gridTemplateColumns: `repeat(${steps.length}, minmax(0, 1fr))` }}
    >
      {steps.map((label, i) => {
        const done = i < current;
        const now = i === current;
        return (
          <li key={label} className="space-y-2">
            <div className="relative h-1 overflow-hidden rounded-full bg-border">
              <m.div
                className="absolute inset-y-0 left-0 w-full origin-left rounded-full"
                style={{ background: now ? "var(--accent)" : "var(--primary)" }}
                initial={{ scaleX: 0 }}
                animate={{ scaleX: done ? 1 : now ? 0.5 : 0 }}
                transition={{ duration: DURATION.slow, ease: EASE_OUT, delay: 0.08 * i }}
              />
            </div>
            <p
              className={`text-[12px] ${now ? "font-medium text-text" : done ? "text-muted" : "text-muted/60"}`}
            >
              {label}
              {now ? <span className="sr-only"> (now)</span> : null}
            </p>
          </li>
        );
      })}
    </ol>
  );
}
