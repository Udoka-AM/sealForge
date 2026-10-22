import Link from "next/link";
import { ConsoleNavLink } from "./console-nav-link";
import { WalletNavLink } from "./wallet/wallet-nav-link";

const NAV = [
  { href: "/", label: "Auctions", mobile: true },
  { href: "/verify", label: "Verify", mobile: false },
  { href: "/how-it-works", label: "How it works", mobile: false },
  { href: "/me", label: "My bids", mobile: true },
];

export function SiteHeader() {
  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-14 w-full max-w-[1120px] items-center justify-between px-4 md:px-6">
        <Link href="/" className="font-semibold tracking-tight">
          seal<span className="text-primary">Forge</span>
        </Link>
        <nav aria-label="Main" className="flex gap-4 text-sm text-muted">
          {NAV.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className={`py-2 hover:text-text ${item.mobile ? "" : "hidden sm:inline"}`}
            >
              {item.label}
            </Link>
          ))}
          <WalletNavLink />
          <ConsoleNavLink />
        </nav>
      </div>
    </header>
  );
}
