import Link from "next/link";

const NAV = [
  { href: "/", label: "Auctions" },
  { href: "/verify", label: "Verify" },
  { href: "/how-it-works", label: "How it works" },
  { href: "/me", label: "My bids" },
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
            <Link key={item.href} href={item.href} className="hover:text-text">
              {item.label}
            </Link>
          ))}
        </nav>
      </div>
    </header>
  );
}
