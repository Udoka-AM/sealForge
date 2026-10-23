"use client";

import { ArrowUp } from "lucide-react";
import { m, useMotionValueEvent, useScroll, useSpring } from "motion/react";
import { useState } from "react";

/** Reading progress for the long home page, as a ring that doubles as "back to top". */
export function ScrollRing() {
  const { scrollY, scrollYProgress } = useScroll();
  const p = useSpring(scrollYProgress, { stiffness: 160, damping: 30, mass: 0.4 });
  // Hidden over the hero, which has its own way down.
  const [shown, setShown] = useState(false);
  useMotionValueEvent(scrollY, "change", (y) => setShown(y > window.innerHeight * 0.7));
  return (
    <button
      type="button"
      onClick={() => window.scrollTo({ top: 0, behavior: "smooth" })}
      tabIndex={shown ? 0 : -1}
      aria-hidden={!shown}
      className={`fixed right-4 bottom-4 z-30 inline-flex size-11 items-center justify-center rounded-full border border-border bg-bg/80 text-muted backdrop-blur-md transition-[opacity,translate,color] duration-300 hover:text-text md:right-6 md:bottom-6 ${
        shown ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-2 opacity-0"
      }`}
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
