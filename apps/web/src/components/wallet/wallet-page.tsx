"use client";

import { CircleAlert, LoaderCircle } from "lucide-react";
import { useState } from "react";
import { Dashboard } from "./dashboard";
import { Onboarding, PrivacyNotes, RestoreWallet, UnlockForm } from "./onboarding";
import { useWallet } from "./wallet-provider";

export function WalletPage() {
  const wallet = useWallet();
  const [restoring, setRestoring] = useState(false);

  if (wallet.status === "loading")
    return (
      <p className="flex items-center gap-2 text-muted">
        <LoaderCircle size={16} className="animate-spin" aria-hidden="true" /> Opening the wallet…
      </p>
    );
  if (wallet.status === "unavailable")
    return (
      <p
        role="alert"
        className="flex items-start gap-2 rounded-[var(--radius-card)] border border-border p-5"
      >
        <CircleAlert size={18} aria-hidden="true" className="mt-0.5 shrink-0 text-warning" />
        {wallet.unavailableReason}
      </p>
    );
  if (wallet.status === "none") return <Onboarding />;
  if (wallet.status === "locked")
    return restoring ? (
      <div className="space-y-4">
        <p className="rounded-[var(--radius-control)] border border-warning/60 p-3 text-sm">
          Restoring replaces the wallet stored in this browser. Only continue with the recovery
          phrase of the wallet you want to keep.
        </p>
        <RestoreWallet onBack={() => setRestoring(false)} />
      </div>
    ) : (
      <div className="space-y-6">
        <UnlockForm onForgot={() => setRestoring(true)} />
        <PrivacyNotes />
      </div>
    );
  return <Dashboard />;
}
