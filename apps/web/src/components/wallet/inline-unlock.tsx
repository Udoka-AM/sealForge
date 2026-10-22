"use client";

import { LoaderCircle, Lock } from "lucide-react";
import { useId, useState } from "react";
import { Button } from "@/components/button";
import { useWallet } from "./wallet-provider";

/** Unlock the stored wallet without leaving the page (bid form, My bids). */
export function InlineUnlock({ prompt }: { prompt: string }) {
  const wallet = useWallet();
  const id = useId();
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function unlock() {
    if (!passphrase || busy) return;
    setBusy(true);
    setError(null);
    try {
      await wallet.unlock(passphrase);
      setPassphrase("");
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  // Not a <form>: it sits inside the bid form, and nested forms are invalid.
  return (
    <fieldset className="space-y-2 rounded-[var(--radius-control)] border border-border p-3">
      <legend className="sr-only">Unlock wallet</legend>
      <label htmlFor={id} className="flex items-center gap-2 text-sm">
        <Lock size={14} aria-hidden="true" className="text-muted" /> {prompt}
      </label>
      <div className="flex gap-2">
        <input
          id={id}
          type="password"
          autoComplete="current-password"
          placeholder="Wallet passphrase"
          value={passphrase}
          onChange={(e) => {
            setPassphrase(e.target.value);
            setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void unlock();
            }
          }}
          aria-invalid={Boolean(error)}
          className="h-11 min-w-0 flex-1 rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
        />
        <Button
          type="button"
          variant="secondary"
          onClick={() => void unlock()}
          disabled={busy || passphrase.length === 0}
        >
          {busy ? <LoaderCircle size={16} className="animate-spin" aria-hidden="true" /> : null}
          Unlock
        </Button>
      </div>
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
    </fieldset>
  );
}
