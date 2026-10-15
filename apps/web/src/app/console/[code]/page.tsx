import { notFound } from "next/navigation";
import { ConsoleAuction } from "@/components/console/console-auction";

export const metadata = { title: "Auction console" };

type Props = { params: Promise<{ code: string }> };

export default async function ConsoleAuctionPage({ params }: Props) {
  const { code } = await params;
  if (!/^[0-9A-HJKMNP-TV-Z]{10}$/.test(code)) notFound();
  return <ConsoleAuction code={code} />;
}
