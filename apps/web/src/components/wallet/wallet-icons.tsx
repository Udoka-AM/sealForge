/**
 * Duotone illustrations for the wallet's three ways in, on a 48-unit grid: a tinted fill
 * (primary at low opacity) under a crisp primary stroke. Decorative; the card text carries the
 * meaning.
 */

const base = {
  width: 48,
  height: 48,
  viewBox: "0 0 48 48",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

/** A new seal being cut: the hexagon ingot with a spark at one corner. */
export function CreateIcon() {
  return (
    <svg {...base} aria-hidden="true">
      <path d="M22 6 37 14.7V32.3L22 41 7 32.3V14.7Z" fill="currentColor" fillOpacity="0.12" />
      <path d="M15.5 17.5H27l1.6 1.6v2.2h-7.2l7.2 6.6v3.3H17l-1.6-1.6v-2.1h7.2l-7.2-6.6Z" />
      <path d="M40 4v6M37 7h6" />
      <path d="M43 15.5v3M41.5 17h3" strokeWidth="1.2" />
    </svg>
  );
}

/** Two hexagons joined: the wallet you have, linked into this one. */
export function LinkIcon() {
  return (
    <svg {...base} aria-hidden="true">
      <path d="M15 9 25 14.8V26.2L15 32 5 26.2V14.8Z" fill="currentColor" fillOpacity="0.12" />
      <path d="M33 16 43 21.8V33.2L33 39 23 33.2V21.8Z" />
      <path d="M20.5 22.5h7M24.5 19.5l3 3-3 3" />
      <circle cx="15" cy="20.5" r="2" fill="currentColor" stroke="none" />
      <circle cx="33" cy="27.5" r="2" fill="currentColor" fillOpacity="0.35" stroke="none" />
    </svg>
  );
}

/** A phone showing a payment QR code: pay from the wallet you already use. */
export function QrIcon() {
  return (
    <svg {...base} aria-hidden="true">
      <rect x="12" y="4" width="24" height="40" rx="4" fill="currentColor" fillOpacity="0.12" />
      <path d="M21 8h6" />
      <rect x="16.5" y="13" width="6" height="6" rx="1" />
      <rect x="25.5" y="13" width="6" height="6" rx="1" />
      <rect x="16.5" y="22" width="6" height="6" rx="1" />
      <path d="M25.5 22h2v2M31.5 22v3M25.5 28h6M29 25.5v2.5" />
      <rect x="18.5" y="15" width="2" height="2" fill="currentColor" stroke="none" />
      <rect x="27.5" y="15" width="2" height="2" fill="currentColor" stroke="none" />
      <rect x="18.5" y="24" width="2" height="2" fill="currentColor" stroke="none" />
      <path d="M20 37h8" />
    </svg>
  );
}
