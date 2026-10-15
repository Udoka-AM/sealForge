"use client";

import { Check, KeyRound, LoaderCircle, ShieldCheck } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/button";
import { zec } from "@/lib/format";
import {
  api,
  forgetSigner,
  KeyFileError,
  loadTrusteeIdentity,
  type StoredSigner,
} from "@/lib/sfsig";
import { commitBatch, keyInfo, signBatch } from "@/lib/signer";
import type { TrusteeBatch } from "@/lib/types";
import { KeyLoader } from "./key-loader";
import { usePolling, useSigner } from "./use-signer";

const card = "rounded-[var(--radius-card)] border border-border bg-surface p-5";

type Phase =
  | "review"
  | "committing"
  | "waiting"
  | "signing"
  | "signed"
  | "not-needed"
  | "lost-nonces"
  | "done";

/** S16: review escrow batches and co-sign them with this trustee's key share. */
export function TrusteeSigning({ code }: { code: string }) {
  const role = `trustee:${code}` as const;
  const { state, reload, forget } = useSigner(role);
  const signer = state.kind === "ready" ? state.signer : null;
  const [keyHex, setKeyHex] = useState<string | null>(null);
  const [keyError, setKeyError] = useState<string | null>(null);
  const [batches, setBatches] = useState<TrusteeBatch[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Round 1 nonces by batch id: in memory only, dropped after round 2.
  const nonces = useRef(new Map<string, string[]>());
  const [approved, setApproved] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<Set<string>>(new Set());

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

  // Round 2 runs on its own once this trustee approved and was chosen.
  useEffect(() => {
    if (!signer || !keyHex || !batches) return;
    for (const b of batches) {
      const mine = nonces.current.get(b.id);
      const chosen = signer.slot !== undefined && b.signer_slots.includes(signer.slot);
      const ready = Object.keys(b.signer_commitments).length >= 2;
      if (
        b.status === "awaiting_signatures" &&
        mine &&
        chosen &&
        ready &&
        !b.i_shared &&
        !busy.has(b.id)
      ) {
        setBusy((s) => new Set(s).add(b.id));
        (async () => {
          try {
            const shares = await signBatch(
              keyHex,
              b.sighash,
              b.spends.map((s) => s.alpha),
              b.signer_commitments,
              mine,
            );
            await api("POST", `/v1/batches/${b.id}/shares`, signer, { shares });
            nonces.current.delete(b.id);
            await load();
          } catch (e) {
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
  }, [batches, keyHex, signer, busy, load]);

  async function approve(b: TrusteeBatch) {
    if (!signer || !keyHex) return;
    setApproved((s) => new Set(s).add(b.id));
    setBusy((s) => new Set(s).add(b.id));
    try {
      const round1 = await commitBatch(keyHex, b.spends.length);
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

  function phase(b: TrusteeBatch, me: number | undefined): Phase {
    if (b.status !== "awaiting_signatures") return b.i_shared ? "signed" : "done";
    if (b.i_shared) return "signed";
    if (busy.has(b.id)) return b.i_committed ? "signing" : "committing";
    if (!b.i_committed) return "review";
    const chosen = me !== undefined && b.signer_slots.includes(me);
    if (b.signer_slots.length >= 2 && !chosen) return "not-needed";
    if (!nonces.current.has(b.id)) return "lost-nonces";
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
                  <BatchSummary b={b} />
                  <div aria-live="polite" className="text-sm">
                    <PhaseLine phase={p} />
                  </div>
                  {p === "review" ? (
                    <Button disabled={!keyHex || approved.has(b.id)} onClick={() => approve(b)}>
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
                <BatchSummary b={b} />
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

function BatchSummary({ b }: { b: TrusteeBatch }) {
  return (
    <dl className="grid grid-cols-2 gap-x-6 gap-y-3 text-sm sm:grid-cols-4">
      <div>
        <dt className="text-muted">Kind</dt>
        <dd className="capitalize">{b.kind}</dd>
      </div>
      <div>
        <dt className="text-muted">Pays out</dt>
        <dd className="num">{zec(b.total_out_zat)}</dd>
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
    signed: [done, "Your signature share is in."],
    "not-needed": [done, "Two other trustees are signing this one. You're not needed."],
    "lost-nonces": [
      null,
      "This page was reloaded after you committed. If you're chosen to sign, the round restarts after 30 minutes.",
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
