"use client";

import { Button, ButtonLink } from "@/components/button";

/** S18: anything unexpected. Bids and refunds never depend on this page. */
export default function ErrorPage({ reset }: { error: Error; reset: () => void }) {
  return (
    <div className="max-w-[480px] space-y-4">
      <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">
        Something went wrong
      </h1>
      <p className="text-muted">
        This page hit an error. Bids already sent are safe: they&rsquo;re payments on Zcash, not
        records on this site. Receipts saved in this browser are still here.
      </p>
      <div className="flex flex-wrap gap-2">
        <Button onClick={reset}>Try again</Button>
        <ButtonLink href="/" variant="secondary">
          Back to auctions
        </ButtonLink>
      </div>
    </div>
  );
}
