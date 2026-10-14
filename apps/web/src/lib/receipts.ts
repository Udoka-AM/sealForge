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

export async function listReceipts(): Promise<StoredReceipt[]> {
  const s = store();
  const all = await Promise.all((await keys(s)).map((k) => get<StoredReceipt>(k as string, s)));
  return all
    .filter((r): r is StoredReceipt => Boolean(r))
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
}

/** Trigger a JSON download of the receipt (without local-only fields). */
export function downloadReceipt(r: StoredReceipt): void {
  const { title: _t, created_at: _c, backed_up: _b, ...receipt } = r;
  const blob = new Blob([JSON.stringify(receipt, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = receiptFileName(r);
  a.click();
  URL.revokeObjectURL(url);
}
