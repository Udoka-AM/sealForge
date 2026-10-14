import Link from "next/link";
import { CoordinatorError } from "@/components/coordinator-error";
import { StatusPill } from "@/components/status-pill";
import { CoordinatorUnavailable, listAuctions } from "@/lib/coordinator";
import { modeLabel, zec } from "@/lib/format";
import type { AuctionView } from "@/lib/types";

export const dynamic = "force-dynamic";

async function load(): Promise<{ auctions: AuctionView[]; error?: string }> {
  try {
    return { auctions: await listAuctions() };
  } catch (e) {
    if (e instanceof CoordinatorUnavailable) return { auctions: [], error: e.message };
    throw e;
  }
}

export default async function HomePage() {
  const { auctions, error } = await load();
  const live = auctions.filter((a) => a.status === "open");
  const past = auctions.filter((a) => a.status !== "open");

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
      </section>

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
              <AuctionRow key={a.code} auction={a} />
            ))}
          </ul>
        )}
      </section>

      {past.length > 0 ? (
        <section aria-labelledby="past" className="space-y-4">
          <h2 id="past" className="text-[22px] leading-7 font-semibold">
            Recently closed
          </h2>
          <ul className="divide-y divide-border border-y border-border">
            {past.map((a) => (
              <li key={a.code}>
                <Link
                  href={`/a/${a.code}`}
                  className="flex items-center justify-between gap-4 py-3 hover:text-primary"
                >
                  <span className="truncate">{a.title}</span>
                  <StatusPill status={a.status} />
                </Link>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}

function AuctionRow({ auction: a }: { auction: AuctionView }) {
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
            <dt className="text-muted">Closes at block</dt>
            <dd className="num">{a.close_height.toLocaleString("en-US")}</dd>
          </div>
        </dl>
        <p className="mt-3 text-[13px] text-muted">{modeLabel(a.mode)}</p>
      </Link>
    </li>
  );
}
