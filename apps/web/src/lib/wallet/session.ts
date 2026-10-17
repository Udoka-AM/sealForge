"use client";

import { getBlockRange, getLatestHeight, getTreeState, sendTransaction } from "@/lib/grpc-web";
import type { Network } from "@/lib/types";
import { createVault, type OpenVault, saveVault, unlockVault, type VaultSecrets } from "./vault";

/** Shapes returned by crates/sealforge-wallet (JSON). */
export interface WalletNote {
  pool: "orchard" | "ironwood";
  txid: string;
  height: number;
  action_index: number;
  value_zat: number;
  address_index: number | null;
  change: boolean;
  nullifier: string;
  spent_txid: string | null;
  spent_height: number | null;
}

export interface SentTx {
  txid: string;
  to: string;
  amount_zat: number;
  fee_zat: number;
  memo: string | null;
  label: string | null;
  built_at_height: number;
  expiry_height: number;
  spends: string[];
  mined_height: number | null;
  expired: boolean;
}

export interface Balance {
  spendable_zat: number;
  pending_spend_zat: number;
  orchard_zat: number;
  total_zat: number;
}

export interface WalletSummary {
  scannedHeight: number;
  birthday: number;
  balance: Balance;
  notes: WalletNote[];
  sent: SentTx[];
  fundingAddress: string;
}

export interface Built {
  txid: string;
  raw: string;
  fee_zat: number;
  expiry_height: number;
}

export interface PaymentRequest {
  to: string;
  amount_zat: number;
  memo?: string;
  label?: string;
}

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };

/** Request/response over the wallet worker (public/wallet/worker.js). */
class WorkerClient {
  private worker: Worker;
  private next = 1;
  private pending = new Map<number, Pending>();

  constructor() {
    this.worker = new Worker("/wallet/worker.js", { type: "module" });
    this.worker.onmessage = (
      e: MessageEvent<{ id: number; ok: boolean; result?: unknown; error?: string }>,
    ) => {
      const p = this.pending.get(e.data.id);
      if (!p) return;
      this.pending.delete(e.data.id);
      if (e.data.ok) p.resolve(e.data.result);
      else p.reject(new Error(e.data.error ?? "wallet error"));
    };
    this.worker.onerror = (e) => {
      for (const p of this.pending.values())
        p.reject(new Error(e.message || "The wallet worker failed to load."));
      this.pending.clear();
    };
  }

  call<T>(op: string, args?: Record<string, unknown>, transfer: Transferable[] = []): Promise<T> {
    const id = this.next++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.worker.postMessage({ id, op, args }, transfer);
    });
  }

  terminate() {
    this.worker.terminate();
    for (const p of this.pending.values()) p.reject(new Error("The wallet was locked."));
    this.pending.clear();
  }
}

let shared: WorkerClient | null = null;
function worker(): WorkerClient {
  shared ??= new WorkerClient();
  return shared;
}

/** One-off helpers that don't need an open wallet. */
export const walletTools = {
  newPhrase: () => worker().call<string>("newPhrase"),
  phraseIsValid: (phrase: string) => worker().call<boolean>("phraseIsValid", { phrase }),
  /** Build the proving key in the background (a few seconds, once per page). */
  prepareProver: () => worker().call<boolean>("prepareProver"),
};

/** Blocks per batch: small enough to save progress often, large enough to be quick. */
const BATCH = 400;

export interface SyncProgress {
  scanned: number;
  tip: number;
}

/**
 * An unlocked wallet. The phrase and the AES key live only in memory (and in
 * the worker), and are dropped by [`WalletSession.lock`].
 */
export class WalletSession {
  private secrets: VaultSecrets;
  private syncing: Promise<WalletSummary> | null = null;

  private constructor(
    readonly network: Network,
    readonly endpoint: string,
    private vault: OpenVault,
    phrase: string,
    state: string,
  ) {
    this.secrets = { phrase, state };
  }

  /** Create a new wallet (or restore one from a phrase) born at `birthday`. */
  static async create(args: {
    network: Network;
    endpoint: string;
    phrase: string;
    passphrase: string;
    /** First block to scan. Omit for a new wallet: the current tip. */
    birthday?: number;
  }): Promise<WalletSession> {
    const tip = await getLatestHeight(args.endpoint);
    if (tip <= 0) throw new Error("The lightwallet server didn't report a chain tip.");
    const birthday = Math.max(1, Math.min(args.birthday ?? tip, tip));
    const treeState = await getTreeState(args.endpoint, birthday - 1);
    const state = await worker().call<string>(
      "create",
      { network: args.network, phrase: args.phrase, birthday, treeState },
      [treeState.buffer],
    );
    const vault = await createVault(args.network, args.passphrase, birthday, {
      phrase: args.phrase,
      state,
    });
    return new WalletSession(args.network, args.endpoint, vault, args.phrase, state);
  }

