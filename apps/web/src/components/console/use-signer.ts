"use client";

import { useCallback, useEffect, useState } from "react";
import { forgetSigner, getSigner, type Role, type StoredSigner } from "@/lib/sfsig";

export type SignerState =
  | { kind: "loading" }
  | { kind: "none" }
  | { kind: "ready"; signer: StoredSigner };

/** The signing key stored in this browser for `role`, if any. */
export function useSigner(role: Role) {
  const [state, setState] = useState<SignerState>({ kind: "loading" });

  const reload = useCallback(async () => {
    const signer = await getSigner(role);
    setState(signer ? { kind: "ready", signer } : { kind: "none" });
  }, [role]);

  useEffect(() => {
    reload();
  }, [reload]);

  const forget = useCallback(async () => {
    await forgetSigner(role);
    setState({ kind: "none" });
    window.dispatchEvent(new Event("sf-signer-changed"));
  }, [role]);

  return { state, reload, forget };
}

/**
 * Poll `load` every `ms`. By default only while the tab is visible; pass
 * `background` to keep going in a hidden tab (trustee signing must finish
 * even if the trustee switches tabs while waiting for a second signer).
 */
export function usePolling(
  load: () => Promise<void>,
  ms: number,
  enabled = true,
  background = false,
) {
  useEffect(() => {
    if (!enabled) return;
    load();
    const id = setInterval(() => {
      if (background || document.visibilityState === "visible") load();
    }, ms);
    return () => clearInterval(id);
  }, [load, ms, enabled, background]);
}
