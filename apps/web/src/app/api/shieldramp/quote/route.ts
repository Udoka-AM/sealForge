import { NextResponse } from "next/server";

/**
 * Server-side proxy for NEAR Intents 1Click quotes (task 1.7).
 * Keeps ONECLICK_JWT on the server. Must not log client IPs or request bodies.
 */
export async function POST() {
  if (!process.env.ONECLICK_JWT) {
    return NextResponse.json({ error: "ShieldRamp is not configured" }, { status: 503 });
  }
  return NextResponse.json({ error: "Not implemented: task 1.7" }, { status: 501 });
}
