"use client";

import { formatZec, prepareReveal, REVEAL_AMOUNT_ZAT } from "@sealforge/bid";
import { ExternalLink, Eye } from "lucide-react";
import QRCode from "qrcode";
import { useEffect, useState } from "react";
import { Button, buttonClass } from "@/components/button";
import { networkOfUa, unit } from "@/lib/format";
import { markRevealSent, type StoredReceipt } from "@/lib/receipts";
import { api } from "@/lib/sfsig";
import type { AuctionView, ChainView } from "@/lib/types";

/**
 * Commit-reveal (PRD F13): after bidding closes, the bidder sends a small
 * payment whose memo opens their sealed bid. It's built here from the receipt;
 * nothing is sent to our server.
 */
export function RevealPanel({ receipt, onSent }: { receipt: StoredReceipt; onSent: () => void }) {
  const [auction, setAuction] = useState<AuctionView | null>(null);
  const [tip, setTip] = useState<number | null>(null);
  const [open, setOpen] = useState(false);
  const [qr, setQr] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      api<AuctionView>("GET", `/v1/auctions/${receipt.auction_code}`, null),
      api<ChainView>("GET", "/v1/chain", null),
    ])
      .then(([a, c]) => {
        setAuction(a);
        setTip(c.tip);
      })
      .catch(() => {});
  }, [receipt.auction_code]);

  const reveal =
    auction?.escrow_ua && receipt.price_zat !== undefined
      ? prepareReveal(receipt, auction.escrow_ua)
      : null;

  const uri = reveal?.uri;
  useEffect(() => {
    if (!open || !uri) return;
    QRCode.toString(uri, { type: "svg", margin: 2, errorCorrectionLevel: "M" })
      .then((svg) => setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`))
      .catch(() => setQr(null));
  }, [open, uri]);

  if (auction?.bid_mode !== "commit_reveal" || !auction.reveal_close_height) return null;
  const from = auction.close_height + 1;
  const until = auction.reveal_close_height;
  const before = tip !== null && tip < from;
  const after = tip !== null && tip > until;

  if (receipt.reveal_sent)
    return (
      <p className="text-sm text-success" role="status">
        Reveal sent. It counts once it confirms by block {until.toLocaleString("en-US")}.
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
      {open && reveal ? (
        <div className="grid gap-4 sm:grid-cols-[180px_minmax(0,1fr)] sm:items-center">
          <div className="rounded-[var(--radius-control)] bg-white p-2">
            {qr ? (
              // biome-ignore lint/performance/noImgElement: a local data-URI SVG
              <img
                src={qr}
                alt={`QR code to send ${formatZec(REVEAL_AMOUNT_ZAT)} ${unit(networkOfUa(receipt.refund_ua))} with your reveal memo`}
                className="aspect-square w-full"
              />
            ) : (
              <div className="aspect-square" />
            )}
          </div>
          <div className="space-y-3 text-sm">
            <p className="text-muted">
              Send {formatZec(REVEAL_AMOUNT_ZAT)} {unit(networkOfUa(receipt.refund_ua))} with this
              memo. It comes back with your refund.
            </p>
            <a href={reveal.uri} className={buttonClass({ variant: "secondary" })}>
              <ExternalLink size={16} aria-hidden="true" /> Open in wallet
            </a>
            <Button
              type="button"
              onClick={async () => {
                await markRevealSent(receipt.commitment).catch(() => {});
                onSent();
              }}
            >
              I&rsquo;ve sent it
            </Button>
          </div>
        </div>
      ) : (
        <Button type="button" onClick={() => setOpen(true)}>
          <Eye size={16} aria-hidden="true" /> Reveal my bid
        </Button>
      )}
    </div>
  );
}
