"use client";

import { m } from "motion/react";
import type { ReactNode } from "react";

const EASE = [0.22, 1, 0.36, 1] as const;

export interface TimelineStep {
  icon: ReactNode;
  title: string;
  body: ReactNode;
}

/**
 * Numbered steps joined by a rule that draws in when the timeline enters view: across four
 * columns on desktop, down the left edge on phones. Each step settles in after the rule reaches it.
 */
export function StepTimeline({ steps }: { steps: TimelineStep[] }) {
  return (
    <m.ol
      initial="hidden"
      whileInView="shown"
      viewport={{ once: true, amount: 0.3 }}
      className="relative grid gap-8 md:grid-cols-4 md:gap-6"
    >
      {/* The rule: horizontal through the icon centres from md up, vertical below. */}
      <m.span
        aria-hidden="true"
        variants={{ hidden: { scaleX: 0 }, shown: { scaleX: 1 } }}
        transition={{ duration: 1.2, ease: EASE }}
        className="absolute top-5 right-[12.5%] left-[12.5%] hidden h-px origin-left bg-border md:block"
      />
      <m.span
        aria-hidden="true"
        variants={{ hidden: { scaleY: 0 }, shown: { scaleY: 1 } }}
        transition={{ duration: 1.2, ease: EASE }}
        className="absolute top-5 bottom-5 left-5 w-px origin-top bg-border md:hidden"
      />
      {steps.map((s, i) => (
        <m.li
          key={s.title}
          variants={{ hidden: { opacity: 0, y: 14 }, shown: { opacity: 1, y: 0 } }}
          transition={{ duration: 0.6, ease: EASE, delay: 0.25 + i * 0.22 }}
          className="relative grid grid-cols-[40px_minmax(0,1fr)] gap-x-4 md:grid-cols-1 md:justify-items-center md:text-center"
        >
          <span
            aria-hidden="true"
            className="relative z-10 inline-flex size-10 items-center justify-center rounded-full border border-border bg-bg text-primary shadow-[var(--shadow-card)]"
          >
            {s.icon}
          </span>
          <div className="space-y-2 md:mt-5">
            <p className="font-mono text-[11px] tracking-[0.16em] text-muted uppercase">
              Step 0{i + 1}
            </p>
            <h3 className="text-[18px] leading-6 font-semibold tracking-tight">{s.title}</h3>
            <p className="text-[14.5px] leading-6 text-muted">{s.body}</p>
          </div>
        </m.li>
      ))}
    </m.ol>
  );
}
