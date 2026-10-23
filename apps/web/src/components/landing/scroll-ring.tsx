"use client";

import { ArrowUp } from "lucide-react";
import { m, useScroll, useSpring } from "motion/react";

/** Reading progress for the long home page, as a ring that doubles as "back to top". */
export function ScrollRing() {
  const { scrollYProgress } = useScroll();
  const p = useSpring(scrollYProgress, { stiffness: 160, damping: 30, mass: 0.4 });
  return (
    <button
      type="button"
      onClick={() => window.scrollTo({ top: 0, behavior: "smooth" })}
      className="fixed right-4 bottom-4 z-30 inline-flex size-11 items-center justify-center rounded-full border border-border bg-bg/80 text-muted backdrop-blur-md transition-colors duration-150 hover:text-text md:right-6 md:bottom-6"
      aria-label="Back to top"
    >
      <svg viewBox="0 0 44 44" className="absolute inset-0 size-full -rotate-90" aria-hidden="true">
        <m.circle
          cx="22"
          cy="22"
          r="20.5"
          fill="none"
          stroke="var(--primary)"
          strokeWidth="1.5"
          style={{ pathLength: p }}
        />
      </svg>
      <ArrowUp size={16} aria-hidden="true" />
    </button>
  );
}
