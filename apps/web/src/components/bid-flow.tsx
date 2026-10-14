"use client";

import {
  formatZec,
  isCanonical,
  looksLikeUnifiedAddress,
  type PreparedBid,
  parseZec,
  prepareBid,
  roundingOptions,
} from "@sealforge/bid";
import { ArrowLeft, Download, ExternalLink } from "lucide-react";
import Link from "next/link";
import QRCode from "qrcode";
import { useEffect, useId, useRef, useState } from "react";
import { Button, buttonClass } from "@/components/button";
import { HashChip } from "@/components/hash-chip";
import { SealMark } from "@/components/seal-mark";
import { downloadReceipt, markBackedUp, type StoredReceipt, saveReceipt } from "@/lib/receipts";
import type { AuctionView } from "@/lib/types";

type Step = "amount" | "pay" | "sealed";

export function BidFlow({ auction }: { auction: AuctionView }) {
  const [step, setStep] = useState<Step>("amount");
  const [bid, setBid] = useState<PreparedBid | null>(null);
  const [stored, setStored] = useState<StoredReceipt | null>(null);
  const [storageFailed, setStorageFailed] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);

  // Move focus to the step heading so keyboard and screen reader users follow along.
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs on purpose whenever the step changes
  useEffect(() => {
    heading.current?.focus();
  }, [step]);

  async function sealed(prepared: PreparedBid) {
    const receipt: StoredReceipt = {
      ...prepared.receipt,
      title: auction.title,
      created_at: new Date().toISOString(),
      backed_up: false,
    };
    try {
      await saveReceipt(receipt);
      setStorageFailed(false);
    } catch {
      setStorageFailed(true);
    }
    setBid(prepared);
    setStored(receipt);
    setStep("pay");
  }

  async function download() {
    if (!stored) return;
    downloadReceipt(stored);
    const updated = { ...stored, backed_up: true };
    setStored(updated);
    await markBackedUp(stored.commitment).catch(() => {});
  }

  return (
    <div className="mx-auto max-w-[480px] space-y-6">
      <Link
        href={`/a/${auction.code}`}
        className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-text"
      >
        <ArrowLeft size={16} aria-hidden="true" /> {auction.title}
      </Link>

      {step === "amount" ? (
        <AmountStep auction={auction} heading={heading} onSealed={sealed} />
      ) : null}

      {step === "pay" && bid && stored ? (
        <PayStep
          auction={auction}
          bid={bid}
          heading={heading}
          storageFailed={storageFailed}
          onDownload={download}
          onChangeAmount={() => setStep("amount")}
          onSent={() => setStep("sealed")}
        />
      ) : null}

      {step === "sealed" && stored ? (
        <SealedStep auction={auction} receipt={stored} heading={heading} onDownload={download} />
      ) : null}
    </div>
  );
}

type HeadingRef = React.RefObject<HTMLHeadingElement | null>;

function StepHeading({ heading, children }: { heading: HeadingRef; children: React.ReactNode }) {
  return (
    <h1
      ref={heading}
      tabIndex={-1}
      className="text-[28px] leading-[34px] font-semibold tracking-tight outline-none"
    >
      {children}
    </h1>
  );
}

