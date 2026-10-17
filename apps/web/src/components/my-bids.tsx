"use client";

import { formatZec, receiptMemo } from "@sealforge/bid";
import {
  CircleAlert,
  CircleCheck,
  Download,
  ExternalLink,
  FileCheck,
  LoaderCircle,
  ScanSearch,
  Wallet,
} from "lucide-react";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { ButtonLink, buttonClass } from "@/components/button";
import { checkReceipt } from "@/lib/auditor";
import { blocksEta, networkOfUa, unit, zec } from "@/lib/format";
import {
  downloadReceipt,
  listReceipts,
  markBackedUp,
  type StoredReceipt,
  updateReceipt,
} from "@/lib/receipts";
import type {
  AuctionView,
  ChainView,
  PublishedResult,
  ReceiptCheck,
  RefundProgress,
} from "@/lib/types";
import type { WalletSummary } from "@/lib/wallet/session";
import { RevealPanel } from "./reveal-panel";
import { InlineUnlock } from "./wallet/inline-unlock";
import { PaymentStatus, WalletPay } from "./wallet/wallet-pay";
import { useWallet } from "./wallet/wallet-provider";

/** Public facts about one auction, all from the coordinator's public API. */
interface AuctionFacts {
  auction: AuctionView | null;
  result: PublishedResult | null;
  refunds: RefundProgress | null;
  error: string | null;
}

async function getJson<T>(url: string): Promise<T | null> {
  const res = await fetch(url, { cache: "no-store" });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return (await res.json()) as T;
}

async function loadFacts(code: string): Promise<AuctionFacts> {
  try {
    const auction = await getJson<AuctionView>(`/api/coordinator/v1/auctions/${code}`);
    const published = auction && ["published", "refunding", "settled"].includes(auction.status);
    const [result, refunds] = published
      ? await Promise.all([
          getJson<PublishedResult>(`/api/auctions/${code}/result`),
          getJson<RefundProgress>(`/api/coordinator/v1/auctions/${code}/refunds`),
        ])
      : [null, null];
    return { auction, result, refunds, error: null };
  } catch {
    return {
      auction: null,
      result: null,
      refunds: null,
      error: "The auction server can't be reached right now.",
    };
  }
}

type Tone = "action" | "progress" | "good" | "neutral" | "bad";

interface Lifecycle {
  label: string;
  tone: Tone;
  /** 0 paid, 1 bidding, 2 reveal, 3 result, 4 refunds, 5 settled. */
  step: number;
  group: "attention" | "active" | "done";
}

/** Refund notes that reached this bid's wallet address (on chain, seen by the wallet). */
function refundsReceived(receipt: StoredReceipt, summary: WalletSummary | null) {
  if (receipt.wallet_address_index === undefined || !summary) return null;
  const notes = summary.notes.filter(
    (n) => !n.change && n.address_index === receipt.wallet_address_index,
  );
  return {
    zat: notes.reduce((s, n) => s + n.value_zat, 0),
    count: notes.length,
    lastHeight: notes.reduce((h, n) => Math.max(h, n.height), 0),
  };
}

