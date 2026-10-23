"use client";

import { ArrowUpRight } from "lucide-react";
import { m, useReducedMotion, useScroll, useTransform } from "motion/react";
import Link from "next/link";
import { useRef } from "react";

const ROW_A = "Seal the bid ✦ Seal the bid ✦ Seal the bid ✦ ";
const ROW_B = "Prove the price ✦ Prove the price ✦ Prove the price ✦ ";

/**
 * The close of the home page: two giant lines that slide in opposite directions as you scroll,
 * one solid and one outlined, over the two ways in.
 */
export function ClosingCta({ bidHref }: { bidHref: string }) {
  const ref = useRef<HTMLElement>(null);
  const { scrollYProgress: p } = useScroll({ target: ref, offset: ["start end", "end start"] });
  const reduce = useReducedMotion();
  const a = useTransform(p, (v) => (reduce ? "-6%" : `${-28 * v}%`));
  const b = useTransform(p, (v) => (reduce ? "-14%" : `${-28 * (1 - v)}%`));
  return (
    <section ref={ref} aria-labelledby="close-h" className="space-y-14">
      <h2 id="close-h" className="sr-only">
        Seal the bid. Prove the price.
      </h2>
      <div
        aria-hidden="true"
        className="full-bleed space-y-1 overflow-hidden text-[clamp(64px,13vw,200px)] leading-[0.9] font-bold tracking-[-0.05em] whitespace-nowrap select-none"
      >
        <m.p style={{ x: a }}>{ROW_A}</m.p>
        <m.p style={{ x: b }} className="text-outline">
          {ROW_B}
        </m.p>
      </div>
      <div className="grid overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface md:grid-cols-2">
        <Way
          href={bidHref}
          title="Bid without being seen"
          body="Pay from the wallet built into this site or any Zcash wallet. Your receipt stays in your browser."
          cta="Place a sealed bid"
        />
        <Way
          href="/console"
          title="Run an auction nobody has to trust"
          body="Three trustees hold the escrow, any two sign. You publish the result; anyone can recompute it."
          cta="Open the console"
          border
        />
      </div>
    </section>
  );
}

function Way({
  href,
  title,
  body,
  cta,
  border,
}: {
  href: string;
  title: string;
  body: string;
  cta: string;
  border?: boolean;
}) {
  return (
    <Link
      href={href}
      className={`group flex flex-col justify-between gap-8 p-7 transition-colors duration-200 hover:bg-surface-2 md:p-9 ${
        border ? "border-t border-border md:border-t-0 md:border-l" : ""
      }`}
    >
      <span className="space-y-2">
        <span className="block text-[22px] leading-7 font-semibold tracking-tight">{title}</span>
        <span className="block max-w-[42ch] text-muted">{body}</span>
      </span>
      <span className="inline-flex items-center gap-1.5 text-sm text-primary">
        {cta}
        <ArrowUpRight
          size={16}
          aria-hidden="true"
          className="transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
        />
      </span>
    </Link>
  );
}
