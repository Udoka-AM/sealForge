"use client";

import { type MotionValue, m, useReducedMotion, useScroll, useTransform } from "motion/react";
import { useRef } from "react";
import { SealMark } from "@/components/seal-mark";

// An illustration of an auction on a transparent chain: every bidder and amount in the open.
// Made-up addresses and amounts, labelled as such on screen.
const LEAKS = [
  ["t1Rv…8kQ2", "4.20"],
  ["t1Xa…3mPz", "3.95"],
  ["t1Lc…q7Wd", "5.10"],
  ["t1Gh…2nVb", "2.75"],
  ["t1Ko…9sJe", "4.05"],
  ["t1Pe…6tYr", "6.40"],
  ["t1Wm…1cHa", "3.30"],
  ["t1Bz…7dNf", "4.80"],
  ["t1Yu…5gKs", "2.10"],
  ["t1Nd…4hQx", "5.55"],
  ["t1Fj…0aZc", "3.60"],
  ["t1Sq…8eLm", "4.45"],
  ["t1Ct…3wBv", "2.95"],
  ["t1Hr…6pTo", "5.85"],
] as const;

// Scattered positions (percent of the stage), fixed so server and client agree.
const SPOTS: readonly (readonly [number, number, number])[] = [
  [8, 10, -6],
  [58, 4, 4],
  [30, 22, 3],
  [74, 26, -3],
  [4, 40, 5],
  [46, 44, -4],
  [80, 52, 6],
  [20, 60, -2],
  [62, 70, 3],
  [36, 76, -5],
  [6, 82, 2],
  [76, 84, -4],
  [50, 14, -2],
  [24, 92, 4],
] as const;

/**
 * Piecewise-linear and clamped. Opacity goes through this function rather than useTransform's
 * offset arrays: Motion hands offset-array opacity to a native scroll timeline, which only keeps
 * the given offsets and drifts back to the base value outside them.
 */
function ramp(stops: number[], values: number[]) {
  return (v: number) => {
    if (v <= (stops[0] ?? 0)) return values[0] ?? 0;
    for (let i = 1; i < stops.length; i++) {
      const a = stops[i - 1] ?? 0;
      const b = stops[i] ?? 1;
      if (v <= b) {
        const t = (v - a) / (b - a || 1);
        return (values[i - 1] ?? 0) + ((values[i] ?? 0) - (values[i - 1] ?? 0)) * t;
      }
    }
    return values[values.length - 1] ?? 0;
  };
}

export interface SealedOutcome {
  title: string;
  price: string;
  bids: number | null;
}

/**
 * Pinned scroll scene. First: a transparent-chain auction, every bid an address and an amount.
 * As you scroll, the bids fold into the seal and one public figure remains: the clearing price
 * of a real sealForge auction.
 */
export function LeakToSeal({ outcome }: { outcome: SealedOutcome | null }) {
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress: p } = useScroll({ target: ref, offset: ["start start", "end end"] });

  const leakOpacity = useTransform(p, ramp([0.3, 0.45], [1, 0]));
  const leakY = useTransform(p, ramp([0.3, 0.45], [0, -24]));
  const sealOpacity = useTransform(p, ramp([0.5, 0.62], [0, 1]));
  const sealY = useTransform(p, ramp([0.5, 0.62], [24, 0]));
  const markScale = useTransform(p, ramp([0.35, 0.65], [0.2, 1]));
  const markOpacity = useTransform(p, ramp([0.35, 0.55], [0, 1]));
  const markRotate = useTransform(p, ramp([0.35, 0.65], [-90, 0]));
  const priceOpacity = useTransform(p, ramp([0.62, 0.74], [0, 1]));

  const reduce = useReducedMotion();
  if (reduce) return <StillLeakToSeal outcome={outcome} />;

  return (
    <section ref={ref} aria-labelledby="leak-h" className="relative h-[280vh]">
      <div className="sticky top-[var(--header-h)] grid h-[calc(100dvh-var(--header-h))] items-center gap-8 overflow-hidden md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
        <div className="relative z-10 min-h-[250px] md:min-h-[220px]">
          <m.div
            style={{ opacity: leakOpacity, y: leakY }}
            className="absolute inset-x-0 top-0 space-y-4"
          >
            <p className="eyebrow">01 · The problem</p>
            <h2 id="leak-h" className="display-2">
              On a public chain, every bid is a <span className="text-primary">confession.</span>
            </h2>
            <p className="max-w-[44ch] text-muted">
              Who bid, how much, and when, all in the open. Snipers wait for the last block. Big
              bidders get profiled. The usual fix is to trust the organiser with everything.
            </p>
          </m.div>
          <m.div
            style={{ opacity: sealOpacity, y: sealY }}
            className="absolute inset-x-0 top-0 space-y-4"
          >
            <p className="eyebrow">02 · sealForge</p>
            <p className="display-2">
              One price in public. <span className="text-primary">No bidders.</span>
            </p>
            <p className="max-w-[44ch] text-muted">
              Bids go into a shielded escrow on Zcash. After the close, only the result is
              published: the clearing price, the winners&rsquo; count and a root anyone can check.
            </p>
          </m.div>
        </div>

        <div className="relative h-[56vh] min-h-[340px] md:h-[70vh]">
          <m.span
            style={{ opacity: leakOpacity }}
            className="absolute top-0 left-0 font-mono text-[10.5px] tracking-[0.16em] text-muted uppercase"
          >
            Illustration · a transparent-chain auction
          </m.span>
          {/* Inset so the scattered cards stay on screen on narrow phones. */}
          <div className="absolute inset-x-[16%] inset-y-8 md:inset-x-[4%]">
            {LEAKS.map(([addr, amt], i) => (
              <LeakCard
                key={addr}
                addr={addr}
                amount={amt}
                spot={SPOTS[i] ?? [50, 50, 0]}
                index={i}
                p={p}
              />
            ))}
            <m.div
              style={{ scale: markScale, opacity: markOpacity, rotate: markRotate }}
              className="absolute top-1/2 left-1/2 -mt-[72px] -ml-[72px] drop-shadow-[0_0_40px_rgba(244,183,40,0.25)]"
            >
              <SealMark size={144} />
            </m.div>
            {outcome ? (
              <m.div
                style={{ opacity: priceOpacity }}
                className="absolute top-1/2 left-1/2 mt-[96px] w-[280px] -translate-x-1/2 text-center"
              >
                <p className="num text-[34px] leading-10 font-medium tracking-tight">
                  {outcome.price}
                </p>
                <p className="text-[13px] text-muted">
                  {outcome.title} cleared here
                  {outcome.bids !== null ? `. ${outcome.bids} sealed bids, none named.` : "."}
                </p>
              </m.div>
            ) : null}
          </div>
        </div>
      </div>
    </section>
  );
}