function lifecycle(
  receipt: StoredReceipt,
  facts: AuctionFacts | undefined,
  check: ReceiptCheck | null,
  tip: number | null,
  summary: WalletSummary | null,
): Lifecycle {
  const paid = receipt.paid_txid ? summary?.sent.find((t) => t.txid === receipt.paid_txid) : null;
  const a = facts?.auction;
  if (paid?.expired) return { label: "Payment expired", tone: "bad", step: 0, group: "attention" };
  if (!a) return { label: "Auction unavailable", tone: "neutral", step: 0, group: "active" };
  if (a.status === "cancelled")
    return { label: "Cancelled", tone: "neutral", step: 0, group: "done" };
  const sealed = a.bid_mode === "commit_reveal" && a.reveal_close_height;
  if (paid && paid.mined_height === null)
    return { label: "Payment confirming", tone: "progress", step: 0, group: "active" };
  if (a.status === "open" && (tip === null || tip < a.close_height))
    return {
      label: receipt.paid_txid ? "Sealed bid placed" : "Bidding open",
      tone: "progress",
      step: 1,
      group: receipt.paid_txid ? "active" : "attention",
    };
  const revealed = Boolean(receipt.reveal_txid || receipt.reveal_sent);
  if (
    sealed &&
    a.status === "open" &&
    tip !== null &&
    tip > a.close_height &&
    tip <= (a.reveal_close_height ?? 0) &&
    !revealed
  )
    return { label: "Reveal your bid", tone: "action", step: 2, group: "attention" };
  const reveal = receipt.reveal_txid
    ? summary?.sent.find((t) => t.txid === receipt.reveal_txid)
    : null;
  if (reveal && reveal.mined_height === null && !reveal.expired && !facts?.result)
    return { label: "Reveal confirming", tone: "progress", step: 2, group: "active" };
  if (!facts?.result)
    return {
      label: "Waiting for the result",
      tone: "progress",
      step: sealed ? 3 : 1,
      group: "active",
    };
  if (!check) return { label: "Checking the result…", tone: "progress", step: 3, group: "active" };
  if (!check.found) return { label: "Not in the result", tone: "bad", step: 3, group: "attention" };
  const received = refundsReceived(receipt, summary);
  const due = check.refund_due_zat - check.dust_zat;
  if (a.status === "settled" && (received === null || received.zat >= due))
    return {
      label: check.is_winner ? "Won · settled" : "Settled",
      tone: "good",
      step: 5,
      group: "done",
    };
  if (received && due > 0 && received.zat >= due)
    return {
      label: check.is_winner ? "Won · refund received" : "Refund received",
      tone: "good",
      step: 4,
      group: "done",
    };
  if (due > 0)
    return {
      label: check.is_winner ? "Won · refund on its way" : "Refund on its way",
      tone: "progress",
      step: 4,
      group: "active",
    };
  return {
    label: check.is_winner ? "Won" : "Lost",
    tone: check.is_winner ? "good" : "neutral",
    step: 4,
    group: "active",
  };
}

const TONE: Record<Tone, string> = {
  action: "var(--primary)",
  progress: "var(--info)",
  good: "var(--success)",
  neutral: "var(--text-muted)",
  bad: "var(--danger)",
};

