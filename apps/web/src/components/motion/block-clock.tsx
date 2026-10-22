"use client";

import { m } from "motion/react";
import { useChainTip } from "@/lib/chain-tip";
import { blocksEta } from "@/lib/format";
import type { AuctionStatus, Network } from "@/lib/types";
import { CountUp } from "./count-up";
import { DURATION, EASE_OUT } from "./motion-provider";

export type Window = {
  phase: "pending" | "bidding" | "reveal" | "closed";
  /** 0..1 through the current window. */
  progress: number;
  blocksLeft: number;
  endsAt: number | null;
};

/** Where an auction is in time, from block heights alone. */
export function auctionWindow(
  a: {
    status: AuctionStatus;
    open_height: number | null;
    close_height: number;
    reveal_close_height: number | null;
  },
  tip: number | null,
): Window {
  if (a.status !== "open" || a.open_height === null || tip === null)
    return {
      phase: a.status === "open" ? "bidding" : a.open_height === null ? "pending" : "closed",
      progress: a.status === "open" ? 0 : 1,
      blocksLeft: 0,
      endsAt: a.status === "open" ? a.close_height : null,
    };
  if (tip < a.close_height) {
    const span = Math.max(1, a.close_height - a.open_height);
    return {
      phase: "bidding",
      progress: Math.min(1, Math.max(0, (tip - a.open_height) / span)),
      blocksLeft: a.close_height - tip,
      endsAt: a.close_height,
    };
  }
  if (a.reveal_close_height !== null && tip <= a.reveal_close_height) {
    const span = Math.max(1, a.reveal_close_height - a.close_height);
    return {
      phase: "reveal",
      progress: Math.min(1, (tip - a.close_height) / span),
      blocksLeft: a.reveal_close_height - tip,
      endsAt: a.reveal_close_height,
    };
  }
  return { phase: "closed", progress: 1, blocksLeft: 0, endsAt: null };
}

const LABEL: Record<Window["phase"], string> = {
  pending: "Not open yet",
  bidding: "Bidding closes in",
  reveal: "Reveal window ends in",
  closed: "Bidding closed",
};

/** A ring that fills as the current window runs, counting down in blocks. */
export function BlockClock({
  auction,
  initialTip,
  network,
  size = 132,
}: {
  auction: Parameters<typeof auctionWindow>[0];
  initialTip: number | null;
  network: Network;
  size?: number;
}) {
  const { tip } = useChainTip(initialTip);
  const w = auctionWindow(auction, tip);
  const stroke = w.phase === "reveal" ? "var(--accent)" : "var(--primary)";
  return (
    <div className="flex items-center gap-5">
      <div className="relative shrink-0" style={{ width: size, height: size }}>
        <svg viewBox="0 0 100 100" className="size-full -rotate-90" aria-hidden="true">
          <circle cx="50" cy="50" r="44" fill="none" stroke="var(--border)" strokeWidth="5" />
          <m.circle
            cx="50"
            cy="50"
            r="44"
            fill="none"
            stroke={stroke}
            strokeWidth="5"
            strokeLinecap="round"
            initial={{ pathLength: 0 }}
            animate={{ pathLength: w.progress }}
            transition={{ duration: DURATION.slow * 3, ease: EASE_OUT }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center text-center">
          {w.phase === "bidding" || w.phase === "reveal" ? (
            <>
              <CountUp
                value={w.blocksLeft}
                fromZero={false}
                className="text-[28px] leading-8 font-medium"
              />
              <span className="text-[11px] text-muted">blocks</span>
            </>
          ) : (
            <span className="text-[13px] text-muted">
              {w.phase === "pending" ? "Soon" : "Closed"}
            </span>
          )}
        </div>
      </div>
      <div className="min-w-0 space-y-1" aria-live="polite">
        <p className="text-sm text-muted">{LABEL[w.phase]}</p>
        {w.phase === "bidding" || w.phase === "reveal" ? (
          <p className="text-[22px] leading-7 font-semibold tracking-tight">
            {blocksEta(w.blocksLeft, network)}
          </p>
        ) : null}
        {w.endsAt ? (
          <p className="text-[13px] text-muted">
            at block <span className="num">{w.endsAt.toLocaleString("en-US")}</span>
            {tip ? (
              <>
                {" "}
                · now <span className="num">{tip.toLocaleString("en-US")}</span>
              </>
            ) : null}
          </p>
        ) : null}
      </div>
    </div>
  );
}