function AmountStep({
  auction,
  heading,
  onSealed,
}: {
  auction: AuctionView;
  heading: HeadingRef;
  onSealed: (bid: PreparedBid) => void;
}) {
  const amountId = useId();
  const refundId = useId();
  const [amount, setAmount] = useState("");
  const [refund, setRefund] = useState("");
  const [errors, setErrors] = useState<{ amount?: string; refund?: string }>({});
  const reserve = BigInt(auction.reserve_zat);
  const parsed = parseZec(amount);
  const rounding =
    parsed !== null && parsed > 0n && !isCanonical(parsed) ? roundingOptions(parsed) : null;
  const suggestions = rounding
    ? [rounding.lower, rounding.upper].filter((d): d is bigint => d !== null && d >= reserve)
    : [];

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const next: typeof errors = {};
    if (parsed === null || parsed <= 0n)
      next.amount = "Enter an amount in ZEC, up to 8 decimal places.";
    else if (parsed < reserve) next.amount = `Below the ${formatZec(reserve)} ZEC reserve.`;
    const refundUa = refund.trim();
    if (!looksLikeUnifiedAddress(refundUa, auction.network))
      next.refund = `Paste a Unified Address for ${auction.network} (it starts with ${
        auction.network === "mainnet"
          ? "u1"
          : auction.network === "testnet"
            ? "utest1"
            : "uregtest1"
      }).`;
    setErrors(next);
    if (next.amount || next.refund || parsed === null || !auction.escrow_ua) return;
    onSealed(
      prepareBid({
        auctionCode: auction.code,
        escrowUa: auction.escrow_ua,
        amountZat: parsed,
        refundUa,
      }),
    );
  }

  return (
    <form onSubmit={submit} noValidate className="space-y-6">
      <StepHeading heading={heading}>Your sealed bid</StepHeading>

      <div className="space-y-2">
        <label htmlFor={amountId} className="block text-sm font-medium">
          Maximum bid
        </label>
        <div className="flex h-14 items-center rounded-[var(--radius-control)] border border-border bg-surface-2 focus-within:border-primary">
          <input
            id={amountId}
            inputMode="decimal"
            autoComplete="off"
            placeholder="0.00"
            value={amount}
            onChange={(e) => {
              setAmount(e.target.value);
              setErrors((x) => ({ ...x, amount: undefined }));
            }}
            aria-invalid={Boolean(errors.amount)}
            aria-describedby={`${amountId}-help`}
            className="num h-full w-full bg-transparent px-4 text-[24px] outline-none"
          />
          <span className="pr-4 text-muted">ZEC</span>
        </div>
        <p
          id={`${amountId}-help`}
          className={`text-sm ${errors.amount ? "text-danger" : "text-muted"}`}
        >
          {errors.amount ??
            `Reserve ${formatZec(reserve)} ZEC. You send this amount now; anything above the price you pay comes back.`}
        </p>
        {suggestions.length > 0 ? (
          <div className="flex flex-wrap items-center gap-2 pt-1">
            <span className="text-[13px] text-muted">
              Round to a common amount so it blends in:
            </span>
            {suggestions.map((d) => (
              <button
                key={d.toString()}
                type="button"
                onClick={() => {
                  setAmount(formatZec(d));
                  setErrors((x) => ({ ...x, amount: undefined }));
                }}
                className="num rounded-[var(--radius-chip)] border border-border px-2.5 py-1 text-[13px] transition-colors duration-150 hover:border-primary hover:text-primary"
              >
                {formatZec(d)} ZEC
              </button>
            ))}
          </div>
        ) : null}
      </div>

      <div className="space-y-2">
        <label htmlFor={refundId} className="block text-sm font-medium">
          Refund address
        </label>
        <textarea
          id={refundId}
          rows={3}
          spellCheck={false}
          autoComplete="off"
          value={refund}
          onChange={(e) => {
            setRefund(e.target.value);
            setErrors((x) => ({ ...x, refund: undefined }));
          }}
          aria-invalid={Boolean(errors.refund)}
          aria-describedby={`${refundId}-help`}
          className="num w-full resize-none rounded-[var(--radius-control)] border border-border bg-surface-2 p-3 text-[13px] break-all outline-none focus:border-primary"
        />
        <p
          id={`${refundId}-help`}
          className={`text-sm ${errors.refund ? "text-danger" : "text-muted"}`}
        >
          {errors.refund ??
            "A fresh Unified Address from your wallet, used only for this bid. Refunds and change arrive here."}
        </p>
      </div>

      <Button type="submit" size="lg" className="w-full">
        Continue
      </Button>
      <p className="text-[13px] text-muted">
        Your receipt is created and kept in this browser. Nothing you enter here is sent to our
        server.
      </p>
    </form>
  );
}

