"use client";

import { formatZec, type Receipt } from "@sealforge/bid";
import {
  CircleAlert,
  CircleCheck,
  FileJson,
  LoaderCircle,
  ShieldCheck,
  Upload,
} from "lucide-react";
import { AnimatePresence, m } from "motion/react";
import Link from "next/link";
import { useEffect, useId, useState } from "react";
import { HashChip } from "@/components/hash-chip";
import { Guilloche } from "@/components/motion/guilloche";
import { SealMark } from "@/components/seal-mark";
import { checkReceipt } from "@/lib/auditor";
import { networkOfUa, unit, zec } from "@/lib/format";
import { listReceipts, type StoredReceipt } from "@/lib/receipts";
import type { PublishedResult, ReceiptCheck } from "@/lib/types";

/** A receipt from this device or a downloaded file. Extra local fields are ignored by the auditor. */
type Loaded = Receipt & { title?: string };

type State =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "unpublished"; code: string }
  | { kind: "error"; message: string }
  | { kind: "done"; check: ReceiptCheck; result: PublishedResult; receipt: Loaded };

export function VerifyReceipt() {
  const fileId = useId();
  const [saved, setSaved] = useState<StoredReceipt[]>([]);
  const [state, setState] = useState<State>({ kind: "idle" });
  const [dragging, setDragging] = useState(false);
  const [checkingId, setCheckingId] = useState<string | null>(null);

  useEffect(() => {
    listReceipts()
      .then(setSaved)
      .catch(() => setSaved([]));
  }, []);

  async function verify(receipt: Loaded) {
    setCheckingId(receipt.commitment);
    setState({ kind: "checking" });
    try {
      const res = await fetch(`/api/auctions/${encodeURIComponent(receipt.auction_code)}/result`);
      if (res.status === 404) return setState({ kind: "unpublished", code: receipt.auction_code });
      if (!res.ok)
        return setState({
          kind: "error",
          message: "The result couldn't be loaded. Try again shortly.",
        });
      const result = (await res.json()) as PublishedResult;
      const check = await checkReceipt(result, receipt);
      setState({ kind: "done", check, result, receipt });
    } catch {
      setState({ kind: "error", message: "Verification failed to run in this browser." });
    } finally {
      setCheckingId(null);
    }
  }

  async function fromFile(file: File) {
    try {
      const parsed = JSON.parse(await file.text()) as Loaded;
      if (typeof parsed.auction_code !== "string" || typeof parsed.commitment !== "string")
        throw new Error();
      await verify(parsed);
    } catch {
      setState({ kind: "error", message: "That file isn't a sealForge receipt." });
    }
  }

  return (
    <div className="space-y-8">
      <section className="space-y-3">
        {saved.length > 0 ? (
          <>
            <h2 className="eyebrow">Receipts on this device</h2>
            <ul className="divide-y divide-border overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-card)]">
              {saved.map((r, i) => (
                <m.li
                  key={r.commitment}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1], delay: 0.2 + i * 0.06 }}
                  className="group flex items-center justify-between gap-3 px-4 py-3.5 transition-colors duration-150 hover:bg-surface-2"
                >
                  <span className="flex min-w-0 items-center gap-3">
                    <span className="inline-flex size-9 shrink-0 items-center justify-center rounded-full border border-border bg-bg text-muted transition-colors duration-150 group-hover:text-primary">
                      <FileJson size={16} aria-hidden="true" />
                    </span>
                    <span className="min-w-0">
                      <span className="block truncate font-medium">{r.title}</span>
                      <span className="num text-[13px] text-muted">
                        {zec(r.amount_zat, networkOfUa(r.refund_ua))} · {r.auction_code}
                      </span>
                    </span>
                  </span>
                  <button
                    type="button"
                    onClick={() => verify(r)}
                    disabled={checkingId !== null}
                    className="inline-flex h-10 shrink-0 items-center gap-2 rounded-[var(--radius-control)] border border-border px-4 text-sm transition-[border-color,background-color] duration-150 hover:border-primary disabled:opacity-60"
                  >
                    {checkingId === r.commitment ? (
                      <LoaderCircle size={15} className="animate-spin" aria-hidden="true" />
                    ) : (
                      <ShieldCheck size={15} aria-hidden="true" />
                    )}
                    Check
                  </button>
                </m.li>
              ))}
            </ul>
          </>
        ) : null}
        <label
          htmlFor={fileId}
          onDragOver={(e) => {
            e.preventDefault();
            setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDragging(false);
            const f = e.dataTransfer.files?.[0];
            if (f) void fromFile(f);
          }}
          className={`group flex cursor-pointer items-center gap-4 rounded-[var(--radius-card)] border border-dashed p-5 text-sm transition-[border-color,background-color] duration-200 focus-within:border-primary hover:border-muted ${
            dragging ? "border-primary bg-primary/5" : "border-border"
          }`}
        >
          <m.span
            animate={dragging ? { y: -3, scale: 1.06 } : { y: 0, scale: 1 }}
            transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
            className={`inline-flex size-11 shrink-0 items-center justify-center rounded-full border transition-colors duration-200 ${
              dragging
                ? "border-primary text-primary"
                : "border-border text-muted group-hover:text-text"
            }`}
          >
            <Upload size={18} aria-hidden="true" />
          </m.span>
          <span>
            <span className="font-medium">
              {dragging ? "Drop it here" : "Load or drop a receipt file"}
            </span>
            <span className="block text-muted">
              sealforge-receipt-….json. It&rsquo;s read in your browser, not uploaded.
            </span>
          </span>
          <input
            id={fileId}
            type="file"
            accept="application/json,.json"
            className="sr-only"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void fromFile(f);
              e.target.value = "";
            }}
          />
        </label>
      </section>

      <section aria-live="polite">
        <AnimatePresence>
          {state.kind === "checking" ? (
            <m.div
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="space-y-3 rounded-[var(--radius-card)] border border-border bg-surface p-5"
            >
              <p className="flex items-center gap-2 text-sm">
                <LoaderCircle size={16} className="animate-spin text-primary" aria-hidden="true" />
                Checking your receipt against the published result…
              </p>
              <div className="relative h-px overflow-hidden bg-border">
                <span className="scan-bar absolute inset-y-0 w-1/3 bg-primary" />
              </div>
            </m.div>
          ) : null}
        </AnimatePresence>
        {state.kind === "error" ? (
          <p role="alert" className="text-danger">
            {state.message}
          </p>
        ) : null}
        {state.kind === "unpublished" ? (
          <p className="text-muted">
            The result for auction <span className="num">{state.code}</span> isn&rsquo;t published
            yet. It appears after bidding closes.
          </p>
        ) : null}
        {state.kind === "done" ? <Outcome {...state} /> : null}
      </section>
    </div>
  );
}

