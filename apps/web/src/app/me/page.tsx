import type { Metadata } from "next";
import { MyBids } from "@/components/my-bids";

export const metadata: Metadata = { title: "My bids" };

export default function MyBidsPage() {
  return (
    <div className="mx-auto max-w-[640px] space-y-6">
      <div className="space-y-2">
        <h1 className="text-[34px] leading-[38px] font-semibold tracking-[-0.025em] md:text-[40px] md:leading-[44px]">
          My bids
        </h1>
        <p className="text-muted">
          Stored only in this browser. Download each receipt so you don&rsquo;t lose it.
        </p>
      </div>
      <MyBids />
    </div>
  );
}
