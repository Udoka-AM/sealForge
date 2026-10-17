"use client";

import type { Receipt } from "@sealforge/bid";
import { createStore, get, keys, set } from "idb-keyval";

/**
 * Receipts live only in this browser (IndexedDB). They are never sent to a
 * server: the salt inside proves the bid is yours.
 */
export interface StoredReceipt extends Receipt {
  title: string;
  created_at: string;
  backed_up: boolean;
  /** Commit-reveal: the bidder said they sent the reveal (we can't see it on chain here). */
  reveal_sent?: boolean;
  /** Built-in wallet: the address index the refund goes to (so arrivals can be counted). */
  wallet_address_index?: number;
  /** Built-in wallet: the payment that funded the bid (confirmed when the wallet sees it mined). */
  paid_txid?: string;
  /** Built-in wallet: the payment that sent the reveal. */
  reveal_txid?: string;
}

const store = () => createStore("sealforge", "receipts");

export function receiptFileName(r: Pick<Receipt, "auction_code" | "commitment">): string {
  return `sealforge-receipt-${r.auction_code}-${r.commitment.slice(0, 8)}.json`;
}

export async function saveReceipt(receipt: StoredReceipt): Promise<void> {
  await set(receipt.commitment, receipt, store());
}

export async function markBackedUp(commitment: string): Promise<void> {
  const existing = await get<StoredReceipt>(commitment, store());
  if (existing) await set(commitment, { ...existing, backed_up: true }, store());
}

/** Merge fields into a stored receipt (e.g. the wallet payment that funded it). */
export async function updateReceipt(
  commitment: string,
  patch: Partial<StoredReceipt>,
): Promise<void> {
  const existing = await get<StoredReceipt>(commitment, store());
  if (existing) await set(commitment, { ...existing, ...patch }, store());
}

export async function markRevealSent(commitment: string): Promise<void> {
  const existing = await get<StoredReceipt>(commitment, store());
  if (existing) await set(commitment, { ...existing, reveal_sent: true }, store());
}

export async function listReceipts(): Promise<StoredReceipt[]> {
  const s = store();
  const all = await Promise.all((await keys(s)).map((k) => get<StoredReceipt>(k as string, s)));
  return all
    .filter((r): r is StoredReceipt => Boolean(r))
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
}

/** Trigger a JSON download of the receipt (without local-only fields). */
export function downloadReceipt(r: StoredReceipt): void {
  const {
    title: _t,
    created_at: _c,
    backed_up: _b,
    reveal_sent: _r,
    wallet_address_index: _w,
    paid_txid: _p,
    reveal_txid: _x,
    ...receipt
  } = r;
  const blob = new Blob([JSON.stringify(receipt, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = receiptFileName(r);
  a.click();
  URL.revokeObjectURL(url);
}
