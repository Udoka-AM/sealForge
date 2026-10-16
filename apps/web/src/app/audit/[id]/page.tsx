import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AuditRunner } from "@/components/audit-runner";
import { ButtonLink } from "@/components/button";
import { ChainAudit } from "@/components/chain-audit";
import { CoordinatorError } from "@/components/coordinator-error";
import { HashChip } from "@/components/hash-chip";
import { CoordinatorUnavailable, getAuction, getResult } from "@/lib/coordinator";
import { modeLabel, zec } from "@/lib/format";
import type { AuctionView, PublishedResult } from "@/lib/types";

export const dynamic = "force-dynamic";
export const metadata: Metadata = { title: "Audit" };

/** Default gRPC-web lightwallet endpoint offered for the browser rescan (read at request time). */
function lightwalletd(): string {
  return (
    process.env.LIGHTWALLETD_GRPCWEB ??
    process.env.NEXT_PUBLIC_LIGHTWALLETD_GRPCWEB ??
    "http://localhost:8081"
  );
}

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

          <section aria-labelledby="chain" className="space-y-3">
            <h2 id="chain" className="text-[18px] leading-6 font-semibold">
              Rescan the chain
            </h2>
            <p className="max-w-[68ch] text-sm text-muted">
              The independent check. Your browser downloads blocks{" "}
              {result.scan_from_height.toLocaleString("en-US")} to{" "}
              {result.scan_to_height.toLocaleString("en-US")} from a lightwallet server you choose,
              decrypts every payment to the escrow with the viewing key above, and recomputes the
              price, volume, winners, every bidder&rsquo;s outcome and the Merkle root from those
              payments alone. A bid left out of the result would show up here.
            </p>
            <ChainAudit result={result} defaultEndpoint={lightwalletd()} />
            <details className="text-sm">
              <summary className="cursor-pointer text-muted hover:text-text">
                Prefer the command line?
              </summary>
              <pre className="num mt-2 break-all whitespace-pre-wrap rounded-[var(--radius-control)] bg-surface-2 p-3 text-[13px]">
                {`cargo install --git https://github.com/Udoka-AM/sealForge sealforge-auditor\nsealforge-audit --published https://<this site>/api/auctions/${result.auction_code}/result --endpoint <lightwalletd URL>`}
              </pre>
            </details>
          </section>

          <section aria-labelledby="recompute" className="space-y-3">
            <h2 id="recompute" className="text-[18px] leading-6 font-semibold">
              Quick check of the published list
            </h2>
            <p className="max-w-[68ch] text-sm text-muted">
              Recomputes the result from the published bid list only. It catches wrong arithmetic or
              a wrong root in seconds, but not a bid left out of the list: the rescan above does.
            </p>
            <AuditRunner result={result} />
          </section>
        </>
      )}
    </div>
  );
}
