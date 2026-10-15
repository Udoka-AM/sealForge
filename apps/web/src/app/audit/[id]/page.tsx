import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AuditRunner } from "@/components/audit-runner";
import { ButtonLink } from "@/components/button";
import { CoordinatorError } from "@/components/coordinator-error";
import { HashChip } from "@/components/hash-chip";
import { CoordinatorUnavailable, getAuction, getResult } from "@/lib/coordinator";
import { modeLabel, zec } from "@/lib/format";
import type { AuctionView, PublishedResult } from "@/lib/types";

export const dynamic = "force-dynamic";
export const metadata: Metadata = { title: "Audit" };

export default async function AuditPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  let auction: AuctionView | null;
  let result: PublishedResult | null;
  try {
    [auction, result] = await Promise.all([getAuction(id), getResult(id)]);
  } catch (e) {
    if (e instanceof CoordinatorUnavailable) return <CoordinatorError message={e.message} />;
    throw e;
  }
  if (!auction) notFound();

  return (
    <div className="max-w-[760px] space-y-8">
      <header className="space-y-2">
        <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">
          Audit: {auction.title}
        </h1>
        <p className="text-muted">
          Don&rsquo;t trust the organiser. Recompute the result yourself.
        </p>
      </header>

      {!result ? (
        <div className="space-y-4">
          <p className="text-muted">
            The result isn&rsquo;t published yet. It appears after bidding closes at block{" "}
            <span className="num">{auction.close_height.toLocaleString("en-US")}</span>.
          </p>
          <ButtonLink href={`/a/${auction.code}`} variant="secondary">
            Back to the auction
          </ButtonLink>
        </div>
      ) : (
        <>
          <dl className="grid grid-cols-2 gap-x-6 gap-y-4 rounded-[var(--radius-card)] border border-border bg-surface p-5 sm:grid-cols-4">
            <div>
              <dt className="text-sm text-muted">Clearing price</dt>
              <dd className="num text-[18px]">
                {result.clearing_price_zat === null ? "None" : zec(result.clearing_price_zat)}
              </dd>
            </div>
            <div>
              <dt className="text-sm text-muted">Total volume</dt>
              <dd className="num text-[18px]">{zec(result.total_volume_zat)}</dd>
            </div>
            <div>
              <dt className="text-sm text-muted">Valid bids</dt>
              <dd className="num text-[18px]">{result.valid_bid_count}</dd>
            </div>
            <div>
              <dt className="text-sm text-muted">Winners</dt>
              <dd className="num text-[18px]">
                {result.winner_count} of {result.rules.quantity}
              </dd>
            </div>
          </dl>

          <dl className="flex flex-wrap gap-x-8 gap-y-4 text-sm">
            <div className="w-full space-y-1.5">
              <dt className="text-muted">Merkle root of all bids</dt>
              <dd>
                <HashChip value={result.merkle_root} label="Merkle root" head={12} tail={10} />
              </dd>
            </div>
            <div className="w-full space-y-1.5">
              <dt className="text-muted">Escrow incoming viewing key (UIVK)</dt>
              <dd>
                <HashChip value={result.uivk} label="viewing key" head={14} tail={10} />
              </dd>
            </div>
            <div>
              <dt className="text-muted">Blocks to scan</dt>
              <dd className="num">
                {result.scan_from_height.toLocaleString("en-US")} to{" "}
                {result.scan_to_height.toLocaleString("en-US")}
              </dd>
            </div>
            <div>
              <dt className="text-muted">Rules</dt>
              <dd>
                {modeLabel(result.rules.mode)}, reserve {zec(result.rules.reserve_zat)}
              </dd>
            </div>
          </dl>

          <section aria-labelledby="recompute" className="space-y-3">
            <h2 id="recompute" className="text-[18px] leading-6 font-semibold">
              Recompute
            </h2>
            <p className="max-w-[68ch] text-sm text-muted">
              This runs sealForge&rsquo;s auditor, compiled to WebAssembly, on the published bid
              list. It recomputes the price, volume, winners, every bidder&rsquo;s outcome and the
              Merkle root, and compares each one. On its own it can&rsquo;t prove that no bid was
              left out of the list. For that, rescan the chain yourself with the viewing key above.
            </p>
            <AuditRunner result={result} />
            <div className="space-y-2 text-sm">
              <p className="text-muted">
                Independent check from the chain, with any lightwallet server you trust:
              </p>
              <pre className="num break-all whitespace-pre-wrap rounded-[var(--radius-control)] bg-surface-2 p-3 text-[13px]">
                {`cargo install --git https://github.com/Udoka-AM/sealForge sealforge-auditor\nsealforge-audit --published https://<this site>/api/auctions/${result.auction_code}/result --endpoint <lightwalletd URL>`}
              </pre>
              <p className="text-muted">
                It scans blocks {result.scan_from_height.toLocaleString("en-US")} to{" "}
                {result.scan_to_height.toLocaleString("en-US")} with the viewing key, decrypts every
                payment to the escrow, and recomputes the result from those payments alone.
              </p>
            </div>
          </section>
        </>
      )}
    </div>
  );
}
