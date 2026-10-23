"use client";

import { AnimatePresence, animate, m } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { SealMark } from "@/components/seal-mark";
import { CURTAIN_EVENT, CURTAIN_KEY } from "@/lib/curtain";

/**
 * The opening curtain, once per tab: the mark, a 000→100 count and a filling rule, then the
 * curtain lifts. About 1.3 s. It covers the page but never delays it: the page renders beneath.
 */
export function Preloader() {
  const [done, setDone] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const count = useRef<HTMLSpanElement>(null);
  const bar = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const el = root.current;
    if (!el || getComputedStyle(el).display === "none") {
      setDone(true);
      return;
    }
    document.documentElement.style.overflow = "hidden";
    const controls = animate(0, 100, {
      duration: 1.05,
      ease: [0.65, 0, 0.35, 1],
      onUpdate: (v) => {
        if (count.current) count.current.textContent = String(Math.round(v)).padStart(3, "0");
        if (bar.current) bar.current.style.transform = `scaleX(${v / 100})`;
      },
      onComplete: () => {
        try {
          sessionStorage.setItem(CURTAIN_KEY, "1");
        } catch {}
        setTimeout(() => {
          window.dispatchEvent(new Event(CURTAIN_EVENT));
          setDone(true);
        }, 160);
      },
    });
    return () => {
      controls.stop();
      document.documentElement.style.overflow = "";
    };
  }, []);

  useEffect(() => {
    if (done) document.documentElement.style.overflow = "";
  }, [done]);

  return (
    // The class comes off once the curtain has lifted, so a later visit to "/" in this tab
    // (client-side navigation) keeps it hidden.
    <AnimatePresence onExitComplete={() => document.documentElement.classList.remove("sf-curtain")}>
      {done ? null : (
        <m.div
          key="curtain"
          ref={root}
          data-curtain=""
          exit={{ y: "-100%" }}
          transition={{ duration: 0.7, ease: [0.76, 0, 0.24, 1] }}
          className="fixed inset-0 z-[100] flex flex-col justify-between bg-bg px-5 py-6 md:px-10 md:py-8"
          aria-hidden="true"
        >
          <span className="flex items-center gap-2 font-mono text-[11px] tracking-[0.2em] text-muted uppercase">
            <SealMark size={18} /> sealForge
          </span>
          <span
            ref={count}
            className="self-center font-mono text-[22vw] leading-none font-medium tracking-[-0.06em] text-text md:text-[13vw]"
          >
            000
          </span>
          <span className="space-y-3">
            <span className="flex justify-between font-mono text-[11px] tracking-[0.2em] text-muted uppercase">
              <span>Sealing the room</span>
              <span>Zcash · testnet</span>
            </span>
            <span className="block h-px w-full bg-border">
              <span
                ref={bar}
                className="block h-px w-full origin-left bg-primary"
                style={{ transform: "scaleX(0)" }}
              />
            </span>
          </span>
        </m.div>
      )}
    </AnimatePresence>
  );
}
