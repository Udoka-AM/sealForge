import "server-only";
import type { AuctionView, PublishedResult } from "./types";

/** Server-side base URL of the sealForge coordinator. */
function baseUrl(): string {
  return (
    process.env.COORDINATOR_URL ??
    process.env.NEXT_PUBLIC_COORDINATOR_URL ??
    "http://localhost:8080"
  ).replace(/\/$/, "");
}

export class CoordinatorUnavailable extends Error {}

async function get<T>(path: string): Promise<T | null> {
  let res: Response;
  try {
    res = await fetch(`${baseUrl()}${path}`, { cache: "no-store" });
  } catch {
    throw new CoordinatorUnavailable("The auction server can't be reached right now.");
  }
  if (res.status === 404) return null;
  if (!res.ok) throw new CoordinatorUnavailable(`The auction server returned ${res.status}.`);
  return (await res.json()) as T;
}

export async function listAuctions(): Promise<AuctionView[]> {
  return (await get<AuctionView[]>("/v1/auctions")) ?? [];
}

export async function getAuction(code: string): Promise<AuctionView | null> {
  if (!/^[0-9A-HJKMNP-TV-Z]{10}$/.test(code)) return null;
  return get<AuctionView>(`/v1/auctions/${code}`);
}

/** The published result, or null until the organiser publishes. */
export async function getResult(code: string): Promise<PublishedResult | null> {
  if (!/^[0-9A-HJKMNP-TV-Z]{10}$/.test(code)) return null;
  return get<PublishedResult>(`/v1/auctions/${code}/result`);
}
