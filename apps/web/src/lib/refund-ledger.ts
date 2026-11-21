/**
 * The refund batches this trustee signed in this browser (security review D1,
 * second part): the browser's copy of the CLI's `refund-ledger.json`. Before a
 * refund share leaves the page, its batch is recorded here, so this trustee
 * never signs the same note in another batch while the first may still be
 * mined. Kept in IndexedDB, per auction and trustee key.
 *
 * It lives only in this browser. Clearing site data, or signing the same
 * auction from another browser or the CLI, starts a separate ledger. Any two
 * signing pairs share a trustee, so a note can only be paid twice if two
 * trustees lose theirs; still, use one place to sign each auction.
 */
import { get, set } from "idb-keyval";
import { findTransaction } from "./grpc-web";
import type { RefundLedgerEntry } from "./types";

const key = (code: string, keyId: string) => `sf-refund-ledger:${code}:${keyId}`;

export async function readLedger(code: string, keyId: string): Promise<RefundLedgerEntry[]> {
  return ((await get(key(code, keyId))) as RefundLedgerEntry[] | undefined) ?? [];
}

/** Add a signed batch, once. Resolves only after IndexedDB has stored it. */
export async function recordSigned(
  code: string,
  keyId: string,
  entry: RefundLedgerEntry,
): Promise<void> {
  const ledger = await readLedger(code, keyId);
  if (ledger.some((e) => e.batch_txid === entry.batch_txid)) return;
  await set(key(code, keyId), [...ledger, entry]);
}

/** Each ledger entry and whether the trustee's lightwallet server has its transaction. */
export async function ledgerOnChain(
  endpoint: string,
  ledger: RefundLedgerEntry[],
): Promise<{ entry: RefundLedgerEntry; on_chain: boolean }[]> {
  const out = [];
  for (const entry of ledger) {
    out.push({ entry, on_chain: (await findTransaction(endpoint, entry.batch_txid)) !== null });
  }
  return out;
}
