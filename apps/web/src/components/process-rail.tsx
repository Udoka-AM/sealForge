"use client";

import { KeyRound, ScanSearch, Stamp } from "lucide-react";
import { m, useScroll, useSpring } from "motion/react";
import { useRef } from "react";
import { DURATION, EASE_OUT } from "./motion/motion-provider";

const STEPS = [
  {
    icon: Stamp,
    title: "Your bid is sealed",
    text: "A shielded payment from any Zcash wallet, or the one built into this site. The amount is committed in the memo; your receipt stays on your device.",
  },
  {
    icon: KeyRound,
    title: "Two of three trustees hold it",
    text: "Funds sit in a FROST escrow that no single party can move. The auction server holds no key share.",
  },
  {
    icon: ScanSearch,
    title: "Anyone recomputes the result",
    text: "After close the viewing key is published. Rescan the chain from any node and check the price and every outcome. Bidders stay hidden.",
  },
];

/** The three steps on a rail that draws itself as you scroll through it. */
export function ProcessRail() {
  const ref = useRef<HTMLOListElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start 85%", "end 55%"] });
  const progress = useSpring(scrollYProgress, { stiffness: 120, damping: 30, mass: 0.3 });
  return (
    <ol ref={ref} className="relative grid gap-10 md:grid-cols-3 md:gap-8">
      {/* The rail: horizontal on desktop, vertical on phones. */}
      <div
        aria-hidden="true"
        className="absolute top-5 right-[16%] left-[16%] hidden h-px bg-border md:block"
      >
        <m.div className="h-full origin-left bg-primary" style={{ scaleX: progress }} />
      </div>
      <div aria-hidden="true" className="absolute top-5 bottom-5 left-5 w-px bg-border md:hidden">
        <m.div
          className="w-full origin-top bg-primary"
          style={{ scaleY: progress, height: "100%" }}
        />
      </div>
      {STEPS.map((s, i) => (
        <m.li
          key={s.title}
          initial={{ opacity: 0, y: 16 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true, margin: "-60px" }}
          transition={{ duration: DURATION.slow, ease: EASE_OUT, delay: i * 0.08 }}
          className="relative flex gap-4 md:block md:space-y-4 md:text-center"
        >
          <span className="relative z-10 inline-flex size-10 shrink-0 items-center justify-center rounded-full border border-border bg-surface text-primary shadow-[var(--shadow-card)] md:mx-auto">
            <s.icon size={18} aria-hidden="true" />
          </span>
          <div className="space-y-1.5">
            <h3 className="font-semibold">{s.title}</h3>
            <p className="text-sm leading-6 text-muted md:mx-auto md:max-w-[34ch]">{s.text}</p>
          </div>
        </m.li>
      ))}
    </ol>
  );
}
