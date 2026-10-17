"use client";

import { CircleAlert, CircleCheck, Clock, LoaderCircle, Wallet } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Button } from "@/components/button";
import { HashChip } from "@/components/hash-chip";
import { zec } from "@/lib/format";
import type { Network } from "@/lib/types";
import type { PaymentRequest } from "@/lib/wallet/session";
import { useWallet } from "./wallet-provider";

type Phase = "idle" | "proving" | "error";

/**
 * Pay a bid deposit (or a reveal) from the built-in wallet. Nothing is shown as
 * paid here: after broadcasting, [`PaymentStatus`] reports what the wallet sees on chain.
 */
export function WalletPay({
  network,
  request,
  closeHeight,
  onPaid,
  cta = "Pay",
}: {
  network: Network;
  request: PaymentRequest & { memo: string };
  /** The payment must be mined by this height to count (bids), or null. */
  closeHeight: number | null;
  onPaid: (txid: string) => void;
  cta?: string;
}) {
  const wallet = useWallet();
  const [fee, setFee] = useState<number | null>(null);
  const [shortfall, setShortfall] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);
  const spendable = wallet.summary?.balance.spendable_zat ?? 0;
  const scanned = wallet.summary?.scannedHeight ?? 0;
  const tip = wallet.sync.progress?.tip ?? scanned;
  const late = closeHeight !== null && tip >= closeHeight;
  // A payment needs a block or two to be mined; warn when the window is nearly over.
  const tight = closeHeight !== null && !late && closeHeight - tip <= 2;

  // Ask the wallet for the exact fee whenever the spendable balance changes.
  // biome-ignore lint/correctness/useExhaustiveDependencies: re-quote when the balance changes
  useEffect(() => {
    let cancelled = false;
    wallet
      .quote(request.amount_zat)
      .then((f) => {
        if (cancelled) return;
        setFee(f);
        setShortfall(null);
      })
      .catch((e: Error) => {
        if (cancelled) return;
        setFee(null);
        setShortfall(e.message);
      });
    return () => {
      cancelled = true;
    };
  }, [request.amount_zat, spendable]);

  async function pay() {
    setPhase("proving");
    setError(null);
    try {
      const built = await wallet.pay(request);
      onPaid(built.txid);
    } catch (e) {
      setError((e as Error).message);
      setPhase("error");
    }
  }

  return (
    <section
      aria-label="Pay from your sealForge wallet"
      className="space-y-4 rounded-[var(--radius-card)] border border-primary/40 bg-surface p-5"
    >
      <p className="flex items-center gap-2 font-medium">
        <Wallet size={18} aria-hidden="true" className="text-primary" /> Pay from your wallet
      </p>
      <dl className="space-y-2 text-sm">
        <div className="flex justify-between gap-4">
          <dt className="text-muted">Amount</dt>
          <dd className="num">{zec(request.amount_zat, network)}</dd>
        </div>
        <div className="flex justify-between gap-4">
          <dt className="text-muted">Network fee</dt>
          <dd className="num">{fee !== null ? zec(fee, network) : "…"}</dd>
        </div>
        <div className="flex justify-between gap-4 border-t border-border pt-2">
          <dt className="text-muted">Spendable in your wallet</dt>
          <dd className="num">{zec(spendable, network)}</dd>
        </div>
      </dl>

      {late ? (
        <p role="alert" className="flex items-start gap-2 text-sm text-danger">
          <CircleAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
          Bidding closed at block {closeHeight?.toLocaleString("en-US")}. A payment now
          wouldn&rsquo;t count.
        </p>
      ) : tight ? (
        <p className="flex items-start gap-2 text-sm text-warning">
          <CircleAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
          Only {(closeHeight ?? 0) - tip} block{(closeHeight ?? 0) - tip === 1 ? "" : "s"} left
          before block {closeHeight?.toLocaleString("en-US")}. A payment sent now may be mined too
          late to count.
        </p>
      ) : shortfall ? (
        <div className="space-y-2 text-sm">
          <p className="flex items-start gap-2 text-warning">
            <CircleAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
            {shortfall}
          </p>
          <p className="text-muted">
            <Link
              href="/wallet"
              target="_blank"
              rel="noopener"
              className="underline hover:text-text"
            >
              Add funds to your wallet
            </Link>{" "}
            (opens a new tab; this bid waits here, and is saved in My bids), or pay from another
            wallet below.
          </p>
        </div>
      ) : null}

      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}

      <Button
        type="button"
        size="lg"
        className="w-full"
        onClick={pay}
        disabled={phase === "proving" || fee === null || late || !wallet.summary}
      >
        {phase === "proving" ? (
          <>
            <LoaderCircle size={18} className="animate-spin" aria-hidden="true" /> Proving and
            signing…
          </>
        ) : (
          `${cta} ${fee !== null ? zec(request.amount_zat + fee, network) : ""}`.trim()
        )}
      </Button>
      <p className="text-[13px] text-muted" aria-live="polite">
        {phase === "proving"
          ? "Building the zero-knowledge proof in this browser. The first payment can take up to a minute; keep this tab open."
          : "Built, proved and signed in this browser, then broadcast through the lightwallet server. It counts once it's mined."}
      </p>
    </section>
  );
}

/** What the wallet has seen of one of its payments, on chain. */
export function PaymentStatus({ txid, label = "Deposit" }: { txid: string; label?: string }) {
  const wallet = useWallet();
  const tx = wallet.summary?.sent.find((t) => t.txid === txid);
  let body: React.ReactNode;
  if (wallet.status !== "unlocked") {
    body = (
      <span className="text-muted">
        Paid from your sealForge wallet.{" "}
        <Link href="/wallet" className="underline hover:text-text">
          Unlock it
        </Link>{" "}
        to see whether it&rsquo;s confirmed.
      </span>
    );
  } else if (!tx) {
    body = (
      <span className="text-muted">
        Paid from a wallet session that isn&rsquo;t open here, so this page can&rsquo;t see it yet.
      </span>
    );
  } else if (tx.mined_height !== null) {
    body = (
      <span className="inline-flex items-center gap-1.5 text-success">
        <CircleCheck size={16} aria-hidden="true" /> {label} confirmed in block{" "}
        {tx.mined_height.toLocaleString("en-US")}
      </span>
    );
  } else if (tx.expired) {
    body = (
      <span className="inline-flex items-center gap-1.5 text-warning">
        <CircleAlert size={16} aria-hidden="true" /> {label} expired without confirming. Your funds
        are spendable again; pay again if bidding is still open.
      </span>
    );
  } else {
    body = (
      <span className="inline-flex items-center gap-1.5">
        <Clock size={16} aria-hidden="true" className="text-warning" /> {label} sent, waiting for a
        block. Checked every 30 seconds.
      </span>
    );
  }
  return (
    <div
      className="space-y-2 rounded-[var(--radius-control)] border border-border p-3 text-sm"
      role="status"
    >
      <p>{body}</p>
      <HashChip value={txid} label="transaction id" head={10} tail={6} />
    </div>
  );
}
