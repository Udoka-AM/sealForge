import { CreateAuctionForm } from "@/components/console/create-auction";
import type { Network } from "@/lib/types";

export const metadata = { title: "New auction" };

function defaultNetwork(): Network {
  const n = process.env.NEXT_PUBLIC_DEFAULT_NETWORK;
  return n === "mainnet" || n === "regtest" ? n : "testnet";
}

export default function NewAuctionPage() {
  return (
    <div className="space-y-6">
      <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">New auction</h1>
      <CreateAuctionForm defaultNetwork={defaultNetwork()} />
    </div>
  );
}
