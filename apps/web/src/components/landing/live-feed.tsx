"use client";

import { ArrowDown } from "lucide-react";
import { AnimatePresence, m } from "motion/react";
import { useEffect, useState } from "react";
import { useChainTip } from "@/lib/chain-tip";

export interface FeedItem {
  label: string;
  title: string;
  detail: string;
}

/**
 * The ruled bar along the base of the hero, on the 12-column grid: the chain tip the server sees,
 * real auctions from the coordinator one at a time, and the way down. Nothing here is invented:
 * with no auctions the middle cell stays empty.
 */
export function HeroStatusBar({
  items,
  initialTip,
}: {
  items: FeedItem[];
  initialTip: number | null;
}) {
  const { tip } = useChainTip(initialTip);
  const [i, setI] = useState(0);
  useEffect(() => {
    if (items.length < 2) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const t = setInterval(() => setI((n) => (n + 1) % items.length), 3600);
    return () => clearInterval(t);
  }, [items.length]);
  const item = items[i];
  return (
    <div className="grid items-center gap-x-8 gap-y-3 border-t border-border py-4 text-[13px] md:grid-cols-12">
      <p className="flex items-center gap-2.5 font-mono text-[11px] tracking-[0.16em] text-muted uppercase md:col-span-3">
        <span className="relative flex size-1.5">
          <span className="absolute inset-0 animate-ping rounded-full bg-success opacity-60 motion-reduce:hidden" />
          <span className="relative size-1.5 rounded-full bg-success" />
        </span>
        Zcash testnet
        {tip !== null ? (
          <span className="num tracking-normal text-text">#{tip.toLocaleString("en-US")}</span>
        ) : null}
      </p>
      <div className="relative h-5 overflow-hidden md:col-span-6" aria-live="polite">
        {item ? (
          <AnimatePresence initial={false}>
            <m.p
              key={i}
              initial={{ y: 20, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: -20, opacity: 0 }}
              transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
              className="absolute inset-0 flex items-baseline gap-3 truncate"
            >
              <span className="shrink-0 font-mono text-[11px] tracking-[0.12em] text-primary uppercase">
                {item.label}
              </span>
              <span className="truncate">
                <span className="font-medium">{item.title}</span>
                <span className="text-muted"> · {item.detail}</span>
              </span>
            </m.p>
          </AnimatePresence>
        ) : null}
      </div>
      <a
        href="#why"
        className="hidden items-center justify-end gap-2 font-mono text-[11px] tracking-[0.16em] text-muted uppercase transition-colors duration-150 hover:text-text md:col-span-3 md:inline-flex"
      >
        Scroll <ArrowDown size={14} aria-hidden="true" />
      </a>
    </div>
  );
}
