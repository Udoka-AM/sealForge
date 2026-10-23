import Link from "next/link";
import { BlockHeartbeat } from "./motion/block-heartbeat";
import { NavLinks } from "./nav-links";
import { SealMark } from "./seal-mark";

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-40 border-b border-border bg-bg/80 backdrop-blur-md supports-[backdrop-filter]:bg-bg/65">
      <div className="mx-auto flex w-full max-w-[1120px] flex-wrap items-center justify-between gap-x-3 px-4 md:h-14 md:flex-nowrap md:px-6">
        <Link
          href="/"
          className="group inline-flex h-12 items-center gap-2 font-semibold tracking-tight md:h-auto"
        >
          <span className="transition-transform duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:-rotate-12">
            <SealMark size={22} />
          </span>
          <span>
            seal<span className="text-primary">Forge</span>
          </span>
        </Link>
        <BlockHeartbeat className="inline-flex md:hidden" />
        {/* Phones: the links get their own row and scroll sideways instead of wrapping. */}
        <div className="no-scrollbar -mx-4 flex w-[calc(100%+2rem)] items-center gap-3 overflow-x-auto px-4 pb-2 [mask-image:linear-gradient(to_right,black_85%,transparent)] md:mx-0 md:w-auto md:overflow-visible md:p-0 md:[mask-image:none]">
          <BlockHeartbeat className="hidden md:inline-flex" />
          <NavLinks />
        </div>
      </div>
    </header>
  );
}
