"use client";

import {
  canonicalDenominations,
  formatZec,
  isCanonical,
  looksLikeUnifiedAddress,
  MIN_DENOMINATION,
  type PreparedBid,
  parseZec,
  prepareBid,
  prepareSealedBid,
  roundingOptions,
} from "@sealforge/bid";
import { ArrowLeft, Download, ExternalLink, Wallet } from "lucide-react";
import { AnimatePresence, m } from "motion/react";
import Link from "next/link";
import QRCode from "qrcode";
import { useEffect, useId, useRef, useState } from "react";
import { Button, buttonClass } from "@/components/button";
import { HashChip } from "@/components/hash-chip";
import { Guilloche } from "@/components/motion/guilloche";
import { SealMark } from "@/components/seal-mark";
import { unit } from "@/lib/format";
import {
  downloadReceipt,
  markBackedUp,
  type StoredReceipt,
  saveReceipt,
  updateReceipt,
} from "@/lib/receipts";
import type { AuctionView } from "@/lib/types";
import { InlineUnlock } from "./wallet/inline-unlock";
import { PaymentStatus, WalletPay } from "./wallet/wallet-pay";
import { useWallet } from "./wallet/wallet-provider";

type Step = "amount" | "pay" | "sealed";

export function BidFlow({ auction }: { auction: AuctionView }) {
  const [step, setStep] = useState<Step>("amount");
  const [bid, setBid] = useState<PreparedBid | null>(null);
  const [stored, setStored] = useState<StoredReceipt | null>(null);
  const [storageFailed, setStorageFailed] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);

  async function sealed(prepared: PreparedBid, walletAddressIndex?: number) {
    const receipt: StoredReceipt = {
      ...prepared.receipt,
      title: auction.title,
      created_at: new Date().toISOString(),
      backed_up: false,
      ...(walletAddressIndex !== undefined ? { wallet_address_index: walletAddressIndex } : {}),
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

  async function paidFromWallet(txid: string) {
    if (!stored) return;
    const updated = { ...stored, paid_txid: txid };
    setStored(updated);
    await updateReceipt(stored.commitment, { paid_txid: txid }).catch(() => {});
    setStep("sealed");
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

      <BidSteps step={step} sealedMode={auction.bid_mode === "commit_reveal"} />

      <AnimatePresence mode="wait" initial={false}>
        <m.div
          key={step}
          initial={{ opacity: 0, x: 16 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: -12, transition: { duration: 0.2 } }}
          transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
        >
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
              onPaidFromWallet={paidFromWallet}
            />
          ) : null}

          {step === "sealed" && stored ? (
            <SealedStep
              auction={auction}
              receipt={stored}
              heading={heading}
              onDownload={download}
            />
          ) : null}
        </m.div>
      </AnimatePresence>
    </div>
  );
}

type HeadingRef = React.RefObject<HTMLHeadingElement | null>;

const STEP_LABELS: { key: Step; label: string }[] = [
  { key: "amount", label: "Your bid" },
  { key: "pay", label: "Pay" },
  { key: "sealed", label: "Sealed" },
];

/** Three steps; the gold fill slides to the current one. */
function BidSteps({ step, sealedMode }: { step: Step; sealedMode: boolean }) {
  const index = STEP_LABELS.findIndex((s) => s.key === step);
  return (
    <ol className="grid grid-cols-3 gap-2" aria-label="Bid progress">
      {STEP_LABELS.map((s, i) => (
        <li key={s.key} className="space-y-1.5">
          <div className="h-1 overflow-hidden rounded-full bg-border">
            <m.div
              className="h-full origin-left rounded-full bg-primary"
              initial={false}
              animate={{ scaleX: i <= index ? 1 : 0 }}
              transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
            />
          </div>
          <p className={`text-[12px] ${i === index ? "font-medium text-text" : "text-muted"}`}>
            {i === 0 && sealedMode ? "Your sealed bid" : s.label}
            {i === index ? <span className="sr-only"> (current step)</span> : null}
          </p>
        </li>
      ))}
    </ol>
  );
}

