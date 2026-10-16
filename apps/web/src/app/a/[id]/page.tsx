import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { ButtonLink } from "@/components/button";
import { CoordinatorError } from "@/components/coordinator-error";
import { HashChip } from "@/components/hash-chip";
import { StatusPill } from "@/components/status-pill";
import { acceptingBids, CoordinatorUnavailable, getAuction, getChainTip } from "@/lib/coordinator";
import { modeExplainer, modeLabel, zec } from "@/lib/format";
import type { AuctionView } from "@/lib/types";

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
  const tip = await getChainTip();
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
    <div className="grid gap-10 lg:grid-cols-[minmax(0,1fr)_360px]">
      <article className="space-y-8">
        <header className="space-y-3">
          <StatusPill status={auction.status} />
          <h1 className="text-[32px] leading-9 font-semibold tracking-tight md:text-[40px] md:leading-[44px]">
            {auction.title}
          </h1>
          {auction.description ? (
            <p className="max-w-[68ch] text-muted">{auction.description}</p>
          ) : null}
        </header>

        {/* On phones the bid action sits under the title, not below the whole article. */}
        <div className="lg:hidden">
          <BidCard auction={auction} placement="inline" open={biddingOpen} revealing={revealing} />
        </div>

        <section aria-labelledby="rules" className="space-y-3">
          <h2 id="rules" className="text-[18px] leading-6 font-semibold">
            Rules
          </h2>
          <dl className="grid grid-cols-2 gap-x-6 gap-y-4 rounded-[var(--radius-card)] border border-border bg-surface p-5 sm:grid-cols-4">
            <div>
              <dt className="text-sm text-muted">Format</dt>
              <dd>
                {modeLabel(auction.mode)}
                {sealedMode ? ", sealed reveal" : ""}
              </dd>
            </div>
            <div>
              <dt className="text-sm text-muted">Lots</dt>
              <dd className="num">{auction.quantity}</dd>
            </div>
            <div>
              <dt className="text-sm text-muted">Reserve</dt>
              <dd className="num">{zec(auction.reserve_zat, auction.network)}</dd>
            </div>
            <div>
              <dt className="text-sm text-muted">Closes at block</dt>
              <dd className="num">{auction.close_height.toLocaleString("en-US")}</dd>
            </div>
          </dl>
          <p className="text-sm text-muted">{modeExplainer(auction.mode, auction.quantity)}</p>
        </section>

        <section aria-labelledby="privacy" className="space-y-2">
          <h2 id="privacy" className="text-[18px] leading-6 font-semibold">
            What stays private
          </h2>
          <ul className="max-w-[68ch] list-disc space-y-1.5 pl-5 text-muted">
            <li>
              Your bid is a shielded Zcash payment. While the auction runs, the public sees nothing.
            </li>
            <li>
              After close, the escrow&rsquo;s viewing key is published so anyone can check the
              price. It shows amounts, never who sent them.
            </li>
            {sealedMode ? (
              <li>
                Bids are sealed: while bidding is open, the escrow&rsquo;s viewers (the trustees
                included) see only deposits. You reveal your bid after the close, and only then does
                it count.
              </li>
            ) : (
              <li>The escrow&rsquo;s trustees can see bid amounts while the auction is open.</li>
            )}
            <li>
              Refunds come back in several standard-sized pieces over a few hours, so they
              can&rsquo;t be matched to your bid.
            </li>
          </ul>
        </section>
      </article>

      <aside className="space-y-6">
        <div className="hidden lg:block">
          <BidCard auction={auction} placement="aside" open={biddingOpen} revealing={revealing} />
        </div>

        <dl className="space-y-4 text-sm">
          {auction.escrow_ua ? (
            <div className="space-y-1.5">
              <dt className="text-muted">Escrow (2-of-3 trustees)</dt>
              <dd>
                <HashChip value={auction.escrow_ua} label="escrow address" />
              </dd>
            </div>
          ) : null}
          <div className="space-y-1.5">
            <dt className="text-muted">Trustees</dt>
            <dd>
              <ul className="space-y-1">
                {auction.trustees.map((t) => (
                  <li key={t.slot}>{t.label}</li>
                ))}
              </ul>
            </dd>
          </div>
          <div className="space-y-1.5">
            <dt className="text-muted">Auction code</dt>
            <dd className="num">{auction.code}</dd>
          </div>
        </dl>
      </aside>
    </div>
  );
}

function BidCard({
  auction: a,
  placement,
  open,
  revealing,
}: {
  auction: AuctionView;
  placement: "inline" | "aside";
  /** Accepting bids right now (status open and the chain is before the close height). */
  open: boolean;
  /** Commit-reveal auction in its reveal window. */
  revealing: boolean;
}) {
  const noteId = `ramp-note-${placement}`;
  return (
    <div className="space-y-3 rounded-[var(--radius-card)] border border-border bg-surface p-5">
      {open ? (
        <>
          <ButtonLink href={`/a/${a.code}/bid`} size="lg" className="w-full">
            Bid with Zcash
          </ButtonLink>
          <button
            type="button"
            disabled
            aria-describedby={noteId}
            className="h-11 w-full rounded-[var(--radius-control)] border border-border text-[15px] text-muted opacity-60"
          >
            Bid with USDC or ETH
          </button>
          <p id={noteId} className="text-[13px] text-muted">
            Cross-chain bidding (ShieldRamp) is coming soon.
          </p>
          <p className="border-t border-border pt-3 text-sm text-muted">
            {typeof a.bid_count === "number" ? (
              <>
                <span className="num text-text">{a.bid_count}</span> sealed bids so far
              </>
            ) : (
              "Be one of the first sealed bids. The count shows from 5 bids, so early activity stays private."
            )}
          </p>
        </>
      ) : (
        <p className="text-sm text-muted">
          {revealing
            ? `Bidding has closed. Reveal window: until block ${a.reveal_close_height?.toLocaleString("en-US")}. If you placed a sealed bid, reveal it from My bids.`
            : a.status === "published" || a.status === "refunding" || a.status === "settled"
              ? "Bidding has closed and the result is public."
              : "Bidding is closed."}
        </p>
      )}
    </div>
  );
}