function Outcome({
  check,
  result,
  receipt,
}: {
  check: ReceiptCheck;
  result: PublishedResult;
  receipt: Loaded;
}) {
  if (!check.receipt_valid) {
    return (
      <Problem title="This receipt doesn't check out">
        Its salt and refund address don&rsquo;t reproduce its commitment, or it belongs to a
        different auction. Use the receipt file you downloaded when you bid.
      </Problem>
    );
  }
  if (!check.found) {
    return (
      <Problem title="This bid isn't in the published result">
        If you paid, the payment either arrived outside the bidding window, reused a commitment, or
        had an unreadable memo. Payments like that are refunded in full to the refund address in
        your memo. If you never sent the payment, there&rsquo;s nothing to refund.
        {receipt.price_zat !== undefined
          ? " This was a sealed bid: if it wasn't revealed within the reveal window, it didn't count and the deposit is refunded in full."
          : null}
      </Problem>
    );
  }
  return (
    <m.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      className="relative space-y-6 overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface p-6 shadow-[var(--shadow-lift)]"
    >
      <div className="flex items-center gap-4">
        {check.included ? (
          <span className="relative inline-flex size-16 shrink-0 items-center justify-center">
            <Guilloche size={96} className="absolute -inset-4 opacity-70" />
            <SealMark size={56} stamp />
          </span>
        ) : (
          <CircleAlert size={40} className="text-danger" aria-hidden="true" />
        )}
        <div>
          <h2 className="text-[22px] leading-7 font-semibold">
            {check.included ? "Your bid is included" : "Inclusion proof failed"}
          </h2>
          <p className="text-sm text-muted">
            {check.included
              ? "Its Merkle proof matches the published root, checked in your browser."
              : "The published root doesn't match the bid list. Report this to the organiser and auditors."}
          </p>
        </div>
      </div>

      <dl className="grid gap-4 sm:grid-cols-2">
        <div>
          <dt className="text-sm text-muted">Outcome</dt>
          <dd className="text-[18px] font-semibold">
            {check.is_winner ? (
              <span className="inline-flex items-center gap-2">
                <CircleCheck size={18} className="text-success" aria-hidden="true" /> Won at{" "}
                {zec(check.price_paid_zat, result.network)}
              </span>
            ) : (
              "Not a winner"
            )}
          </dd>
        </div>
        <div>
          <dt className="text-sm text-muted">Refund due</dt>
          <dd className="num text-[18px]">{zec(check.refund_due_zat, result.network)}</dd>
        </div>
      </dl>

      {check.refund_notes_zat.length > 0 ? (
        <div className="space-y-2 text-sm">
          <p className="text-muted">
            It arrives as {check.refund_notes_zat.length} separate note
            {check.refund_notes_zat.length === 1 ? "" : "s"} over a few hours, so the pieces
            can&rsquo;t be matched to your bid:
          </p>
          <ul className="flex flex-wrap gap-2">
            {check.refund_notes_zat.map((n, i) => (
              <m.li
                // biome-ignore lint/suspicious/noArrayIndexKey: notes can repeat; order is stable
                key={i}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.3 + i * 0.06 }}
                className="num rounded-[var(--radius-chip)] bg-surface-2 px-2.5 py-1"
              >
                {formatZec(BigInt(n))}
              </m.li>
            ))}
          </ul>
        </div>
      ) : null}

      {check.dust_zat > 0 ? (
        <p className="text-sm text-muted">
          Plus <span className="num">{check.dust_zat.toLocaleString("en-US")}</span> zatoshi below
          the smallest note (0.0001 {unit(result.network)}). It isn&rsquo;t sent: it costs more in
          network fees than it&rsquo;s worth, and an odd-sized note would make your refund
          traceable.
        </p>
      ) : null}

      {!check.amount_matches && check.amount_zat !== null ? (
        <p className="text-sm text-warning">
          The chain shows {zec(check.amount_zat, result.network)}, but your receipt says{" "}
          {zec(receipt.amount_zat, result.network)}. The on-chain amount is what counted.
        </p>
      ) : null}

      <dl className="space-y-2 border-t border-border pt-4 text-sm">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <dt className="text-muted">Published root</dt>
          <dd>
            <HashChip value={result.merkle_root} label="Merkle root" />
          </dd>
        </div>
        {check.block_height !== null ? (
          <div className="flex justify-between gap-2">
            <dt className="text-muted">Confirmed in block</dt>
            <dd className="num">{check.block_height.toLocaleString("en-US")}</dd>
          </div>
        ) : null}
      </dl>
      <Link
        href={`/audit/${result.auction_code}`}
        className="inline-block text-sm text-primary hover:underline"
      >
        Audit the whole auction
      </Link>
    </m.div>
  );
}

function Problem({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <m.div
      role="alert"
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-2 rounded-[var(--radius-card)] border border-border bg-surface p-6"
    >
      <h2 className="inline-flex items-center gap-2 text-[18px] font-semibold">
        <CircleAlert size={20} className="text-warning" aria-hidden="true" /> {title}
      </h2>
      <p className="text-sm text-muted">{children}</p>
    </m.div>
  );
}