  static async unlock(
    network: Network,
    endpoint: string,
    passphrase: string,
  ): Promise<WalletSession> {
    const { vault, secrets } = await unlockVault(network, passphrase);
    await worker().call("open", { network, phrase: secrets.phrase, state: secrets.state });
    return new WalletSession(network, endpoint, vault, secrets.phrase, secrets.state);
  }

  /** Check a passphrase again before showing the recovery phrase. */
  async revealPhrase(passphrase: string): Promise<string> {
    const { secrets } = await unlockVault(this.network, passphrase);
    return secrets.phrase;
  }

  async summary(): Promise<WalletSummary> {
    return worker().call<WalletSummary>("summary");
  }

  private async persist(): Promise<WalletSummary> {
    const s = await worker().call<WalletSummary & { state: string }>("summary");
    this.secrets = { phrase: this.secrets.phrase, state: s.state };
    await saveVault(this.vault, this.secrets);
    const { state: _state, ...summary } = s;
    return summary;
  }

  /** Sync to the server's tip. Concurrent calls share one run. */
  sync(onProgress?: (p: SyncProgress) => void): Promise<WalletSummary> {
    this.syncing ??= this.runSync(onProgress).finally(() => {
      this.syncing = null;
    });
    return this.syncing;
  }

  private async runSync(onProgress?: (p: SyncProgress) => void): Promise<WalletSummary> {
    let resets = 0;
    let tip = await getLatestHeight(this.endpoint);
    let scanned = (await this.summary()).scannedHeight;
    while (scanned < tip) {
      const to = Math.min(tip, scanned + BATCH);
      const blocks: Uint8Array[] = [];
      await getBlockRange(this.endpoint, scanned + 1, to, (b) => blocks.push(b));
      const result = await worker().call<{ error: string | null; height: number }>(
        "scanBlocks",
        { blocks },
        blocks.map((b) => b.buffer),
      );
      scanned = result.height;
      if (result.error) {
        // The chain changed, or our tree disagrees with the server's: rescan
        // from the birthday rather than ever spend on a bad witness.
        if (/^(reorg|tree_mismatch)/.test(result.error) && resets < 2) {
          resets += 1;
          scanned = await worker().call<number>("reset");
          continue;
        }
        throw new Error(`Sync stopped at block ${scanned}: ${result.error}`);
      }
      await this.persist();
      onProgress?.({ scanned, tip });
      if (scanned >= tip) tip = await getLatestHeight(this.endpoint);
    }
    onProgress?.({ scanned, tip });
    return this.persist();
  }

  /** A new address used for nothing else (one per bid). Saved before it's returned. */
  async freshAddress(): Promise<{ index: number; address: string }> {
    const a = await worker().call<{ index: number; address: string }>("freshAddress");
    await this.persist();
    return a;
  }

  address(index: number): Promise<string> {
    return worker().call<string>("address", { index });
  }

  /** The fee for paying `amountZat` now; rejects if the balance doesn't cover it. */
  quote(amountZat: number): Promise<number> {
    return worker().call<number>("quote", { amountZat });
  }

  /**
   * Build, prove, sign and broadcast. Resolves once the server accepted it;
   * the payment counts as confirmed only when a later sync sees it mined.
   */
  async pay(request: PaymentRequest): Promise<Built> {
    const built = await worker().call<Built>("pay", { request });
    // Save the lock on its notes before broadcasting, so a reload can't double-spend them.
    await this.persist();
    const raw = Uint8Array.from(built.raw.match(/../g) ?? [], (h) => Number.parseInt(h, 16));
    let verdict: { code: number; message: string };
    try {
      verdict = await sendTransaction(this.endpoint, raw);
    } catch (e) {
      await worker().call("forgetPayment", { txid: built.txid });
      await this.persist();
      throw new Error(`The lightwallet server refused the payment: ${(e as Error).message}`);
    }
    if (verdict.code !== 0) {
      await worker().call("forgetPayment", { txid: built.txid });
      await this.persist();
      throw new Error(
        `The lightwallet server refused the payment (${verdict.code}): ${verdict.message}`,
      );
    }
    return built;
  }

  /** Drop the phrase, the key and the worker's copy of the wallet. */
  lock() {
    this.secrets = { phrase: "", state: "" };
    shared?.terminate();
    shared = null;
  }
}
