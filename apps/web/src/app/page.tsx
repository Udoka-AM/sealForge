import { ArrowUpRight, ScanSearch } from "lucide-react";
import Link from "next/link";
import { CoordinatorError } from "@/components/coordinator-error";
import { HomeHero, type Proof } from "@/components/home-hero";
import { CountUp } from "@/components/motion/count-up";
import { Reveal, Stagger, StaggerItem } from "@/components/motion/reveal";
import { WindowBar } from "@/components/motion/window-bar";
import { ProcessRail } from "@/components/process-rail";
import { StatusPill } from "@/components/status-pill";
import { CoordinatorUnavailable, getChainTip, listAuctions } from "@/lib/coordinator";
import { modeLabel, zec } from "@/lib/format";
import type { AuctionView } from "@/lib/types";

export const dynamic = "force-dynamic";

async function load(): Promise<{ auctions: AuctionView[]; tip: number | null; error?: string }> {
  try {
    const [auctions, tip] = await Promise.all([listAuctions(), getChainTip()]);
    return { auctions, tip };
  } catch (e) {
    if (e instanceof CoordinatorUnavailable) return { auctions: [], tip: null, error: e.message };
    throw e;
  }
}

const finished = (a: AuctionView) =>
  a.status === "published" || a.status === "refunding" || a.status === "settled";

export default async function HomePage() {
  const { auctions, tip, error } = await load();
  const live = auctions.filter((a) => a.status === "open");
  const past = auctions.filter((a) => a.status !== "open");
  const latest = past.find(finished);
  const proof: Proof | null = latest
    ? {
        code: latest.code,
        title: latest.title,
        bids: latest.bid_count ?? null,
        status: latest.status,
      }
    : null;
  // Only counts the coordinator publishes (bid counts are hidden below 5 bids).
  const sealedBids = auctions.reduce((s, a) => s + (a.bid_count ?? 0), 0);
  const settled = auctions.filter((a) => a.status === "settled").length;

  return (
    <div className="space-y-20 md:space-y-28">
      <HomeHero proof={proof} hasLive={live.length > 0} />

      {auctions.length > 0 ? (
        <Reveal as="section" className="grid grid-cols-3 gap-4 border-y border-border py-6">
          <Stat label="Open for bids" value={live.length} />
          <Stat label="Sealed bids counted" value={sealedBids} />
          <Stat label="Auctions settled" value={settled} />
        </Reveal>
      ) : null}

      {error ? <CoordinatorError message={error} /> : null}

      <section id="live" aria-labelledby="live-h" className="scroll-mt-24 space-y-6">
        <div className="flex items-end justify-between gap-4">
          <h2 id="live-h" className="text-[26px] leading-8 font-semibold tracking-tight">
            Open for bids
          </h2>
          {tip !== null ? (
            <span className="text-[13px] text-muted">
              Times are counted in blocks from{" "}
              <span className="num">{tip.toLocaleString("en-US")}</span>
            </span>
          ) : null}
        </div>
        {live.length === 0 && !error ? (
          <EmptyLive />
        ) : (
          <Stagger className="grid gap-4 md:grid-cols-2">
            {live.map((a) => (
              <StaggerItem key={a.code}>
                <AuctionCard auction={a} tip={tip} />
              </StaggerItem>
            ))}
          </Stagger>
        )}
      </section>

      <section aria-labelledby="how-h" className="space-y-10">
        <Reveal className="max-w-[560px] space-y-2">
          <h2 id="how-h" className="text-[26px] leading-8 font-semibold tracking-tight">
            Private while you bid. Public once it counts.
          </h2>
          <p className="text-muted">
            Three things make the result trustworthy without trusting us.{" "}
            <Link href="/how-it-works" className="text-text underline-offset-4 hover:underline">
              The full walkthrough
            </Link>
          </p>
        </Reveal>
        <ProcessRail />
      </section>

      {past.length > 0 ? (
        <section aria-labelledby="past-h" className="space-y-5">
          <h2 id="past-h" className="text-[26px] leading-8 font-semibold tracking-tight">
            Results
          </h2>
          <Stagger className="divide-y divide-border overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-card)]">
            {past.map((a) => (
              <StaggerItem key={a.code}>
                <div className="group flex items-center justify-between gap-4 px-4 py-3.5 transition-colors duration-150 hover:bg-surface-2 sm:px-5">
                  <Link href={`/a/${a.code}`} className="min-w-0 flex-1">
                    <span className="block truncate font-medium group-hover:text-primary">
                      {a.title}
                    </span>
                    <span className="text-[13px] text-muted">
                      {modeLabel(a.mode)} · {a.quantity} lots
                      {typeof a.bid_count === "number" ? ` · ${a.bid_count} bids` : ""}
                    </span>
                  </Link>
                  <span className="flex shrink-0 items-center gap-3">
                    {finished(a) ? (
                      <Link
                        href={`/audit/${a.code}`}
                        className="inline-flex items-center gap-1 text-sm text-muted transition-colors duration-150 hover:text-text"
                      >
                        <ScanSearch size={15} aria-hidden="true" />
                        <span className="hidden sm:inline">Audit</span>
                      </Link>
                    ) : null}
                    <StatusPill status={a.status} />
                  </span>
                </div>
              </StaggerItem>
            ))}
          </Stagger>
        </section>
      ) : null}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="space-y-1 text-center sm:text-left">
      <CountUp
        value={value}
        className="text-[28px] leading-8 font-medium tracking-tight md:text-[34px] md:leading-10"
      />
      <p className="text-[13px] text-muted">{label}</p>
    </div>
  );
}

function EmptyLive() {
  return (
    <div className="flex flex-col items-start gap-3 rounded-[var(--radius-card)] border border-dashed border-border p-6 sm:flex-row sm:items-center sm:justify-between">
      <p className="text-muted">
        No auction is taking bids right now. Results below stay auditable.
      </p>
      <Link
        href="/console"
        className="inline-flex items-center gap-1 text-sm text-text hover:text-primary"
      >
        Seal your own auction <ArrowUpRight size={15} aria-hidden="true" />
      </Link>
    </div>
  );
}

function AuctionCard({ auction: a, tip }: { auction: AuctionView; tip: number | null }) {
  return (
    <Link
      href={`/a/${a.code}`}
      className="lift group block h-full space-y-5 rounded-[var(--radius-card)] border border-border bg-surface p-5"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          <h3 className="truncate text-[19px] leading-6 font-semibold tracking-tight">{a.title}</h3>
          <p className="text-[13px] text-muted">
            {modeLabel(a.mode)}
            {a.bid_mode === "commit_reveal" ? " · sealed reveal" : ""}
          </p>
        </div>
        <StatusPill status={a.status} />
      </div>
      <dl className="grid grid-cols-3 gap-3 text-sm">
        <div>
          <dt className="text-[12px] text-muted">Lots</dt>
          <dd className="num">{a.quantity}</dd>
        </div>
        <div>
          <dt className="text-[12px] text-muted">Reserve</dt>
          <dd className="num">{zec(a.reserve_zat, a.network)}</dd>
        </div>
        <div>
          <dt className="text-[12px] text-muted">Sealed bids</dt>
          <dd className="num">{typeof a.bid_count === "number" ? a.bid_count : "under 5"}</dd>
        </div>
      </dl>
      <WindowBar auction={a} initialTip={tip} />
      <span className="inline-flex items-center gap-1 text-sm text-primary">
        Bid on this{" "}
        <ArrowUpRight
          size={15}
          aria-hidden="true"
          className="transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
        />
      </span>
    </Link>
  );
}
