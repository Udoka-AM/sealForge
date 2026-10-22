"use client";

import Link from "next/link";
import { useWallet } from "./wallet-provider";

const DOT: Record<string, string> = {
  unlocked: "var(--success)",
  locked: "var(--text-muted)",
};

/** "Wallet" in the header, with a dot when one is stored (green while unlocked). */
export function WalletNavLink() {
  const { status } = useWallet();
  if (status === "unavailable") return null;
  const dot = DOT[status];
  return (
    <Link href="/wallet" className="inline-flex items-center gap-1.5 py-2 hover:text-text">
      {dot ? (
        <span aria-hidden="true" className="size-1.5 rounded-full" style={{ background: dot }} />
      ) : null}
      Wallet
      {status === "unlocked" ? <span className="sr-only"> (unlocked)</span> : null}
    </Link>
  );
}
