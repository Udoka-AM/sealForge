"use client";

import { Check, KeyRound, LoaderCircle, ShieldCheck } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/button";
import { checkEscrow } from "@/lib/auditor";
import { zec } from "@/lib/format";
import {
  api,
  forgetSigner,
  KeyFileError,
  loadTrusteeIdentity,
  type StoredSigner,
  signRound2Inputs,
  verifyRound2Inputs,
} from "@/lib/sfsig";
import { commitBatch, keyInfo, round2InputsDigest, signBatch, verifyBatch } from "@/lib/signer";
import {
  type AuctionView,
  BATCH_KIND_LABEL,
  type Network,
  type PublishedResult,
  type TrusteeBatch,
  type TrusteeDkgState,
  type VerifiedBatch,
} from "@/lib/types";
import { KeyLoader } from "./key-loader";
import { usePolling, useSigner } from "./use-signer";

const card = "rounded-[var(--radius-card)] border border-border bg-surface p-5";

type Phase =
  | "review"
  | "committing"
  | "waiting"
  | "signing"
  | "co-signer-check"
  | "refused"
  | "signed"
  | "not-needed"
  | "lost-nonces"
  | "inputs-changed"
  | "done";

/** Shares already made for a batch, and the exact inputs they were made over. */
type SignedRecord = { inputs: string; shares: string[] };

/** Everything a round 2 share depends on besides the key and the nonces. */
function signingInputs(b: TrusteeBatch): string {
  const commitments = Object.entries(b.signer_commitments)
    .map(([slot, values]) => [Number(slot), values.map((v) => v.toLowerCase())] as const)
    .sort(([a], [b]) => a - b);
  return JSON.stringify([
    b.sighash.toLowerCase(),
    b.spends.map((s) => s.alpha.toLowerCase()),
    commitments,
  ]);
}

/** The coordinator showed the two signers different inputs: never sign this batch. */
class Equivocation extends Error {}

function equivocation(batchId: string, why: string): Equivocation {
  return new Equivocation(
    `Refused batch ${batchId.slice(0, 8)}: ${why} Don't sign; tell the organiser and the other trustees, and treat the coordinator as compromised.`,
  );
}

function base64ToHex(b64: string): string {
  return Array.from(atob(b64), (c) => c.charCodeAt(0).toString(16).padStart(2, "0")).join("");
}

/**
 * Before committing nonces or sending shares (security review F15): the public
 * page bidders pay from must be up, list this trustee's own key in its slot,
 * and carry valid signatures from every trustee over the address it shows.
 * This trustee's own valid signature there proves it attested exactly that
 * address. Anything else throws, and nothing is signed.
 */
async function requirePublicEscrow(code: string, signer: StoredSigner): Promise<AuctionView> {
  let view: AuctionView;
  try {
    view = await api<AuctionView>("GET", `/v1/auctions/${code}`, null);
  } catch {
    throw new Error(
      "Refusing to co-sign: the public auction page isn't available, so you can't confirm it shows bidders the escrow address you attested.",
    );
  }
  if (signer.slot === undefined)
    throw new Error("Refusing to co-sign: this key file doesn't say which trustee slot it is.");
  const check = await checkEscrow({ kind: "view", view }, [
    [signer.slot, base64ToHex(signer.keyId)],
  ]);
  if (check.verdict !== "verified" || !view.escrow_ua || check.escrow_ua !== view.escrow_ua)
    throw new Error(
      "Refusing to co-sign: the public auction page doesn't show the escrow address you attested, signed by every trustee. Tell the organiser and the other trustees.",
    );
  return view;
}

/**
 * Decode and check a batch's transaction in this browser before taking part in
 * signing it (security review D1). The escrow's viewing key is rebuilt from
 * this trustee's own key share and must give the address on the public page,
 * which `requirePublicEscrow` checked carries this trustee's own attestation.
 */
