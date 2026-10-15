"use client";

import { paymentUri } from "@sealforge/bid";
import { Check, Circle, ExternalLink, LoaderCircle } from "lucide-react";
import Link from "next/link";
import QRCode from "qrcode";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { Button, ButtonLink, buttonClass } from "@/components/button";
import { HashChip } from "@/components/hash-chip";
import { StatusPill } from "@/components/status-pill";
import { modeLabel, zec } from "@/lib/format";
import { ApiError, api, type StoredSigner } from "@/lib/sfsig";
import type {
  AuctionView,
  ChainView,
  ConsoleBatch,
  RefundProgress,
  TrusteeView,
} from "@/lib/types";
import { usePolling, useSigner } from "./use-signer";

const BLOCK_SECS = 75;

function eta(blocks: number): string {
  if (blocks <= 0) return "now";
  const mins = Math.round((blocks * BLOCK_SECS) / 60);
  if (mins < 60) return `~${mins} min`;
  const h = Math.floor(mins / 60);
  return `~${h} h ${mins % 60} min`;
}

const card = "rounded-[var(--radius-card)] border border-border bg-surface p-5";
const h2 = "text-[18px] leading-6 font-semibold";

/** S15: one auction, its single next action, trustees, refunds and payout. */
export function ConsoleAuction({ code }: { code: string }) {
  const { state } = useSigner("organiser");
  const signer = state.kind === "ready" ? state.signer : null;
  const [auction, setAuction] = useState<AuctionView | null | undefined>(undefined);
  const [tip, setTip] = useState<number | null>(null);
  const [batches, setBatches] = useState<ConsoleBatch[]>([]);
  const [progress, setProgress] = useState<RefundProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!signer) return;
    let a: AuctionView;
    try {
      a = await api<AuctionView>("GET", `/v1/console/auctions/${code}`, signer);
      setAuction(a);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) setAuction(null);
      else setError(e instanceof Error ? e.message : "Something went wrong.");
      return;
    }
    try {
      setTip((await api<ChainView>("GET", "/v1/chain", null)).tip);
      if (["published", "refunding", "settled"].includes(a.status)) {
        const [b, p] = await Promise.all([
          api<ConsoleBatch[]>("GET", `/v1/console/auctions/${code}/batches`, signer),
          api<RefundProgress>("GET", `/v1/auctions/${code}/refunds`, null),
        ]);
        setBatches(b);
        setProgress(p);
      }
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Something went wrong.");
    }
  }, [code, signer]);
  usePolling(load, 10_000, signer !== null);

  if (state.kind === "loading" || (signer && auction === undefined && !error))
    return <p className="text-muted">Loading…</p>;
  if (state.kind === "none")
    return (
      <p className="text-muted">
        Load your organiser key on the{" "}
        <Link href="/console" className="text-text underline">
          console
        </Link>{" "}
        first.
      </p>
    );
  if (auction === null)
    return (
      <p className="text-muted">
        No auction {code} belongs to this organiser key.{" "}
        <Link href="/console" className="text-text underline">
          Back to your auctions
        </Link>
      </p>
    );
  if (!auction)
    return (
      <p role="alert" className={card}>
        {error}
      </p>
    );

  return (
    <div className="space-y-8">
      <header className="space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <StatusPill status={auction.status} />
          <span className="num text-sm text-muted">
            {auction.code} · {auction.network}
          </span>
        </div>
        <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">{auction.title}</h1>
        <p className="text-sm text-muted">
          {modeLabel(auction.mode)} · {auction.quantity} {auction.quantity === 1 ? "lot" : "lots"} ·
          reserve {zec(auction.reserve_zat)} · closes at block{" "}
          <span className="num">{auction.close_height.toLocaleString("en-US")}</span>
          {tip !== null ? (
            <>
              {" "}
              (now <span className="num">{tip.toLocaleString("en-US")}</span>)
            </>
          ) : null}
        </p>
      </header>

      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}

      <NextAction auction={auction} tip={tip} signer={signer as StoredSigner} onChange={load} />

      <Trustees trustees={auction.trustees} />

      {auction.escrow_ua ? <Escrow auction={auction} /> : null}

      {progress ? (
        <Refunds progress={progress} batches={batches.filter((b) => b.kind === "refund")} />
      ) : null}

      {auction.owner ? (
        <section aria-labelledby="payout" className="space-y-3">
          <h2 id="payout" className={h2}>
            Payout
          </h2>
          <div className={`${card} space-y-3 text-sm`}>
            <div className="flex flex-wrap items-center justify-between gap-3">
              <span className="text-muted">Payout address</span>
              <HashChip value={auction.owner.payout_ua} label="payout address" />
            </div>
            <PayoutStatus auction={auction} batches={batches} />
          </div>
        </section>
      ) : null}
    </div>
  );
}

