/**
 * The signature detail: a wax seal that stamps sealed bids and verified
 * receipts (docs/04-UI-UX-BRIEF.md §4.5). One 300 ms stamp; none with reduced motion.
 */
export function SealMark({ size = 56, stamp = false }: { size?: number; stamp?: boolean }) {
  const scallops = Array.from({ length: 16 }, (_, i) => {
    const a = (i / 16) * Math.PI * 2;
    return { cx: 32 + Math.cos(a) * 25, cy: 32 + Math.sin(a) * 25 };
  });
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      aria-hidden="true"
      className={stamp ? "seal-stamp" : undefined}
    >
      <g fill="var(--accent)">
        {scallops.map((s) => (
          <circle key={`${s.cx}-${s.cy}`} cx={s.cx} cy={s.cy} r="6" />
        ))}
        <circle cx="32" cy="32" r="25" />
      </g>
      <circle
        cx="32"
        cy="32"
        r="18"
        fill="none"
        stroke="var(--bg)"
        strokeOpacity="0.35"
        strokeWidth="1.5"
      />
      <text
        x="32"
        y="37.5"
        textAnchor="middle"
        fontSize="15"
        fontWeight="600"
        fill="var(--bg)"
        fillOpacity="0.85"
        style={{ fontFamily: "var(--font-plex-sans)" }}
      >
        SF
      </text>
    </svg>
  );
}
