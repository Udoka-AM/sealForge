import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { BidFlow } from "@/components/bid-flow";
import { ButtonLink } from "@/components/button";
import { CoordinatorError } from "@/components/coordinator-error";
import { CoordinatorUnavailable, getAuction } from "@/lib/coordinator";
import type { AuctionView } from "@/lib/types";

export const dynamic = "force-dynamic";
export const metadata: Metadata = { title: "Place a sealed bid" };

export default async function BidPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  let auction: AuctionView | null;
  try {
    auction = await getAuction(id);
  } catch (e) {
    if (e instanceof CoordinatorUnavailable) return <CoordinatorError message={e.message} />;
    throw e;
  }
  if (!auction) notFound();
  if (auction.status !== "open" || !auction.escrow_ua) {
    return (
      <div className="mx-auto max-w-[480px] space-y-4">
        <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">
          Bidding is closed
        </h1>
        <p className="text-muted">
          This auction isn&rsquo;t accepting bids. It closed at block{" "}
          <span className="num">{auction.close_height.toLocaleString("en-US")}</span>.
        </p>
        <ButtonLink href={`/a/${auction.code}`} variant="secondary">
          Back to the auction
        </ButtonLink>
      </div>
    );
  }
  return <BidFlow auction={auction} />;
}
