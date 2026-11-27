import type { NextConfig } from "next";

import { assetCsp } from "./src/lib/csp";

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
        ],
      },
      // Pages get a per-response nonce CSP from src/proxy.ts. Workers and the
      // WebAssembly modules' files get this fixed one.
      ...(process.env.NODE_ENV === "production"
        ? ["/wallet/:path*", "/signer/:path*", "/auditor/:path*", "/scanner/:path*"].map(
            (source) => ({
              source,
              headers: [{ key: "Content-Security-Policy", value: assetCsp() }],
            }),
          )
        : []),
    ];
  },
};

export default nextConfig;
