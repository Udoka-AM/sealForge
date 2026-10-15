"use client";

import { WifiOff } from "lucide-react";
import { useEffect, useState } from "react";

/** App Flow §3.7: a calm global notice while the browser is offline. */
export function OfflineBanner() {
  const [offline, setOffline] = useState(false);
  useEffect(() => {
    const update = () => setOffline(!navigator.onLine);
    update();
    window.addEventListener("online", update);
    window.addEventListener("offline", update);
    return () => {
      window.removeEventListener("online", update);
      window.removeEventListener("offline", update);
    };
  }, []);
  if (!offline) return null;
  return (
    <div role="status" className="border-b border-border bg-surface-2">
      <p className="mx-auto flex max-w-[1120px] items-center gap-2 px-4 py-2 text-sm md:px-6">
        <WifiOff size={16} aria-hidden="true" className="shrink-0 text-warning" />
        You&rsquo;re offline. Bids already sent are safe. We&rsquo;ll refresh when you&rsquo;re
        back.
      </p>
    </div>
  );
}
