import { ArrowLeft, Eye, EyeOff, KeyRound, ScanSearch, Shuffle } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ButtonLink } from "@/components/button";
import { CoordinatorError } from "@/components/coordinator-error";
import { HashChip } from "@/components/hash-chip";
import { BlockClock } from "@/components/motion/block-clock";
import { CountUp } from "@/components/motion/count-up";
import { Guilloche } from "@/components/motion/guilloche";
import { PhaseTrack } from "@/components/motion/phase-track";
import { Reveal } from "@/components/motion/reveal";
import { ZecCountUp } from "@/components/motion/zec-count-up";
import { SealMark } from "@/components/seal-mark";
import { StatusPill } from "@/components/status-pill";
import {
  acceptingBids,
  CoordinatorUnavailable,
  getAuction,
  getChainTip,
  getResult,
} from "@/lib/coordinator";
import { modeExplainer, modeLabel, zec } from "@/lib/format";
import type { AuctionView, PublishedResult } from "@/lib/types";

export const dynamic = "force-dynamic";

type Props = { params: Promise<{ id: string }> };

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { id } = await params;
  try {
    const auction = await getAuction(id);
    return { title: auction?.title ?? "Auction" };
  } catch {
    return { title: "Auction" };
  }
}

export default async function AuctionPage({ params }: Props) {
  const { id } = await params;
  let auction: AuctionView | null;
  try {
    auction = await getAuction(id);
  } catch (e) {
    if (e instanceof CoordinatorUnavailable) return <CoordinatorError message={e.message} />;
    throw e;
  }
  if (!auction) notFound();
  const [tip, result] = await Promise.all([
    getChainTip(),
    ["published", "refunding", "settled"].includes(auction.status)
      ? getResult(auction.code).catch(() => null)
      : Promise.resolve(null),
  ]);
  const biddingOpen = acceptingBids(auction, tip);
  const sealedMode = auction.bid_mode === "commit_reveal";
  const revealing =
    sealedMode &&
    auction.status === "open" &&
    tip !== null &&
    tip >= auction.close_height &&
    auction.reveal_close_height !== null &&
    tip <= auction.reveal_close_height;

  return (
    <div className="space-y-10">
      <header className="relative space-y-5">
        <Guilloche
          size={420}
          className="absolute -top-28 -right-24 -z-10 hidden opacity-60 md:block"
        />
        <Link
          href="/"
          className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-text"
        >
          <ArrowLeft size={15} aria-hidden="true" /> All auctions
        </Link>
        <div className="space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <StatusPill status={auction.status} />
            <span className="rounded-[var(--radius-chip)] border border-border px-2.5 py-1 text-[13px] text-muted">
              {modeLabel(auction.mode)}
              {sealedMode ? " · sealed reveal" : ""}
            </span>
          </div>
          <h1 className="max-w-[18ch] text-[36px] leading-[40px] font-semibold tracking-[-0.03em] text-balance md:text-[52px] md:leading-[54px]">
            {auction.title}
          </h1>
          {auction.description ? (
            <p className="max-w-[62ch] text-[17px] leading-7 text-muted">{auction.description}</p>
          ) : null}
        </div>
        <div className="max-w-[720px] pt-2">
          <PhaseTrack auction={auction} initialTip={tip} />
        </div>
      </header>

      <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_360px]">
        <article className="space-y-10">
          {/* On phones the bid action sits right under the header. */}
          <div className="lg:hidden">
            <BidCard auction={auction} tip={tip} open={biddingOpen} revealing={revealing} />
          </div>

          {result ? <ResultPanel auction={auction} result={result} /> : null}

          <Reveal as="section" className="space-y-4">
            <h2 className="text-[20px] leading-7 font-semibold tracking-tight">Rules</h2>
            <dl className="grid grid-cols-2 overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface shadow-[var(--shadow-card)] sm:grid-cols-4">
              <Rule label="Lots" value={<span className="num">{auction.quantity}</span>} />
              <Rule
                label="Reserve"
                value={<span className="num">{zec(auction.reserve_zat, auction.network)}</span>}
              />
              <Rule
                label="Bidding closes"
                value={<span className="num">#{auction.close_height.toLocaleString("en-US")}</span>}
              />
              <Rule
                label={sealedMode ? "Reveals until" : "Network"}
                value={
                  sealedMode && auction.reveal_close_height ? (
                    <span className="num">
                      #{auction.reveal_close_height.toLocaleString("en-US")}
                    </span>
                  ) : (
                    <span className="capitalize">{auction.network}</span>
                  )
                }
              />
            </dl>
            <p className="text-sm text-muted">{modeExplainer(auction.mode, auction.quantity)}</p>
          </Reveal>

          <Reveal as="section" className="space-y-4">
            <h2 className="text-[20px] leading-7 font-semibold tracking-tight">
              What stays private
            </h2>
            <ul className="grid gap-3 sm:grid-cols-2">
              <Fact icon={EyeOff} title="Your bid is shielded">
                While the auction runs, the public sees nothing: not you, not your amount.
              </Fact>
              {sealedMode ? (
                <Fact icon={Eye} title="Prices are sealed until you reveal">
                  The trustees see only deposits while bidding is open. Your bid counts once you
                  reveal it.
                </Fact>
              ) : (
                <Fact icon={Eye} title="Trustees see amounts live">
                  In this auction the escrow&rsquo;s trustees can see bid amounts while it runs.
                </Fact>
              )}
              <Fact icon={ScanSearch} title="The result is checkable">
                After close the viewing key is published. It shows amounts, never who sent them.
              </Fact>
              <Fact icon={Shuffle} title="Refunds can’t be matched to you">
                They come back in standard-sized pieces, at random times, mixed with other
                bidders&rsquo;.
              </Fact>
            </ul>
          </Reveal>
        </article>

        <aside className="space-y-6 lg:sticky lg:top-20 lg:self-start">
          <div className="hidden lg:block">
            <BidCard auction={auction} tip={tip} open={biddingOpen} revealing={revealing} />
          </div>
          <EscrowCard auction={auction} />
        </aside>
      </div>
    </div>
  );
}

