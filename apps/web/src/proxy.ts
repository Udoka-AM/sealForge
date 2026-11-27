import { type NextRequest, NextResponse } from "next/server";
import { pageCsp } from "@/lib/csp";

/**
 * Mints a nonce for every page response and sets the strict CSP built from it
 * (src/lib/csp.ts). Next reads the nonce from the request's CSP header and puts
 * it on its own scripts; the root layout passes it to the one inline script it
 * renders. Development builds skip it: React Refresh needs eval.
 */
export function proxy(request: NextRequest) {
  if (process.env.NODE_ENV !== "production") return NextResponse.next();
  const nonce = btoa(crypto.randomUUID());
  const csp = pageCsp(nonce);
  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("x-nonce", nonce);
  requestHeaders.set("content-security-policy", csp);
  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set("content-security-policy", csp);
  return response;
}

export const config = {
  matcher: [
    {
      // Pages only: not the API, Next's static files, icons, or the
      // WebAssembly modules and workers (next.config.ts gives those their own
      // policy).
      source:
        "/((?!api/|_next/static|_next/image|favicon|icon|apple-icon|embed\\.js|wallet/|signer/|auditor/|scanner/).*)",
      missing: [
        { type: "header", key: "next-router-prefetch" },
        { type: "header", key: "purpose", value: "prefetch" },
      ],
    },
  ],
};
