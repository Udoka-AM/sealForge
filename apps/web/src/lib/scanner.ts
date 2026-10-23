"use client";

import { getBlockRange, getTransaction } from "./grpc-web";
import type { Network } from "./types";

/**
 * The escrow scanner, compiled from crates/sealforge-scan to WebAssembly
 * (scripts/build-scanner-wasm.sh): the same code `sealforge-audit` runs.
 */
interface ScannerModule {
  default: (init: { module_or_path: string }) => Promise<unknown>;
  Scanner: {
    new (
      network: string,
      uivk: string,
    ): {
      scan_block: (block: Uint8Array) => string;
      decrypt: (raw: Uint8Array, height: number) => string;
      free: () => void;
    };
    block_height: (block: Uint8Array) => number;
  };
}

let loading: Promise<ScannerModule> | null = null;

function load(): Promise<ScannerModule> {
  loading ??= (async () => {
    const url = "/scanner/sealforge_scan.js";
    const mod = (await import(
      /* webpackIgnore: true */ /* turbopackIgnore: true */ url
    )) as ScannerModule;
    await mod.default({ module_or_path: "/scanner/sealforge_scan_bg.wasm" });
    return mod;
  })();
  return loading;
}

export interface ChainPayment {
  txid: string;
  output_index: number;
  block_height: number;
  amount_zat: number;
  memo: string | null;
}

export interface ScanProgress {
  scanned: number;
  total: number;
  found: number;
  /** Payment transactions fetched and decrypted so far (after the block scan). */
  decrypted?: number;
}

/**
 * Every payment to the escrow in `from..=to`, read from `endpoint` (a gRPC-web
 * lightwallet server) and decrypted here with the published viewing key.
 */
export async function scanEscrow(args: {
  endpoint: string;
  network: Network;
  uivk: string;
  from: number;
  to: number;
  onProgress?: (p: ScanProgress) => void;
}): Promise<ChainPayment[]> {
  const mod = await load();
  const scanner = new mod.Scanner(args.network, args.uivk);
  try {
    const total = args.to - args.from + 1;
    const hits = new Map<string, number>();
    let scanned = 0;
    await getBlockRange(args.endpoint, args.from, args.to, (block) => {
      scanned += 1;
      const txids = JSON.parse(scanner.scan_block(block)) as string[];
      if (txids.length > 0) {
        const height = mod.Scanner.block_height(block);
        for (const t of txids) hits.set(t, height);
      }
      if (scanned % 25 === 0 || scanned === total)
        args.onProgress?.({ scanned, total, found: hits.size });
    });
    // Fetch the matching transactions a few at a time; the result is sorted below.
    const out: ChainPayment[] = [];
    const queue = [...hits];
    let decrypted = 0;
    const worker = async () => {
      for (let next = queue.shift(); next; next = queue.shift()) {
        const [txid, height] = next;
        const raw = await getTransaction(args.endpoint, txid);
        out.push(...(JSON.parse(scanner.decrypt(raw, height)) as ChainPayment[]));
        decrypted += 1;
        args.onProgress?.({ scanned: total, total, found: hits.size, decrypted });
      }
    };
    await Promise.all(Array.from({ length: Math.min(4, queue.length) }, worker));
    return out.sort(
      (a, b) =>
        a.block_height - b.block_height ||
        a.txid.localeCompare(b.txid) ||
        a.output_index - b.output_index,
    );
  } finally {
    scanner.free();
  }
}
