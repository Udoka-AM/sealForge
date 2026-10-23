import { formatZec } from "@sealforge/bid";
import { ArrowUpRight, ScanSearch } from "lucide-react";
import Link from "next/link";
import { CoordinatorError } from "@/components/coordinator-error";
import { HomeHero } from "@/components/home-hero";
import { ClosingCta } from "@/components/landing/closing-cta";
import type { FeedItem } from "@/components/landing/live-feed";
import { Marquee } from "@/components/landing/marquee";
import { type MemoSample, MemoWalkthrough } from "@/components/landing/memo-walkthrough";
import { Odometer } from "@/components/landing/odometer";
import { SectionHead } from "@/components/landing/section-head";
import { type SealedOutcome, ShieldedLedger } from "@/components/landing/shielded-ledger";
import { type EscrowSample, ViewingKeyDemo } from "@/components/landing/viewing-key-demo";
import { Stagger, StaggerItem } from "@/components/motion/reveal";
import { WindowBar } from "@/components/motion/window-bar";
import { StatusPill } from "@/components/status-pill";
import { CoordinatorUnavailable, getChainTip, getResult, listAuctions } from "@/lib/coordinator";
import { modeLabel, unit, zec } from "@/lib/format";
import type { AuctionView, PublishedResult } from "@/lib/types";

export const dynamic = "force-dynamic";

interface Loaded {
  auctions: AuctionView[];
  tip: number | null;
  showcase: { auction: AuctionView; result: PublishedResult } | null;
  error?: string;
}

const finished = (a: AuctionView) =>
  a.status === "published" || a.status === "refunding" || a.status === "settled";

async function load(): Promise<Loaded> {
  try {
    const [auctions, tip] = await Promise.all([listAuctions(), getChainTip()]);
    // The busiest finished auction is the one the page explains with.
    const best = auctions
      .filter(finished)
      .sort((a, b) => (b.bid_count ?? 0) - (a.bid_count ?? 0))[0];
    const result = best ? await getResult(best.code) : null;
    return {
      auctions,
      tip,
      showcase: best && result && result.bids.length > 0 ? { auction: best, result } : null,
    };
  } catch (e) {
    if (e instanceof CoordinatorUnavailable)
      return { auctions: [], tip: null, showcase: null, error: e.message };
    throw e;
  }
}

const STATUS_WORD: Record<AuctionView["status"], string> = {
  draft: "Draft",
  dkg: "Setting up",
  ready: "Ready",
  open: "Taking bids",
  closed: "Closed",
  published: "Published",
  refunding: "Refunding",
  settled: "Settled",
  cancelled: "Cancelled",
};

const short = (h: string) => `${h.slice(0, 8)}…${h.slice(-6)}`;

