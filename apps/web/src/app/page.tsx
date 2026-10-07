import { Placeholder } from "@/components/placeholder";

export default function HomePage() {
  return (
    <div className="space-y-8">
      <section className="space-y-3">
        <h1 className="text-[32px] leading-9 font-semibold md:text-[40px] md:leading-[44px]">
          Sealed bids. Public price. Private bidders.
        </h1>
        <p className="max-w-[68ch] text-muted">
          Bid from any Zcash wallet, or from USDC and ETH through ShieldRamp. After close, anyone
          can recompute the clearing price from the chain. Nobody can see who bid.
        </p>
      </section>
      <Placeholder screen="S1 Home: live auctions" task="3.6" />
    </div>
  );
}
