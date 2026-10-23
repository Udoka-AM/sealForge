import { ImageResponse } from "next/og";
import { BOTTOM_FACET, HEX, S_CUT, TOP_FACET } from "@/lib/brand";

export const size = { width: 180, height: 180 };
export const contentType = "image/png";

/** Home-screen icon: the mark on the dark background (iOS fills transparency with black). */
export default function AppleIcon() {
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "#0b0d10",
      }}
    >
      <svg width="124" height="124" viewBox="0 0 64 64" aria-hidden="true">
        <path d={`${HEX}${S_CUT}`} fillRule="evenodd" fill="#f4b728" />
        <path d={TOP_FACET} fill="#fff" fillOpacity="0.22" />
        <path d={BOTTOM_FACET} fill="#000" fillOpacity="0.2" />
      </svg>
    </div>,
    size,
  );
}
