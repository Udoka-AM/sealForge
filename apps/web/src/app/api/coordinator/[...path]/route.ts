import { type NextRequest, NextResponse } from "next/server";

/**
 * Same-origin pass-through to the coordinator's `/v1` API for the console and
 * trustee pages. The path, query, body and `Authorization` header are
 * forwarded byte for byte, because SF-Sig signs all of them. Nothing is
 * logged or stored here.
 *
 * The visitor's address goes along in `x-forwarded-for` so the coordinator can
 * apply its per-client rate limits (it keys them by a salted hash and never
 * stores the address). Behind Cloudflare, `cf-connecting-ip` is set by
 * Cloudflare itself and can't be forged by the client, so it wins.
 */

function clientAddress(req: NextRequest): string {
  return (
    req.headers.get("cf-connecting-ip")?.trim() ||
    req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ||
    "unknown"
  );
}

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

  const headers: Record<string, string> = { "x-forwarded-for": clientAddress(req) };
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
      ...(res.headers.get("retry-after")
        ? { "retry-after": res.headers.get("retry-after") as string }
        : {}),
    },
  });
}

export const GET = forward;
export const POST = forward;
export const PATCH = forward;
export const PUT = forward;
