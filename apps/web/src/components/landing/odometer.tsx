"use client";

import { m, useInView, useReducedMotion } from "motion/react";
import { useRef } from "react";

const DIGITS = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/**
 * A figure whose digits roll into place like a mechanical counter, once, when it scrolls into
 * view. Non-digit characters (commas, slashes, units) sit still. Screen readers get the plain text.
 */
export function Odometer({ value, className = "" }: { value: string; className?: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true, margin: "-10% 0px" });
  const reduce = useReducedMotion();
  const chars = value.split("");
  return (
    <span ref={ref} className={`inline-flex ${className}`}>
      <span className="sr-only">{value}</span>
      <span aria-hidden="true" className="inline-flex">
        {chars.map((ch, i) => {
          const d = DIGITS.indexOf(ch);
          if (d < 0)
            return (
              // biome-ignore lint/suspicious/noArrayIndexKey: characters of a fixed string
              <span key={i}>{ch}</span>
            );
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: characters of a fixed string
            <span key={i} className="relative inline-block h-[1em] overflow-hidden leading-none">
              <m.span
                className="flex flex-col"
                initial={{ y: reduce ? `-${d * 10}%` : "0%" }}
                animate={inView ? { y: `-${d * 10}%` } : undefined}
                transition={{ duration: 1.1 + i * 0.08, ease: [0.22, 1, 0.36, 1], delay: i * 0.05 }}
              >
                {DIGITS.map((n) => (
                  <span key={n} className="h-[1em] leading-none">
                    {n}
                  </span>
                ))}
              </m.span>
            </span>
          );
        })}
      </span>
    </span>
  );
}
