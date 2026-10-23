import type { Metadata } from "next";
import { PageHead } from "@/components/page-head";
import { WalletPage } from "@/components/wallet/wallet-page";

export const metadata: Metadata = { title: "Wallet" };

export default function Wallet() {
  return (
    <div className="space-y-12">
      <PageHead
        label="Wallet"
        title="A shielded wallet, in your browser."
        lead="Create one or link the wallet you already use. Pay bids in a tap; refunds arrive at a fresh address for every bid."
      />
      <div className="max-w-[760px]">
        <WalletPage />
      </div>
    </div>
  );
}
