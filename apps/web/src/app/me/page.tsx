import type { Metadata } from "next";
import { MyBids } from "@/components/my-bids";

export const metadata: Metadata = { title: "My bids" };

export default function MyBidsPage() {
  return (
    <div className="mx-auto max-w-[640px] space-y-6">
      <div className="space-y-2">
        <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">My bids</h1>
        <p className="text-muted">
          Stored only in this browser. Download each receipt so you don&rsquo;t lose it.
        </p>
      </div>
      <MyBids />
    </div>
  );
}
