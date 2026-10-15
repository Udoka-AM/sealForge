import { ConsoleHome } from "@/components/console/console-home";

export const metadata = { title: "Console" };

export default function ConsolePage() {
  return (
    <div className="space-y-6">
      <h1 className="text-[28px] leading-[34px] font-semibold tracking-tight">Console</h1>
      <ConsoleHome />
    </div>
  );
}
