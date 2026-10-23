import { Calculator, GitMerge, Lock, ShieldCheck } from "lucide-react";
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { Stagger, StaggerItem } from "@/components/motion/reveal";
import { PageHead } from "@/components/page-head";
import { VerifyReceipt } from "@/components/verify-receipt";

export const metadata: Metadata = { title: "Verify your bid" };

export default function VerifyPage() {
  return (
    <div className="space-y-12">
      <PageHead
        label="Verify"
        title="Check your own bid."
        lead="Was it counted, did it win, and how will the refund arrive? Your receipt never leaves this browser."
      />
      <div className="grid items-start gap-x-8 gap-y-12 md:grid-cols-12">
        <div className="md:col-span-7">
          <VerifyReceipt />
        </div>
        <aside className="md:col-span-4 md:col-start-9" aria-labelledby="checks-h">
          <h2 id="checks-h" className="eyebrow mb-4">
            What this checks
          </h2>
          <Stagger className="space-y-px overflow-hidden rounded-[var(--radius-card)] border border-border bg-border">
            <Check icon={<GitMerge size={16} />} title="Inclusion">
              Your bid&rsquo;s Merkle proof against the published root, computed here.
            </Check>
            <Check icon={<Calculator size={16} />} title="Outcome">
              Whether you won, at what price, and the refund you&rsquo;re due.
            </Check>
            <Check icon={<ShieldCheck size={16} />} title="Your receipt">
              That its salt and refund address reproduce the commitment in your memo.
            </Check>
            <Check icon={<Lock size={16} />} title="Nothing sent">
              The receipt stays on this device. Only the public result is downloaded.
            </Check>
          </Stagger>
        </aside>
      </div>
    </div>
  );
}

function Check({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <StaggerItem className="flex gap-3 bg-surface p-4">
      <span
        aria-hidden="true"
        className="inline-flex size-8 shrink-0 items-center justify-center rounded-full border border-border text-primary"
      >
        {icon}
      </span>
      <span className="text-[14px] leading-6">
        <span className="block font-medium">{title}</span>
        <span className="text-muted">{children}</span>
      </span>
    </StaggerItem>
  );
}
