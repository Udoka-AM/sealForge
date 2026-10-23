/**
 * The home page's opening curtain (components/landing/preloader.tsx). Plain constants, so the
 * server layout can inline the gate script.
 */

export const CURTAIN_KEY = "sf-preloaded";

/** Fired as the curtain starts to lift, so the hero can time its entrance to it. */
export const CURTAIN_EVENT = "sf:curtain";

/**
 * Runs before hydration (app/layout.tsx): shows the curtain on a first visit to the home page in
 * this tab, unless the user prefers reduced motion. CSS keeps it hidden otherwise, including
 * without JavaScript.
 */
export const CURTAIN_GATE = `try{if(location.pathname==="/"&&!sessionStorage.getItem("${CURTAIN_KEY}")&&!matchMedia("(prefers-reduced-motion: reduce)").matches)document.documentElement.classList.add("sf-curtain")}catch(_){}`;

/** True when no curtain is showing (repeat visit, reduced motion, or already lifted). */
export function curtainUp(): boolean {
  const el = document.querySelector("[data-curtain]");
  return !el || getComputedStyle(el).display === "none";
}
