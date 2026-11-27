/**
 * Content Security Policy (#59).
 *
 * Pages get a strict, nonce-based policy: a script runs only if it carries the
 * nonce minted for that response, or was loaded by one that does
 * ('strict-dynamic'). An injected `<script>` or inline handler can't run, which
 * matters on the console, trustee and wallet pages, where keys live in the
 * page. WebAssembly (auditor, signer, scanner, wallet) needs
 * 'wasm-unsafe-eval', which allows compiling WebAssembly and nothing else.
 *
 * Styles keep 'unsafe-inline': the motion system animates through style
 * attributes, and CSS can't run code. Workers and the WebAssembly modules'
 * own files get ASSET_CSP, with no nonce, since nothing renders them as pages.
 */

/** The lightwallet server's origin, for the wallet and the audit rescan. */
function lightwalletdOrigin(): string {
  try {
    const url = process.env.LIGHTWALLETD_GRPCWEB ?? process.env.NEXT_PUBLIC_LIGHTWALLETD_GRPCWEB;
    return new URL(url ?? "").origin;
  } catch {
    return "";
  }
}

// The Audit page rescans the chain from a lightwallet server the auditor
// chooses, so connections may go to any HTTPS origin.
const connectSrc = (): string => `connect-src 'self' https: ${lightwalletdOrigin()}`.trim();

export function pageCsp(nonce: string): string {
  return [
    "default-src 'self'",
    `script-src 'self' 'nonce-${nonce}' 'strict-dynamic' 'wasm-unsafe-eval'`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob:",
    "font-src 'self'",
    "worker-src 'self'",
    connectSrc(),
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'self'",
    "frame-ancestors 'none'",
  ].join("; ");
}

/** For workers and the WebAssembly glue under /wallet, /signer, /auditor, /scanner. */
export function assetCsp(): string {
  return [
    "default-src 'self'",
    "script-src 'self' 'wasm-unsafe-eval'",
    connectSrc(),
    "object-src 'none'",
    "base-uri 'none'",
    "frame-ancestors 'none'",
  ].join("; ");
}
