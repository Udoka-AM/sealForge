import { Eye, EyeOff, KeyRound, ScanSearch, Shuffle, Stamp } from "lucide-react";
import Link from "next/link";
import type { ReactNode } from "react";
import { Guilloche } from "@/components/motion/guilloche";
import { Reveal } from "@/components/motion/reveal";

export const metadata = { title: "How it works" };

const h2 = "text-[22px] leading-7 font-semibold tracking-tight";

function Step({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <li className="relative grid grid-cols-[40px_minmax(0,1fr)] gap-x-4 gap-y-1 pb-8 last:pb-0 [&:last-child>span:first-child]:hidden">
      {/* The rail between steps. */}
      <span aria-hidden="true" className="absolute top-11 bottom-1 left-5 w-px bg-border" />
      <span
        className="relative z-10 inline-flex size-10 items-center justify-center rounded-full border border-border bg-surface text-primary shadow-[var(--shadow-card)]"
        aria-hidden="true"
      >
        {icon}
      </span>
      <h3 className="self-center font-semibold">{title}</h3>
      <p className="col-start-2 leading-7 text-muted">{children}</p>
    </li>
  );
}

/** S17: the threat model in plain words. The full version is docs/THREAT-MODEL.md. */
export default function HowItWorksPage() {
  return (
    <div className="max-w-[720px] space-y-12">
      <header className="relative space-y-3">
        <Guilloche
          size={360}
          className="absolute -top-24 -right-40 -z-10 hidden opacity-50 md:block"
        />
        <h1 className="text-[40px] leading-[44px] font-semibold tracking-[-0.03em] md:text-[52px] md:leading-[54px]">
          How it works
        </h1>
        <p className="text-muted">
          A sealed-bid auction where the price is public, the bidders aren&rsquo;t, and you
          don&rsquo;t have to trust the organiser or us. Here&rsquo;s what happens, what stays
          private, and what doesn&rsquo;t.
        </p>
      </header>

      <Reveal as="section" className="space-y-6">
        <h2 id="flow" className={h2}>
          An auction, step by step
        </h2>
        <ol className="space-y-6">
          <Step icon={<KeyRound size={18} />} title="Three trustees hold the escrow">
            Before bidding opens, three trustees generate the escrow key together, each on their own
            machine. Any two of them can sign refunds and the payout; one alone can&rsquo;t, and the
            auction server holds no key at all.
          </Step>
          <Step icon={<Stamp size={18} />} title="You bid with a shielded payment">
            You scan one QR code with any Zcash wallet. The payment goes to the escrow in
            Zcash&rsquo;s shielded pool, with a memo holding a sealed commitment and a fresh refund
            address. Your receipt is created in your browser and never sent to us.
          </Step>
          <Step icon={<ScanSearch size={18} />} title="After close, anyone can check the result">
            The organiser publishes the clearing price, every bid&rsquo;s outcome, a Merkle root and
            the escrow&rsquo;s viewing key. With that key, anyone can rescan the chain and recompute
            the result. If a single number differs, the audit fails. Your receipt proves your own
            bid was counted.
          </Step>
          <Step icon={<Shuffle size={18} />} title="Refunds come back in pieces, on purpose">
            Whatever you didn&rsquo;t spend comes back as several standard-sized notes, released at
            random times over a few hours and mixed with other bidders&rsquo; refunds, so they
            can&rsquo;t be matched to your bid.
          </Step>
        </ol>
      </Reveal>

      <Reveal as="section" className="space-y-4">
        <h2 id="private" className={h2}>
          What stays private
        </h2>
        <ul className="space-y-3">
          <li className="flex gap-3">
            <EyeOff size={18} aria-hidden="true" className="mt-1 shrink-0 text-success" />
            <span>
              <span className="font-medium">Who bid.</span>{" "}
              <span className="text-muted">
                The published viewing key shows amounts and memos arriving at the escrow, never who
                sent them.
              </span>
            </span>
          </li>
          <li className="flex gap-3">
            <EyeOff size={18} aria-hidden="true" className="mt-1 shrink-0 text-success" />
            <span>
              <span className="font-medium">Bid amounts, until close.</span>{" "}
              <span className="text-muted">
                The public sees nothing while bidding is open. In auctions marked{" "}
                <em>sealed reveal</em>, nobody sees amounts at all, not even the trustees: you
                deposit a standard budget and reveal your bid after the close.
              </span>
            </span>
          </li>
          <li className="flex gap-3">
            <EyeOff size={18} aria-hidden="true" className="mt-1 shrink-0 text-success" />
            <span>
              <span className="font-medium">Where your refund goes.</span>{" "}
              <span className="text-muted">
                Refunds go to the fresh address in your memo, in pieces that blend in with everyone
                else&rsquo;s.
              </span>
            </span>
          </li>
        </ul>
      </Reveal>

      <Reveal as="section" className="space-y-4">
        <h2 id="not-private" className={h2}>
          What doesn&rsquo;t
        </h2>
        <ul className="space-y-3">
          <li className="flex gap-3">
            <Eye size={18} aria-hidden="true" className="mt-1 shrink-0 text-warning" />
            <span className="text-muted">
              <span className="font-medium text-text">
                In ordinary auctions, the trustees and the auction server see bid amounts while
                bidding is open.
              </span>{" "}
              The auction page says which kind of auction it is.
            </span>
          </li>
          <li className="flex gap-3">
            <Eye size={18} aria-hidden="true" className="mt-1 shrink-0 text-warning" />
            <span className="text-muted">
              <span className="font-medium text-text">After close, amounts are public</span>, so
              anyone can check the price. They aren&rsquo;t linked to you.
            </span>
          </li>
          <li className="flex gap-3">
            <Eye size={18} aria-hidden="true" className="mt-1 shrink-0 text-warning" />
            <span className="text-muted">
              <span className="font-medium text-text">Your own wallet&rsquo;s history.</span> If you
              reuse a refund address or move refunds out all at once, you can undo the mixing
              yourself.
            </span>
          </li>
        </ul>
      </Reveal>

      <Reveal as="section" className="space-y-3">
        <h2 id="check" className={h2}>
          Check us
        </h2>
        <p className="text-muted">
          Every published auction has an{" "}
          <Link href="/" className="text-text underline">
            audit page
          </Link>
          . For a check that trusts nothing on this site, run the open-source auditor against any
          Zcash node you choose. The full threat model, security review and evidence are in{" "}
          <a href="https://github.com/Udoka-AM/sealForge" className="text-text underline">
            the repository
          </a>
          .
        </p>
      </Reveal>
    </div>
  );
}
