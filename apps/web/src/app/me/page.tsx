import { CircleCheck, Coins, GitMerge, Lock, Undo2 } from "lucide-react";
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { Stagger, StaggerItem } from "@/components/motion/reveal";
import { MyBids } from "@/components/my-bids";
import { PageHead } from "@/components/page-head";

export const metadata: Metadata = { title: "My bids" };

export default function MyBidsPage() {
  return (
    <div className="space-y-12">
      <PageHead
        label="My bids"
        title="Every bid, from the chain."
        lead="Stored only in this browser. Download your receipts so you don't lose them."
      />
      <div className="grid items-start gap-x-8 gap-y-12 md:grid-cols-12">
        <div className="md:col-span-8">
          <MyBids />
        </div>
        <aside className="md:col-span-3 md:col-start-10" aria-labelledby="life-h">
          <h2 id="life-h" className="eyebrow mb-4">
            A bid&rsquo;s life
          </h2>
          <Stagger as="ol" className="relative space-y-5 border-l border-border pl-5">
            <Stage icon={<Lock size={13} />} title="Paid">
              The payment is confirmed in a block.
            </Stage>
            <Stage icon={<GitMerge size={13} />} title="Included">
              It&rsquo;s in the published result; the Merkle proof checks in your browser.
            </Stage>
            <Stage icon={<CircleCheck size={13} />} title="Won or not">
              Decided by the clearing price, the same for every winner.
            </Stage>
            <Stage icon={<Undo2 size={13} />} title="Refunded">
              The rest returns in standard pieces at random times.
            </Stage>
            <Stage icon={<Coins size={13} />} title="Settled">
              Refunds and the payout are confirmed on chain.
            </Stage>
          </Stagger>
          <p className="mt-6 text-[13px] leading-5 text-muted">
            A step is marked done only with evidence: a block, a proof or a received note.
          </p>
        </aside>
      </div>
    </div>
  );
}

function Stage({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <StaggerItem className="relative text-[14px] leading-6">
      <span
        aria-hidden="true"
        className="absolute top-0.5 -left-[31px] inline-flex size-[21px] items-center justify-center rounded-full border border-border bg-bg text-primary"
      >
        {icon}
      </span>
      <span className="block font-medium">{title}</span>
      <span className="text-muted">{children}</span>
    </StaggerItem>
  );
}
