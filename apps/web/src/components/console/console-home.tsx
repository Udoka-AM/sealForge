"use client";

import { Plus } from "lucide-react";
import Link from "next/link";
import { useCallback, useState } from "react";
import { Button, ButtonLink } from "@/components/button";
import { StatusPill } from "@/components/status-pill";
import { api, loadOrganiserKey } from "@/lib/sfsig";
import type { AuctionView } from "@/lib/types";
import { KeyLoader } from "./key-loader";
import { usePolling, useSigner } from "./use-signer";

/** S13: the organiser's auctions. */
export function ConsoleHome() {
  const { state, reload, forget } = useSigner("organiser");
  const [me, setMe] = useState<string | null>(null);
  const [auctions, setAuctions] = useState<AuctionView[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const signer = state.kind === "ready" ? state.signer : null;
  const load = useCallback(async () => {
    if (!signer) return;
    try {
      const [who, list] = await Promise.all([
        api<{ display_name: string }>("GET", "/v1/organisers/me", signer),
        api<AuctionView[]>("GET", "/v1/console/auctions", signer),
      ]);
      setMe(who.display_name);
      setAuctions(list);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error && "status" in e && e.status === 403
          ? "The auction server doesn't recognise this organiser key. Run `sealforge init` first, or load a different key."
          : e instanceof Error
            ? e.message
            : "Something went wrong.",
      );
    }
  }, [signer]);
  usePolling(load, 15_000, signer !== null);

  if (state.kind === "loading") return <p className="text-muted">Loading…</p>;
  if (state.kind === "none")
    return (
      <div className="space-y-6">
        <p className="max-w-[68ch] text-muted">
          Run auctions from the browser with the organiser key that{" "}
          <code className="num text-text">sealforge init</code> created. Everything here can also be
          done with the CLI.
        </p>
        <KeyLoader
          title="Load your organiser key"
          fileHint="~/.sealforge/organiser.key"
          accept=".key,text/plain"
          onLoad={async (text) => {
            await loadOrganiserKey(text);
            await reload();
          }}
        />
      </div>
    );

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-muted">
          {me ? (
            <>
              Signed in as <span className="font-medium text-text">{me}</span>
            </>
          ) : (
            "Checking your key…"
          )}
        </p>
        <div className="flex gap-2">
          <Button variant="ghost" onClick={forget}>
            Forget key
          </Button>
          <ButtonLink href="/console/new">
            <Plus size={16} aria-hidden="true" />
            New auction
          </ButtonLink>
        </div>
      </div>

      {error ? (
        <p
          role="alert"
          className="rounded-[var(--radius-card)] border border-border bg-surface p-4 text-sm"
        >
          {error}
        </p>
      ) : null}

      {auctions === null ? null : auctions.length === 0 ? (
        <div className="rounded-[var(--radius-card)] border border-border bg-surface p-6">
          <p className="font-medium">No auctions yet</p>
          <p className="mt-1 text-sm text-muted">
            Create one, invite three trustees, and open it once they've set up the escrow.
          </p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-[var(--radius-card)] border border-border">
          <table className="w-full text-left text-sm">
            <thead className="bg-surface text-muted">
              <tr>
                <th scope="col" className="px-4 py-3 font-medium">
                  Auction
                </th>
                <th scope="col" className="px-4 py-3 font-medium">
                  Status
                </th>
                <th scope="col" className="px-4 py-3 font-medium">
                  Network
                </th>
                <th scope="col" className="px-4 py-3 text-right font-medium">
                  Closes at block
                </th>
              </tr>
            </thead>
            <tbody>
              {auctions.map((a) => (
                <tr key={a.code} className="border-t border-border">
                  <td className="px-4 py-3">
                    <Link href={`/console/${a.code}`} className="font-medium hover:text-primary">
                      {a.title}
                    </Link>
                    <div className="num text-[13px] text-muted">{a.code}</div>
                  </td>
                  <td className="px-4 py-3">
                    <StatusPill status={a.status} />
                  </td>
                  <td className="px-4 py-3 capitalize">{a.network}</td>
                  <td className="num px-4 py-3 text-right">
                    {a.close_height.toLocaleString("en-US")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
