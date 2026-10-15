import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { bidCommitment, bidMemo, priceCommitment, revealMemo } from "./memo";
import { paymentUri } from "./zip321";

/**
 * The receipt a bidder keeps. Same JSON shape as sealforge-core::receipt::Receipt.
 * The salt is generated here and never sent to any server.
 */
export interface Receipt {
  /** 1: deposit bid. 2: commit-reveal (amount_zat is the deposit, price_zat the hidden bid). */
  version: 1 | 2;
  auction_code: string;
  amount_zat: number;
  refund_ua: string;
  salt: string;
  commitment: string;
  price_zat?: number;
}

export interface PreparedBid {
  receipt: Receipt;
  memo: string;
  uri: string;
}

/** Create a fresh bid: random salt, commitment, memo and ZIP-321 URI. */
export function prepareBid(args: {
  auctionCode: string;
  escrowUa: string;
  amountZat: bigint;
  refundUa: string;
  salt?: Uint8Array;
}): PreparedBid {
  const salt = args.salt ?? crypto.getRandomValues(new Uint8Array(32));
  const commitment = bidCommitment(args.auctionCode, args.refundUa, salt);
  const memo = bidMemo(args.auctionCode, commitment, args.refundUa);
  return {
    receipt: {
      version: 1,
      auction_code: args.auctionCode,
      amount_zat: Number(args.amountZat),
      refund_ua: args.refundUa,
      salt: bytesToHex(salt),
      commitment: bytesToHex(commitment),
    },
    memo,
    uri: paymentUri(args.escrowUa, args.amountZat, memo),
  };
}

/**
 * Commit-reveal bid: the memo commits to the hidden price, and the payment is
 * only the deposit (a budget of at least the price).
 */
export function prepareSealedBid(args: {
  auctionCode: string;
  escrowUa: string;
  priceZat: bigint;
  depositZat: bigint;
  refundUa: string;
  salt?: Uint8Array;
}): PreparedBid {
  if (args.depositZat < args.priceZat) throw new Error("deposit must cover the price");
  const salt = args.salt ?? crypto.getRandomValues(new Uint8Array(32));
  const commitment = priceCommitment(args.auctionCode, args.refundUa, args.priceZat, salt);
  const memo = bidMemo(args.auctionCode, commitment, args.refundUa);
  return {
    receipt: {
      version: 2,
      auction_code: args.auctionCode,
      amount_zat: Number(args.depositZat),
      refund_ua: args.refundUa,
      salt: bytesToHex(salt),
      commitment: bytesToHex(commitment),
      price_zat: Number(args.priceZat),
    },
    memo,
    uri: paymentUri(args.escrowUa, args.depositZat, memo),
  };
}

/** Amount sent with a reveal. It's refunded with the deposit's refund. */
export const REVEAL_AMOUNT_ZAT = 10_000n;

/** The reveal payment for a commit-reveal receipt, to send after the bidding close. */
export function prepareReveal(receipt: Receipt, escrowUa: string): { memo: string; uri: string } {
  if (receipt.price_zat === undefined) throw new Error("not a commit-reveal receipt");
  const memo = revealMemo(
    receipt.auction_code,
    hexToBytes(receipt.commitment),
    BigInt(receipt.price_zat),
    hexToBytes(receipt.salt),
  );
  return { memo, uri: paymentUri(escrowUa, REVEAL_AMOUNT_ZAT, memo) };
}

/** Check a loaded receipt is internally consistent (its commitment matches its salt). */
export function verifyReceipt(receipt: Receipt): boolean {
  try {
    const salt = hexToBytes(receipt.salt);
    const expected =
      receipt.price_zat === undefined
        ? bidCommitment(receipt.auction_code, receipt.refund_ua, salt)
        : priceCommitment(
            receipt.auction_code,
            receipt.refund_ua,
            BigInt(receipt.price_zat),
            salt,
          );
    return bytesToHex(expected) === receipt.commitment;
  } catch {
    return false;
  }
}
