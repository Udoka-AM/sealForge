"use client";

import { m } from "motion/react";
import type { ReactNode } from "react";

/** Each page settles in (a short rise and fade) on navigation. */
export default function Template({ children }: { children: ReactNode }) {
  return (
    <m.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
    >
      {children}
    </m.div>
  );
}
