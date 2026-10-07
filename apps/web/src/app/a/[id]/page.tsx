import { Placeholder } from "@/components/placeholder";

export default async function AuctionPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return (
    <div className="space-y-4">
      <h1 className="text-[28px] leading-[34px] font-semibold">
        Auction <span className="num">{id}</span>
      </h1>
      <Placeholder screen="S2 Auction page" task="3.6" />
    </div>
  );
}
