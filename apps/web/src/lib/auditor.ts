"use client";

import type { AuctionView, AuditReport, EscrowCheck, PublishedResult, ReceiptCheck } from "./types";

/**
 * The public auditor, compiled from crates/sealforge-auditor to WebAssembly
 * (scripts/build-auditor-wasm.sh). The browser runs the same Rust code the
 * coordinator used, so results don't depend on trusting this website's JS math.
 */
interface AuditorModule {
  default: (init: { module_or_path: string }) => Promise<unknown>;
  auditor_version: () => string;
  audit_published: (published: string) => string;
  check_receipt: (published: string, receipt: string) => string;
  check_escrow_json: (source: string, pinned: string) => string;
}

let loading: Promise<AuditorModule> | null = null;

function load(): Promise<AuditorModule> {
  loading ??= (async () => {
    const url = "/auditor/sealforge_auditor.js";
    const mod = (await import(
      /* webpackIgnore: true */ /* turbopackIgnore: true */ url
    )) as AuditorModule;
    await mod.default({ module_or_path: "/auditor/sealforge_auditor_bg.wasm" });
    return mod;
  })();
  return loading;
}

export async function auditorVersion(): Promise<string> {
  return (await load()).auditor_version();
}

export async function auditPublished(result: PublishedResult): Promise<AuditReport> {
  return JSON.parse((await load()).audit_published(JSON.stringify(result))) as AuditReport;
}

export async function checkReceipt(
  result: PublishedResult,
  receipt: unknown,
): Promise<ReceiptCheck> {
  return JSON.parse(
    (await load()).check_receipt(JSON.stringify(result), JSON.stringify(receipt)),
  ) as ReceiptCheck;
}

/** Full audit from payments decoded from the chain (lib/scanner.ts). */
export async function auditWithPayments(
  result: PublishedResult,
  payments: unknown[],
): Promise<AuditReport> {
  const mod = (await load()) as AuditorModule & {
    audit_with_payments: (published: string, payments: string) => string;
  };
  return JSON.parse(
    mod.audit_with_payments(JSON.stringify(result), JSON.stringify(payments)),
  ) as AuditReport;
}

/**
 * Check every trustee's signature over the escrow address, from the auction
 * page's data or a published result. `pinned` holds keys you got from the
 * trustees themselves; a listed key that differs fails the check.
 */
export async function checkEscrow(
  source: AuctionView | PublishedResult,
  pinned: [number, string][] = [],
): Promise<EscrowCheck> {
  return JSON.parse(
    (await load()).check_escrow_json(JSON.stringify(source), JSON.stringify(pinned)),
  ) as EscrowCheck;
}
