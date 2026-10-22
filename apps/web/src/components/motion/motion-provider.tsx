"use client";

import { domAnimation, LazyMotion, MotionConfig } from "motion/react";
import type { ReactNode } from "react";

/** The app's one motion lane: tween on the ease-out curve (globals.css tokens). */
export const EASE_OUT = [0.22, 1, 0.36, 1] as const;
export const DURATION = { fast: 0.12, base: 0.2, medium: 0.28, slow: 0.4 } as const;

/**
 * Motion for the whole app, code-split (`LazyMotion` + `m` components only, enforced by
 * `strict`), and switched off when the user asks for reduced motion.
 */
export function MotionProvider({ children }: { children: ReactNode }) {
  return (
    <LazyMotion features={domAnimation} strict>
      <MotionConfig reducedMotion="user" transition={{ duration: DURATION.medium, ease: EASE_OUT }}>
        {children}
      </MotionConfig>
    </LazyMotion>
  );
}
