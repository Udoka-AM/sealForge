import { type NextRequest, NextResponse } from "next/server";

/**
 * Same-origin pass-through to the coordinator's `/v1` API for the console and
 * trustee pages. The path, query, body and `Authorization` header are
 * forwarded byte for byte, because SF-Sig signs all of them. Nothing is
 * logged or stored here.
 */

function upstream(): string {
  return (
    process.env.COORDINATOR_URL ??
    process.env.NEXT_PUBLIC_COORDINATOR_URL ??
    "http://localhost:8080"
  ).replace(/\/$/, "");
}

async function forward(req: NextRequest): Promise<Response> {
  const url = new URL(req.url);
  const path = url.pathname.replace(/^\/api\/coordinator/, "");
  if (!path.startsWith("/v1/")) return NextResponse.json({ error: "not found" }, { status: 404 });

  const headers: Record<string, string> = {};
  const auth = req.headers.get("authorization");
  if (auth) headers.authorization = auth;
  const type = req.headers.get("content-type");
  if (type) headers["content-type"] = type;
  const body = req.method === "GET" ? undefined : await req.arrayBuffer();

  let res: Response;
  try {
    res = await fetch(`${upstream()}${path}${url.search}`, {
      method: req.method,
      headers,
      body,
      cache: "no-store",
    });
  } catch {
    return NextResponse.json(
      { code: "unavailable", error: "The auction server can't be reached right now." },
      { status: 503 },
    );
  }
  return new Response(await res.arrayBuffer(), {
    status: res.status,
    headers: {
      "content-type": res.headers.get("content-type") ?? "application/json",
      "cache-control": "no-store",
    },
  });
}

export const GET = forward;
export const POST = forward;
export const PATCH = forward;
