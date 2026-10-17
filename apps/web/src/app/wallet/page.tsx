import type { Metadata } from "next";
import { WalletPage } from "@/components/wallet/wallet-page";

export const metadata: Metadata = { title: "Wallet" };

export default function Wallet() {
  return (
    <div className="mx-auto max-w-[760px] space-y-6">
      <div className="space-y-2">
        <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">Wallet</h1>
        <p className="text-muted">
          A shielded Zcash wallet for bidding, built into this page. Fund it once, pay bids in a
          tap, and watch refunds arrive at a fresh address for every bid.
        </p>
      </div>
      <WalletPage />
    </div>
  );
}
