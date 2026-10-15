import { notFound } from "next/navigation";
import { TrusteeSigning } from "@/components/console/trustee-signing";

export const metadata = { title: "Trustee signing" };

type Props = { params: Promise<{ code: string }> };

export default async function TrusteePage({ params }: Props) {
  const { code } = await params;
  if (!/^[0-9A-HJKMNP-TV-Z]{10}$/.test(code)) notFound();
  return (
    <div className="space-y-6">
      <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">
        Sign escrow transactions <span className="num text-muted">{code}</span>
      </h1>
      <TrusteeSigning code={code} />
    </div>
  );
}
