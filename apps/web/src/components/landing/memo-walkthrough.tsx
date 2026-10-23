"use client";

import { ArrowUpRight, Check } from "lucide-react";
import { m, useMotionValueEvent, useReducedMotion, useScroll, useTransform } from "motion/react";
import Link from "next/link";
import { type ReactNode, useEffect, useRef, useState } from "react";

export interface MemoSample {
  code: string;
  /** A real commitment from the published result. */
  commitment: string;
  /** More real commitments, for the background column. */
  more: string[];
  merkleRoot: string;
  /** The auction's network unit, e.g. "TAZ". */
  unit: string;
}

const short = (h: string, n = 6) => `${h.slice(0, n)}…${h.slice(-n)}`;

/**
 * Pinned horizontal scroll: one real bid memo, `SF1:<code>:<commitment>:<refund address>`, read
 * field by field. Vertical scrolling slides the panels sideways and lights the field each panel
 * explains. On phones the panels simply stack.
 */
export function MemoWalkthrough({ sample }: { sample: MemoSample }) {
  const ref = useRef<HTMLElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const [shift, setShift] = useState(0);
  const [active, setActive] = useState(0);
  const { scrollYProgress: p } = useScroll({ target: ref, offset: ["start start", "end end"] });
  // With reduced motion the panels stack and nothing slides.
  const reduce = useReducedMotion();
  const x = useTransform(p, (v) => (reduce ? 0 : -v * shift));
  useMotionValueEvent(p, "change", (v) => setActive(Math.min(3, Math.floor(v * 4))));

  useEffect(() => {
    const el = track.current;
    if (!el) return;
    // Slide until the last panel's right edge meets the content column's right edge.
    const measure = () => {
      const parent = el.parentElement;
      if (!parent || !window.matchMedia("(min-width: 768px)").matches) return setShift(0);
      const cs = getComputedStyle(parent);
      const avail = parent.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
      setShift(Math.max(0, el.scrollWidth - avail));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const lit = (seg: number) => active === 3 || seg === active;
  const panels: { k: string; title: string; body: ReactNode; art: ReactNode }[] = [
    {
      k: "Which auction",
      title: "One QR code, any wallet",
      body: (
        <>
          The memo opens with the format version and the auction&rsquo;s public code. Bidding is a
          single ZIP-321 payment request, so any Zcash wallet with shielded payments can send it, or
          the wallet built into this site.
        </>
      ),
      art: (
        <span className="font-mono text-[clamp(30px,6vw,64px)] leading-none tracking-[0.06em] text-text">
          {sample.code.split("").map((ch, i) => (
            <m.span
              // biome-ignore lint/suspicious/noArrayIndexKey: characters of a fixed code
              key={i}
              initial={{ opacity: 0.15 }}
              whileInView={{ opacity: 1 }}
              viewport={{ once: true }}
              transition={{ delay: 0.15 + i * 0.05, duration: 0.2 }}
            >
              {ch}
            </m.span>
          ))}
        </span>
      ),
    },
    {
      k: "The commitment",
      title: "Your bid, sealed",
      body: (
        <>
          BLAKE2b-256 over the code, your refund address and a salt that never leaves your browser.
          In sealed-reveal mode it also binds your hidden price. Nobody can open it until you do.
        </>
      ),
      art: <HexRain lines={[sample.commitment, ...sample.more]} />,
    },
    {
      k: "Refund address",
      title: "Fresh for every bid",
      body: (
        <>
          A new address used only for this bid. Refunds come back in standard pieces, after random
          delays, batched with other bidders&rsquo; refunds, so the pieces don&rsquo;t point back to
          you.
        </>
      ),
      art: <RefundPieces unit={sample.unit} />,
    },
    {
      k: "Checked by anyone",
      title: "The whole memo, provable",
      body: (
        <>
          After the close, every counted bid becomes a leaf in a Merkle tree. The root is published
          with the escrow&rsquo;s viewing key, so anyone can rescan the chain and recompute the
          result.
        </>
      ),
      art: (
        <span className="flex flex-col items-center gap-4">
          <span className="inline-flex size-24 items-center justify-center rounded-full border border-primary/60 text-primary">
            <Check size={44} strokeWidth={1.5} aria-hidden="true" />
          </span>
          <span className="font-mono text-[12px] text-muted">
            root {short(sample.merkleRoot, 8)}
          </span>
          <Link
            href={`/audit/${sample.code}`}
            className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
          >
            Recompute {sample.code} <ArrowUpRight size={14} aria-hidden="true" />
          </Link>
        </span>
      ),
    },
  ];

  return (
    <section
      ref={ref}
      aria-labelledby="memo-h"
      className={`relative ${reduce ? "" : "md:h-[420vh]"}`}
    >
      <div
        className={
          reduce
            ? ""
            : "md:full-bleed md:sticky md:top-[var(--header-h)] md:flex md:h-[calc(100dvh-var(--header-h))] md:flex-col md:justify-center md:overflow-hidden"
        }
      >
        <div className="md:mx-auto md:w-full md:max-w-[1120px] md:px-6">
          <div className="space-y-5">
            <p className="eyebrow">03 · How a bid is built</p>
            <h2 id="memo-h" className="display-2 max-w-[18ch]">
              One memo carries the whole bid.
            </h2>
            <p className="flex flex-wrap items-center gap-x-0.5 gap-y-1 font-mono text-[12px] md:text-[13px]">
              <span className="sr-only">A sealForge bid memo: </span>
              <Seg on={lit(0)}>SF1</Seg>
              <span className="text-muted">:</span>
              <Seg on={lit(0)}>{sample.code}</Seg>
              <span className="text-muted">:</span>
              <Seg on={lit(1)}>
                <span className="hidden lg:inline">{sample.commitment}</span>
                <span className="lg:hidden">{short(sample.commitment, 10)}</span>
              </Seg>
              <span className="text-muted">:</span>
              <Seg on={lit(2)}>utest1…refund</Seg>
            </p>
            <p className="min-h-5 text-[13px] text-muted">
              <span className="text-text">{panels[active]?.k}.</span> {short(sample.commitment, 4)}{" "}
              is a real commitment from auction {sample.code}.
            </p>
          </div>

          <m.div
            ref={track}
            style={{ x }}
            className={`mt-8 flex flex-col gap-4 md:mt-10 ${reduce ? "" : "md:w-max md:flex-row md:gap-5"}`}
          >
            {panels.map((pn, i) => (
              <article
                key={pn.k}
                className={`grid gap-6 rounded-[var(--radius-card)] border bg-surface p-6 transition-colors duration-300 md:h-[46vh] md:min-h-[320px] md:w-[min(72vw,860px)] md:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] md:p-8 ${
                  i === active ? "border-primary/50" : "border-border"
                }`}
              >
                <div className="flex flex-col justify-between gap-4">
                  <p className="font-mono text-[11px] tracking-[0.16em] text-muted uppercase">
                    0{i + 1} · {pn.k}
                  </p>
                  <div className="space-y-2">
                    <h3 className="text-[26px] leading-8 font-semibold tracking-tight">
                      {pn.title}
                    </h3>
                    <p className="text-[15px] leading-6 text-muted">{pn.body}</p>
                  </div>
                </div>
                <div className="relative flex min-h-[180px] items-center justify-center overflow-hidden rounded-[var(--radius-control)] bg-bg/60">
                  {pn.art}
                </div>
              </article>
            ))}
          </m.div>
        </div>
      </div>
    </section>
  );
}

function Seg({ on, children }: { on: boolean; children: ReactNode }) {
  return (
    <span
      className={`rounded-[4px] px-1 py-0.5 break-all transition-colors duration-300 ${
        on ? "bg-primary/15 text-primary" : "text-muted"
      }`}
    >
      {children}
    </span>
  );
}

/** Real commitments sliding upward, the first one lit. */
function HexRain({ lines }: { lines: string[] }) {
  const rows = [...lines, ...lines].slice(0, 24);
  return (
    <div className="absolute inset-0 overflow-hidden [mask-image:linear-gradient(transparent,black_20%,black_80%,transparent)]">
      <div className="hex-rain space-y-1.5 p-4 font-mono text-[11px] leading-4 break-all">
        {[...rows, ...rows].map((h, i) => (
          <p
            // biome-ignore lint/suspicious/noArrayIndexKey: a repeated decorative list
            key={i}
            className={i % rows.length === 0 ? "text-primary" : "text-muted/60"}
          >
            {h}
          </p>
        ))}
      </div>
    </div>
  );
}

/** Refund pieces leaving at different times. Illustrative, and labelled so. */
function RefundPieces({ unit }: { unit: string }) {
  const pieces = [
    ["0.01", "+0 h 41 m"],
    ["0.005", "+2 h 07 m"],
    ["0.001", "+3 h 52 m"],
  ];
  return (
    <div className="w-full space-y-3 px-6">
      {pieces.map(([amt, at], i) => (
        <m.div
          key={amt}
          initial={{ opacity: 0, x: -24 }}
          whileInView={{ opacity: 1, x: i * 28 }}
          viewport={{ once: true, amount: 0.6 }}
          transition={{ delay: 0.2 + i * 0.25, duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
          className="flex w-[220px] items-center justify-between rounded-full border border-border bg-surface px-4 py-2 font-mono text-[12px]"
        >
          <span className="text-text">
            {amt} {unit}
          </span>
          <span className="text-muted">{at}</span>
        </m.div>
      ))}
      <p className="pt-1 font-mono text-[10.5px] tracking-[0.12em] text-muted uppercase">
        Illustrative split and delays
      </p>
    </div>
  );
}
