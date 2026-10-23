"use client";

import { ArrowUpRight, KeyRound, Lock } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ButtonLink } from "@/components/button";

export interface EscrowNote {
  block: string;
  amount: string;
  commitment: string;
}

export interface EscrowSample {
  code: string;
  title: string;
  notes: EscrowNote[];
  total: number;
  price: string;
  winners: number;
  root: string;
}

const GLYPHS = "▚▞▖▗▘▝▙▛▜▟░▒#%&$@0123456789abcdef";
const scrambleOf = (s: string) =>
  s
    .split("")
    .map((ch) =>
      ch === " " || ch === "." ? ch : GLYPHS[Math.floor(Math.random() * GLYPHS.length)],
    )
    .join("");

/**
 * Light, inverted section: real escrow notes from a published auction, as the public chain shows
 * them (nothing) and as the published viewing key shows them (amounts and memo commitments). The
 * sender column stays hidden either way: a shielded note doesn't carry one.
 */
export function ViewingKeyDemo({ sample }: { sample: EscrowSample }) {
  const [applied, setApplied] = useState(false);
  return (
    <section aria-labelledby="vk-h" className="full-bleed invert-section py-20 md:py-28">
      <div className="container-site grid items-center gap-x-8 gap-y-12 md:grid-cols-12">
        <div className="space-y-5 md:col-span-5">
          <p className="eyebrow flex gap-3 border-t border-border pt-5">
            <span className="text-primary">04</span>
            <span>Verify</span>
          </p>
          <h2 id="vk-h" className="display-2">
            Private while you bid. Open to anyone after.
          </h2>
          <p className="max-w-[44ch] text-muted">
            Until the close, the escrow is a shielded address: the chain shows nothing. Then the
            organiser publishes its viewing key. It reveals what arrived, never who sent it.
          </p>
          <div className="flex flex-wrap items-center gap-3">
            <button
              type="button"
              onClick={() => setApplied((a) => !a)}
              aria-pressed={applied}
              className="inline-flex h-11 items-center gap-2 rounded-[var(--radius-control)] bg-text px-4 text-sm font-medium text-bg transition-transform duration-150 active:scale-[0.98]"
            >
              {applied ? (
                <Lock size={16} aria-hidden="true" />
              ) : (
                <KeyRound size={16} aria-hidden="true" />
              )}
              {applied ? "Show what the public sees" : "Apply the viewing key"}
            </button>
            <ButtonLink href={`/audit/${sample.code}`} variant="ghost" className="group">
              Audit {sample.code}
              <ArrowUpRight
                size={15}
                aria-hidden="true"
                className="transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
              />
            </ButtonLink>
          </div>
        </div>

        <div className="overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-lift)] md:col-span-7">
          <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-3 text-sm">
            <span className="font-medium">
              {sample.title} escrow · <span className="num">{sample.total}</span> notes
            </span>
            <span className="font-mono text-[11px] tracking-[0.12em] text-muted uppercase">
              {applied ? "Viewing key applied" : "Public chain"}
            </span>
          </div>
          <table className="w-full text-left text-[12.5px]">
            <thead>
              <tr className="border-b border-border font-mono text-[10.5px] tracking-[0.12em] text-muted uppercase">
                <th className="px-4 py-2 font-normal">Block</th>
                <th className="px-4 py-2 font-normal">Amount</th>
                <th className="hidden px-4 py-2 font-normal sm:table-cell">Memo commitment</th>
                <th className="px-4 py-2 font-normal">Sender</th>
              </tr>
            </thead>
            <tbody>
              {sample.notes.map((n, i) => (
                <tr key={n.commitment} className="border-b border-border last:border-0">
                  <td className="px-4 py-2.5">
                    <Scramble text={n.block} on={applied} delay={i * 70} />
                  </td>
                  <td className="px-4 py-2.5">
                    <Scramble text={n.amount} on={applied} delay={i * 70 + 60} />
                  </td>
                  <td className="hidden px-4 py-2.5 sm:table-cell">
                    <Scramble text={n.commitment} on={applied} delay={i * 70 + 120} />
                  </td>
                  <td className="px-4 py-2.5 text-muted">
                    <span className="inline-flex items-center gap-1.5">
                      <Lock size={12} aria-hidden="true" /> hidden
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border bg-surface-2/60 px-4 py-3 text-[12.5px]">
            <span className="text-muted">
              {applied ? (
                <>
                  Cleared at <span className="num text-text">{sample.price}</span> ·{" "}
                  <span className="num text-text">{sample.winners}</span> winners · root{" "}
                  <span className="num text-text">{sample.root.slice(0, 10)}…</span>
                </>
              ) : (
                "Shielded outputs: no amounts, no memos, no senders."
              )}
            </span>
            {sample.total > sample.notes.length ? (
              <span className="text-muted">+{sample.total - sample.notes.length} more</span>
            ) : null}
          </div>
        </div>
      </div>
    </section>
  );
}

/** Text that decodes left to right when `on`, and dissolves back to noise when not. */
function Scramble({ text, on, delay }: { text: string; on: boolean; delay: number }) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) {
      el.textContent = on ? text : scrambleOf(text);
      return;
    }
    let frame = 0;
    let raf = 0;
    const total = 18 + Math.min(18, text.length / 3);
    const startAt = performance.now() + delay;
    const tick = (now: number) => {
      if (now < startAt) {
        raf = requestAnimationFrame(tick);
        return;
      }
      frame++;
      const done = Math.floor((frame / total) * text.length);
      el.textContent = on ? text.slice(0, done) + scrambleOf(text.slice(done)) : scrambleOf(text);
      if (on && done < text.length) raf = requestAnimationFrame(tick);
      if (!on && frame < 8) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [on, text, delay]);
  return (
    <span className={`num whitespace-nowrap ${on ? "text-text" : "text-muted"}`}>
      <span className="sr-only">{on ? text : "hidden"}</span>
      <span ref={ref} aria-hidden="true" suppressHydrationWarning>
        {/* A fixed placeholder on the server; the effect scrambles it on the client. */}
        {"░".repeat(text.length)}
      </span>
    </span>
  );
}
