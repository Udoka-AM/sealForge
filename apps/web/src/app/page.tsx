import { KeyRound, ScanSearch, Stamp } from "lucide-react";
import Link from "next/link";
import { ButtonLink } from "@/components/button";
import { CoordinatorError } from "@/components/coordinator-error";
import { StatusPill } from "@/components/status-pill";
import { CoordinatorUnavailable, getChainTip, listAuctions } from "@/lib/coordinator";
import { blocksEta, modeLabel, zec } from "@/lib/format";
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

const STEPS = [
  {
    icon: Stamp,
    title: "Bid sealed",
    text: "Pay from any Zcash wallet. Your bid is a shielded payment; your receipt stays on your device.",
  },
  {
    icon: KeyRound,
    title: "Held by 2-of-3 trustees",
    text: "Funds sit in an escrow no single party can move, and the auction server holds no key.",
  },
  {
    icon: ScanSearch,
    title: "Anyone can audit",
    text: "After close, the price and every outcome can be recomputed from the chain. Bidders stay hidden.",
  },
];

export default async function HomePage() {
  const { auctions, tip, error } = await load();
  const live = auctions.filter((a) => a.status === "open");
  const past = auctions.filter((a) => a.status !== "open");
  const published = (a: AuctionView) =>
    a.status === "published" || a.status === "refunding" || a.status === "settled";

  return (
    <div className="space-y-12">
      <section className="max-w-[640px] space-y-3">
        <h1 className="text-[32px] leading-9 font-semibold tracking-tight md:text-[40px] md:leading-[44px]">
          Sealed bids. Public price. Private bidders.
        </h1>
        <p className="text-muted">
          Bid from any Zcash wallet. After close, anyone can recompute the clearing price from the
          chain. Nobody can see who bid.
        </p>
        <div className="flex flex-wrap gap-2 pt-2">
          <ButtonLink href="/how-it-works" variant="secondary">
            How it works
          </ButtonLink>
          <ButtonLink href="/console" variant="ghost">
            Run your own auction
          </ButtonLink>
        </div>
      </section>

      <ol className="grid gap-6 md:grid-cols-3">
        {STEPS.map((s) => (
          <li key={s.title} className="space-y-2">
            <s.icon size={22} aria-hidden="true" className="text-primary" />
            <h2 className="font-semibold">{s.title}</h2>
            <p className="text-sm text-muted">{s.text}</p>
          </li>
        ))}
      </ol>

      {error ? <CoordinatorError message={error} /> : null}

      <section aria-labelledby="live" className="space-y-4">
        <h2 id="live" className="text-[22px] leading-7 font-semibold">
          Open for bids
        </h2>
        {live.length === 0 && !error ? (
          <p className="text-muted">No live auctions right now.</p>
        ) : (
          <ul className="grid gap-4 md:grid-cols-2">
            {live.map((a) => (
              <AuctionRow key={a.code} auction={a} tip={tip} />
            ))}
          </ul>
        )}
      </section>

      {past.length > 0 ? (
        <section aria-labelledby="past" className="space-y-4">
          <h2 id="past" className="text-[22px] leading-7 font-semibold">
            Results
          </h2>
          <ul className="divide-y divide-border border-y border-border">
            {past.map((a) => (
              <li key={a.code} className="flex items-center justify-between gap-4 py-3">
                <Link href={`/a/${a.code}`} className="min-w-0 truncate hover:text-primary">
                  {a.title}
                </Link>
                <span className="flex shrink-0 items-center gap-3">
                  {published(a) ? (
                    <Link
                      href={`/audit/${a.code}`}
                      className="text-sm text-muted underline-offset-2 hover:text-text hover:underline"
                    >
                      Audit
                    </Link>
                  ) : null}
                  <StatusPill status={a.status} />
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}

function AuctionRow({ auction: a, tip }: { auction: AuctionView; tip: number | null }) {
  const left = tip === null ? null : a.close_height - tip;
  return (
    <li>
      <Link
        href={`/a/${a.code}`}
        className="block rounded-[var(--radius-card)] border border-border bg-surface p-5 transition-[border-color] duration-150 hover:border-muted"
      >
        <div className="flex items-start justify-between gap-3">
          <h3 className="text-[18px] leading-6 font-semibold">{a.title}</h3>
          <StatusPill status={a.status} />
        </div>
        <dl className="mt-4 grid grid-cols-3 gap-3 text-sm">
          <div>
            <dt className="text-muted">Lots</dt>
            <dd className="num">{a.quantity}</dd>
          </div>
          <div>
            <dt className="text-muted">Reserve</dt>
            <dd className="num">{zec(a.reserve_zat)}</dd>
          </div>
          <div>
            <dt className="text-muted">Closes</dt>
            <dd className="num">
              {left === null
                ? `block ${a.close_height.toLocaleString("en-US")}`
                : left > 0
                  ? `in ${blocksEta(left)}`
                  : "closing"}
            </dd>
          </div>
        </dl>
        <p className="mt-3 text-[13px] text-muted">
          {modeLabel(a.mode)}
          {a.bid_mode === "commit_reveal" ? " · sealed reveal" : ""}
        </p>
      </Link>
    </li>
  );
}
