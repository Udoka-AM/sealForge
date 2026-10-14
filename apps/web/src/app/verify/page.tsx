import type { Metadata } from "next";
import { VerifyReceipt } from "@/components/verify-receipt";

export const metadata: Metadata = { title: "Verify your bid" };

export default function VerifyPage() {
  return (
    <div className="mx-auto max-w-[640px] space-y-6">
      <div className="space-y-2">
        <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">Verify your bid</h1>
        <p className="text-muted">
          Check that your sealed bid was counted, whether it won, and how your refund will arrive.
          Your receipt never leaves this browser.
        </p>
      </div>
      <VerifyReceipt />
    </div>
  );
}
