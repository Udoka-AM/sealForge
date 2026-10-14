import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { bidCommitment, bidMemo } from "./memo";
import { paymentUri } from "./zip321";

/**
 * The receipt a bidder keeps. Same JSON shape as sealforge-core::receipt::Receipt.
 * The salt is generated here and never sent to any server.
 */
export interface Receipt {
  version: 1;
  auction_code: string;
  amount_zat: number;
  refund_ua: string;
  salt: string;
  commitment: string;
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

/** Check a loaded receipt is internally consistent (its commitment matches its salt). */
export function verifyReceipt(receipt: Receipt): boolean {
  try {
    const expected = bidCommitment(
      receipt.auction_code,
      receipt.refund_ua,
      hexToBytes(receipt.salt),
    );
    return bytesToHex(expected) === receipt.commitment;
  } catch {
    return false;
  }
}
