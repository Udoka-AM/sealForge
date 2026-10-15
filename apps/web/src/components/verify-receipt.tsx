"use client";

import { formatZec, type Receipt } from "@sealforge/bid";
import { CircleAlert, CircleCheck, Upload } from "lucide-react";
import Link from "next/link";
import { useEffect, useId, useState } from "react";
import { HashChip } from "@/components/hash-chip";
import { SealMark } from "@/components/seal-mark";
import { checkReceipt } from "@/lib/auditor";
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

const zec = (zat: number) => `${formatZec(BigInt(zat))} ZEC`;

export function VerifyReceipt() {
  const fileId = useId();
  const [saved, setSaved] = useState<StoredReceipt[]>([]);
  const [state, setState] = useState<State>({ kind: "idle" });

  useEffect(() => {
    listReceipts()
      .then(setSaved)
      .catch(() => setSaved([]));
  }, []);

  async function verify(receipt: Loaded) {
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
            <h2 className="text-[18px] leading-6 font-semibold">Receipts on this device</h2>
            <ul className="divide-y divide-border border-y border-border">
              {saved.map((r) => (
                <li key={r.commitment} className="flex items-center justify-between gap-3 py-3">
                  <span className="min-w-0">
                    <span className="block truncate">{r.title}</span>
                    <span className="num text-sm text-muted">{zec(r.amount_zat)}</span>
                  </span>
                  <button
                    type="button"
                    onClick={() => verify(r)}
                    className="h-10 shrink-0 rounded-[var(--radius-control)] border border-border px-4 text-sm transition-[border-color] duration-150 hover:border-primary"
                  >
                    Check
                  </button>
                </li>
              ))}
            </ul>
          </>
        ) : null}
        <label
          htmlFor={fileId}
          className="flex cursor-pointer items-center gap-3 rounded-[var(--radius-card)] border border-dashed border-border p-5 text-sm transition-[border-color] duration-150 hover:border-muted focus-within:border-primary"
        >
          <Upload size={18} aria-hidden="true" className="text-muted" />
          <span>
            <span className="font-medium">Load a receipt file</span>
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
        {state.kind === "checking" ? (
          <p className="text-muted">Checking inclusion against the published result…</p>
        ) : null}
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
      </Problem>
    );
  }
  return (
    <div className="space-y-6 rounded-[var(--radius-card)] border border-border bg-surface p-6">
      <div className="flex items-center gap-4">
        {check.included ? (
          <SealMark size={56} stamp />
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
                {zec(check.price_paid_zat)}
              </span>
            ) : (
              "Not a winner"
            )}
          </dd>
        </div>
        <div>
          <dt className="text-sm text-muted">Refund due</dt>
          <dd className="num text-[18px]">{zec(check.refund_due_zat)}</dd>
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
              // biome-ignore lint/suspicious/noArrayIndexKey: notes can repeat; order is stable
              <li key={i} className="num rounded-[var(--radius-chip)] bg-surface-2 px-2.5 py-1">
                {formatZec(BigInt(n))}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {check.dust_zat > 0 ? (
        <p className="text-sm text-muted">
          Plus <span className="num">{check.dust_zat.toLocaleString("en-US")}</span> zatoshi below
          the smallest note (0.0001 ZEC). It isn&rsquo;t sent: it costs more in network fees than
          it&rsquo;s worth, and an odd-sized note would make your refund traceable.
        </p>
      ) : null}

      {!check.amount_matches && check.amount_zat !== null ? (
        <p className="text-sm text-warning">
          The chain shows {zec(check.amount_zat)}, but your receipt says {zec(receipt.amount_zat)}.
          The on-chain amount is what counted.
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
    </div>
  );
}

function Problem({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div
      role="alert"
      className="space-y-2 rounded-[var(--radius-card)] border border-border bg-surface p-6"
    >
      <h2 className="inline-flex items-center gap-2 text-[18px] font-semibold">
        <CircleAlert size={20} className="text-warning" aria-hidden="true" /> {title}
      </h2>
      <p className="text-sm text-muted">{children}</p>
    </div>
  );
}
