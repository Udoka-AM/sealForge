"use client";

import { ArrowRight } from "lucide-react";
import { m } from "motion/react";
import { useEffect, useState } from "react";
import { CURTAIN_EVENT, curtainUp } from "@/lib/curtain";
import { ButtonLink } from "./button";
import { type FeedItem, HeroStatusBar } from "./landing/live-feed";
import { VaultCanvas } from "./landing/vault-canvas";

const LINES = [
  { text: "Sealed bids.", gold: false },
  { text: "Public price.", gold: false },
  { text: "Private bidders.", gold: true },
];

const EASE = [0.22, 1, 0.36, 1] as const;

/**
 * The home hero on the 12-column grid: the promise in seven columns, the seal as a framed 3D
 * figure in five, and a ruled status bar along the base. The entrance waits for the opening
 * curtain when it shows.
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
    initial: { opacity: 0, y: 16 },
    animate: go ? { opacity: 1, y: 0 } : undefined,
    transition: { duration: 0.8, ease: EASE, delay: 0.35 + i * 0.08 },
  });

  return (
    // Sized so the status bar and the protocol strip (64 px, app/page.tsx) end at the fold.
    <section className="-mt-8 flex min-h-[calc(100dvh-var(--header-h)-64px)] flex-col md:-mt-12">
      <div className="grid flex-1 items-center gap-x-8 gap-y-10 py-10 md:grid-cols-12 md:py-10">
        <div className="space-y-7 md:col-span-7">
          <m.p {...rise(0)} className="eyebrow">
            Sealed-bid auctions on Zcash
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
          <m.p
            {...rise(1)}
            className="max-w-[44ch] text-[17px] leading-7 text-muted md:text-[18px]"
          >
            Bid from any Zcash wallet. Nobody sees who bid, or how much, and anyone can recompute
            the result from the chain.
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
          <m.ul
            {...rise(3)}
            className="flex flex-wrap gap-x-5 gap-y-1 font-mono text-[11px] tracking-[0.16em] text-muted uppercase"
          >
            <li>Zcash Ironwood</li>
            <li>FROST 2-of-3 escrow</li>
            <li>ZIP-321</li>
            <li>Open source</li>
          </m.ul>
        </div>

        <m.figure
          initial={{ opacity: 0 }}
          animate={go ? { opacity: 1 } : undefined}
          transition={{ duration: 1.2, ease: EASE, delay: 0.2 }}
          className="md:col-span-5"
        >
          <div className="relative mx-auto aspect-square w-full max-w-[400px] md:max-w-[min(480px,58dvh)]">
            <Corners />
            <VaultCanvas className="absolute inset-[6%]" />
          </div>
          <figcaption className="mx-auto mt-3 flex max-w-[400px] justify-between font-mono text-[10.5px] tracking-[0.14em] text-muted uppercase md:max-w-[min(480px,58dvh)]">
            <span>Fig. 1 · Bids entering the escrow</span>
            <span className="hidden sm:inline">2 of 3 to open</span>
          </figcaption>
        </m.figure>
      </div>

      <m.div {...rise(4)}>
        <HeroStatusBar items={feed} initialTip={tip} />
      </m.div>
    </section>
  );
}

/** Registration marks at the four corners of the figure. */
function Corners() {
  const c = "absolute size-4 border-border";
  return (
    <span aria-hidden="true">
      <span className={`${c} top-0 left-0 border-t border-l`} />
      <span className={`${c} top-0 right-0 border-t border-r`} />
      <span className={`${c} bottom-0 left-0 border-b border-l`} />
      <span className={`${c} right-0 bottom-0 border-r border-b`} />
    </span>
  );
}
