import {
  ArrowUpRight,
  Eye,
  EyeOff,
  FileSearch,
  KeyRound,
  ScanSearch,
  Shuffle,
  Stamp,
} from "lucide-react";
import type { ReactNode } from "react";
import { ButtonLink } from "@/components/button";
import { SectionHead } from "@/components/landing/section-head";
import { Reveal, Stagger, StaggerItem } from "@/components/motion/reveal";
import { PageHead } from "@/components/page-head";
import { StepTimeline } from "@/components/step-timeline";

export const metadata = { title: "How it works" };

/** S17: the threat model in plain words. The full version is docs/THREAT-MODEL.md. */
export default function HowItWorksPage() {
  return (
    <div className="space-y-24 md:space-y-32">
      <PageHead
        label="How it works"
        title="Private while you bid. Provable after."
        lead="A sealed-bid auction where the price is public, the bidders aren't, and you don't have to trust the organiser or us."
      />

      <section aria-labelledby="flow-h" className="space-y-12">
        <SectionHead
          index="01"
          label="Step by step"
          titleId="flow-h"
          title="An auction in four moves."
        />
        <StepTimeline
          steps={[
            {
              icon: <KeyRound size={18} />,
              title: "Three trustees hold the escrow",
              body: "They generate the escrow key together, each on their own machine. Any two sign refunds and the payout; one alone can't, and the server holds no key.",
            },
            {
              icon: <Stamp size={18} />,
              title: "You bid with a shielded payment",
              body: "One QR code, any Zcash wallet. The memo holds a sealed commitment and a fresh refund address. Your receipt is made in your browser.",
            },
            {
              icon: <ScanSearch size={18} />,
              title: "After close, anyone checks it",
              body: "The organiser publishes the price, every outcome, a Merkle root and the escrow's viewing key. Anyone can rescan the chain and recompute it.",
            },
            {
              icon: <Shuffle size={18} />,
              title: "Refunds come back in pieces",
              body: "What you didn't spend returns as standard-sized notes at random times, mixed with other bidders' refunds, so they can't be matched to you.",
            },
          ]}
        />
      </section>

      <section aria-labelledby="privacy-h" className="space-y-12">
        <SectionHead
          index="02"
          label="Privacy, honestly"
          titleId="privacy-h"
          title="What stays private, and what doesn't."
        />
        <div className="grid gap-4 md:grid-cols-2">
          <Panel tone="private" title="Private">
            <Fact title="Who bid.">
              The published viewing key shows amounts and memos arriving at the escrow, never who
              sent them.
            </Fact>
            <Fact title="Bid amounts, until close.">
              The public sees nothing while bidding is open. In <em>sealed reveal</em> auctions,
              nobody sees amounts, not even the trustees, until you reveal after the close.
            </Fact>
            <Fact title="Where your refund goes.">
              Refunds go to the fresh address in your memo, in pieces that blend in with everyone
              else&rsquo;s.
            </Fact>
          </Panel>
          <Panel tone="public" title="Not private">
            <Fact title="Amounts in ordinary auctions, to the operators.">
              The trustees and the auction server see bid amounts while bidding is open. The auction
              page says which kind of auction it is.
            </Fact>
            <Fact title="Amounts after close.">
              They&rsquo;re published so anyone can check the price. They aren&rsquo;t linked to
              you.
            </Fact>
            <Fact title="Your own wallet’s habits.">
              Reuse a refund address or move all your refunds at once, and you can undo the mixing
              yourself.
            </Fact>
          </Panel>
        </div>
      </section>

      <Reveal
        as="section"
        className="grid items-center gap-8 rounded-[var(--radius-card)] border border-border bg-surface p-7 md:grid-cols-12 md:p-10"
      >
        <div className="space-y-3 md:col-span-7">
          <p className="eyebrow flex gap-3">
            <span className="text-primary">03</span> Check us
          </p>
          <h2 className="display-2">Don&rsquo;t take our word for it.</h2>
          <p className="max-w-[52ch] text-muted">
            Every published auction has an audit page. For a check that trusts nothing on this site,
            run the open-source auditor against any Zcash node you choose.
          </p>
        </div>
        <div className="flex flex-wrap gap-2 md:col-span-5 md:justify-end">
          <ButtonLink href="/#live" size="lg" className="group">
            <FileSearch size={18} aria-hidden="true" /> Audit an auction
          </ButtonLink>
          <ButtonLink
            href="https://github.com/Udoka-AM/sealForge"
            size="lg"
            variant="secondary"
            className="group"
          >
            Threat model
            <ArrowUpRight
              size={16}
              aria-hidden="true"
              className="transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5"
            />
          </ButtonLink>
        </div>
      </Reveal>
    </div>
  );
}

function Panel({
  tone,
  title,
  children,
}: {
  tone: "private" | "public";
  title: string;
  children: ReactNode;
}) {
  const Icon = tone === "private" ? EyeOff : Eye;
  return (
    <div className="overflow-hidden rounded-[var(--radius-card)] border border-border bg-surface">
      <p
        className={`flex items-center gap-2 border-b border-border px-5 py-3 font-mono text-[11px] tracking-[0.16em] uppercase ${
          tone === "private" ? "text-success" : "text-warning"
        }`}
      >
        <Icon size={14} aria-hidden="true" /> {title}
      </p>
      <Stagger className="divide-y divide-border">{children}</Stagger>
    </div>
  );
}

function Fact({ title, children }: { title: string; children: ReactNode }) {
  return (
    <StaggerItem className="px-5 py-4 text-[15px] leading-6">
      <span className="block font-medium">{title}</span>
      <span className="text-muted">{children}</span>
    </StaggerItem>
  );
}