function Rule({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="space-y-1 border-border p-4 odd:border-r sm:border-r sm:last:border-r-0 [&:nth-child(-n+2)]:border-b sm:[&:nth-child(-n+2)]:border-b-0">
      <dt className="text-[12px] text-muted">{label}</dt>
      <dd className="text-[17px]">{value}</dd>
    </div>
  );
}

function Fact({
  icon: Icon,
  title,
  children,
}: {
  icon: typeof Eye;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <li className="flex gap-3 rounded-[var(--radius-card)] p-1">
      <span className="mt-0.5 inline-flex size-8 shrink-0 items-center justify-center rounded-full bg-surface-2 text-primary">
        <Icon size={15} aria-hidden="true" />
      </span>
      <span className="space-y-0.5">
        <span className="block text-[15px] font-medium">{title}</span>
        <span className="block text-sm leading-6 text-muted">{children}</span>
      </span>
    </li>
  );
}

function ResultPanel({ auction, result }: { auction: AuctionView; result: PublishedResult }) {
  return (
    <Reveal
      as="section"
      className="relative overflow-hidden rounded-[var(--radius-sheet)] border border-primary/30 bg-surface p-6 shadow-[var(--shadow-lift)]"
    >
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-1">
          <p className="text-sm text-muted">Clearing price</p>
          {result.clearing_price_zat === null ? (
            <p className="text-[34px] leading-10 font-semibold">No winners</p>
          ) : (
            <ZecCountUp
              zat={result.clearing_price_zat}
              network={auction.network}
              className="block text-[40px] leading-[44px] font-semibold tracking-tight"
            />
          )}
        </div>
        <ButtonLink href={`/audit/${auction.code}`} variant="secondary">
          <ScanSearch size={16} aria-hidden="true" /> Recompute it yourself
        </ButtonLink>
      </div>
      <dl className="mt-6 grid grid-cols-3 gap-4 border-t border-border pt-5 text-sm">
        <div>
          <dt className="text-muted">Winners</dt>
          <dd className="text-[20px]">
            <CountUp value={result.winner_count} />{" "}
            <span className="text-muted">of {auction.quantity}</span>
          </dd>
        </div>
        <div>
          <dt className="text-muted">Valid bids</dt>
          <dd className="text-[20px]">
            <CountUp value={result.valid_bid_count} />
          </dd>
        </div>
        <div>
          <dt className="text-muted">Volume</dt>
          <dd className="text-[20px]">
            <ZecCountUp zat={result.total_volume_zat} network={auction.network} />
          </dd>
        </div>
      </dl>
      <div className="mt-5 flex flex-wrap items-center gap-2 text-sm text-muted">
        Merkle root of every bid <HashChip value={result.merkle_root} label="Merkle root" />
      </div>
    </Reveal>
  );
}

