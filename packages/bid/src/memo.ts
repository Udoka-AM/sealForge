import { blake2b } from "@noble/hashes/blake2b";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";

/** Mirrors sealforge-core::memo and ::receipt. Any change must keep the shared test vector passing. */

export const MAX_MEMO_BYTES = 512;
const COMMITMENT_PERSONAL = utf8ToBytes("sealforge_bid_v1"); // exactly 16 bytes
const PRICE_COMMITMENT_PERSONAL = utf8ToBytes("sealforge_rev_v1"); // exactly 16 bytes

/** 10-char Crockford base32 auction code (no I, L, O, U). */
export function isValidAuctionCode(code: string): boolean {
  return /^[0-9A-HJKMNP-TV-Z]{10}$/.test(code);
}

/** Loose client-side check. The coordinator and the scanner fully validate the address. */
export function looksLikeUnifiedAddress(
  value: string,
  network: "mainnet" | "testnet" | "regtest",
): boolean {
  const prefix = network === "mainnet" ? "u1" : network === "testnet" ? "utest1" : "uregtest1";
  return (
    value.startsWith(prefix) && value.length >= 60 && value.length <= 400 && !value.includes(":")
  );
}

/** BLAKE2b-256(personal = "sealforge_bid_v1", code ‖ 0 ‖ refund_ua ‖ 0 ‖ salt). */
export function bidCommitment(auctionCode: string, refundUa: string, salt: Uint8Array): Uint8Array {
  if (salt.length !== 32) throw new Error("salt must be 32 bytes");
  const code = utf8ToBytes(auctionCode);
  const refund = utf8ToBytes(refundUa);
  const input = new Uint8Array(code.length + 1 + refund.length + 1 + salt.length);
  input.set(code, 0);
  input.set(refund, code.length + 1);
  input.set(salt, code.length + 1 + refund.length + 1);
  return blake2b(input, { dkLen: 32, personalization: COMMITMENT_PERSONAL });
}

/** `SF1:<code>:<commitment-hex>:<refund_ua>`, at most 512 bytes. */
export function bidMemo(auctionCode: string, commitment: Uint8Array, refundUa: string): string {
  if (!isValidAuctionCode(auctionCode)) throw new Error("invalid auction code");
  if (!refundUa || refundUa.includes(":")) throw new Error("invalid refund address");
  const memo = `SF1:${auctionCode}:${bytesToHex(commitment)}:${refundUa}`;
  if (utf8ToBytes(memo).length > MAX_MEMO_BYTES) throw new Error("memo too long");
  return memo;
}

/**
 * Commit-reveal mode (PRD F13): BLAKE2b-256(personal = "sealforge_rev_v1",
 * code ‖ 0 ‖ refund_ua ‖ 0 ‖ price_zat (u64 LE) ‖ salt). Mirrors
 * sealforge-core::receipt::price_commitment.
 */
export function priceCommitment(
  auctionCode: string,
  refundUa: string,
  priceZat: bigint,
  salt: Uint8Array,
): Uint8Array {
  if (salt.length !== 32) throw new Error("salt must be 32 bytes");
  if (priceZat < 0n || priceZat > 0xffffffffffffffffn) throw new Error("price out of range");
  const code = utf8ToBytes(auctionCode);
  const refund = utf8ToBytes(refundUa);
  const price = new Uint8Array(8);
  new DataView(price.buffer).setBigUint64(0, priceZat, true);
  const input = new Uint8Array(code.length + 1 + refund.length + 1 + 8 + salt.length);
  let at = 0;
  for (const part of [code, new Uint8Array(1), refund, new Uint8Array(1), price, salt]) {
    input.set(part, at);
    at += part.length;
  }
  return blake2b(input, { dkLen: 32, personalization: PRICE_COMMITMENT_PERSONAL });
}

/** `SF1R:<code>:<commitment-hex>:<price_zat>:<salt-hex>`: the reveal sent after the close. */
export function revealMemo(
  auctionCode: string,
  commitment: Uint8Array,
  priceZat: bigint,
  salt: Uint8Array,
): string {
  if (!isValidAuctionCode(auctionCode)) throw new Error("invalid auction code");
  return `SF1R:${auctionCode}:${bytesToHex(commitment)}:${priceZat.toString()}:${bytesToHex(salt)}`;
}
