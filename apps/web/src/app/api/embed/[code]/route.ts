import { NextResponse } from "next/server";
import { acceptingBids, CoordinatorUnavailable, getAuction, getChainTip } from "@/lib/coordinator";

/**
 * Public card data for the embeddable widget (public/embed.js, task 4.4).
 * Only fields already shown on the public auction page. CORS is open so any
 * organiser's site can render the card; nothing here is per-user.
 */
const CORS = {
  "access-control-allow-origin": "*",
  "cache-control": "public, max-age=30",
};

export async function GET(req: Request, { params }: { params: Promise<{ code: string }> }) {
  const { code } = await params;
  try {
    const [auction, tip] = await Promise.all([getAuction(code), getChainTip()]);
    if (!auction) return NextResponse.json({ error: "not found" }, { status: 404, headers: CORS });
    const origin = new URL(req.url).origin;
    return NextResponse.json(
      {
        code: auction.code,
        title: auction.title,
        status: auction.status,
        accepting_bids: acceptingBids(auction, tip),
        sealed_reveal: auction.bid_mode === "commit_reveal",
        mode: auction.mode,
        quantity: auction.quantity,
        reserve_zat: auction.reserve_zat,
        close_height: auction.close_height,
        blocks_left: tip === null ? null : Math.max(0, auction.close_height - tip),
        bid_count: auction.bid_count,
        url: `${origin}/a/${auction.code}`,
        audit_url: `${origin}/audit/${auction.code}`,
      },
      { headers: CORS },
    );
  } catch (e) {
    if (e instanceof CoordinatorUnavailable)
      return NextResponse.json({ error: "unavailable" }, { status: 503, headers: CORS });
    throw e;
  }
}