function LeakCard({
  addr,
  amount,
  spot,
  index,
  p,
}: {
  addr: string;
  amount: string;
  spot: readonly [number, number, number];
  index: number;
  p: MotionValue<number>;
}) {
  const [x, y, r] = spot;
  // Each card drifts a little while it is readable, then gets pulled into the centre.
  const start = 0.3 + (index % 7) * 0.012;
  const end = start + 0.22;
  const left = useTransform(p, [0, start, end], [`${x}%`, `${x + (index % 2 ? 2 : -2)}%`, "50%"]);
  const top = useTransform(p, [0, start, end], [`${y}%`, `${y + 1.5}%`, "50%"]);
  const scale = useTransform(p, ramp([start, end], [1, 0.1]));
  const opacity = useTransform(p, ramp([0, 0.04, start + 0.12, end], [0.4, 1, 0.9, 0]));
  const rotate = useTransform(p, ramp([start, end], [r, r * 6]));
  return (
    <m.div
      style={{ left, top, scale, opacity, rotate }}
      className="absolute -translate-x-1/2 -translate-y-1/2 rounded-[8px] border border-border bg-surface px-3 py-2 font-mono text-[11px] leading-4 whitespace-nowrap shadow-[var(--shadow-card)]"
      aria-hidden="true"
    >
      <span className="block text-muted">{addr}</span>
      <span className="block text-text">
        bid <span className="text-primary">{amount}</span> ZEC
      </span>
    </m.div>
  );
}

/** The same story without scroll-driven motion: both statements and the result, side by side. */
function StillLeakToSeal({ outcome }: { outcome: SealedOutcome | null }) {
  return (
    <section aria-labelledby="leak-h" className="grid gap-12 md:grid-cols-2">
      <div className="space-y-4">
        <p className="eyebrow">01 · The problem</p>
        <h2 id="leak-h" className="display-2">
          On a public chain, every bid is a <span className="text-primary">confession.</span>
        </h2>
        <p className="max-w-[44ch] text-muted">
          Who bid, how much, and when, all in the open. Snipers wait for the last block. Big bidders
          get profiled. The usual fix is to trust the organiser with everything.
        </p>
      </div>
      <div className="space-y-4">
        <p className="eyebrow">02 · sealForge</p>
        <p className="display-2">
          One price in public. <span className="text-primary">No bidders.</span>
        </p>
        <p className="max-w-[44ch] text-muted">
          Bids go into a shielded escrow on Zcash. After the close, only the result is published:
          the clearing price, the winners&rsquo; count and a root anyone can check.
        </p>
        {outcome ? (
          <p className="flex items-center gap-4 pt-2">
            <SealMark size={56} />
            <span>
              <span className="num block text-[26px] leading-8">{outcome.price}</span>
              <span className="text-[13px] text-muted">{outcome.title}&rsquo;s clearing price</span>
            </span>
          </p>
        ) : null}
      </div>
    </section>
  );
}
