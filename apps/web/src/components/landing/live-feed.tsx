"use client";

import { AnimatePresence, m } from "motion/react";
import { useEffect, useState } from "react";
import { useChainTip } from "@/lib/chain-tip";

export interface FeedItem {
  label: string;
  title: string;
  detail: string;
}

/**
 * A small live panel in the hero: the chain tip the server sees, and real auctions from the
 * coordinator, one at a time. Nothing here is invented: with no auctions it shows the tip only.
 */
export function LiveFeed({ items, initialTip }: { items: FeedItem[]; initialTip: number | null }) {
  const { tip } = useChainTip(initialTip);
  const [i, setI] = useState(0);
  useEffect(() => {
    if (items.length < 2) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) return;
    const t = setInterval(() => setI((n) => (n + 1) % items.length), 3600);
    return () => clearInterval(t);
  }, [items.length]);
  const item = items[i];
  return (
    <div className="w-[300px] max-w-full rounded-[var(--radius-card)] border border-border bg-surface/85 p-4 shadow-[var(--shadow-lift)] backdrop-blur-md">
      <p className="flex items-center justify-between gap-3 font-mono text-[10.5px] tracking-[0.16em] text-muted uppercase">
        <span className="inline-flex items-center gap-2">
          <span className="relative flex size-1.5">
            <span className="absolute inset-0 animate-ping rounded-full bg-success opacity-60 motion-reduce:hidden" />
            <span className="relative size-1.5 rounded-full bg-success" />
          </span>
          Live · Zcash testnet
        </span>
        {tip !== null ? (
          <span className="num tracking-normal">#{tip.toLocaleString("en-US")}</span>
        ) : null}
      </p>
      {item ? (
        <div className="relative mt-3 h-[44px] overflow-hidden" aria-live="polite">
          <AnimatePresence mode="popLayout" initial={false}>
            <m.div
              key={i}
              initial={{ y: 24, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: -24, opacity: 0 }}
              transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
              className="absolute inset-x-0"
            >
              <p className="flex items-baseline justify-between gap-3 text-sm">
                <span className="truncate font-medium">{item.title}</span>
                <span className="shrink-0 font-mono text-[11px] tracking-[0.08em] text-primary uppercase">
                  {item.label}
                </span>
              </p>
              <p className="truncate text-[13px] text-muted">{item.detail}</p>
            </m.div>
          </AnimatePresence>
        </div>
      ) : null}
    </div>
  );
}