export function MyBids() {
  const wallet = useWallet();
  const [receipts, setReceipts] = useState<StoredReceipt[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [facts, setFacts] = useState<Record<string, AuctionFacts>>({});
  const [checks, setChecks] = useState<Record<string, ReceiptCheck | null>>({});
  const [tip, setTip] = useState<number | null>(null);

  useEffect(() => {
    listReceipts()
      .then(setReceipts)
      .catch(() => setFailed(true));
  }, []);

  const codes = useMemo(
    () => [...new Set((receipts ?? []).map((r) => r.auction_code))],
    [receipts],
  );

  const refresh = useCallback(async () => {
    if (codes.length === 0) return;
    const chain = await getJson<ChainView>("/api/coordinator/v1/chain").catch(() => null);
    setTip(chain?.tip ?? null);
    const loaded = await Promise.all(codes.map(async (c) => [c, await loadFacts(c)] as const));
    setFacts(Object.fromEntries(loaded));
  }, [codes]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, 60_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  // Inclusion, outcome and refund for each receipt, recomputed in the browser (WASM auditor).
  useEffect(() => {
    for (const r of receipts ?? []) {
      const result = facts[r.auction_code]?.result;
      if (!result || checks[r.commitment] !== undefined) continue;
      checkReceipt(result, r)
        .then((c) => setChecks((x) => ({ ...x, [r.commitment]: c })))
        .catch(() => setChecks((x) => ({ ...x, [r.commitment]: null })));
    }
  }, [receipts, facts, checks]);

  const patch = (commitment: string, p: Partial<StoredReceipt>) =>
    setReceipts(
      (all) => all?.map((x) => (x.commitment === commitment ? { ...x, ...p } : x)) ?? null,
    );

  if (failed)
    return (
      <p role="alert" className="text-muted">
        This browser can&rsquo;t open saved receipts (private mode or blocked storage).
      </p>
    );
  if (receipts === null)
    return (
      <div
        aria-busy="true"
        className="h-24 animate-pulse rounded-[var(--radius-card)] bg-surface"
      />
    );
  if (receipts.length === 0)
    return (
      <div className="space-y-3 rounded-[var(--radius-card)] border border-border bg-surface p-6">
        <p>Your sealed bids will appear here. They&rsquo;re stored only on this device.</p>
        <ButtonLink href="/" variant="secondary">
          Find an auction
        </ButtonLink>
      </div>
    );

  const rows = receipts.map((r) => ({
    receipt: r,
    life: lifecycle(r, facts[r.auction_code], checks[r.commitment] ?? null, tip, wallet.summary),
  }));
  const groups = [
    { key: "attention", title: "Needs you" },
    { key: "active", title: "In progress" },
    { key: "done", title: "Finished" },
  ] as const;

  return (
    <div className="space-y-8">
      {wallet.status === "locked" ? (
        <InlineUnlock prompt="Unlock your wallet to see payment confirmations and refunds as they arrive." />
      ) : null}
      {groups.map((g) => {
        const items = rows.filter((x) => x.life.group === g.key);
        if (items.length === 0) return null;
        return (
          <section key={g.key} aria-labelledby={`g-${g.key}`} className="space-y-3">
            <h2 id={`g-${g.key}`} className="text-sm font-medium text-muted">
              {g.title} <span className="num">({items.length})</span>
            </h2>
            <ul className="space-y-3">
              {items.map(({ receipt, life }) => (
                <BidCard
                  key={receipt.commitment}
                  receipt={receipt}
                  life={life}
                  facts={facts[receipt.auction_code]}
                  check={checks[receipt.commitment] ?? null}
                  tip={tip}
                  onPatch={(p) => patch(receipt.commitment, p)}
                />
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}

const STEPS = ["Paid", "Bidding", "Reveal", "Result", "Refund", "Settled"];

function Steps({ step: current, sealed }: { step: number; sealed: boolean }) {
  // Settled is the end: every step is done.
  const step = current >= STEPS.length - 1 ? STEPS.length : current;
  const steps = STEPS.map((label, i) => ({ label, i })).filter((s) => sealed || s.i !== 2);
  return (
    <ol className="flex items-center gap-1.5" aria-label="Progress">
      {steps.map((s) => (
        <li key={s.label} className="flex items-center gap-1.5">
          <span
            className={`h-1.5 w-6 rounded-full sm:w-8 ${s.i < step ? "bg-primary" : s.i === step ? "bg-primary/60" : "bg-border"}`}
            aria-hidden="true"
          />
          <span className="sr-only">
            {s.label}: {s.i < step ? "done" : s.i === step ? "current" : "to come"}
          </span>
        </li>
      ))}
    </ol>
  );
}

function BidCard({
  receipt,
  life,
  facts,
  check,
  tip,
  onPatch,
}: {
  receipt: StoredReceipt;
  life: Lifecycle;
  facts: AuctionFacts | undefined;
  check: ReceiptCheck | null;
  tip: number | null;
  onPatch: (p: Partial<StoredReceipt>) => void;
}) {
  const wallet = useWallet();
  const network = networkOfUa(receipt.refund_ua);
  const a = facts?.auction ?? null;
  const sealed = a?.bid_mode === "commit_reveal" || receipt.price_zat !== undefined;
  const received = refundsReceived(receipt, wallet.summary);
  const due = check ? check.refund_due_zat - check.dust_zat : null;
  const canPayFromWallet =
    !receipt.paid_txid &&
    a?.status === "open" &&
    a.escrow_ua &&
    tip !== null &&
    tip < a.close_height &&
    wallet.status === "unlocked" &&
    wallet.network === a.network;
  const [paying, setPaying] = useState(false);

  return (
    <li className="space-y-4 rounded-[var(--radius-card)] border border-border bg-surface p-5 fade-in">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          <Link
            href={`/a/${receipt.auction_code}`}
            className="block truncate font-medium hover:text-primary"
          >
            {receipt.title}
          </Link>
          <p className="text-[13px] text-muted">
            {sealed && receipt.price_zat !== undefined ? (
              <>
                Hidden bid{" "}
                <span className="num">
                  {formatZec(BigInt(receipt.price_zat))} {unit(network)}
                </span>{" "}
                · deposit{" "}
              </>
            ) : (
              "Bid "
            )}
            <span className="num">
              {formatZec(BigInt(receipt.amount_zat))} {unit(network)}
            </span>{" "}
            · {new Date(receipt.created_at).toLocaleDateString(undefined, { dateStyle: "medium" })}
          </p>
        </div>
        <span className="inline-flex items-center gap-2 rounded-[var(--radius-chip)] border border-border px-2.5 py-1 text-[13px] font-medium">
          <span
            aria-hidden="true"
            className="size-2 rounded-full"
            style={{ background: TONE[life.tone] }}
          />
          {life.label}
        </span>
      </div>

      <Steps step={life.step} sealed={sealed} />

      {facts?.error ? (
        <p className="flex items-center gap-2 text-sm text-muted">
          <CircleAlert size={14} aria-hidden="true" /> {facts.error}
        </p>
      ) : null}

      {/* Payment evidence */}
      {receipt.paid_txid ? (
        <PaymentStatus txid={receipt.paid_txid} />
      ) : a?.status === "open" && tip !== null && tip < a.close_height ? (
        <p className="text-sm text-muted">
          Paid from another wallet? This page can&rsquo;t see that payment; your bid shows up in the
          published result after close. Bidding closes in{" "}
          {blocksEta(a.close_height - tip, a.network)} (block{" "}
          <span className="num">{a.close_height.toLocaleString("en-US")}</span>).
        </p>
      ) : null}

      {canPayFromWallet && a?.escrow_ua && paying ? (
        <WalletPay
          network={a.network}
          request={{
            to: a.escrow_ua,
            amount_zat: receipt.amount_zat,
            memo: receiptMemo(receipt),
            label: `bid:${a.code}`,
          }}
          closeHeight={a.close_height}
          onPaid={async (txid) => {
            await updateReceipt(receipt.commitment, { paid_txid: txid }).catch(() => {});
            onPatch({ paid_txid: txid });
            setPaying(false);
          }}
        />
      ) : null}

      {/* Reveal */}
      {a && sealed && receipt.price_zat !== undefined ? (
        <RevealPanel receipt={receipt} auction={a} tip={tip} onChanged={onPatch} />
      ) : null}

      {/* Result */}
      {check ? (
        check.found ? (
          <dl className="grid gap-x-6 gap-y-2 text-sm sm:grid-cols-3">
            <div>
              <dt className="text-muted">In the published result</dt>
              <dd className="inline-flex items-center gap-1.5">
                {check.included ? (
                  <>
                    <CircleCheck size={14} aria-hidden="true" className="text-success" /> Merkle
                    proof checks
                  </>
                ) : (
                  <span className="text-danger">Proof failed</span>
                )}
              </dd>
            </div>
            <div>
              <dt className="text-muted">Outcome</dt>
              <dd>
                {check.is_winner ? (
                  <span className="text-success">
                    Won at <span className="num">{zec(check.price_paid_zat, network)}</span>
                  </span>
                ) : (
                  "Not a winning bid"
                )}
              </dd>
            </div>
            <div>
              <dt className="text-muted">Refund due</dt>
              <dd className="num">{zec(due ?? 0, network)}</dd>
            </div>
          </dl>
        ) : (
          <p className="text-sm text-danger">
            This bid isn&rsquo;t in the published result. If its payment confirmed after bidding
            closed, or a sealed bid wasn&rsquo;t revealed, it&rsquo;s refunded in full; the{" "}
            <Link href={`/audit/${receipt.auction_code}`} className="underline">
              chain audit
            </Link>{" "}
            lists every payment to the escrow.
          </p>
        )
      ) : null}

      {/* Refunds */}
      {check?.found && due !== null && due > 0 ? (
        <RefundLine
          received={received}
          due={due}
          network={network}
          refunds={facts?.refunds ?? null}
        />
      ) : null}

      <div className="flex flex-wrap gap-2 pt-1">
        {canPayFromWallet && !paying ? (
          <button
            type="button"
            className={buttonClass({ size: "md" })}
            onClick={() => setPaying(true)}
          >
            <Wallet size={16} aria-hidden="true" /> Pay from wallet
          </button>
        ) : null}
        <Link href={`/verify`} className={buttonClass({ variant: "secondary" })}>
          <FileCheck size={16} aria-hidden="true" /> Verify
        </Link>
        {facts?.result ? (
          <Link
            href={`/audit/${receipt.auction_code}`}
            className={buttonClass({ variant: "secondary" })}
          >
            <ScanSearch size={16} aria-hidden="true" /> Audit
          </Link>
        ) : null}
        <button
          type="button"
          className={buttonClass({ variant: receipt.backed_up ? "ghost" : "secondary" })}
          onClick={async () => {
            downloadReceipt(receipt);
            await markBackedUp(receipt.commitment).catch(() => {});
            onPatch({ backed_up: true });
          }}
        >
          <Download size={16} aria-hidden="true" />{" "}
          {receipt.backed_up ? "Receipt" : "Back up receipt"}
        </button>
        <Link href={`/a/${receipt.auction_code}`} className={buttonClass({ variant: "ghost" })}>
          <ExternalLink size={16} aria-hidden="true" /> Auction
        </Link>
      </div>
    </li>
  );
}

function RefundLine({
  received,
  due,
  network,
  refunds,
}: {
  received: { zat: number; count: number; lastHeight: number } | null;
  due: number;
  network: ReturnType<typeof networkOfUa>;
  refunds: RefundProgress | null;
}) {
  const pct = received ? Math.min(100, Math.round((received.zat / due) * 100)) : null;
  return (
    <div className="space-y-2 text-sm">
      {received ? (
        <>
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <span>
              Received <span className="num">{zec(received.zat, network)}</span> of{" "}
              <span className="num">{zec(due, network)}</span>
              {received.count > 0 ? (
                <span className="text-muted">
                  {" "}
                  · {received.count} note{received.count === 1 ? "" : "s"}, last in block{" "}
                  {received.lastHeight.toLocaleString("en-US")}
                </span>
              ) : null}
            </span>
            {pct !== null && pct < 100 ? (
              <span className="inline-flex items-center gap-1 text-muted">
                <LoaderCircle size={12} className="animate-spin" aria-hidden="true" /> arriving
              </span>
            ) : null}
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-border" aria-hidden="true">
            <div
              className="h-full rounded-full bg-success transition-[width] duration-500"
              style={{ width: `${pct}%` }}
            />
          </div>
        </>
      ) : (
        <p className="text-muted">
          Refunds go to the address in your receipt, from another wallet; check it there.
        </p>
      )}
      {refunds ? (
        <p className="text-[13px] text-muted">
          {refunds.notes_total === 0
            ? "Auction-wide: refunds are being planned."
            : `Auction-wide: ${refunds.notes_sent} of ${refunds.notes_total} refund notes sent so far (counts for all bidders, nothing per bid).`}
        </p>
      ) : null}
    </div>
  );
}
