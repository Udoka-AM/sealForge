import type { AuctionStatus } from "@/lib/types";

const LABEL: Record<AuctionStatus, string> = {
  draft: "Draft",
  dkg: "Setting up escrow",
  ready: "Ready",
  open: "Open for bids",
  closed: "Closed",
  published: "Result published",
  refunding: "Refunding",
  settled: "Settled",
  cancelled: "Cancelled",
};

const TONE: Record<AuctionStatus, string> = {
  draft: "var(--text-muted)",
  dkg: "var(--warning)",
  ready: "var(--info)",
  open: "var(--success)",
  closed: "var(--text-muted)",
  published: "var(--primary)",
  refunding: "var(--warning)",
  settled: "var(--text-muted)",
  cancelled: "var(--danger)",
};

/** Status with a dot and a text label (never colour alone). */
export function StatusPill({ status }: { status: AuctionStatus }) {
  return (
    <span className="inline-flex items-center gap-2 rounded-[var(--radius-chip)] border border-border px-2.5 py-1 text-[13px] font-medium">
      <span
        aria-hidden="true"
        className="size-2 rounded-full"
        style={{ background: TONE[status] }}
      />
      {LABEL[status]}
    </span>
  );
}
