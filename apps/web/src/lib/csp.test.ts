import { describe, expect, it } from "vitest";
import { assetCsp, pageCsp } from "./csp";

function directive(policy: string, name: string): string {
  return policy.split("; ").find((d) => d.startsWith(`${name} `)) ?? "";
}

describe("content security policy", () => {
  it("lets page scripts run only with the response's nonce", () => {
    const policy = pageCsp("abc123");
    const scripts = directive(policy, "script-src");
    expect(scripts).toContain("'nonce-abc123'");
    expect(scripts).toContain("'strict-dynamic'");
    expect(scripts).not.toContain("'unsafe-inline'");
    expect(scripts).not.toContain("'unsafe-eval'");
  });

  it("refuses framing, plugins and base-URL changes", () => {
    const policy = pageCsp("n");
    expect(policy).toContain("frame-ancestors 'none'");
    expect(policy).toContain("object-src 'none'");
    expect(policy).toContain("base-uri 'none'");
  });

  it("gives workers and module files a policy without inline scripts", () => {
    const scripts = directive(assetCsp(), "script-src");
    expect(scripts).toBe("script-src 'self' 'wasm-unsafe-eval'");
  });
});
