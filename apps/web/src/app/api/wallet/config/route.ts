import { NextResponse } from "next/server";

export const dynamic = "force-dynamic";

/**
 * What the in-browser wallet needs to know at runtime: which network this
 * deployment serves and the gRPC-web lightwallet endpoint it syncs from.
 * Nothing about any wallet is sent here; this only reads the server's config.
 */
export function GET() {
  const network = process.env.NEXT_PUBLIC_DEFAULT_NETWORK ?? process.env.SF_NETWORK ?? "mainnet";
  const grpcWeb =
    process.env.LIGHTWALLETD_GRPCWEB ?? process.env.NEXT_PUBLIC_LIGHTWALLETD_GRPCWEB ?? null;
  return NextResponse.json(
    { network: ["mainnet", "testnet", "regtest"].includes(network) ? network : "mainnet", grpcWeb },
    { headers: { "cache-control": "no-store" } },
  );
}
