"use client";

import { animate, useInView, useReducedMotion } from "motion/react";
import { useEffect, useRef } from "react";
import { EASE_OUT } from "./motion-provider";

/**
 * A number that counts to its value when it first comes into view, and to each
 * new value after that. The final value is in the markup (server render, screen
 * readers); only the visible digits animate.
 */
export function CountUp({
  value,
  format = (n) => Math.round(n).toLocaleString("en-US"),
  duration = 0.9,
  className = "",
  fromZero = true,
}: {
  value: number;
  format?: (n: number) => string;
  duration?: number;
  className?: string;
  /** Count up from zero the first time (totals), or start at the value and only animate changes (block heights). */
  fromZero?: boolean;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true, margin: "-20px" });
  const reduce = useReducedMotion();
  const shown = useRef<number | null>(null);
  // Callers pass inline formatters; keep the latest without re-running the animation.
  const formatRef = useRef(format);
  formatRef.current = format;

  useEffect(() => {
    const el = ref.current;
    if (!el || !inView) return;
    if (reduce) {
      el.textContent = formatRef.current(value);
      shown.current = value;
      return;
    }
    const from = shown.current ?? (fromZero ? 0 : value);
    const controls = animate(from, value, {
      duration,
      ease: EASE_OUT,
      onUpdate: (v) => {
        el.textContent = formatRef.current(v);
      },
    });
    shown.current = value;
    return () => controls.stop();
  }, [value, inView, reduce, duration, fromZero]);

  return (
    <span className={`num ${className}`}>
      <span ref={ref} aria-hidden="true">
        {format(value)}
      </span>
      <span className="sr-only">{format(value)}</span>
    </span>
  );
}
