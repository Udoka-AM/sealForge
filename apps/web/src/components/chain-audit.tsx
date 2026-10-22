"use client";

import { m } from "motion/react";

import { useEffect, useId, useState } from "react";
import { Button } from "@/components/button";
import { auditWithPayments } from "@/lib/auditor";
import { type ScanProgress, scanEscrow } from "@/lib/scanner";
import type { AuditReport, PublishedResult } from "@/lib/types";
import { ReportView } from "./audit-runner";

const STORAGE_KEY = "sealforge:lightwalletd-grpcweb";

/**
 * Independent audit in the browser (task 3.5, AC7): fetch the escrow's blocks
 * from a lightwallet server the auditor picks, decrypt payments with the
 * published viewing key (WebAssembly), and recompute the result. Nothing
 * comes from the sealForge server except the published result being checked.
 */
export function ChainAudit({
  result,
  defaultEndpoint,
}: {
  result: PublishedResult;
  defaultEndpoint: string;
}) {
  const endpointId = useId();
  const [endpoint, setEndpoint] = useState(defaultEndpoint);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [report, setReport] = useState<AuditReport | null>(null);
  const [found, setFound] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) setEndpoint(saved);
    } catch {
      // storage unavailable: keep the default
    }
  }, []);

  async function run() {
    setRunning(true);
    setError(null);
    setReport(null);
    setProgress({
      scanned: 0,
      total: result.scan_to_height - result.scan_from_height + 1,
      found: 0,
    });
    try {
      try {
        localStorage.setItem(STORAGE_KEY, endpoint);
      } catch {
        // fine
      }
      const payments = await scanEscrow({
        endpoint,
        network: result.network,
        uivk: result.uivk,
        from: result.scan_from_height,
        to: result.scan_to_height,
        onProgress: setProgress,
      });
      setFound(payments.length);
      setReport(await auditWithPayments(result, payments));
    } catch (e) {
      setError(e instanceof Error ? e.message : "The rescan failed.");
    } finally {
      setRunning(false);
    }
  }

  // Two phases: scanning blocks, then fetching and decrypting the payments found.
  const decrypting = progress !== null && progress.scanned >= progress.total && progress.found > 0;
  const pct = !progress
    ? 0
    : decrypting
      ? Math.round(((progress.decrypted ?? 0) / progress.found) * 100)
      : Math.round((progress.scanned / progress.total) * 100);

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <label htmlFor={endpointId} className="text-sm font-medium">
          Lightwallet server (gRPC-web)
        </label>
        <input
          id={endpointId}
          value={endpoint}
          onChange={(e) => setEndpoint(e.target.value)}
          spellCheck={false}
          className="num h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface px-3 text-[14px]"
        />
        <p className="text-[13px] text-muted">
          Use any server you trust. The blocks come from it, not from sealForge.
        </p>
      </div>
      <Button type="button" onClick={run} disabled={running || !endpoint} size="lg">
        {running ? "Rescanning…" : report ? "Rescan again" : "Rescan the chain in my browser"}
      </Button>
      <div aria-live="polite" className="space-y-4">
        {progress && running ? (
          <div className="space-y-1.5">
            <div
              className="h-2 overflow-hidden rounded-full bg-surface-2"
              role="progressbar"
              aria-valuenow={pct}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={decrypting ? "Payments decrypted" : "Blocks scanned"}
            >
              <m.div
                className="h-full origin-left bg-primary"
                initial={{ scaleX: 0 }}
                animate={{ scaleX: pct / 100 }}
                transition={{ duration: 0.2, ease: "linear" }}
              />
            </div>
            <p className="num text-[13px] text-muted">
              {decrypting
                ? `Decrypting payments: ${progress.decrypted ?? 0} of ${progress.found}`
                : `${progress.scanned.toLocaleString("en-US")} of ${progress.total.toLocaleString("en-US")} blocks · ${progress.found} transaction(s) to the escrow`}
            </p>
          </div>
        ) : null}
        {error ? (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        ) : null}
        {report ? (
          <>
            <p className="text-sm text-muted">
              Found {found} payment(s) to the escrow in blocks{" "}
              {result.scan_from_height.toLocaleString("en-US")}–
              {result.scan_to_height.toLocaleString("en-US")}, and recomputed the result from them
              alone.
            </p>
            <ReportView report={report} />
          </>
        ) : null}
      </div>
    </div>
  );
}