async function checkTransaction(
  code: string,
  signer: StoredSigner,
  keyHex: string,
  b: TrusteeBatch,
  view: AuctionView,
): Promise<VerifiedBatch> {
  if (!signer.encSecret)
    throw new Error(
      "Load identity.json again: this browser needs it to check each transaction before signing.",
    );
  const dkg = await api<TrusteeDkgState>("GET", `/v1/auctions/${code}/dkg`, signer);
  if (!dkg.escrow_view_secret_for_me || !view.escrow_ua)
    throw new Error(
      "Refusing to co-sign: the coordinator sent no escrow viewing key to check with.",
    );
  let refundTotalDueZat: number | null = null;
  if (b.kind === "refund") {
    const result = await api<PublishedResult>("GET", `/v1/auctions/${code}/result`, null);
    refundTotalDueZat = result.bids.reduce((sum, bid) => sum + bid.refund_due_zat, 0);
  }
  return verifyBatch({
    keyPackageHex: keyHex,
    encSecretHex: signer.encSecret,
    sealedViewSecretHex: dkg.escrow_view_secret_for_me,
    escrowUa: view.escrow_ua,
    network: view.network,
    batch: b,
    refundTotalDueZat,
  });
}

/** S16: review escrow batches and co-sign them with this trustee's key share. */
export function TrusteeSigning({ code }: { code: string }) {
  const role = `trustee:${code}` as const;
  const { state, reload, forget } = useSigner(role);
  const signer = state.kind === "ready" ? state.signer : null;
  const [keyHex, setKeyHex] = useState<string | null>(null);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [batches, setBatches] = useState<TrusteeBatch[] | null>(null);
  const [network, setNetwork] = useState<Network>("mainnet");
  const [error, setError] = useState<string | null>(null);
  // Round 1 nonces by batch id: in memory only, dropped the moment round 2 signs.
  const nonces = useRef(new Map<string, string[]>());
  // Shares this page already made, by batch id. A failed send is retried with
  // these exact bytes, and only for the exact same inputs: signing again with the
  // same nonces over different inputs would reveal this trustee's key share.
  const signed = useRef(new Map<string, SignedRecord>());
  const [approved, setApproved] = useState<Set<string>>(new Set());
  // What this browser decoded in each batch's transaction, or why it refused.
  const [checked, setChecked] = useState<Map<string, VerifiedBatch | string>>(new Map());
  const [busy, setBusy] = useState<Set<string>>(new Set());
  // Batches refused because the coordinator equivocated: never retried.
  const [refused, setRefused] = useState<Map<string, string>>(new Map());

  const load = useCallback(async () => {
    if (!signer) return;
    try {
      setBatches(await api<TrusteeBatch[]>("GET", `/v1/auctions/${code}/batches`, signer));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't load batches.");
    }
  }, [code, signer]);
  usePolling(load, 5_000, signer !== null, true);

  // Amounts read as TAZ on test networks.
  useEffect(() => {
    api<AuctionView>("GET", `/v1/auctions/${code}`, null)
      .then((a) => setNetwork(a.network))
      .catch(() => {});
  }, [code]);

  // Round 2 runs on its own once this trustee approved and was chosen.
  useEffect(() => {
    if (!signer || !keyHex || !batches) return;
    for (const b of batches) {
      const mine = nonces.current.get(b.id);
      const record = signed.current.get(b.id);
      const chosen = signer.slot !== undefined && b.signer_slots.includes(signer.slot);
      const ready = Object.keys(b.signer_commitments).length >= 2;
      if (
        b.status === "awaiting_signatures" &&
        (mine || record) &&
        chosen &&
        ready &&
        !b.i_shared &&
        !busy.has(b.id) &&
        !refused.has(b.id)
      ) {
        const inputs = signingInputs(b);
        // Already signed over other inputs: never sign again with these nonces.
        if (record && record.inputs !== inputs) continue;
        setBusy((s) => new Set(s).add(b.id));
        (async () => {
          try {
            const view = await requirePublicEscrow(code, signer);
            let shares: string[];
            if (record) {
              shares = record.shares;
            } else if (mine && signer.slot !== undefined) {
              // Between rounds (stream 2c): state what this page signs over,
              // and sign only once the co-signer has stated the same.
              const me = signer.slot;
              const digest = await round2InputsDigest(
                b.sighash,
                b.spends.map((s) => s.alpha),
                b.signer_commitments,
              );
              const stated = b.signer_inputs?.[String(me)];
              if (!stated) {
                const signature = await signRound2Inputs(signer, code, b.id, digest);
                await api("POST", `/v1/batches/${b.id}/inputs`, signer, { digest, signature });
                await load();
                return;
              }
              if (stated.digest !== digest)
                throw equivocation(
                  b.id,
                  "the coordinator now shows different round 2 inputs than the ones you stated.",
                );
              const coSigner = b.signer_slots.find((s) => s !== me);
              const theirs =
                coSigner === undefined ? undefined : b.signer_inputs?.[String(coSigner)];
              if (coSigner === undefined || !theirs) return;
              const theirKey = view.trustees.find((t) => t.slot === coSigner)?.pubkey;
              if (
                !theirKey ||
                !(await verifyRound2Inputs(
                  theirKey,
                  theirs.signature,
                  code,
                  b.id,
                  coSigner,
                  theirs.digest,
                ))
              )
                throw equivocation(
                  b.id,
                  `slot ${coSigner}'s statement doesn't verify under its public key.`,
                );
              if (theirs.digest !== digest)
                throw equivocation(
                  b.id,
                  `slot ${coSigner} was shown different round 2 inputs (commitments, sighash or randomizers) than you were.`,
                );
              // Check the transaction again right before signing it.
              await checkTransaction(code, signer, keyHex, b, view);
              shares = await signBatch(
                keyHex,
                b.sighash,
                b.spends.map((s) => s.alpha),
                b.signer_commitments,
                mine,
              );
              // Consume the nonces before the shares leave this page.
              signed.current.set(b.id, { inputs, shares });
              nonces.current.delete(b.id);
            } else {
              return;
            }
            await api("POST", `/v1/batches/${b.id}/shares`, signer, { shares });
            await load();
          } catch (e) {
            if (e instanceof Equivocation) setRefused((m) => new Map(m).set(b.id, e.message));
            setError(e instanceof Error ? e.message : "Signing failed.");
          } finally {
            setBusy((s) => {
              const n = new Set(s);
              n.delete(b.id);
              return n;
            });
          }
        })();
      }
    }
  }, [batches, keyHex, signer, busy, refused, load, code]);

  async function approve(b: TrusteeBatch) {
    if (!signer || !keyHex) return;
    setApproved((s) => new Set(s).add(b.id));
    setBusy((s) => new Set(s).add(b.id));
    try {
      const view = await requirePublicEscrow(code, signer);
      await checkTransaction(code, signer, keyHex, b, view);
      const round1 = await commitBatch(keyHex, b.spends.length);
      // A fresh round: shares from an earlier, reset round are void.
      signed.current.delete(b.id);
      nonces.current.set(b.id, round1.nonces);
      await api("POST", `/v1/batches/${b.id}/commitments`, signer, {
        commitments: round1.commitments,
      });
      await load();
    } catch (e) {
      nonces.current.delete(b.id);
      setError(e instanceof Error ? e.message : "Couldn't send commitments.");
    } finally {
      setBusy((s) => {
        const n = new Set(s);
        n.delete(b.id);
        return n;
      });
    }
  }

  // Check each batch waiting for this trustee as soon as it appears, so the
  // decoded payments are on screen before "Approve and sign".
  useEffect(() => {
    if (!signer || !keyHex || !batches) return;
    for (const b of batches) {
      if (b.status !== "awaiting_signatures" || b.i_committed || checked.has(b.id)) continue;
      setChecked((m) => new Map(m).set(b.id, "Checking the transaction\u2026"));
      (async () => {
        let outcome: VerifiedBatch | string;
        try {
          const view = await requirePublicEscrow(code, signer);
          outcome = await checkTransaction(code, signer, keyHex, b, view);
        } catch (e) {
          outcome = e instanceof Error ? e.message : String(e);
        }
        setChecked((m) => new Map(m).set(b.id, outcome));
      })();
    }
  }, [batches, keyHex, signer, checked, code]);

  function phase(b: TrusteeBatch, me: number | undefined): Phase {
    if (b.status !== "awaiting_signatures") return b.i_shared ? "signed" : "done";
    if (b.i_shared) return "signed";
    if (busy.has(b.id)) return b.i_committed ? "signing" : "committing";
    if (!b.i_committed) return "review";
    const chosen = me !== undefined && b.signer_slots.includes(me);
    if (b.signer_slots.length >= 2 && !chosen) return "not-needed";
    if (refused.has(b.id)) return "refused";
    const record = signed.current.get(b.id);
    if (record && chosen && record.inputs !== signingInputs(b)) return "inputs-changed";
    if (!nonces.current.has(b.id) && !record) return "lost-nonces";
    if (chosen && me !== undefined && b.signer_inputs?.[String(me)]) return "co-signer-check";
    return "waiting";
  }

  if (state.kind === "loading") return <p className="text-muted">Loading…</p>;
  if (state.kind === "none")
    return (
      <KeyLoader
        title={`Load your trustee identity for ${code}`}
        fileHint={`~/.sealforge/trustees/${code}/identity.json`}
        accept=".json,application/json"
        onLoad={async (text) => {
          const { code: fileCode } = await loadTrusteeIdentity(text);
          if (fileCode !== code) {
            await forgetSigner(`trustee:${fileCode}`);
            throw new KeyFileError(`That identity is for auction ${fileCode}, not ${code}.`);
          }
          await reload();
        }}
      />
    );

  const me = (signer as StoredSigner).slot;
  const pending = batches?.filter((b) => b.status === "awaiting_signatures") ?? [];
  const finished = batches?.filter((b) => b.status !== "awaiting_signatures") ?? [];

  return (
    <div className="space-y-8">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-muted">
          Trustee <span className="num text-text">{me}</span>
          {signer?.label ? <span className="text-text"> · {signer.label}</span> : null}
        </p>
        <Button variant="ghost" onClick={forget}>
          Forget identity
        </Button>
      </div>

      {keyHex ? (
        <p className="flex items-center gap-2 text-sm text-muted">
          <ShieldCheck size={16} aria-hidden="true" className="text-success" />
          Key share loaded for this page only. Keep the page open until signing finishes.
        </p>
      ) : (
        <section className={`${card} max-w-[560px] space-y-3`}>
          <div className="flex items-center gap-3">
            <KeyRound size={18} aria-hidden="true" className="text-primary" />
            <h2 className="font-semibold">Key share</h2>
          </div>
          <p className="text-sm text-muted">
            To sign, load <code className="num text-text">key-package.hex</code> from{" "}
            <code className="num text-text">~/.sealforge/trustees/{code}/</code>. It stays in this
            page's memory and is never stored or sent.
          </p>
          <input
            type="file"
            accept=".hex,text/plain"
            aria-label="Key share file"
            className="block text-sm file:mr-3 file:h-10 file:rounded-[var(--radius-control)] file:border file:border-border file:bg-surface-2 file:px-4 file:text-text"
            onChange={async (e) => {
              const file = e.target.files?.[0];
              if (!file) return;
              setKeyError(null);
              try {
                const text = (await file.text()).trim();
                const info = await keyInfo(text);
                if (info.slot !== me)
                  throw new Error(`That key share is for trustee ${info.slot}, not ${me}.`);
                setKeyHex(text);
              } catch (err) {
                setKeyError(err instanceof Error ? err.message : "Couldn't read the key share.");
              }
            }}
          />
          {keyError ? (
            <p role="alert" className="text-sm text-danger">
              {keyError}
            </p>
          ) : null}
        </section>
      )}

      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}

      <section aria-labelledby="pending" className="space-y-3">
        <h2 id="pending" className="text-[18px] leading-6 font-semibold">
          Waiting for your signature
        </h2>
        {batches === null ? (
          <p className="text-muted">Loading…</p>
        ) : pending.length === 0 ? (
          <p className="text-muted">
            Nothing to sign right now. This page checks every few seconds.
          </p>
        ) : (
          <ul className="space-y-3">
            {pending.map((b) => {
              const p = phase(b, me);
              return (
                <li key={b.id} className={`${card} space-y-4`}>
                  <BatchSummary b={b} network={network} />
                  {p === "review" ? <CheckedTransaction outcome={checked.get(b.id)} /> : null}
                  <div aria-live="polite" className="text-sm">
                    <PhaseLine phase={p} />
                  </div>
                  {p === "review" ? (
                    <Button
                      disabled={
                        !keyHex || approved.has(b.id) || typeof checked.get(b.id) !== "object"
                      }
                      onClick={() => approve(b)}
                    >
                      Approve and sign
                    </Button>
                  ) : null}
                </li>
              );
            })}
          </ul>
        )}
      </section>

      {finished.length > 0 ? (
        <section aria-labelledby="finished" className="space-y-3">
          <h2 id="finished" className="text-[18px] leading-6 font-semibold">
            Signed, broadcasting
          </h2>
          <ul className="space-y-3">
            {finished.map((b) => (
              <li key={b.id} className={`${card} space-y-2`}>
                <BatchSummary b={b} network={network} />
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <p className="text-sm text-muted">
        The CLI does the same:{" "}
        <code className="num text-text">sealforge trustee sign {code} --wait</code>
      </p>
    </div>
  );
}

/** What this browser found in the transaction, before the trustee approves. */
function CheckedTransaction({ outcome }: { outcome: VerifiedBatch | string | undefined }) {
  if (outcome === undefined) return null;
  if (typeof outcome === "string")
    return (
      <p
        role={outcome.startsWith("Checking") ? undefined : "alert"}
        className={`text-sm ${outcome.startsWith("Checking") ? "text-muted" : "text-danger"}`}
      >
        {outcome}
      </p>
    );
  return (
    <div className="space-y-1.5 rounded-[var(--radius-control)] bg-surface-2 p-3 text-[13px]">
      <p className="inline-flex items-center gap-1.5 font-medium text-success">
        <ShieldCheck size={15} aria-hidden="true" /> Transaction checked in this browser
      </p>
      <ul className="num space-y-0.5 break-all text-muted">
        {outcome.lines.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
    </div>
  );
}

function BatchSummary({ b, network }: { b: TrusteeBatch; network: Network }) {
  return (
    <dl className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm sm:grid-cols-4">
      <div>
        <dt className="text-muted">Kind</dt>
        <dd>{BATCH_KIND_LABEL[b.kind] ?? b.kind}</dd>
      </div>
      <div>
        <dt className="text-muted">
          {b.kind === "consolidate" ? "Moves within the escrow" : "Pays out"}
        </dt>
        <dd className="num">{zec(b.total_out_zat, network)}</dd>
      </div>
      <div>
        <dt className="text-muted">Outputs</dt>
        <dd className="num">{b.output_count}</dd>
      </div>
      <div>
        <dt className="text-muted">Escrow spends</dt>
        <dd className="num">{b.spends.length}</dd>
      </div>
      <div className="col-span-2 sm:col-span-4">
        {b.kind === "consolidate" ? (
          <p className="mb-3 text-muted">
            Merges the escrow&rsquo;s oldest notes into one note at the escrow&rsquo;s own internal
            address, so the next refund fits in one transaction. Nothing leaves the escrow except
            the fee.
          </p>
        ) : null}
        <dt className="text-muted">Sighash (compare with the CLI)</dt>
        <dd className="num text-[13px] break-all">{b.sighash}</dd>
      </div>
    </dl>
  );
}

function PhaseLine({ phase }: { phase: Phase }) {
  const spin = <LoaderCircle size={14} aria-hidden="true" className="animate-spin text-warning" />;
  const done = <Check size={14} aria-hidden="true" className="text-success" />;
  const line: Record<Phase, [ReactNode, string]> = {
    review: [null, "Check the amount and outputs, then approve."],
    committing: [spin, "Sending your commitments…"],
    waiting: [spin, "Committed. Waiting for a second trustee; keep this page open."],
    signing: [spin, "Signing…"],
    "co-signer-check": [
      spin,
      "You stated what you'll sign. Waiting for your co-signer to do the same before your share is sent; keep this page open.",
    ],
    refused: [
      null,
      "Refused: the coordinator showed you and your co-signer different things to sign. Nothing was sent. Tell the organiser and the other trustees.",
    ],
    signed: [done, "Your signature share is in."],
    "not-needed": [done, "Two other trustees are signing this one. You're not needed."],
    "lost-nonces": [
      null,
      "This page was reloaded after you committed. If you're chosen to sign, the round restarts after 30 minutes.",
    ],
    "inputs-changed": [
      null,
      "The coordinator changed what it's asking you to sign after you signed. Not signing again. The round restarts after 30 minutes; if this repeats, stop signing and report it.",
    ],
    done: [done, "Nothing left for you to do."],
  };
  const [icon, text] = line[phase];
  return (
    <p className="flex items-center gap-2">
      {icon}
      {text}
    </p>
  );
}
