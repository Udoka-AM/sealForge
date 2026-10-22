import type { Metadata } from "next";
import { VerifyReceipt } from "@/components/verify-receipt";

export const metadata: Metadata = { title: "Verify your bid" };

export default function VerifyPage() {
  return (
    <div className="mx-auto max-w-[640px] space-y-6">
      <div className="space-y-2">
        <h1 className="text-[34px] leading-[38px] font-semibold tracking-[-0.025em] md:text-[40px] md:leading-[44px]">
          Verify your bid
        </h1>
        <p className="text-muted">
          Check that your sealed bid was counted, whether it won, and how your refund will arrive.
          Your receipt never leaves this browser.
        </p>
      </div>
      <VerifyReceipt />
    </div>
  );
}
