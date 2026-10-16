import type { NextConfig } from "next";

// Production CSP. Scripts and WebAssembly (auditor, trustee signer) come only from this
// origin; 'unsafe-inline' covers Next's hydration scripts. Framing is refused so the
// console and trustee pages, which sign with keys stored here, can't be clickjacked.
// Dev mode needs eval for React Refresh, so the policy applies to production builds only.
function lightwalletdOrigin(): string {
  try {
    const url = process.env.LIGHTWALLETD_GRPCWEB ?? process.env.NEXT_PUBLIC_LIGHTWALLETD_GRPCWEB;
    return new URL(url ?? "").origin;
  } catch {
    return "";
  }
}

const csp = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self'",
  // The Audit page rescans the chain from a lightwallet server the auditor chooses.
  `connect-src 'self' https: ${lightwalletdOrigin()}`.trim(),
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "frame-ancestors 'none'",
].join("; ");

const nextConfig: NextConfig = {
  reactStrictMode: true,
  transpilePackages: ["@sealforge/shieldramp", "@sealforge/bid"],
  poweredByHeader: false,
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          { key: "Referrer-Policy", value: "no-referrer" },
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=()" },
          { key: "X-Frame-Options", value: "DENY" },
          ...(process.env.NODE_ENV === "production"
            ? [{ key: "Content-Security-Policy", value: csp }]
            : []),
        ],
      },
    ];
  },
};

export default nextConfig;
