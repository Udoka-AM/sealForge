"use client";

import { ArrowRight, ScanSearch } from "lucide-react";
import { m } from "motion/react";
import Link from "next/link";
import { ButtonLink } from "./button";
import { Guilloche } from "./motion/guilloche";
import { DURATION, EASE_OUT } from "./motion/motion-provider";
import { SealMark } from "./seal-mark";

export interface Proof {
  code: string;
  title: string;
  bids: number | null;
  status: string;
}

const rise = (delay: number) => ({
  initial: { opacity: 0, y: 14 },
  animate: { opacity: 1, y: 0 },
  transition: { duration: DURATION.slow + 0.1, ease: EASE_OUT, delay },
});

/** The home hero: promise on the left, the seal in its security-print rosette on the right. */
export function HomeHero({ proof, hasLive }: { proof: Proof | null; hasLive: boolean }) {
  return (
    <section className="relative grid items-center gap-10 md:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
      <div className="relative z-10 space-y-5">
        <m.h1
          {...rise(0)}
          className="text-[40px] leading-[44px] font-semibold tracking-[-0.03em] text-balance md:text-[58px] md:leading-[60px]"
        >
          Sealed bids.
          <br />
          Public price.
          <br />
          <span className="text-primary">Private bidders.</span>
        </m.h1>
        <m.p {...rise(0.08)} className="max-w-[46ch] text-[17px] leading-7 text-muted">
          Auctions on Zcash where nobody sees who bid, and anyone can recompute the result from the
          chain.
        </m.p>
        <m.div {...rise(0.16)} className="flex flex-wrap gap-2 pt-1">
          <ButtonLink href={hasLive ? "#live" : "/how-it-works"} size="lg" className="group">
            {hasLive ? "Place a sealed bid" : "See how it works"}
            <ArrowRight
              size={18}
              aria-hidden="true"
              className="transition-transform duration-200 group-hover:translate-x-0.5"
            />
          </ButtonLink>
          <ButtonLink href="/console" size="lg" variant="secondary">
            Seal your own auction
          </ButtonLink>
        </m.div>
      </div>

      <div className="relative mx-auto flex aspect-square w-full max-w-[460px] items-center justify-center">
        <Guilloche size={460} className="absolute inset-0 size-full" />
        <m.div
          initial={{ opacity: 0, scale: 1.18, rotate: -8 }}
          animate={{ opacity: 1, scale: 1, rotate: 0 }}
          transition={{ duration: DURATION.medium, ease: EASE_OUT, delay: 1.1 }}
          className="relative drop-shadow-[0_10px_24px_rgba(217,87,59,0.35)]"
        >
          <SealMark size={112} />
        </m.div>
        {proof ? (
          <m.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: DURATION.slow, ease: EASE_OUT, delay: 1.35 }}
            className="absolute right-0 bottom-6 left-auto w-[230px] rounded-[var(--radius-card)] border border-border bg-surface/90 p-3.5 text-sm shadow-[var(--shadow-lift)] backdrop-blur-sm md:-right-2"
          >
            <p className="text-[12px] text-muted">Latest result</p>
            <p className="truncate font-medium">{proof.title}</p>
            <p className="text-[13px] text-muted">
              {proof.bids !== null ? (
                <>
                  <span className="num text-text">{proof.bids}</span> sealed bids ·{" "}
                </>
              ) : null}
              {proof.status === "settled" ? "settled" : "result published"}
            </p>
            <Link
              href={`/audit/${proof.code}`}
              className="mt-2 inline-flex items-center gap-1.5 text-[13px] text-primary hover:underline"
            >
              <ScanSearch size={14} aria-hidden="true" /> Recompute it yourself
            </Link>
          </m.div>
        ) : null}
      </div>
    </section>
  );
}