function StepHeading({ heading, children }: { heading: HeadingRef; children: React.ReactNode }) {
  // Each step's heading takes focus when it arrives (after the previous step has
  // animated out), so keyboard and screen reader users follow along.
  useEffect(() => {
    heading.current?.focus();
  }, [heading]);
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
  onSealed: (bid: PreparedBid, walletAddressIndex?: number) => void;
}) {
  const wallet = useWallet();
  const walletReady = wallet.status === "unlocked" && wallet.network === auction.network;
  const [refundTo, setRefundTo] = useState<"wallet" | "other">("wallet");
  const useWalletRefund = walletReady && refundTo === "wallet";
  const [busy, setBusy] = useState(false);
  const amountId = useId();
  const refundId = useId();
  const depositId = useId();
  // Commit-reveal: the amount is the hidden bid, and only the deposit is visible.
  const sealedMode = auction.bid_mode === "commit_reveal";
  const [deposit, setDeposit] = useState<bigint | null>(null);
  const [amount, setAmount] = useState("");
  const [refund, setRefund] = useState("");
  const [errors, setErrors] = useState<{ amount?: string; refund?: string }>({});
  const reserve = BigInt(auction.reserve_zat);
  const parsed = parseZec(amount);
  const rounding =
    parsed !== null && parsed > 0n && !isCanonical(parsed) ? roundingOptions(parsed) : null;
  const suggestions =
    rounding && !sealedMode
      ? [rounding.lower, rounding.upper].filter((d): d is bigint => d !== null && d >= reserve)
      : [];
  // Deposits are standard amounts at or above the bid, so they reveal little about it.
  const depositChoices =
    sealedMode && parsed !== null && parsed > 0n
      ? canonicalDenominations()
          .filter((d) => d >= parsed)
          .slice(0, 3)
      : [];
  const chosenDeposit =
    deposit !== null && depositChoices.includes(deposit) ? deposit : (depositChoices[0] ?? null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const next: typeof errors = {};
    if (parsed === null || parsed <= 0n)
      next.amount = `Enter an amount in ${unit(auction.network)}.`;
    // Bids in whole 0.0001 ZEC steps leave no refund dust (see docs/evidence/linkability.md).
    else if (parsed % MIN_DENOMINATION !== 0n)
      next.amount = `Use at most 4 decimal places (0.0001 ${unit(auction.network)}).`;
    else if (parsed < reserve)
      next.amount = `Below the ${formatZec(reserve)} ${unit(auction.network)} reserve.`;
    let refundUa = refund.trim();
    if (!useWalletRefund && !looksLikeUnifiedAddress(refundUa, auction.network))
      next.refund = `Paste a Unified Address for ${auction.network} (it starts with ${
        auction.network === "mainnet"
          ? "u1"
          : auction.network === "testnet"
            ? "utest1"
            : "uregtest1"
      }).`;
    setErrors(next);
    if (next.amount || next.refund || parsed === null || !auction.escrow_ua) return;
    // A fresh wallet address, used for this bid only, so its refunds can be recognised
    // (and can't be linked to other bids or to the funding address).
    let walletIndex: number | undefined;
    if (useWalletRefund) {
      setBusy(true);
      try {
        const fresh = await wallet.freshAddress();
        refundUa = fresh.address;
        walletIndex = fresh.index;
      } catch (err) {
        setBusy(false);
        setErrors({ refund: (err as Error).message });
        return;
      }
      setBusy(false);
    }
    if (sealedMode) {
      if (chosenDeposit === null) return;
      onSealed(
        prepareSealedBid({
          auctionCode: auction.code,
          escrowUa: auction.escrow_ua,
          priceZat: parsed,
          depositZat: chosenDeposit,
          refundUa,
        }),
        walletIndex,
      );
      return;
    }
    onSealed(
      prepareBid({
        auctionCode: auction.code,
        escrowUa: auction.escrow_ua,
        amountZat: parsed,
        refundUa,
      }),
      walletIndex,
    );
  }

  return (
    <form onSubmit={submit} noValidate className="space-y-6">
      <StepHeading heading={heading}>Your sealed bid</StepHeading>

      <div className="space-y-2">
        <label htmlFor={amountId} className="block text-sm font-medium">
          {sealedMode ? "Your bid (hidden until you reveal it)" : "Maximum bid"}
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
          <span className="pr-4 text-muted">{unit(auction.network)}</span>
        </div>
        <p
          id={`${amountId}-help`}
          className={`text-sm ${errors.amount ? "text-danger" : "text-muted"}`}
        >
          {errors.amount ??
            (sealedMode
              ? `Reserve ${formatZec(reserve)} ${unit(auction.network)}. You don't send this amount: it stays in your receipt until you reveal it after bidding closes.`
              : `Reserve ${formatZec(reserve)} ${unit(auction.network)}. You send this amount now; anything above the price you pay comes back.`)}
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
                {formatZec(d)} {unit(auction.network)}
              </button>
            ))}
          </div>
        ) : null}
      </div>

      {sealedMode && depositChoices.length > 0 ? (
        <fieldset className="space-y-2">
          <legend id={depositId} className="text-sm font-medium">
            Deposit
          </legend>
          <div className="flex flex-wrap gap-2">
            {depositChoices.map((d) => (
              <label
                key={d.toString()}
                className={`num cursor-pointer rounded-[var(--radius-control)] border px-4 py-2.5 text-[15px] transition-colors duration-150 has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-primary ${
                  d === chosenDeposit
                    ? "border-primary text-primary"
                    : "border-border hover:border-muted"
                }`}
              >
                <input
                  type="radio"
                  name="deposit"
                  value={d.toString()}
                  checked={d === chosenDeposit}
                  onChange={() => setDeposit(d)}
                  className="sr-only"
                />
                {formatZec(d)} {unit(auction.network)}
              </label>
            ))}
          </div>
          <p className="text-sm text-muted">
            This is what you send now, and all that the escrow&rsquo;s viewers (including the
            trustees) see. A larger deposit hides your bid better. Everything above the price you
            pay comes back.
          </p>
        </fieldset>
      ) : null}

      <fieldset className="space-y-2">
        <legend className="text-sm font-medium">Refund address</legend>
        {walletReady ? (
          <div className="grid gap-2 sm:grid-cols-2">
            {(
              [
                ["wallet", "My sealForge wallet", "A fresh address for this bid only"],
                ["other", "Another wallet", "Paste a Unified Address"],
              ] as const
            ).map(([key, title, hint]) => (
              <label
                key={key}
                className={`cursor-pointer rounded-[var(--radius-control)] border px-3 py-2.5 text-sm transition-colors duration-150 has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-primary ${
                  refundTo === key ? "border-primary" : "border-border hover:border-muted"
                }`}
              >
                <input
                  type="radio"
                  name="refund-to"
                  value={key}
                  checked={refundTo === key}
                  onChange={() => setRefundTo(key)}
                  className="sr-only"
                />
                <span className="block font-medium">{title}</span>
                <span className="block text-muted">{hint}</span>
              </label>
            ))}
          </div>
        ) : null}
        {useWalletRefund ? (
          <p className="flex items-start gap-2 text-sm text-muted">
            <Wallet size={16} aria-hidden="true" className="mt-0.5 shrink-0 text-primary" />
            Refunds and change go to a new address in your wallet, used for nothing else. The wallet
            shows them when they arrive.
          </p>
        ) : (
          <>
            <label htmlFor={refundId} className="sr-only">
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
          </>
        )}
        {wallet.status === "locked" && wallet.network === auction.network ? (
          <InlineUnlock prompt="Unlock your sealForge wallet to fill this in and pay in one step." />
        ) : null}
        {wallet.status === "none" && wallet.network === auction.network ? (
          <p className="text-sm text-muted">
            No Zcash wallet?{" "}
            <Link href="/wallet" className="underline hover:text-text">
              Create one here
            </Link>{" "}
            in a minute; your bid form keeps its place in this tab.
          </p>
        ) : null}
      </fieldset>

      <Button type="submit" size="lg" className="w-full" disabled={busy}>
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
  onPaidFromWallet,
}: {
  auction: AuctionView;
  bid: PreparedBid;
  heading: HeadingRef;
  storageFailed: boolean;
  onDownload: () => void;
  onChangeAmount: () => void;
  onSent: () => void;
  onPaidFromWallet: (txid: string) => void;
}) {
  const wallet = useWallet();
  const walletReady = wallet.status === "unlocked" && wallet.network === auction.network;
  const [qr, setQr] = useState<string | null>(null);
  const [qrFailed, setQrFailed] = useState(false);

  useEffect(() => {
    QRCode.toString(bid.uri, { type: "svg", margin: 2, errorCorrectionLevel: "M" })
      .then((svg) => setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`))
      .catch(() => setQrFailed(true));
  }, [bid.uri]);

  return (
    <div className="space-y-6">
      <StepHeading heading={heading}>
        {walletReady ? "Pay your deposit" : "Send it from your wallet"}
      </StepHeading>

      {storageFailed ? (
        <p
          role="alert"
          className="rounded-[var(--radius-control)] border border-warning p-3 text-sm"
        >
          This browser can&rsquo;t keep your receipt (private mode?). Download it before you pay.
        </p>
      ) : null}

      {walletReady && auction.escrow_ua ? (
        <WalletPay
          network={auction.network}
          request={{
            to: auction.escrow_ua,
            amount_zat: bid.receipt.amount_zat,
            memo: bid.memo,
            label: `bid:${auction.code}`,
          }}
          closeHeight={auction.close_height}
          onPaid={onPaidFromWallet}
        />
      ) : null}

      <QrPayment walletReady={walletReady}>
        <div className="rounded-[var(--radius-card)] bg-white p-4">
          {qr ? (
            // biome-ignore lint/performance/noImgElement: a local data-URI SVG; next/image adds nothing here
            <img
              src={qr}
              alt={`Payment QR code for ${formatZec(BigInt(bid.receipt.amount_zat))} ${unit(auction.network)} with the bid memo`}
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
      </QrPayment>

      <dl className="space-y-3 text-sm">
        <div className="flex items-baseline justify-between gap-4">
          <dt className="text-muted">
            {auction.bid_mode === "commit_reveal" ? "Deposit" : "Amount"}
          </dt>
          <dd className="num text-[18px]">
            {formatZec(BigInt(bid.receipt.amount_zat))} {unit(auction.network)}
          </dd>
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
          {auction.bid_mode === "commit_reveal"
            ? " In this auction the commitment also seals your bid amount: nobody, the trustees included, can read it until you reveal it."
            : null}
        </p>
      </details>

      <p className="text-sm text-muted">
        Your bid counts once it confirms on chain at or before block{" "}
        <span className="num">{auction.close_height.toLocaleString("en-US")}</span>.
      </p>

      <div className="flex flex-col gap-3 sm:flex-row">
        <Button
          type="button"
          variant={walletReady ? "secondary" : "primary"}
          onClick={onSent}
          className="flex-1"
        >
          {walletReady ? "I paid from another wallet" : "I\u2019ve sent it"}
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

/** The QR / deep-link payment. Behind a disclosure when the built-in wallet can pay. */
function QrPayment({ walletReady, children }: { walletReady: boolean; children: React.ReactNode }) {
  if (!walletReady) return <>{children}</>;
  return (
    <details className="group space-y-4 rounded-[var(--radius-card)] border border-border p-4">
      <summary className="cursor-pointer text-sm text-muted group-open:mb-4 hover:text-text">
        Pay from another wallet instead (QR code)
      </summary>
      <div className="space-y-4">{children}</div>
    </details>
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
        <div className="relative flex size-24 shrink-0 items-center justify-center">
          <Guilloche size={96} className="absolute inset-0" />
          <SealMark size={64} stamp />
        </div>
        <div>
          <StepHeading heading={heading}>Bid sealed</StepHeading>
          <p className="text-muted">The price will be public after close. You won&rsquo;t be.</p>
        </div>
      </div>

      {receipt.paid_txid ? <PaymentStatus txid={receipt.paid_txid} /> : null}

      <dl className="space-y-3 rounded-[var(--radius-card)] border border-border bg-surface p-5 text-sm">
        <div className="flex items-baseline justify-between gap-4">
          <dt className="text-muted">Auction</dt>
          <dd className="text-right">{auction.title}</dd>
        </div>
        <div className="flex items-baseline justify-between gap-4">
          <dt className="text-muted">
            {auction.bid_mode === "commit_reveal" ? "Deposit" : "Amount"}
          </dt>
          <dd className="num">
            {formatZec(BigInt(receipt.amount_zat))} {unit(auction.network)}
          </dd>
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
        {auction.bid_mode === "commit_reveal" && auction.reveal_close_height ? (
          <li>
            <span className="font-medium">Reveal your bid</span>
            <span className="text-muted">
              {" "}
              from{" "}
              <Link href="/me" className="underline">
                My bids
              </Link>{" "}
              between blocks{" "}
              <span className="num">{(auction.close_height + 1).toLocaleString("en-US")}</span> and{" "}
              <span className="num">{auction.reveal_close_height.toLocaleString("en-US")}</span>. A
              bid that isn&rsquo;t revealed doesn&rsquo;t count, and its deposit is refunded.
            </span>
          </li>
        ) : null}
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
