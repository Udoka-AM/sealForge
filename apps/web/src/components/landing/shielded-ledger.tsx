"use client";

import { Check, Lock } from "lucide-react";
import { AnimatePresence, m, useMotionValueEvent, useScroll, useTransform } from "motion/react";
import { type ReactNode, useRef, useState } from "react";
import { SectionHead } from "./section-head";

export interface SealedOutcome {
  title: string;
  /** The network unit, e.g. "TAZ", so the illustrative rows match the real result. */
  unit: string;
  price: string;
  bids: number | null;
  winners: number | null;
}

// Illustrative rows (labelled on screen): what an auction's payments look like on a chain explorer.
const ROWS = [
  { block: "4,509,870", from: "t1Rv…8kQ2", amount: "0.0120" },
  { block: "4,509,872", from: "t1Xa…3mPz", amount: "0.0095" },
  { block: "4,509,879", from: "t1Lc…q7Wd", amount: "0.0150" },
  { block: "4,509,894", from: "t1Gh…2nVb", amount: "0.0075" },
  { block: "4,509,903", from: "t1Pe…6tYr", amount: "0.0110" },
];

const STEPS = [
  {
    tab: "Transparent",
    title: "On a public chain, every bid is visible",
    body: "Who sent it, how much and when. Snipers wait for the last block; big bidders get profiled.",
  },
  {
    tab: "Shielded",
    title: "On Zcash, the bid is sealed",
    body: "The payment is encrypted. The chain proves a transaction happened, not who sent it or how much.",
  },
  {
    tab: "Published",
    title: "After the close, one result",
    body: "The organiser publishes the clearing price and the escrow's viewing key. Anyone can recompute it.",
  },
];

const EASE = [0.22, 1, 0.36, 1] as const;

/**
 * Pinned scene: one explorer panel showing the same auction payments three ways as you scroll:
 * transparent (sender and amount readable), shielded (only the block is public) and published
 * (one real result). The same structure on phones, with the steps as tabs above the panel.
 */
export function ShieldedLedger({ outcome }: { outcome: SealedOutcome | null }) {
  const unit = outcome?.unit ?? "ZEC";
  const ref = useRef<HTMLElement>(null);
  const [phase, setPhase] = useState(0);
  const { scrollYProgress: p } = useScroll({ target: ref, offset: ["start start", "end end"] });
  useMotionValueEvent(p, "change", (v) => setPhase(v < 0.34 ? 0 : v < 0.67 ? 1 : 2));
  // Progress within the current step, for the step's rule.
  const within = useTransform(p, (v) => Math.min(1, ((v * 3) % 1) + (v >= 0.999 ? 1 : 0)));

  return (
    <section ref={ref} aria-labelledby="why-h" className="relative h-[300vh]">
      <div className="sticky top-[var(--header-h)] flex h-[calc(100dvh-var(--header-h))] flex-col justify-center gap-6 py-6 md:gap-10">
        <SectionHead
          index="01"
          label="Why sealed"
          titleId="why-h"
          title="The same auction, three ways."
          lead={<span className="hidden md:inline">Scroll to watch the payments seal.</span>}
        />

        <div className="grid items-start gap-5 md:grid-cols-12 md:gap-8">
          {/* Steps: a list on desktop, tabs on phones. */}
          <ol className="grid grid-cols-3 gap-2 md:col-span-4 md:grid-cols-1 md:gap-0">
            {STEPS.map((s, i) => (
              <li
                key={s.tab}
                className={`relative transition-colors duration-300 md:border-l md:py-4 md:pl-5 ${
                  i === phase ? "text-text" : "text-muted"
                } border-border`}
              >
                <span className="absolute top-0 -left-px hidden h-full w-px md:block">
                  {i === phase ? (
                    <m.span
                      style={{ scaleY: within }}
                      className="block h-full origin-top bg-primary"
                    />
                  ) : i < phase ? (
                    <span className="block h-full bg-primary/40" />
                  ) : null}
                </span>
                <span
                  className={`flex items-baseline gap-1.5 border-t pt-2 font-mono text-[10px] tracking-[0.08em] uppercase md:gap-2 md:border-t-0 md:pt-0 md:text-[11px] md:tracking-[0.14em] ${
                    i === phase ? "border-primary" : "border-border"
                  }`}
                >
                  <span className={i === phase ? "text-primary" : ""}>0{i + 1}</span>
                  {s.tab}
                </span>
                <span className="mt-2 hidden md:block">
                  <span className="block text-[17px] leading-6 font-semibold">{s.title}</span>
                  <span className="mt-1 block text-[14px] leading-6 text-muted">{s.body}</span>
                </span>
              </li>
            ))}
          </ol>

          <div className="space-y-3 md:col-span-8">
            {/* Phones: the active step's words, above the panel. */}
            <AnimatePresence mode="wait" initial={false}>
              <m.p
                key={phase}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.25 }}
                className="text-[14px] leading-6 md:hidden"
              >
                <span className="font-semibold">{STEPS[phase]?.title}.</span>{" "}
                <span className="text-muted">{STEPS[phase]?.body}</span>
              </m.p>
            </AnimatePresence>

            <Explorer phase={phase} outcome={outcome} unit={unit} />
          </div>
        </div>
      </div>
    </section>
  );
}

