import { Placeholder } from "@/components/placeholder";

export const metadata = { title: "Console" };

export default function ConsolePage() {
  return (
    <div className="space-y-4">
      <h1 className="text-[28px] leading-[34px] font-semibold">Console</h1>
      <Placeholder screen="S13 Organiser console" task="3.10" />
    </div>
  );
}
