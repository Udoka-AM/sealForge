"use client";

import { m } from "motion/react";
import type { ReactNode } from "react";

const EASE = [0.22, 1, 0.36, 1] as const;

/**
 * The opening of an inner page, on the same 12-column grid as the home sections: a rule that
 * draws in, the label (3 columns), the title rising out of a mask (6) and the lead (3).
 */
export function PageHead({
  label,
  title,
  lead,
}: {
  label: string;
  title: string;
  lead?: ReactNode;
}) {
  return (
    <header className="space-y-5">
      <m.div
        aria-hidden="true"
        className="h-px origin-left bg-border"
        initial={{ scaleX: 0 }}
        animate={{ scaleX: 1 }}
        transition={{ duration: 0.9, ease: EASE }}
      />
      <div className="grid gap-x-8 gap-y-4 md:grid-cols-12">
        <m.p
          className="eyebrow md:col-span-3"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.5, delay: 0.15 }}
        >
          {label}
        </m.p>
        <h1 className="display-2 overflow-hidden pb-[0.08em] md:col-span-6">
          <m.span
            className="block"
            initial={{ y: "105%" }}
            animate={{ y: "0%" }}
            transition={{ duration: 0.9, ease: EASE, delay: 0.1 }}
          >
            {title}
          </m.span>
        </h1>
        {lead ? (
          <m.div
            className="text-[15px] leading-6 text-muted md:col-span-3 md:self-end"
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6, ease: EASE, delay: 0.3 }}
          >
            {lead}
          </m.div>
        ) : null}
      </div>
    </header>
  );
}
