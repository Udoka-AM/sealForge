"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { getSigner } from "@/lib/sfsig";

/** "Console" in the top bar, shown only when an organiser key is loaded here. */
export function ConsoleNavLink() {
  const [show, setShow] = useState(false);
  useEffect(() => {
    const check = () => getSigner("organiser").then((s) => setShow(s !== null));
    check();
    window.addEventListener("sf-signer-changed", check);
    return () => window.removeEventListener("sf-signer-changed", check);
  }, []);
  if (!show) return null;
  return (
    <Link href="/console" className="hidden py-2 hover:text-text sm:inline">
      Console
    </Link>
  );
}
