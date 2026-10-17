"use client";

import { formatZec, prepareReveal, REVEAL_AMOUNT_ZAT } from "@sealforge/bid";
import { ExternalLink, Eye } from "lucide-react";
import QRCode from "qrcode";
import { useEffect, useState } from "react";
import { Button, buttonClass } from "@/components/button";
import { networkOfUa, unit } from "@/lib/format";
import { markRevealSent, type StoredReceipt, updateReceipt } from "@/lib/receipts";
import type { AuctionView } from "@/lib/types";
import { PaymentStatus, WalletPay } from "./wallet/wallet-pay";
import { useWallet } from "./wallet/wallet-provider";

/**
 * Commit-reveal (PRD F13): after bidding closes, the bidder sends a small
 * payment whose memo opens their sealed bid. It's built here from the receipt;
 * nothing is sent to our server. With the built-in wallet it's one tap.
 */
export function RevealPanel({
  receipt,
  auction,
  tip,
  onChanged,
}: {
  receipt: StoredReceipt;
  auction: AuctionView;
  tip: number | null;
  onChanged: (patch: Partial<StoredReceipt>) => void;
}) {
  const wallet = useWallet();
  const [open, setOpen] = useState(false);
  const [qr, setQr] = useState<string | null>(null);

  const reveal =
    auction.escrow_ua && receipt.price_zat !== undefined
      ? prepareReveal(receipt, auction.escrow_ua)
      : null;

  const uri = reveal?.uri;
  useEffect(() => {
    if (!open || !uri) return;
    QRCode.toString(uri, { type: "svg", margin: 2, errorCorrectionLevel: "M" })
      .then((svg) => setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`))
      .catch(() => setQr(null));
  }, [open, uri]);

  if (auction.bid_mode !== "commit_reveal" || !auction.reveal_close_height) return null;
  const from = auction.close_height + 1;
  const until = auction.reveal_close_height;
  const before = tip !== null && tip < from;
  const after = tip !== null && tip > until;
  const network = networkOfUa(receipt.refund_ua);
  const walletReady = wallet.status === "unlocked" && wallet.network === auction.network;

  if (receipt.reveal_txid) return <PaymentStatus txid={receipt.reveal_txid} label="Reveal" />;
  if (receipt.reveal_sent)
    return (
      <p className="text-sm text-muted" role="status">
        You marked the reveal as sent from another wallet. It counts if it confirmed by block{" "}
        {until.toLocaleString("en-US")}; the published result will show it.
      </p>
    );
  if (before)
    return (
      <p className="text-sm text-muted">
        Reveal your bid between blocks {from.toLocaleString("en-US")} and{" "}
        {until.toLocaleString("en-US")}, after bidding closes.
      </p>
    );
  if (after)
    return (
      <p className="text-sm text-muted">
        The reveal window closed at block {until.toLocaleString("en-US")}.
      </p>
    );

  return (
    <div className="w-full space-y-3 rounded-[var(--radius-card)] border border-primary bg-surface p-4">
      <p className="text-sm">
        <span className="font-medium">Reveal your bid</span>{" "}
        <span className="text-muted">
          by block {until.toLocaleString("en-US")}. Unrevealed bids don&rsquo;t count and are
          refunded in full.
        </span>
      </p>
      {walletReady && reveal && auction.escrow_ua ? (
        <WalletPay
          network={auction.network}
          request={{
            to: auction.escrow_ua,
            amount_zat: Number(REVEAL_AMOUNT_ZAT),
            memo: reveal.memo,
            label: `reveal:${auction.code}`,
          }}
          closeHeight={until}
          cta="Reveal for"
          onPaid={async (txid) => {
            await updateReceipt(receipt.commitment, { reveal_txid: txid }).catch(() => {});
            onChanged({ reveal_txid: txid });
          }}
        />
      ) : null}
      {open && reveal ? (
        <div className="grid gap-4 sm:grid-cols-[180px_minmax(0,1fr)] sm:items-center">
          <div className="rounded-[var(--radius-control)] bg-white p-2">
            {qr ? (
              // biome-ignore lint/performance/noImgElement: a local data-URI SVG
              <img
                src={qr}
                alt={`QR code to send ${formatZec(REVEAL_AMOUNT_ZAT)} ${unit(network)} with your reveal memo`}
                className="aspect-square w-full"
              />
            ) : (
              <div className="aspect-square" />
            )}
          </div>
          <div className="space-y-3 text-sm">
            <p className="text-muted">
              Send {formatZec(REVEAL_AMOUNT_ZAT)} {unit(network)} with this memo. It comes back with
              your refund.
            </p>
            <a href={reveal.uri} className={buttonClass({ variant: "secondary" })}>
              <ExternalLink size={16} aria-hidden="true" /> Open in wallet
            </a>
            <Button
              type="button"
              onClick={async () => {
                await markRevealSent(receipt.commitment).catch(() => {});
                onChanged({ reveal_sent: true });
              }}
            >
              I&rsquo;ve sent it
            </Button>
          </div>
        </div>
      ) : (
        <Button
          type="button"
          variant={walletReady ? "ghost" : "primary"}
          onClick={() => setOpen(true)}
        >
          <Eye size={16} aria-hidden="true" />{" "}
          {walletReady ? "Reveal from another wallet (QR code)" : "Reveal my bid"}
        </Button>
      )}
    </div>
  );
}
