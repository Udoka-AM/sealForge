"use client";

import { ArrowDown, ArrowRight } from "lucide-react";
import { m } from "motion/react";
import { useEffect, useState } from "react";
import { CURTAIN_EVENT, curtainUp } from "@/lib/curtain";
import { ButtonLink } from "./button";
import { type FeedItem, LiveFeed } from "./landing/live-feed";
import { VaultCanvas } from "./landing/vault-canvas";

const LINES = [
  { text: "Sealed bids.", gold: false },
  { text: "Public price.", gold: false },
  { text: "Private bidders.", gold: true },
];

const EASE = [0.22, 1, 0.36, 1] as const;

/**
 * The home hero: display headline rising out of line masks, the 3D seal with bids streaming in,
 * and a live panel of real auctions. The entrance waits for the opening curtain when it shows.
 */
export function HomeHero({
  hasLive,
  feed,
  tip,
}: {
  hasLive: boolean;
  feed: FeedItem[];
  tip: number | null;
}) {
  const [go, setGo] = useState(false);
  useEffect(() => {
    if (curtainUp()) {
      setGo(true);
      return;
    }
    const on = () => setGo(true);
    window.addEventListener(CURTAIN_EVENT, on, { once: true });
    // Never leave the hero hidden if the event is missed.
    const t = setTimeout(on, 2200);
    return () => {
      window.removeEventListener(CURTAIN_EVENT, on);
      clearTimeout(t);
    };
  }, []);

  const rise = (i: number) => ({
    initial: { opacity: 0, y: 18 },
    animate: go ? { opacity: 1, y: 0 } : undefined,
    transition: { duration: 0.8, ease: EASE, delay: 0.35 + i * 0.08 },
  });

  return (
    <section className="relative -mt-8 flex min-h-[calc(100dvh-var(--header-h))] flex-col justify-center pt-10 pb-24 md:-mt-12 md:pt-0">
      <m.div
        initial={{ opacity: 0, scale: 0.92 }}
        animate={go ? { opacity: 1, scale: 1 } : undefined}
        transition={{ duration: 1.4, ease: EASE, delay: 0.1 }}
        className="pointer-events-none absolute top-[24%] right-[-34vw] h-[min(96vw,820px)] w-[min(96vw,820px)] -translate-y-1/2 opacity-35 md:top-1/2 md:right-[calc(50%-50vw-8vw)] md:h-[min(68vw,860px)] md:w-[min(68vw,860px)] md:opacity-100"
      >
        <VaultCanvas className="size-full" />
      </m.div>

      <div className="relative z-10 max-w-[760px] space-y-7">
        <m.p {...rise(0)} className="eyebrow">
          Sealed-bid auctions · Zcash
        </m.p>
        <h1 className="display-1">
          {LINES.map((l, i) => (
            <span key={l.text} className="block overflow-hidden pb-[0.06em]">
              <m.span
                className={`block ${l.gold ? "text-primary" : ""}`}
                initial={{ y: "110%" }}
                animate={go ? { y: "0%" } : undefined}
                transition={{ duration: 1, ease: EASE, delay: 0.1 + i * 0.1 }}
              >
                {l.text}
              </m.span>
            </span>
          ))}
        </h1>
        <m.p {...rise(1)} className="max-w-[46ch] text-[17px] leading-7 text-muted md:text-[18px]">
          Bid from any Zcash wallet. Nobody sees who bid, or how much, and anyone can recompute the
          result from the chain.
        </m.p>
        <m.div {...rise(2)} className="flex flex-wrap gap-2">
          <ButtonLink href={hasLive ? "#live" : "/how-it-works"} size="lg" className="group">
            {hasLive ? "Place a sealed bid" : "See how it works"}
            <ArrowRight
              size={18}
              aria-hidden="true"
              className="transition-transform duration-200 group-hover:translate-x-0.5"
            />
          </ButtonLink>
          <ButtonLink href="/console" size="lg" variant="secondary">
            Run your own auction
          </ButtonLink>
        </m.div>
        <m.p
          {...rise(3)}
          className="flex flex-wrap gap-x-4 gap-y-1 font-mono text-[11px] tracking-[0.16em] text-muted uppercase"
        >
          <span>Zcash Ironwood</span>
          <span aria-hidden="true">·</span>
          <span>FROST 2-of-3 escrow</span>
          <span aria-hidden="true">·</span>
          <span>ZIP-321</span>
          <span aria-hidden="true">·</span>
          <span>Open source</span>
        </m.p>
      </div>

      <m.div
        initial={{ opacity: 0, y: 16 }}
        animate={go ? { opacity: 1, y: 0 } : undefined}
        transition={{ duration: 0.8, ease: EASE, delay: 0.9 }}
        className="relative z-10 mt-10 md:absolute md:right-0 md:bottom-16 md:mt-0"
      >
        <LiveFeed items={feed} initialTip={tip} />
      </m.div>

      <m.a
        href="#problem"
        initial={{ opacity: 0 }}
        animate={go ? { opacity: 1 } : undefined}
        transition={{ duration: 0.6, delay: 1.2 }}
        className="absolute bottom-6 left-0 hidden items-center gap-2 font-mono text-[11px] tracking-[0.18em] text-muted uppercase hover:text-text md:inline-flex"
      >
        <ArrowDown size={14} aria-hidden="true" /> Scroll
      </m.a>
    </section>
  );
}