function Explorer({
  phase,
  outcome,
  unit,
}: {
  phase: number;
  outcome: SealedOutcome | null;
  unit: string;
}) {
  return (
    <div className="overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-lift)]">
      <div className="flex items-center justify-between gap-3 border-b border-border px-4">
        <div className="flex">
          {STEPS.map((s, i) => (
            <span
              key={s.tab}
              className={`relative px-1 py-3 text-[12.5px] transition-colors duration-300 sm:px-2 ${
                i === phase ? "text-text" : "text-muted"
              } ${i > 0 ? "ml-3" : ""}`}
            >
              {s.tab}
              {i === phase ? (
                <m.span
                  layoutId="ledger-tab"
                  className="absolute inset-x-0 -bottom-px h-px bg-primary"
                  transition={{ duration: 0.35, ease: EASE }}
                />
              ) : null}
            </span>
          ))}
        </div>
        <span className="hidden font-mono text-[10.5px] tracking-[0.14em] text-muted uppercase sm:inline">
          Escrow payments
        </span>
      </div>

      <table className="w-full text-left text-[12.5px]">
        <thead>
          <tr className="border-b border-border font-mono text-[10.5px] tracking-[0.12em] text-muted uppercase">
            <th className="px-4 py-2 font-normal">Block</th>
            <th className="px-4 py-2 font-normal">From</th>
            <th className="px-4 py-2 text-right font-normal">Amount</th>
          </tr>
        </thead>
        <tbody>
          {ROWS.map((r, i) => (
            <tr
              key={r.block}
              className={`border-b border-border transition-opacity duration-500 last:border-0 ${
                phase === 2 ? "opacity-60" : ""
              }`}
            >
              <td className="num px-4 py-2.5 text-muted">{r.block}</td>
              <td className="px-4 py-2.5">
                <Cell k={phase === 0 ? "open" : "sealed"} delay={i * 0.06}>
                  {phase === 0 ? <span className="num">{r.from}</span> : <Sealed />}
                </Cell>
              </td>
              <td className="px-4 py-2.5 text-right">
                <Cell
                  k={phase === 0 ? "open" : phase === 1 ? "sealed" : "counted"}
                  delay={i * 0.06 + 0.04}
                  right
                >
                  {phase === 0 ? (
                    <span className="num">
                      {r.amount} {unit}
                    </span>
                  ) : phase === 1 ? (
                    <Sealed />
                  ) : (
                    <span className="inline-flex items-center gap-1 text-muted">
                      <Check size={13} className="text-success" aria-hidden="true" /> counted
                    </span>
                  )}
                </Cell>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <div className="relative min-h-[58px] border-t border-border bg-surface-2/60">
        <AnimatePresence initial={false} mode="wait">
          {phase === 2 && outcome ? (
            <m.div
              key="result"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.45, ease: EASE }}
              className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 px-4 py-3"
            >
              <span className="font-mono text-[10.5px] tracking-[0.14em] text-primary uppercase">
                {outcome.title} · published result
              </span>
              <span className="flex flex-wrap items-baseline gap-x-4 text-[13px]">
                <span>
                  <span className="text-muted">Clearing price </span>
                  <span className="num text-[17px] text-text">{outcome.price}</span>
                </span>
                {outcome.bids !== null ? (
                  <span className="text-muted">
                    <span className="num text-text">{outcome.bids}</span> sealed bids
                  </span>
                ) : null}
                {outcome.winners !== null ? (
                  <span className="text-muted">
                    <span className="num text-text">{outcome.winners}</span> winners
                  </span>
                ) : null}
              </span>
            </m.div>
          ) : (
            <m.p
              key="note"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="px-4 py-3 font-mono text-[10.5px] leading-[34px] tracking-[0.14em] text-muted uppercase"
            >
              {phase === 0
                ? "Illustrative transactions · transparent chain"
                : "Same transactions · shielded pool"}
            </m.p>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

/** A table cell whose content cross-fades with a little blur when its key changes. */
function Cell({
  k,
  delay,
  right,
  children,
}: {
  k: string;
  delay: number;
  right?: boolean;
  children: ReactNode;
}) {
  return (
    <span className={`relative inline-grid ${right ? "justify-items-end" : ""}`}>
      <AnimatePresence initial={false} mode="popLayout">
        <m.span
          key={k}
          initial={{ opacity: 0, y: 6, filter: "blur(4px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: -6, filter: "blur(4px)" }}
          transition={{ duration: 0.4, ease: EASE, delay }}
          className="col-start-1 row-start-1 whitespace-nowrap"
        >
          {children}
        </m.span>
      </AnimatePresence>
    </span>
  );
}

function Sealed() {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-[4px] bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-muted">
      <Lock size={11} aria-hidden="true" />
      shielded
    </span>
  );
}
