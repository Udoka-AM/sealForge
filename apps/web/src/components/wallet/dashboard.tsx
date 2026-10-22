"use client";

import {
  ArrowDownLeft,
  ArrowUpRight,
  CircleAlert,
  Clock,
  Globe,
  Lock,
  QrCode,
  RefreshCw,
  Repeat,
  Trash2,
  Wallet,
} from "lucide-react";
import { AnimatePresence, m } from "motion/react";
import Link from "next/link";
import QRCode from "qrcode";
import { useEffect, useId, useMemo, useState } from "react";
import { Button } from "@/components/button";
import { HashChip } from "@/components/hash-chip";
import { Guilloche } from "@/components/motion/guilloche";
import { ZecCountUp } from "@/components/motion/zec-count-up";
import { unit, zec } from "@/lib/format";
import { listReceipts, type StoredReceipt } from "@/lib/receipts";
import type { Network } from "@/lib/types";
import type { SentTx, WalletNote } from "@/lib/wallet/session";
import { useWallet } from "./wallet-provider";

export function Dashboard() {
  const wallet = useWallet();
  const [receipts, setReceipts] = useState<StoredReceipt[]>([]);
  useEffect(() => {
    listReceipts()
      .then(setReceipts)
      .catch(() => setReceipts([]));
  }, []);
  if (!wallet.summary || !wallet.network) return null;
  return (
    <div className="space-y-6 fade-in">
      <BalanceCard />
      <div className="grid gap-6 md:grid-cols-[1fr_1fr]">
        <FundPanel network={wallet.network} address={wallet.summary.fundingAddress} />
        <NetworkPanel />
      </div>
      <Activity receipts={receipts} />
      <SecurityPanel />
    </div>
  );
}

function BalanceCard() {
  const { summary, network, sync } = useWallet();
  if (!summary || !network) return null;
  const b = summary.balance;
  const behind = sync.progress && sync.progress.scanned < sync.progress.tip;
  return (
    <section
      aria-label="Balance"
      className="relative overflow-hidden rounded-[var(--radius-sheet)] border border-border bg-surface p-6 shadow-[var(--shadow-lift)]"
    >
      <Guilloche size={300} className="absolute -top-16 -right-20 opacity-50" />
      <p className="relative flex items-center gap-2 text-sm text-muted">
        <Wallet size={16} aria-hidden="true" /> Spendable
        {network !== "mainnet" ? (
          <span className="rounded-[var(--radius-chip)] border border-border px-2 py-0.5 text-[12px]">
            {network}: test coins, no value
          </span>
        ) : null}
      </p>
      <ZecCountUp
        zat={b.spendable_zat}
        network={network}
        className="relative mt-2 block text-[44px] leading-[52px] font-medium tracking-tight"
      />
      <dl className="mt-4 flex flex-wrap gap-x-6 gap-y-2 text-sm">
        {b.pending_spend_zat > 0 ? (
          <div className="flex gap-1.5">
            <dt className="text-muted">Waiting for a payment to confirm</dt>
            <dd className="num">{zec(b.pending_spend_zat, network)}</dd>
          </div>
        ) : null}
        {b.orchard_zat > 0 ? (
          <div className="flex gap-1.5">
            <dt className="text-muted">Orchard pool (not spendable here)</dt>
            <dd className="num">{zec(b.orchard_zat, network)}</dd>
          </div>
        ) : null}
        <div className="flex gap-1.5">
          <dt className="text-muted">Total</dt>
          <dd className="num">{zec(b.total_zat, network)}</dd>
        </div>
      </dl>
      {sync.running && behind && sync.progress ? (
        <div className="mt-4 h-1 overflow-hidden rounded-full bg-border" aria-hidden="true">
          <m.div
            className="h-full origin-left rounded-full bg-primary"
            initial={{ scaleX: 0 }}
            animate={{
              scaleX:
                (sync.progress.scanned - summary.birthday) /
                Math.max(1, sync.progress.tip - summary.birthday),
            }}
            transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
          />
        </div>
      ) : null}
      <p className="mt-4 text-[13px] text-muted" aria-live="polite">
        {sync.running
          ? behind
            ? `Syncing: block ${sync.progress?.scanned.toLocaleString("en-US")} of ${sync.progress?.tip.toLocaleString("en-US")}…`
            : "Checking for new blocks…"
          : sync.error
            ? null
            : `Up to date at block ${summary.scannedHeight.toLocaleString("en-US")}.`}
      </p>
      {sync.error ? (
        <p role="alert" className="mt-2 flex items-start gap-2 text-sm text-danger">
          <CircleAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
          {sync.error} The balance shown is as of block{" "}
          {summary.scannedHeight.toLocaleString("en-US")}.
        </p>
      ) : null}
    </section>
  );
}

