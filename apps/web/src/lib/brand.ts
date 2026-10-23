/**
 * The sealForge mark, on a 64-unit grid: a forged hexagonal ingot with an angular S cut through
 * it. The S is a mirrored Z (for Zcash) and has half-turn symmetry about the centre, so it reads
 * the same stamped either way up. `app/icon.svg` repeats these paths for the favicon.
 */

/** Pointy-top hexagon, radius 28. */
export const HEX = "M32 4 56.2 18V46L32 60 7.8 46V18Z";

/** The angular S, with chamfered top-right and bottom-left corners. Sits wholly inside HEX. */
export const S_CUT = "M21 20H40L43 23V26H30L43 38V44H24L21 41V38H34L21 26Z";

/** Bevels above and below the S: a lit top facet and a shaded bottom one. */
export const TOP_FACET = "M32 4 56.2 18H7.8Z";
export const BOTTOM_FACET = "M7.8 46H56.2L32 60Z";

/** A broken outer hexagon (radius 31.5): six edges with the corners left open. */
export const HALO = Array.from({ length: 6 }, (_, i) => {
  const at = (k: number, t: number) => {
    const a = ((k * 60 - 90) * Math.PI) / 180;
    const b = (((k + 1) * 60 - 90) * Math.PI) / 180;
    const x = 32 + 31.5 * (Math.cos(a) + (Math.cos(b) - Math.cos(a)) * t);
    const y = 32 + 31.5 * (Math.sin(a) + (Math.sin(b) - Math.sin(a)) * t);
    return `${x.toFixed(2)} ${y.toFixed(2)}`;
  };
  return `M${at(i, 0.18)}L${at(i, 0.82)}`;
}).join("");