function NextAction({
  auction,
  tip,
  signer,
  onChange,
}: {
  auction: AuctionView;
  tip: number | null;
  signer: StoredSigner;
  onChange: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<"open" | "publish" | "cancel" | null>(null);

  async function act(action: "open" | "publish" | "cancel") {
    setBusy(true);
    setError(null);
    try {
      await api("POST", `/v1/auctions/${auction.code}/${action}`, signer);
      setConfirming(null);
      await onChange();
    } catch (e) {
      setError(e instanceof Error ? e.message : "That didn't work.");
    } finally {
      setBusy(false);
    }
  }

  const done = auction.trustees.filter((t) => t.dkg_status === "complete").length;
  const blocksLeft = tip !== null ? auction.close_height - tip : null;

  let body: ReactNode;
  let primary: { label: string; action: "open" | "publish"; note: string } | null = null;
  const cancellable = ["draft", "dkg", "ready"].includes(auction.status);

  switch (auction.status) {
    case "draft":
    case "dkg":
      body = (
        <>
          <p className="font-medium">Waiting for the trustees to set up the escrow</p>
          <p className="text-sm text-muted">
            {done} of 3 trustees finished key generation. Each runs{" "}
            <code className="num text-text">sealforge trustee dkg {auction.code} --wait</code>.
          </p>
        </>
      );
      break;
    case "ready":
      body = (
        <>
          <p className="font-medium">The escrow is ready</p>
          <p className="text-sm text-muted">
            Opening publishes the escrow address and starts accepting bids until block{" "}
            {auction.close_height.toLocaleString("en-US")}
            {blocksLeft !== null ? ` (${eta(blocksLeft)})` : ""}. The rules can't change after this.
          </p>
        </>
      );
      primary = {
        label: "Open for bids",
        action: "open",
        note: "Open now? Bidders can pay straight away, and the rules are fixed from here.",
      };
      break;
    case "open":
      body = (
        <>
          <p className="font-medium">Bidding is open</p>
          <p className="text-sm text-muted">
            Closes automatically at block {auction.close_height.toLocaleString("en-US")}
            {blocksLeft !== null ? ` (${eta(blocksLeft)})` : ""}
            {auction.bid_mode === "commit_reveal" && auction.reveal_close_height
              ? `. Bids are sealed: bidders reveal them until block ${auction.reveal_close_height.toLocaleString("en-US")}, then the result is computed after 10 confirmations.`
              : ", then waits 10 confirmations before computing the result."}
          </p>
        </>
      );
      break;
    case "closed":
      body = (
        <>
          <p className="font-medium">The result is computed</p>
          <p className="text-sm text-muted">
            Publishing makes the clearing price, the bid set and the viewing key public, so anyone
            can audit it. Refunds start right after.
          </p>
        </>
      );
      primary = {
        label: "Publish result",
        action: "publish",
        note: "Publish now? The result and the escrow's viewing key become public. This can't be undone.",
      };
      break;
    case "published":
    case "refunding":
      body = (
        <>
          <p className="font-medium">Refunds in progress</p>
          <p className="text-sm text-muted">
            Notes go out after random delays, batched with other bidders'. Trustees sign each batch
            with <code className="num text-text">sealforge trustee sign {auction.code}</code>, and
            the payout follows the last refund.
          </p>
        </>
      );
      break;
    case "settled":
      body = (
        <>
          <p className="font-medium">Settled</p>
          <p className="text-sm text-muted">All refunds and the payout are confirmed on chain.</p>
        </>
      );
      break;
    case "cancelled":
      body = <p className="font-medium">This auction was cancelled.</p>;
      break;
  }

  const published = ["published", "refunding", "settled"].includes(auction.status);

  return (
    <section aria-label="Next step" className={`${card} space-y-4`}>
      <div className="space-y-1">{body}</div>
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
      {confirming ? (
        <div className="space-y-3 rounded-[var(--radius-control)] border border-border bg-surface-2 p-4">
          <p className="text-sm">
            {confirming === "cancel" ? "Cancel this auction? It can't be reopened." : primary?.note}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              disabled={busy}
              onClick={() => act(confirming)}
              className={confirming === "cancel" ? "!bg-danger !text-white" : ""}
            >
              {busy ? "Working…" : confirming === "cancel" ? "Cancel auction" : primary?.label}
            </Button>
            <Button variant="secondary" disabled={busy} onClick={() => setConfirming(null)}>
              Back
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex flex-wrap gap-2">
          {primary ? (
            <Button onClick={() => setConfirming(primary.action)}>{primary.label}</Button>
          ) : null}
          {auction.status === "open" || published ? (
            <ButtonLink href={`/a/${auction.code}`} variant="secondary">
              Public page
            </ButtonLink>
          ) : null}
          {published ? (
            <ButtonLink href={`/audit/${auction.code}`} variant="secondary">
              Audit page
            </ButtonLink>
          ) : null}
          {cancellable ? (
            <Button variant="ghost" onClick={() => setConfirming("cancel")}>
              Cancel auction
            </Button>
          ) : null}
        </div>
      )}
    </section>
  );
}

const DKG_LABEL: Record<TrusteeView["dkg_status"], string> = {
  invited: "Invited",
  round1: "Round 1 sent",
  round2: "Round 2 sent",
  complete: "Key share saved",
};

function Trustees({ trustees }: { trustees: TrusteeView[] }) {
  return (
    <section aria-labelledby="trustees" className="space-y-3">
      <h2 id="trustees" className={h2}>
        Trustees
      </h2>
      <ul className="grid gap-3 sm:grid-cols-3">
        {trustees.map((t) => (
          <li key={t.slot} className={`${card} space-y-1`}>
            <p className="font-medium">
              <span className="num text-muted">{t.slot}</span> {t.label}
            </p>
            <p className="flex items-center gap-1.5 text-sm text-muted">
              {t.dkg_status === "complete" ? (
                <Check size={14} aria-hidden="true" className="text-success" />
              ) : (
                <Circle size={10} aria-hidden="true" />
              )}
              {DKG_LABEL[t.dkg_status]}
            </p>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Escrow({ auction }: { auction: AuctionView }) {
  const escrow = auction.escrow_ua as string;
  const buffer = auction.owner?.fee_buffer_zat ?? 0;
  const [qr, setQr] = useState<string | null>(null);
  const [showFund, setShowFund] = useState(false);
  const uri = paymentUri(escrow, 100_000n, `SF1FEE:${auction.code}`);

  useEffect(() => {
    if (!showFund) return;
    QRCode.toString(uri, { type: "svg", margin: 2, errorCorrectionLevel: "M" })
      .then((svg) => setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`))
      .catch(() => setQr(null));
  }, [showFund, uri]);

  const fundable = ["ready", "open", "closed"].includes(auction.status);

  return (
    <section aria-labelledby="escrow" className="space-y-3">
      <h2 id="escrow" className={h2}>
        Escrow
      </h2>
      <div className={`${card} space-y-3 text-sm`}>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="text-muted">2-of-3 escrow address</span>
          <HashChip value={escrow} label="escrow address" />
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="text-muted">Fee buffer</span>
          <span className="num">{zec(buffer)}</span>
        </div>
        <p className="text-muted">
          Refund and payout fees come out of the escrow and reduce your payout. A fee buffer is only
          needed if the auction might end with no winners.
        </p>
        {fundable ? (
          showFund ? (
            <div className="grid gap-4 sm:grid-cols-[200px_minmax(0,1fr)] sm:items-center">
              <div className="rounded-[var(--radius-control)] bg-white p-2">
                {qr ? (
                  // biome-ignore lint/performance/noImgElement: a local data-URI SVG
                  <img
                    src={qr}
                    alt="QR code to send 0.001 ZEC to the escrow as a fee buffer"
                    className="aspect-square w-full"
                  />
                ) : (
                  <div className="aspect-square" />
                )}
              </div>
              <div className="space-y-3">
                <p>
                  Send <span className="num">{zec(100_000)}</span> from a shielded wallet with memo{" "}
                  <code className="num">SF1FEE:{auction.code}</code>.
                </p>
                <a href={uri} className={buttonClass({ variant: "secondary" })}>
                  <ExternalLink size={16} aria-hidden="true" /> Open in wallet
                </a>
              </div>
            </div>
          ) : (
            <Button variant="secondary" onClick={() => setShowFund(true)}>
              Fund fee buffer
            </Button>
          )
        ) : null}
      </div>
    </section>
  );
}

const BATCH_LABEL: Record<ConsoleBatch["status"], string> = {
  building: "Preparing",
  awaiting_signatures: "Waiting for trustees",
  signed: "Signed, broadcasting",
  broadcast: "Sent, confirming",
  confirmed: "Confirmed",
  failed: "Failed",
};

function BatchStatusCell({ b }: { b: ConsoleBatch }) {
  const icon =
    b.status === "confirmed" ? (
      <Check size={14} aria-hidden="true" className="text-success" />
    ) : b.status === "failed" ? (
      <Circle size={10} aria-hidden="true" className="text-danger" />
    ) : (
      <LoaderCircle size={14} aria-hidden="true" className="text-warning" />
    );
  return (
    <span className="inline-flex items-center gap-1.5">
      {icon}
      {BATCH_LABEL[b.status]}
    </span>
  );
}

function Refunds({ progress, batches }: { progress: RefundProgress; batches: ConsoleBatch[] }) {
  const pct = progress.notes_total
    ? Math.round((progress.notes_sent / progress.notes_total) * 100)
    : 100;
  return (
    <section aria-labelledby="refunds" className="space-y-3">
      <h2 id="refunds" className={h2}>
        Refunds
      </h2>
      <div className={`${card} space-y-4`}>
        <div className="space-y-2" aria-live="polite">
          <div className="flex items-baseline justify-between text-sm">
            <span className="text-muted">Notes sent</span>
            <span className="num">
              {progress.notes_sent} of {progress.notes_total}
            </span>
          </div>
          <div
            className="h-2 overflow-hidden rounded-full bg-surface-2"
            role="progressbar"
            aria-valuenow={pct}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label="Refund notes sent"
          >
            <div
              className="h-full bg-primary transition-[width] duration-300"
              style={{ width: `${pct}%` }}
            />
          </div>
          {progress.needs_manual_review > 0 ? (
            <p className="text-sm text-warning">
              {progress.needs_manual_review} payment(s) had no readable refund address and need
              manual review.
            </p>
          ) : null}
        </div>
        {batches.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead className="text-muted">
                <tr>
                  <th scope="col" className="py-2 pr-4 font-medium">
                    Transaction
                  </th>
                  <th scope="col" className="py-2 pr-4 font-medium">
                    Status
                  </th>
                  <th scope="col" className="py-2 pr-4 text-right font-medium">
                    Amount
                  </th>
                  <th scope="col" className="py-2 font-medium">
                    Trustees
                  </th>
                </tr>
              </thead>
              <tbody>
                {batches.map((b) => (
                  <tr key={b.id} className="border-t border-border align-top">
                    <td className="py-2.5 pr-4">
                      <span className="capitalize">{b.kind}</span>
                      {b.output_count !== null ? (
                        <span className="text-muted"> · {b.output_count} out</span>
                      ) : null}
                      {b.txid ? (
                        <div className="mt-1">
                          <HashChip value={b.txid} label="transaction id" />
                        </div>
                      ) : null}
                    </td>
                    <td className="py-2.5 pr-4">
                      <BatchStatusCell b={b} />
                      {b.confirmed_height ? (
                        <div className="num text-[13px] text-muted">
                          block {b.confirmed_height.toLocaleString("en-US")}
                        </div>
                      ) : null}
                    </td>
                    <td className="num py-2.5 pr-4 text-right">
                      {b.total_out_zat !== null ? zec(b.total_out_zat) : "…"}
                    </td>
                    <td className="py-2.5">{Math.min(b.signed, 2)} of 2 signed</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="text-sm text-muted">
            No refund transactions yet. Notes are held until several bidders' refunds can go out
            together.
          </p>
        )}
      </div>
    </section>
  );
}

function PayoutStatus({ auction, batches }: { auction: AuctionView; batches: ConsoleBatch[] }) {
  const payout = batches.find((b) => b.kind === "payout");
  if (!payout)
    return (
      <p className="text-muted">
        After the last refund confirms, everything left in the escrow (winners' payments, any fee
        buffer, minus fees) is sent here.
      </p>
    );
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p>
        {auction.status === "settled" && payout.confirmed_height ? (
          <>
            Paid <span className="num">{zec(payout.total_out_zat ?? 0)}</span> at block{" "}
            <span className="num">{payout.confirmed_height.toLocaleString("en-US")}</span>
          </>
        ) : (
          <>
            <span className="num">{zec(payout.total_out_zat ?? 0)}</span> ·{" "}
            {BATCH_LABEL[payout.status]} · {Math.min(payout.signed, 2)} of 2 signed
          </>
        )}
      </p>
      {payout.txid ? <HashChip value={payout.txid} label="payout transaction id" /> : null}
    </div>
  );
}
