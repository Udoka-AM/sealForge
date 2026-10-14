"use client";

import { Check, Copy } from "lucide-react";
import { useState } from "react";

/** Middle-ellipsis hash or address with a copy button. */
export function HashChip({
  value,
  label,
  head = 8,
  tail = 6,
}: {
  value: string;
  label: string;
  head?: number;
  tail?: number;
}) {
  const [copied, setCopied] = useState(false);
  const short =
    value.length > head + tail + 1 ? `${value.slice(0, head)}…${value.slice(-tail)}` : value;
  return (
    <span className="inline-flex max-w-full items-center gap-1.5 rounded-[var(--radius-chip)] bg-surface-2 py-1 pr-1 pl-2.5">
      <span className="num truncate text-[13px]" title={value}>
        {short}
      </span>
      <button
        type="button"
        aria-label={copied ? `${label} copied` : `Copy ${label}`}
        className="inline-flex size-7 shrink-0 items-center justify-center rounded-[6px] text-muted transition-colors duration-150 hover:bg-border hover:text-text"
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(value);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          } catch {
            // Clipboard denied: the full value stays selectable in the title tooltip.
          }
        }}
      >
        {copied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
      </button>
    </span>
  );
}