export default async function HomePage() {
  const { auctions, tip, showcase, error } = await load();
  const live = auctions.filter((a) => a.status === "open");
  const past = auctions.filter((a) => a.status !== "open");
  // Only counts the coordinator publishes (bid counts are hidden below 5 bids).
  const sealedBids = auctions.reduce((s, a) => s + (a.bid_count ?? 0), 0);
  const settled = auctions.filter((a) => a.status === "settled").length;

  const feed: FeedItem[] = [...live, ...past.filter(finished)].slice(0, 6).map((a) => ({
    label: STATUS_WORD[a.status],
    title: a.title,
    detail:
      a.status === "open"
        ? `${modeLabel(a.mode)} · closes at block ${a.close_height.toLocaleString("en-US")}`
        : `${typeof a.bid_count === "number" ? `${a.bid_count} sealed bids` : "Under 5 bids"} · ${modeLabel(a.mode)}`,
  }));

  let outcome: SealedOutcome | null = null;
  let memo: MemoSample | null = null;
  let escrow: EscrowSample | null = null;
  if (showcase) {
    const { auction: a, result: r } = showcase;
    const net = r.network;
    if (r.clearing_price_zat !== null)
      outcome = {
        title: a.title,
        unit: unit(net),
        price: zec(r.clearing_price_zat, net),
        bids: a.bid_count ?? null,
        winners: r.winner_count,
      };
    memo = {
      code: a.code,
      commitment: r.bids[0]?.commitment ?? "",
      more: r.bids.slice(1, 12).map((b) => b.commitment),
      merkleRoot: r.merkle_root,
      unit: unit(net),
    };
    escrow = {
      code: a.code,
      title: a.title,
      notes: r.bids.slice(0, 6).map((b) => ({
        block: b.block_height.toLocaleString("en-US"),
        amount: `${formatZec(BigInt(b.deposit_zat ?? b.amount_zat))} ${unit(net)}`,
        commitment: short(b.commitment),
      })),
      total: r.bids.length,
      price: r.clearing_price_zat !== null ? zec(r.clearing_price_zat, net) : "no clearing price",
      winners: r.winner_count,
      root: r.merkle_root,
    };
  }

  return (
    <div className="space-y-28 md:space-y-40">
      {/* The hero and the protocol strip share the first screen on a desktop browser. */}
      <div>
        <HomeHero hasLive={live.length > 0} feed={feed} tip={tip} />
        <div className="full-bleed flex h-16 items-center border-y border-border">
          <Marquee seconds={46}>
            {[
              "Zcash",
              "Ironwood shielded pool",
              "FROST 2-of-3",
              "PCZT",
              "ZIP-321",
              "librustzcash",
              "WebAssembly",
              "BLAKE2b-256",
              "Merkle proofs",
              "Viewing keys",
            ].map((t) => (
              <span
                key={t}
                className="flex items-center gap-8 pr-8 font-mono text-[12px] tracking-[0.16em] whitespace-nowrap text-muted uppercase"
              >
                {t}
                <svg viewBox="0 0 64 64" className="size-2.5 text-primary" aria-hidden="true">
                  <path fill="currentColor" d="M32 4 56.2 18V46L32 60 7.8 46V18Z" />
                </svg>
              </span>
            ))}
          </Marquee>
        </div>
      </div>

      {error ? <CoordinatorError message={error} /> : null}

      {auctions.length > 0 ? (
        <section
          aria-label="sealForge in numbers"
          className="grid grid-cols-2 gap-x-6 gap-y-10 md:grid-cols-5"
        >
          <Stat value={String(live.length)} label="Auctions taking bids now" />
          <Stat value={sealedBids.toLocaleString("en-US")} label="Sealed bids counted on testnet" />
          <Stat value={String(settled)} label="Auctions settled, refunds and payout sent" />
          <Stat value="2/3" label="Trustees needed to move escrow funds" />
          <Stat value="0" label="Key shares held by our server" />
        </section>
      ) : null}

      <div id="why" className="scroll-mt-[var(--header-h)]">
        <ShieldedLedger outcome={outcome} />
      </div>

      {memo ? <MemoWalkthrough sample={memo} /> : null}

      <section id="live" aria-labelledby="live-h" className="scroll-mt-24 space-y-8">
        <SectionHead
          index="03"
          label="Open now"
          titleId="live-h"
          title="Open for bids"
          lead={
            tip !== null ? (
              <>
                Times are counted in blocks from{" "}
                <span className="num text-text">{tip.toLocaleString("en-US")}</span>.
              </>
            ) : undefined
          }
        />
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

      {escrow ? <ViewingKeyDemo sample={escrow} /> : null}

      {past.length > 0 ? (
        <section aria-labelledby="past-h" className="space-y-8">
          <SectionHead
            index="05"
            label="On the record"
            titleId="past-h"
            title="Results"
            lead="Every finished auction links to its chain audit."
          />
          <Stagger className="divide-y divide-border overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-card)]">
            {past.map((a) => (
              <StaggerItem key={a.code}>
                <div className="group flex items-center justify-between gap-4 px-4 py-4 transition-colors duration-150 hover:bg-surface-2 sm:px-5">
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

      <ClosingCta bidHref={live.length > 0 ? "#live" : "/how-it-works"} />
    </div>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="space-y-2 border-t border-border pt-4">
      <Odometer
        value={value}
        className="text-[44px] leading-none font-semibold tracking-[-0.04em] md:text-[56px]"
      />
      <p className="max-w-[22ch] text-[13px] leading-5 text-muted">{label}</p>
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