function BidCard({
  auction: a,
  tip,
  open,
  revealing,
}: {
  auction: AuctionView;
  tip: number | null;
  /** Accepting bids right now (status open and the chain is before the close height). */
  open: boolean;
  /** Commit-reveal auction in its reveal window. */
  revealing: boolean;
}) {
  return (
    <div className="space-y-5 rounded-[var(--radius-sheet)] border border-border bg-surface p-5 shadow-[var(--shadow-lift)]">
      {open || revealing || a.status === "open" ? (
        <BlockClock auction={a} initialTip={tip} network={a.network} />
      ) : (
        <div className="flex items-center gap-4">
          <SealMark size={64} stamp />
          <div>
            <p className="text-[20px] leading-7 font-semibold tracking-tight">
              {a.status === "settled"
                ? "Sealed and settled"
                : a.status === "cancelled"
                  ? "Cancelled"
                  : "Bidding closed"}
            </p>
            <p className="text-[13px] text-muted">
              Closed at block <span className="num">{a.close_height.toLocaleString("en-US")}</span>
            </p>
          </div>
        </div>
      )}
      {open ? (
        <>
          <ButtonLink
            href={`/a/${a.code}/bid`}
            size="lg"
            className="w-full shadow-[var(--glow-primary)]"
          >
            Place a sealed bid
          </ButtonLink>
          <div className="flex items-center justify-between border-t border-border pt-4 text-sm">
            <span className="text-muted">Sealed bids so far</span>
            {typeof a.bid_count === "number" ? (
              <CountUp value={a.bid_count} className="text-[18px] text-text" />
            ) : (
              <span className="text-muted">shown from 5</span>
            )}
          </div>
          <p className="text-[13px] text-muted">
            Bid from the wallet built into this site or any Zcash wallet. Cross-chain bidding
            (USDC/ETH) isn&rsquo;t available yet.
          </p>
        </>
      ) : (
        <p className="text-sm text-muted">
          {revealing ? (
            <>
              Bidding has closed. If you placed a sealed bid, reveal it from{" "}
              <Link href="/me" className="text-text underline underline-offset-4">
                My bids
              </Link>{" "}
              before the window ends.
            </>
          ) : a.status === "published" || a.status === "refunding" || a.status === "settled" ? (
            <>
              The result is public. Bid in this auction? Your receipt shows whether you won and how
              your refund arrives:{" "}
              <Link href="/me" className="text-text underline underline-offset-4">
                My bids
              </Link>{" "}
              or{" "}
              <Link href="/verify" className="text-text underline underline-offset-4">
                Verify
              </Link>
              .
            </>
          ) : (
            "Bidding is closed."
          )}
        </p>
      )}
    </div>
  );
}

function EscrowCard({ auction }: { auction: AuctionView }) {
  return (
    <div className="space-y-4 rounded-[var(--radius-card)] border border-border p-5 text-sm">
      <div className="flex items-center gap-2 font-medium">
        <KeyRound size={16} aria-hidden="true" className="text-primary" /> Escrow
        <span className="text-muted">· any 2 of 3 trustees sign</span>
      </div>
      {auction.escrow_ua ? <HashChip value={auction.escrow_ua} label="escrow address" /> : null}
      <ul className="flex flex-wrap gap-2">
        {auction.trustees.map((t) => (
          <li
            key={t.slot}
            className="inline-flex items-center gap-2 rounded-full border border-border py-1 pr-3 pl-1"
          >
            <span className="inline-flex size-6 items-center justify-center rounded-full bg-surface-2 text-[11px] font-medium text-text">
              {t.label.slice(0, 1).toUpperCase()}
            </span>
            {t.label}
          </li>
        ))}
      </ul>
      <p className="text-[13px] text-muted">
        Auction code <span className="num text-text">{auction.code}</span>
      </p>
    </div>
  );
}