function PayStep({
  auction,
  bid,
  heading,
  storageFailed,
  onDownload,
  onChangeAmount,
  onSent,
}: {
  auction: AuctionView;
  bid: PreparedBid;
  heading: HeadingRef;
  storageFailed: boolean;
  onDownload: () => void;
  onChangeAmount: () => void;
  onSent: () => void;
}) {
  const [qr, setQr] = useState<string | null>(null);
  const [qrFailed, setQrFailed] = useState(false);

  useEffect(() => {
    QRCode.toString(bid.uri, { type: "svg", margin: 2, errorCorrectionLevel: "M" })
      .then((svg) => setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`))
      .catch(() => setQrFailed(true));
  }, [bid.uri]);

  return (
    <div className="space-y-6">
      <StepHeading heading={heading}>Send it from your wallet</StepHeading>

      {storageFailed ? (
        <p
          role="alert"
          className="rounded-[var(--radius-control)] border border-warning p-3 text-sm"
        >
          This browser can&rsquo;t keep your receipt (private mode?). Download it before you pay.
        </p>
      ) : null}

      <div className="rounded-[var(--radius-card)] bg-white p-4">
        {qr ? (
          // biome-ignore lint/performance/noImgElement: a local data-URI SVG; next/image adds nothing here
          <img
            src={qr}
            alt={`Payment QR code for ${formatZec(BigInt(bid.receipt.amount_zat))} ZEC with the bid memo`}
            className="mx-auto aspect-square w-full max-w-[280px]"
          />
        ) : (
          <div className="mx-auto flex aspect-square w-full max-w-[280px] items-center justify-center text-sm text-neutral-600">
            {qrFailed ? "QR code unavailable. Use the button below." : "Preparing QR code…"}
          </div>
        )}
      </div>

      <a href={bid.uri} className={`${buttonClass({ size: "lg" })} w-full`}>
        <ExternalLink size={18} aria-hidden="true" /> Open in wallet
      </a>

      <dl className="space-y-3 text-sm">
        <div className="flex items-baseline justify-between gap-4">
          <dt className="text-muted">Amount</dt>
          <dd className="num text-[18px]">{formatZec(BigInt(bid.receipt.amount_zat))} ZEC</dd>
        </div>
        <div className="space-y-1.5">
          <dt className="text-muted">Payment request</dt>
          <dd>
            <HashChip value={bid.uri} label="payment request" head={18} tail={8} />
          </dd>
        </div>
      </dl>

      <details className="text-sm">
        <summary className="cursor-pointer text-muted hover:text-text">
          What&rsquo;s in the memo?
        </summary>
        <p className="num mt-2 rounded-[var(--radius-control)] bg-surface-2 p-3 text-[12px] break-all">
          {bid.memo}
        </p>
        <p className="mt-2 text-muted">
          The auction code, your bid commitment and your refund address. It&rsquo;s encrypted on
          chain, and becomes readable to auditors only after close, without revealing who sent it.
        </p>
      </details>

      <p className="text-sm text-muted">
        Your bid counts once it confirms on chain at or before block{" "}
        <span className="num">{auction.close_height.toLocaleString("en-US")}</span>.
      </p>

      <div className="flex flex-col gap-3 sm:flex-row">
        <Button type="button" onClick={onSent} className="flex-1">
          I&rsquo;ve sent it
        </Button>
        <Button type="button" variant="secondary" onClick={onDownload} className="flex-1">
          <Download size={16} aria-hidden="true" /> Download receipt
        </Button>
      </div>
      <Button type="button" variant="ghost" onClick={onChangeAmount} className="w-full">
        Change amount
      </Button>
      <p className="text-[13px] text-muted">
        Changing the amount creates a new bid. If you already paid this one, it still counts
        separately.
      </p>
    </div>
  );
}

function SealedStep({
  auction,
  receipt,
  heading,
  onDownload,
}: {
  auction: AuctionView;
  receipt: StoredReceipt;
  heading: HeadingRef;
  onDownload: () => void;
}) {
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <SealMark size={64} stamp />
        <div>
          <StepHeading heading={heading}>Bid sealed</StepHeading>
          <p className="text-muted">The price will be public after close. You won&rsquo;t be.</p>
        </div>
      </div>

      <dl className="space-y-3 rounded-[var(--radius-card)] border border-border bg-surface p-5 text-sm">
        <div className="flex items-baseline justify-between gap-4">
          <dt className="text-muted">Auction</dt>
          <dd className="text-right">{auction.title}</dd>
        </div>
        <div className="flex items-baseline justify-between gap-4">
          <dt className="text-muted">Amount</dt>
          <dd className="num">{formatZec(BigInt(receipt.amount_zat))} ZEC</dd>
        </div>
        <div className="flex items-center justify-between gap-4">
          <dt className="text-muted">Commitment</dt>
          <dd>
            <HashChip value={receipt.commitment} label="commitment" />
          </dd>
        </div>
      </dl>

      {receipt.backed_up ? (
        <p className="text-sm text-success" role="status">
          Receipt downloaded.
        </p>
      ) : (
        <Button type="button" size="lg" onClick={onDownload} className="w-full">
          <Download size={18} aria-hidden="true" /> Download receipt
        </Button>
      )}

      <ol className="space-y-3 border-l border-border pl-5 text-sm">
        <li>
          <span className="font-medium">Bidding closes</span>
          <span className="text-muted">
            {" "}
            at block <span className="num">{auction.close_height.toLocaleString("en-US")}</span>.
          </span>
        </li>
        <li>
          <span className="font-medium">The result is published.</span>
          <span className="text-muted"> Your receipt lets you check your bid was counted.</span>
        </li>
        <li>
          <span className="font-medium">Refunds arrive in pieces.</span>
          <span className="text-muted">
            {" "}
            Several standard-sized notes over a few hours, on purpose, so they can&rsquo;t be
            matched to you.
          </span>
        </li>
      </ol>

      <Link href="/me" className={`${buttonClass({ variant: "secondary" })} w-full`}>
        See all my bids
      </Link>
    </div>
  );
}
