"use client";

import { m } from "motion/react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { getSigner } from "@/lib/sfsig";
import { useWallet } from "./wallet/wallet-provider";

type Item = { href: string; label: string; match: (p: string) => boolean };

const ITEMS: Item[] = [
  { href: "/", label: "Auctions", match: (p) => p === "/" || p.startsWith("/a/") },
  { href: "/verify", label: "Verify", match: (p) => p.startsWith("/verify") },
  {
    href: "/how-it-works",
    label: "How it works",
    match: (p) => p.startsWith("/how-it-works"),
  },
  { href: "/me", label: "My bids", match: (p) => p.startsWith("/me") },
];

/** Top-bar links. The active page's pill glides between them. */
export function NavLinks() {
  const pathname = usePathname() ?? "/";
  const wallet = useWallet();
  const [organiser, setOrganiser] = useState(false);
  useEffect(() => {
    const check = () => getSigner("organiser").then((s) => setOrganiser(s !== null));
    check();
    window.addEventListener("sf-signer-changed", check);
    return () => window.removeEventListener("sf-signer-changed", check);
  }, []);

  const items: Item[] = [...ITEMS];
  // Always listed: when this deployment can't run the wallet, the page says why.
  items.push({
    href: "/wallet",
    label: "Wallet",
    match: (p) => p.startsWith("/wallet"),
  });
  if (organiser)
    items.push({
      href: "/console",
      label: "Console",
      match: (p) => p.startsWith("/console"),
    });

  return (
    <nav aria-label="Main" className="flex items-center gap-0.5 text-sm">
      {items.map((item) => {
        const active = item.match(pathname);
        const dot =
          item.href === "/wallet" && (wallet.status === "unlocked" || wallet.status === "locked")
            ? wallet.status === "unlocked"
              ? "var(--success)"
              : "var(--text-muted)"
            : null;
        return (
          <Link
            key={item.href}
            href={item.href}
            aria-current={active ? "page" : undefined}
            className={`relative inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-[var(--radius-chip)] px-2.5 py-1.5 transition-colors duration-150 ${
              active ? "text-text" : "text-muted hover:text-text"
            }`}
          >
            {active ? (
              <m.span
                layoutId="nav-active"
                className="absolute inset-0 -z-10 rounded-[var(--radius-chip)] bg-surface-2"
                transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
              />
            ) : null}
            {dot ? (
              <span
                aria-hidden="true"
                className="size-1.5 rounded-full"
                style={{ background: dot }}
              />
            ) : null}
            {item.label}
            {item.href === "/wallet" && wallet.status === "unlocked" ? (
              <span className="sr-only"> (unlocked)</span>
            ) : null}
          </Link>
        );
      })}
    </nav>
  );
}
