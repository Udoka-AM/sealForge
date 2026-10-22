import Link from "next/link";
import { BlockHeartbeat } from "./motion/block-heartbeat";
import { NavLinks } from "./nav-links";
import { SealMark } from "./seal-mark";

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-40 border-b border-border bg-bg/80 backdrop-blur-md supports-[backdrop-filter]:bg-bg/65">
      <div className="mx-auto flex h-14 w-full max-w-[1120px] items-center justify-between gap-3 px-4 md:px-6">
        <Link
          href="/"
          className="group inline-flex items-center gap-2 font-semibold tracking-tight"
        >
          <span className="transition-transform duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:-rotate-12">
            <SealMark size={22} />
          </span>
          <span>
            seal<span className="text-primary">Forge</span>
          </span>
        </Link>
        <div className="flex items-center gap-3">
          <BlockHeartbeat />
          <NavLinks />
        </div>
      </div>
    </header>
  );
}
