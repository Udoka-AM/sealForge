import { NextResponse } from "next/server";

/**
 * Server-side proxy for NEAR Intents 1Click quotes. Not built yet: ShieldRamp's
 * swap leg follows mainnet (see the roadmap).
 * Keeps ONECLICK_JWT on the server. Must not log client IPs or request bodies.
 */
export async function POST() {
  if (!process.env.ONECLICK_JWT) {
    return NextResponse.json({ error: "ShieldRamp is not configured" }, { status: 503 });
  }
  return NextResponse.json({ error: "ShieldRamp quotes are not available yet" }, { status: 501 });
}
