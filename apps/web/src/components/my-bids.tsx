"use client";

import { formatZec } from "@sealforge/bid";
import { Download } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { buttonClass } from "@/components/button";
import { downloadReceipt, listReceipts, markBackedUp, type StoredReceipt } from "@/lib/receipts";
import { RevealPanel } from "./reveal-panel";

export function MyBids() {
  const [receipts, setReceipts] = useState<StoredReceipt[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    listReceipts()
      .then(setReceipts)
      .catch(() => setFailed(true));
  }, []);

  if (failed) {
    return (
      <p role="alert" className="text-muted">
        This browser can&rsquo;t open saved receipts (private mode or blocked storage).
      </p>
    );
  }
  if (receipts === null) {
    return (
      <div
        aria-busy="true"
        className="h-24 animate-pulse rounded-[var(--radius-card)] bg-surface"
      />
    );
  }
  if (receipts.length === 0) {
    return (
      <div className="space-y-3 rounded-[var(--radius-card)] border border-border bg-surface p-6">
        <p>Your sealed bids will appear here. They&rsquo;re stored only on this device.</p>
        <Link href="/" className={buttonClass({ variant: "secondary" })}>
          Find an auction
        </Link>
      </div>
    );
  }

  return (
    <ul className="divide-y divide-border border-y border-border">
      {receipts.map((r) => (
        <li key={r.commitment} className="flex flex-wrap items-center justify-between gap-3 py-4">
          <div className="min-w-0 space-y-1">
            <Link
              href={`/a/${r.auction_code}`}
              className="block truncate font-medium hover:text-primary"
            >
              {r.title}
            </Link>
            <p className="text-sm text-muted">
              {r.price_zat !== undefined ? (
                <>
                  Bid <span className="num">{formatZec(BigInt(r.price_zat))} ZEC</span> (sealed),
                  deposit{" "}
                </>
              ) : null}
              <span className="num">{formatZec(BigInt(r.amount_zat))} ZEC</span> ·{" "}
              {new Date(r.created_at).toLocaleDateString(undefined, { dateStyle: "medium" })}
              {r.backed_up ? null : <span className="text-warning"> · Not backed up</span>}
            </p>
          </div>
          <button
            type="button"
            className={buttonClass({ variant: "secondary" })}
            onClick={async () => {
              downloadReceipt(r);
              await markBackedUp(r.commitment).catch(() => {});
              setReceipts(
                (all) =>
                  all?.map((x) =>
                    x.commitment === r.commitment ? { ...x, backed_up: true } : x,
                  ) ?? null,
              );
            }}
          >
            <Download size={16} aria-hidden="true" /> Receipt
          </button>
          {r.price_zat !== undefined ? (
            <RevealPanel
              receipt={r}
              onSent={() =>
                setReceipts(
                  (all) =>
                    all?.map((x) =>
                      x.commitment === r.commitment ? { ...x, reveal_sent: true } : x,
                    ) ?? null,
                )
              }
            />
          ) : null}
        </li>
      ))}
    </ul>
  );
}
