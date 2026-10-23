"use client";

import { useSyncExternalStore } from "react";

/**
 * The coordinator's chain tip, polled once for the whole page (however many
 * components show it) through the same-origin public API.
 */
interface Snapshot {
  tip: number | null;
  /** When the tip last moved forward (ms), so a component can pulse once per block. */
  changedAt: number;
}

let snapshot: Snapshot = { tip: null, changedAt: 0 };
const listeners = new Set<() => void>();
let timer: number | null = null;
const POLL_MS = 20_000;

async function poll() {
  try {
    const res = await fetch("/api/coordinator/v1/chain", { cache: "no-store" });
    if (!res.ok) return;
    const tip = ((await res.json()) as { tip: number | null }).tip;
    if (typeof tip === "number" && tip !== snapshot.tip) {
      snapshot = { tip, changedAt: snapshot.tip === null ? 0 : Date.now() };
      for (const l of listeners) l();
    }
  } catch {
    // Offline or coordinator down: keep the last known tip.
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (timer === null) {
    void poll();
    timer = window.setInterval(() => {
      if (document.visibilityState === "visible") void poll();
    }, POLL_MS);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
  };
}

const getSnapshot = () => snapshot;
const serverSnapshot: Snapshot = { tip: null, changedAt: 0 };

/** The live tip; `initial` (from the server render) until the first poll lands. */
export function useChainTip(initial: number | null = null): Snapshot {
  const s = useSyncExternalStore(subscribe, getSnapshot, () => serverSnapshot);
  return s.tip === null ? { tip: initial, changedAt: 0 } : s;
}
