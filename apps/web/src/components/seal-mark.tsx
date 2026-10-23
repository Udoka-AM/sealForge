import { BOTTOM_FACET, HALO, HEX, S_CUT, TOP_FACET } from "@/lib/brand";

/**
 * The sealForge mark (lib/brand.ts). It is also the signature detail that stamps sealed bids and
 * verified receipts (docs/04-UI-UX-BRIEF.md §4.5): one 300 ms stamp; none with reduced motion.
 * From 40 px up it gains the broken outer hexagon.
 */
export function SealMark({ size = 56, stamp = false }: { size?: number; stamp?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      aria-hidden="true"
      className={stamp ? "seal-stamp" : undefined}
    >
      {size >= 40 ? (
        <path
          d={HALO}
          fill="none"
          stroke="var(--primary)"
          strokeOpacity="0.55"
          strokeWidth="1"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      ) : null}
      {/* evenodd turns the S into a hole, so the page shows through it. */}
      <path d={`${HEX}${S_CUT}`} fillRule="evenodd" fill="var(--primary)" />
      <path d={TOP_FACET} fill="#fff" fillOpacity="0.22" />
      <path d={BOTTOM_FACET} fill="#000" fillOpacity="0.2" />
    </svg>
  );
}
