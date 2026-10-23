"use client";

import { usePathname } from "next/navigation";
import { Preloader } from "./preloader";
import { ScrollRing } from "./scroll-ring";

/**
 * Fixed-position pieces of the home page. They mount from the root layout, outside the page
 * transition (whose transform would otherwise become their containing block).
 */
export function HomeChrome() {
  const pathname = usePathname();
  if (pathname !== "/") return null;
  return (
    <>
      <Preloader />
      <ScrollRing />
    </>
  );
}
