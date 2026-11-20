"use client";

import { CircleCheck, CircleX, Info, LoaderCircle, ShieldAlert, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { HashChip } from "@/components/hash-chip";
import { checkEscrow } from "@/lib/auditor";
import { escrowAddressOf } from "@/lib/scanner";
import type { AuctionView, EscrowCheck, PublishedResult } from "@/lib/types";

/**
 * Trustee signatures shipped with this block height (per network). Auctions
 * that started earlier, and whose bidding is over, can't have them, so the page
 * explains that instead of warning. Anything still taking bids is held to the
 * full check, so a coordinator can't use "older auction" to skip it. The CLI
 * auditor stays strict either way (`--allow-unattested`).
 */
const ATTESTATIONS_SINCE: Record<AuctionView["network"], number> = {
  mainnet: 0,
  testnet: 4_641_000,
  regtest: 0,
};

function predatesAttestations(source: AuctionView | PublishedResult): boolean {
  const since = ATTESTATIONS_SINCE[source.network] ?? 0;
  if ("auction_code" in source) return source.scan_from_height < since;
  const biddingOver = !["draft", "dkg", "ready", "open"].includes(source.status);
  return biddingOver && (source.open_height ?? source.close_height) < since;
}

type State =
  | { status: "checking" }
  | { status: "done"; check: EscrowCheck }
  | { status: "error"; message: string };

/**
 * Checks, in this browser with the WebAssembly auditor, that every trustee
 * signed the escrow address shown. The coordinator can't fake this without
 * the trustees' keys, though it could list keys of its own: hence the keys are
 * shown so bidders can compare them with what the trustees post themselves.
 */
export function useEscrowCheck(source: AuctionView | PublishedResult): State {
  const [state, setState] = useState<State>({ status: "checking" });
  const key = JSON.stringify(source);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `key` stands in for `source`
  useEffect(() => {
    let live = true;
    setState({ status: "checking" });
    checkEscrow(source)
      .then((check) => live && setState({ status: "done", check }))
      .catch((e: Error) => live && setState({ status: "error", message: e.message }));
    return () => {
      live = false;
    };
  }, [key]);
  return state;
}

function Summary({ state, older = false }: { state: State; older?: boolean }) {
  if (state.status === "checking")
    return (
      <p className="inline-flex items-center gap-2 text-muted">
        <LoaderCircle
          size={16}
          aria-hidden="true"
          className="animate-spin motion-reduce:animate-none"
        />
        Checking the trustees&rsquo; signatures…
      </p>
    );
  if (state.status === "error")
    return (
      <p role="alert" className="inline-flex items-center gap-2 text-warning">
        <ShieldAlert size={16} aria-hidden="true" /> Couldn&rsquo;t run the check: {state.message}
      </p>
    );
  const { check } = state;
  if (check.verdict === "verified")
    return (
      <p className="inline-flex items-center gap-2 text-success">
        <ShieldCheck size={16} aria-hidden="true" />
        All {check.total} trustees signed this address
      </p>
    );
  if (check.verdict === "unattested" && older)
    return (
      <p className="inline-flex items-start gap-2 text-muted">
        <Info size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
        This auction ran before trustees signed escrow addresses, so there are no signatures to
        check. The chain rescan still checks every bid.
      </p>
    );
  if (check.verdict === "unattested")
    return (
      <p role="alert" className="inline-flex items-start gap-2 text-warning">
        <ShieldAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
        No trustee has signed this address, so only the organiser vouches for it.
      </p>
    );
  return (
    <p role="alert" className="inline-flex items-start gap-2 font-medium text-danger">
      <ShieldAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
      The trustees&rsquo; signatures don&rsquo;t match this address ({check.signed} of {check.total}{" "}
      check out). Don&rsquo;t send funds to it.
    </p>
  );
}

const VERDICT: Record<EscrowCheck["trustees"][number]["verdict"], string> = {
  signed: "signed",
  missing: "not signed",
  invalid: "invalid signature",
  wrong_key: "unexpected key",
};

function TrusteeKeys({ check }: { check: EscrowCheck }) {
  return (
    <ul className="space-y-2">
      {check.trustees.map((t) => (
        <li key={t.slot} className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
          <span className="inline-flex min-w-28 items-center gap-1.5">
            {t.verdict === "signed" ? (
              <CircleCheck size={15} aria-hidden="true" className="text-success" />
            ) : (
              <CircleX size={15} aria-hidden="true" className="text-danger" />
            )}
            {t.label}
            <span className="sr-only">: {VERDICT[t.verdict]}</span>
          </span>
          {t.pubkey ? <HashChip value={t.pubkey} label={`${t.label}'s key`} /> : null}
          {t.verdict !== "signed" ? (
            <span className="text-[13px] text-danger">{VERDICT[t.verdict]}</span>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

/** The auction page's escrow card: did the trustees sign the address shown? */
export function EscrowAttestations({ auction }: { auction: AuctionView }) {
  const state = useEscrowCheck(auction);
  return (
    <div className="space-y-3 border-t border-border pt-4">
      <Summary state={state} older={predatesAttestations(auction)} />
      {state.status === "done" && state.check.total > 0 && state.check.verdict !== "unattested" ? (
        <details className="group">
          <summary className="cursor-pointer text-[13px] text-muted hover:text-text">
            Trustee keys
          </summary>
          <div className="mt-3 space-y-3">
            <TrusteeKeys check={state.check} />
            <p className="max-w-[60ch] text-[13px] text-muted">
              Checked in your browser. Each trustee rebuilt this address from its own share of the
              escrow key and signed it. To rule out a site that invented its own trustees, compare
              these keys with the ones the trustees post themselves.
            </p>
          </div>
        </details>
      ) : null}
    </div>
  );
}

/** Bid flow guard: true when the trustees' signatures contradict the address. */
export function useEscrowBlocked(auction: AuctionView): {
  blocked: boolean;
  state: State;
} {
  const state = useEscrowCheck(auction);
  return { blocked: state.status === "done" && state.check.verdict === "failed", state };
}

export function EscrowWarning({ state }: { state: State }) {
  if (state.status === "checking") return null;
  if (state.status === "done" && state.check.verdict === "verified") return null;
  return (
    <div className="rounded-[var(--radius-control)] border border-current p-3 text-sm">
      <Summary state={state} />
    </div>
  );
}

type KeyMatch =
  | { status: "checking" }
  | { status: "done"; derived: string }
  | { status: "error"; message: string };

/**
 * The audit page's escrow section: the trustees' signatures over the address
 * bidders paid, and whether the published viewing key belongs to that address.
 */
export function EscrowAudit({
  result,
  shownUa,
}: {
  result: PublishedResult;
  shownUa: string | null;
}) {
  const state = useEscrowCheck(result);
  const [keyMatch, setKeyMatch] = useState<KeyMatch>({ status: "checking" });
  useEffect(() => {
    let live = true;
    escrowAddressOf(result.network, result.uivk)
      .then((derived) => live && setKeyMatch({ status: "done", derived }))
      .catch((e: Error) => live && setKeyMatch({ status: "error", message: e.message }));
    return () => {
      live = false;
    };
  }, [result.network, result.uivk]);

  const paid = result.escrow_ua ?? shownUa;
  const rows: { label: string; ok: boolean | null; detail: string }[] = [
    {
      label: "The viewing key receives at the escrow address",
      ok: keyMatch.status !== "done" || !paid ? null : keyMatch.derived === paid,
      detail:
        keyMatch.status === "error"
          ? keyMatch.message
          : keyMatch.status === "done" && paid && keyMatch.derived !== paid
            ? `It receives at ${keyMatch.derived}`
            : "",
    },
  ];
  // Results from before attestations don't name the address; skip the row.
  if (result.escrow_ua && shownUa)
    rows.push({
      label: "The result names the address the auction page showed",
      ok: result.escrow_ua === shownUa,
      detail: "",
    });
  const hasTrustees =
    state.status === "done" && state.check.total > 0 && state.check.verdict !== "unattested";

  return (
    <div className="space-y-4 rounded-[var(--radius-card)] border border-border bg-surface p-5 text-sm">
      <Summary state={state} older={predatesAttestations(result)} />
      <ul className="space-y-2">
        {rows.map((r) => (
          <li key={r.label} className="flex items-start gap-2">
            {r.ok === null ? (
              <LoaderCircle size={15} aria-hidden="true" className="mt-0.5 text-muted" />
            ) : r.ok ? (
              <CircleCheck size={15} aria-hidden="true" className="mt-0.5 text-success" />
            ) : (
              <CircleX size={15} aria-hidden="true" className="mt-0.5 text-danger" />
            )}
            <span>
              {r.label}
              <span className="sr-only">
                : {r.ok === null ? "not checked" : r.ok ? "yes" : "no"}
              </span>
              {r.detail ? <span className="block text-[13px] text-danger">{r.detail}</span> : null}
            </span>
          </li>
        ))}
      </ul>
      {hasTrustees && state.status === "done" ? <TrusteeKeys check={state.check} /> : null}
      <p className="max-w-[68ch] text-[13px] text-muted">
        If the viewing key belonged to another address, the rescan below would audit the wrong
        escrow.
        {hasTrustees ? (
          <>
            {" "}
            To rule out invented trustees, compare the keys with the ones the trustees post
            themselves, or pass them to the command-line auditor with{" "}
            <code className="num">--trustee-key SLOT=KEY</code>.
          </>
        ) : null}
      </p>
    </div>
  );
}
