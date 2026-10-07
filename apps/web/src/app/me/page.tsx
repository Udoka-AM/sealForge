import { Placeholder } from "@/components/placeholder";

export const metadata = { title: "My bids" };

export default function MybidsPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-[28px] leading-[34px] font-semibold">My bids</h1>
      <Placeholder screen="S9 My bids" task="3.7" />
    </div>
  );
}