type RampState = "checking" | "unavailable" | "available";

function FundPanel({ network, address }: { network: Network; address: string }) {
  const [qr, setQr] = useState<string | null>(null);
  const [showQr, setShowQr] = useState(false);
  const [ramp, setRamp] = useState<RampState>("checking");

  useEffect(() => {
    QRCode.toString(`zcash:${address}`, { type: "svg", margin: 2, errorCorrectionLevel: "M" })
      .then((svg) => setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`))
      .catch(() => setQr(null));
  }, [address]);

  // Cross-chain funding only appears when the site's ShieldRamp route actually returns quotes.
  useEffect(() => {
    fetch("/api/shieldramp/quote", { method: "POST", body: "{}" })
      .then((r) => setRamp(r.ok ? "available" : "unavailable"))
      .catch(() => setRamp("unavailable"));
  }, []);

  return (
    <section
      aria-labelledby="fund-h"
      className="space-y-4 rounded-[var(--radius-card)] border border-border p-5"
    >
      <h2 id="fund-h" className="flex items-center gap-2 font-medium">
        <ArrowDownLeft size={18} aria-hidden="true" className="text-primary" /> Add funds
      </h2>
      <div className="space-y-2">
        <p className="text-sm text-muted">Your funding address</p>
        <HashChip value={address} label="funding address" head={14} tail={8} />
        <div>
          <button
            type="button"
            onClick={() => setShowQr((s) => !s)}
            className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-text"
            aria-expanded={showQr}
          >
            <QrCode size={14} aria-hidden="true" /> {showQr ? "Hide QR code" : "Show QR code"}
          </button>
        </div>
        {showQr && qr ? (
          <div className="w-fit rounded-[var(--radius-control)] bg-white p-2">
            {/* biome-ignore lint/performance/noImgElement: a local data-URI SVG */}
            <img src={qr} alt="QR code of your funding address" className="size-44" />
          </div>
        ) : null}
      </div>
      <ul className="space-y-3 text-sm">
        <li className="flex gap-2.5">
          <span aria-hidden="true" className="mt-1.5 size-1.5 shrink-0 rounded-full bg-success" />
          <span>
            <span className="font-medium">From any Zcash wallet.</span>{" "}
            <span className="text-muted">
              Send {unit(network)} to this address from a wallet that supports Unified Addresses.
              Shielded is best; it arrives after one confirmation.
            </span>
          </span>
        </li>
        {network !== "mainnet" ? (
          <li className="flex gap-2.5">
            <span aria-hidden="true" className="mt-1.5 size-1.5 shrink-0 rounded-full bg-success" />
            <span>
              <span className="font-medium">From a testnet faucet.</span>{" "}
              <span className="text-muted">Paste this address into a Zcash {network} faucet.</span>
            </span>
          </li>
        ) : null}
        <li className="flex gap-2.5">
          <span
            aria-hidden="true"
            className={`mt-1.5 size-1.5 shrink-0 rounded-full ${ramp === "available" ? "bg-success" : "bg-border"}`}
          />
          <span>
            <span className="font-medium">With USDC or ETH (NEAR Intents).</span>{" "}
            <span className="text-muted">
              {ramp === "checking"
                ? "Checking availability…"
                : ramp === "available"
                  ? "Available: start from an auction's bid page."
                  : "Not available on this site yet. Native ZEC works without it."}
            </span>
          </span>
        </li>
      </ul>
    </section>
  );
}

function NetworkPanel() {
  const { network, endpoint, summary, sync, syncNow } = useWallet();
  const host = useMemo(() => {
    try {
      return endpoint ? new URL(endpoint).host : "none";
    } catch {
      return endpoint ?? "none";
    }
  }, [endpoint]);
  const tip = sync.progress?.tip ?? null;
  return (
    <section
      aria-labelledby="net-h"
      className="space-y-4 rounded-[var(--radius-card)] border border-border p-5"
    >
      <h2 id="net-h" className="flex items-center gap-2 font-medium">
        <Globe size={18} aria-hidden="true" className="text-muted" /> Network
      </h2>
      <dl className="space-y-2.5 text-sm">
        <Row label="Network">
          <span className="capitalize">{network}</span>
        </Row>
        <Row label="Lightwallet server">
          <span className="num text-[13px]">{host}</span>
        </Row>
        <Row label="Scanned to">
          <span className="num">{summary?.scannedHeight.toLocaleString("en-US")}</span>
        </Row>
        <Row label="Chain tip">
          <span className="num">{tip ? tip.toLocaleString("en-US") : "…"}</span>
        </Row>
        <Row label="Wallet born at">
          <span className="num">{summary?.birthday.toLocaleString("en-US")}</span>
        </Row>
        <Row label="Last synced">
          <span>
            {sync.lastSynced ? new Date(sync.lastSynced).toLocaleTimeString() : "not yet"}
          </span>
        </Row>
      </dl>
      <Button
        type="button"
        variant="secondary"
        onClick={() => void syncNow()}
        disabled={sync.running}
        className="w-full"
      >
        <RefreshCw size={16} aria-hidden="true" className={sync.running ? "animate-spin" : ""} />
        {sync.running ? "Syncing…" : "Sync now"}
      </Button>
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <dt className="text-muted">{label}</dt>
      <dd className="text-right">{children}</dd>
    </div>
  );
}

interface ActivityItem {
  key: string;
  height: number | null;
  kind: "in" | "out";
  title: string;
  detail: string | null;
  amount: number;
  status: "pending" | "confirmed" | "expired";
  txid: string;
  href: string | null;
}

/** "bid:CODE" / "reveal:CODE" labels → words. */
function describeSent(
  tx: SentTx,
  receipts: StoredReceipt[],
): { title: string; href: string | null } {
  const [kind, code] = (tx.label ?? "").split(":");
  const title = receipts.find((r) => r.auction_code === code)?.title ?? code;
  if (kind === "bid" && code) return { title: `Bid deposit · ${title}`, href: `/a/${code}` };
  if (kind === "reveal" && code) return { title: `Bid reveal · ${title}`, href: `/a/${code}` };
  return { title: "Payment", href: null };
}

function activity(notes: WalletNote[], sent: SentTx[], receipts: StoredReceipt[]): ActivityItem[] {
  const items: ActivityItem[] = [];
  const ours = new Set(sent.map((t) => t.txid));
  // Incoming payments, grouped by transaction (change from our own payments is hidden).
  const incoming = new Map<string, WalletNote[]>();
  for (const n of notes) {
    if (n.change || ours.has(n.txid)) continue;
    incoming.set(n.txid, [...(incoming.get(n.txid) ?? []), n]);
  }
  for (const [txid, group] of incoming) {
    const index = group[0]?.address_index ?? null;
    const receipt = receipts.find(
      (r) => r.wallet_address_index !== undefined && r.wallet_address_index === index,
    );
    items.push({
      key: `in-${txid}`,
      height: group[0]?.height ?? null,
      kind: "in",
      title: receipt ? `Refund · ${receipt.title}` : index === 0 ? "Funds received" : "Received",
      detail: group.length > 1 ? `${group.length} notes` : null,
      amount: group.reduce((s, n) => s + n.value_zat, 0),
      status: "confirmed",
      txid,
      href: receipt ? `/a/${receipt.auction_code}` : null,
    });
  }
  // Spends this browser didn't build (another device, or before a restore): what left is
  // the notes spent minus the change that came back, fee included.
  const elsewhere = new Map<string, { height: number | null; out: number }>();
  for (const n of notes) {
    if (!n.spent_txid || ours.has(n.spent_txid)) continue;
    const e = elsewhere.get(n.spent_txid) ?? { height: n.spent_height, out: 0 };
    e.out += n.value_zat;
    elsewhere.set(n.spent_txid, e);
  }
  for (const [txid, e] of elsewhere) {
    const back = notes
      .filter((n) => n.change && n.txid === txid)
      .reduce((s, n) => s + n.value_zat, 0);
    items.push({
      key: `elsewhere-${txid}`,
      height: e.height,
      kind: "out",
      title: "Sent from another session",
      detail: "Amount includes the network fee",
      amount: e.out - back,
      status: "confirmed",
      txid,
      href: null,
    });
  }
  for (const tx of sent) {
    const d = describeSent(tx, receipts);
    items.push({
      key: `out-${tx.txid}`,
      height: tx.mined_height,
      kind: "out",
      title: d.title,
      detail: `Fee ${tx.fee_zat.toLocaleString("en-US")} zat`,
      amount: tx.amount_zat,
      status: tx.mined_height !== null ? "confirmed" : tx.expired ? "expired" : "pending",
      txid: tx.txid,
      href: d.href,
    });
  }
  return items.sort(
    (a, b) => (b.height ?? Number.MAX_SAFE_INTEGER) - (a.height ?? Number.MAX_SAFE_INTEGER),
  );
}

function Activity({ receipts }: { receipts: StoredReceipt[] }) {
  const { summary, network } = useWallet();
  if (!summary || !network) return null;
  const items = activity(summary.notes, summary.sent, receipts);
  return (
    <section aria-labelledby="act-h" className="space-y-3">
      <h2 id="act-h" className="font-medium">
        Activity
      </h2>
      {items.length === 0 ? (
        <p className="rounded-[var(--radius-card)] border border-border p-5 text-sm text-muted">
          Nothing yet. Funds you receive and bids you pay from this wallet appear here once
          they&rsquo;re on chain.
        </p>
      ) : (
        <ul className="divide-y divide-border overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-card)]">
          <AnimatePresence initial={false}>
            {items.map((it) => (
              <m.li
                key={it.key}
                layout="position"
                initial={{ opacity: 0, y: -8, backgroundColor: "rgba(244,183,40,0.08)" }}
                animate={{ opacity: 1, y: 0, backgroundColor: "rgba(244,183,40,0)" }}
                transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
                className="flex flex-wrap items-center gap-3 p-4"
              >
                <span
                  aria-hidden="true"
                  className={`flex size-9 shrink-0 items-center justify-center rounded-full ${
                    it.kind === "in" ? "bg-success/15 text-success" : "bg-surface-2 text-text"
                  }`}
                >
                  {it.kind === "in" ? <ArrowDownLeft size={16} /> : <ArrowUpRight size={16} />}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium">
                    {it.href ? (
                      <Link href={it.href} className="hover:text-primary">
                        {it.title}
                      </Link>
                    ) : (
                      it.title
                    )}
                  </p>
                  <p className="text-[13px] text-muted">
                    <TxStatus status={it.status} height={it.height} />
                    {it.detail ? ` · ${it.detail}` : null}
                  </p>
                </div>
                <div className="text-right">
                  <p className={`num ${it.kind === "in" ? "text-success" : ""}`}>
                    {it.kind === "in" ? "+" : "−"}
                    {zec(it.amount, network)}
                  </p>
                  <HashChip value={it.txid} label="transaction id" head={6} tail={4} />
                </div>
              </m.li>
            ))}
          </AnimatePresence>
        </ul>
      )}
    </section>
  );
}

export function TxStatus({
  status,
  height,
}: {
  status: "pending" | "confirmed" | "expired";
  height: number | null;
}) {
  if (status === "confirmed" && height !== null)
    return <span>Confirmed in block {height.toLocaleString("en-US")}</span>;
  if (status === "expired")
    return (
      <span className="text-warning">
        Expired without confirming. Its funds are spendable again.
      </span>
    );
  return (
    <span className="inline-flex items-center gap-1">
      <Clock size={12} aria-hidden="true" /> Sent, waiting for a block
    </span>
  );
}

function SecurityPanel() {
  const wallet = useWallet();
  const id = useId();
  const [mode, setMode] = useState<"idle" | "reveal" | "remove">("idle");
  const [passphrase, setPassphrase] = useState("");
  const [phrase, setPhrase] = useState<string | null>(null);
  const [confirmText, setConfirmText] = useState("");
  const [error, setError] = useState<string | null>(null);

  return (
    <section
      aria-labelledby="sec-h"
      className="space-y-4 rounded-[var(--radius-card)] border border-border p-5"
    >
      <h2 id="sec-h" className="font-medium">
        Backup and security
      </h2>
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="secondary"
          onClick={() => {
            setMode("reveal");
            setPhrase(null);
            setError(null);
          }}
        >
          <Repeat size={16} aria-hidden="true" /> Show recovery phrase
        </Button>
        <Button type="button" variant="secondary" onClick={() => wallet.lock()}>
          <Lock size={16} aria-hidden="true" /> Lock
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => {
            setMode("remove");
            setError(null);
          }}
        >
          <Trash2 size={16} aria-hidden="true" /> Remove from this browser
        </Button>
      </div>

      {mode === "reveal" && phrase === null ? (
        <form
          className="space-y-3"
          onSubmit={async (e) => {
            e.preventDefault();
            try {
              setPhrase(await wallet.revealPhrase(passphrase));
              setPassphrase("");
            } catch (err) {
              setError((err as Error).message);
            }
          }}
        >
          <label htmlFor={id} className="block text-sm font-medium">
            Type your passphrase to show the phrase
          </label>
          <input
            id={id}
            type="password"
            autoComplete="current-password"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            className="h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
          />
          {error ? (
            <p role="alert" className="text-sm text-danger">
              {error}
            </p>
          ) : null}
          <Button type="submit">Show</Button>
        </form>
      ) : null}
      {mode === "reveal" && phrase ? (
        <div className="space-y-3">
          <ol className="grid grid-cols-2 gap-2 rounded-[var(--radius-control)] bg-surface-2 p-4 sm:grid-cols-3">
            {phrase.split(" ").map((w, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a phrase word is identified by its position
              <li key={`${i}-${w}`} className="num flex gap-2 text-[14px]">
                <span className="w-6 text-right text-muted">{i + 1}</span>
                {w}
              </li>
            ))}
          </ol>
          <Button
            type="button"
            variant="secondary"
            onClick={() => {
              setPhrase(null);
              setMode("idle");
            }}
          >
            Hide
          </Button>
        </div>
      ) : null}

      {mode === "remove" ? (
        <form
          className="space-y-3 rounded-[var(--radius-control)] border border-danger/50 p-4"
          onSubmit={async (e) => {
            e.preventDefault();
            if (confirmText !== "remove") return setError('Type "remove" to confirm.');
            await wallet.remove();
          }}
        >
          <p className="text-sm">
            This deletes the encrypted wallet from this browser. Funds stay on chain, and you can
            only get them back with your 24-word recovery phrase. Pending refunds will still arrive
            at your bids&rsquo; refund addresses, which belong to that phrase.
          </p>
          <label htmlFor={`${id}-rm`} className="block text-sm font-medium">
            Type &ldquo;remove&rdquo; to confirm
          </label>
          <input
            id={`${id}-rm`}
            value={confirmText}
            onChange={(e) => setConfirmText(e.target.value)}
            className="h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
          />
          {error ? (
            <p role="alert" className="text-sm text-danger">
              {error}
            </p>
          ) : null}
          <div className="flex gap-2">
            <Button type="submit" variant="secondary">
              Remove wallet
            </Button>
            <Button type="button" variant="ghost" onClick={() => setMode("idle")}>
              Cancel
            </Button>
          </div>
        </form>
      ) : null}
    </section>
  );
}
