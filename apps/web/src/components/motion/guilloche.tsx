"use client";

import { m } from "motion/react";
import { useMemo } from "react";

/**
 * Security-print linework (the anti-counterfeit rosettes on banknotes), drawn
 * once: sealForge's signature motif. Hypotrochoid curves, generated, not traced.
 */
function rosette(R: number, r: number, d: number, scale: number, steps = 1400): string {
  // Close the curve: run until the inner circle returns to its start.
  const gcd = (a: number, b: number): number => (b ? gcd(b, a % b) : a);
  const turns = r / gcd(R, r);
  const pts: string[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = (i / steps) * Math.PI * 2 * turns;
    const x = (R - r) * Math.cos(t) + d * Math.cos(((R - r) / r) * t);
    const y = (R - r) * Math.sin(t) - d * Math.sin(((R - r) / r) * t);
    pts.push(`${(x * scale).toFixed(2)},${(y * scale).toFixed(2)}`);
  }
  return `M${pts.join("L")}Z`;
}

const LAYERS = [
  { R: 96, r: 36, d: 58, opacity: 0.32, width: 0.7 },
  { R: 90, r: 22, d: 46, opacity: 0.22, width: 0.6 },
  { R: 100, r: 44, d: 30, opacity: 0.16, width: 0.5 },
  { R: 84, r: 30, d: 70, opacity: 0.12, width: 0.5 },
];

export function Guilloche({
  size = 520,
  className = "",
  draw = true,
}: {
  size?: number;
  className?: string;
  /** Draw the lines in on mount (once). */
  draw?: boolean;
}) {
  const paths = useMemo(() => LAYERS.map((l) => ({ ...l, d: rosette(l.R, l.r, l.d, 1.6) })), []);
  return (
    <svg
      viewBox="-260 -260 520 520"
      width={size}
      height={size}
      aria-hidden="true"
      className={`pointer-events-none select-none ${className}`}
      fill="none"
    >
      {paths.map((p, i) => (
        <m.path
          key={p.R * 1000 + p.r}
          d={p.d}
          stroke="var(--primary)"
          strokeOpacity={p.opacity}
          strokeWidth={p.width}
          vectorEffect="non-scaling-stroke"
          initial={draw ? { pathLength: 0, opacity: 0 } : false}
          animate={{ pathLength: 1, opacity: 1 }}
          transition={{ duration: 2.2, delay: 0.15 * i, ease: [0.65, 0, 0.35, 1] }}
        />
      ))}
    </svg>
  );
}
