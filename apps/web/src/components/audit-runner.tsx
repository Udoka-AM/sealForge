"use client";

import { CircleAlert, CircleCheck } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/button";
import { auditorVersion, auditPublished } from "@/lib/auditor";
import type { AuditReport, PublishedResult } from "@/lib/types";

const FIELD_LABEL: Record<string, string> = {
  clearing_price_zat: "Clearing price (zat)",
  total_volume_zat: "Total volume (zat)",
  valid_bid_count: "Valid bids",
  winner_count: "Winners",
  merkle_root: "Merkle root",
  bid_outcomes_mismatched: "Bid outcomes that differ",
  result: "Result",
};

export function AuditRunner({ result }: { result: PublishedResult }) {
  const [report, setReport] = useState<AuditReport | null>(null);
  const [build, setBuild] = useState<{ version: string; sha?: string } | null>(null);
  const [running, setRunning] = useState(false);
  const [failed, setFailed] = useState(false);

  async function run() {
    setRunning(true);
    setFailed(false);
    try {
      const [r, version, meta] = await Promise.all([
        auditPublished(result),
        auditorVersion(),
        fetch("/auditor/BUILD.json")
          .then((res) => (res.ok ? (res.json() as Promise<{ wasm_sha256?: string }>) : {}))
          .catch(() => ({})),
      ]);
      setReport(r);
      setBuild({ version, sha: (meta as { wasm_sha256?: string }).wasm_sha256 });
    } catch {
      setFailed(true);
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="space-y-4">
      <Button type="button" onClick={run} disabled={running} size="lg">
        {running ? "Recomputing…" : report ? "Run again" : "Recompute in my browser"}
      </Button>
      <div aria-live="polite" className="space-y-4">
        {failed ? (
          <p role="alert" className="text-danger">
            The auditor couldn&rsquo;t run in this browser.
          </p>
        ) : null}
        {report ? (
          <>
            <ReportView report={report} />
            {build ? (
              <p className="text-[13px] text-muted">
                sealforge-auditor {build.version} (WebAssembly)
                {build.sha ? (
                  <>
                    , build sha256 <span className="num">{build.sha.slice(0, 16)}…</span>
                  </>
                ) : null}
              </p>
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  );
}

export function ReportView({ report }: { report: AuditReport }) {
  return (
    <>
      <p
        className={`inline-flex items-center gap-2 font-semibold ${report.matches ? "text-success" : "text-danger"}`}
      >
        {report.matches ? (
          <CircleCheck size={20} aria-hidden="true" />
        ) : (
          <CircleAlert size={20} aria-hidden="true" />
        )}
        {report.matches
          ? "Every published figure matches"
          : "Mismatch: the published result doesn't add up"}
      </p>
      <div className="overflow-x-auto rounded-[var(--radius-card)] border border-border">
        <table className="w-full text-left text-sm">
          <thead className="bg-surface-2 text-muted">
            <tr>
              <th scope="col" className="px-4 py-2 font-medium">
                Field
              </th>
              <th scope="col" className="px-4 py-2 font-medium">
                Published
              </th>
              <th scope="col" className="px-4 py-2 font-medium">
                Recomputed
              </th>
              <th scope="col" className="px-4 py-2 font-medium">
                <span className="sr-only">Match</span>
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {report.fields.map((f) => (
              <tr key={f.field}>
                <th scope="row" className="px-4 py-2 font-normal">
                  {FIELD_LABEL[f.field] ?? f.field}
                </th>
                <td className="num max-w-[200px] truncate px-4 py-2" title={f.published}>
                  {f.published}
                </td>
                <td className="num max-w-[200px] truncate px-4 py-2" title={f.recomputed}>
                  {f.recomputed}
                </td>
                <td className="px-4 py-2">
                  {f.ok ? (
                    <CircleCheck size={16} className="text-success" aria-label="matches" />
                  ) : (
                    <CircleAlert size={16} className="text-danger" aria-label="differs" />
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
