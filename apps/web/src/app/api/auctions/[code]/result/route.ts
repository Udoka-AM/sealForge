import { NextResponse } from "next/server";
import { CoordinatorUnavailable, getResult } from "@/lib/coordinator";

/** Same-origin proxy for the public result, so the browser never needs CORS to the coordinator. */
export async function GET(_req: Request, { params }: { params: Promise<{ code: string }> }) {
  const { code } = await params;
  try {
    const result = await getResult(code);
    if (!result) return NextResponse.json({ error: "not published" }, { status: 404 });
    return NextResponse.json(result, { headers: { "Cache-Control": "public, max-age=30" } });
  } catch (e) {
    if (e instanceof CoordinatorUnavailable)
      return NextResponse.json({ error: e.message }, { status: 503 });
    throw e;
  }
}
