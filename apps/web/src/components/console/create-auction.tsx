"use client";

import { parseZec } from "@sealforge/bid";
import { TriangleAlert } from "lucide-react";
import Link from "next/link";
import { type FormEvent, useEffect, useState } from "react";
import { Button, ButtonLink } from "@/components/button";
import { HashChip } from "@/components/hash-chip";
import { ApiError, api } from "@/lib/sfsig";
import type { AuctionMode, ChainView, CreateAuction, CreatedAuction, Network } from "@/lib/types";
import { useSigner } from "./use-signer";

/** Zcash targets 75 s blocks: 48 per hour. */
const BLOCKS_PER_HOUR = 48;
/** Time for the trustees to finish key generation before opening. */
const SETUP_BLOCKS = BLOCKS_PER_HOUR;
/** The coordinator refuses to open with fewer blocks left than this (MIN_OPEN_BLOCKS). */
const MIN_WINDOW_HOURS = 3;

const field =
  "h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface px-3 text-[15px] placeholder:text-muted/70";
const label = "text-sm font-medium";
const hint = "text-[13px] text-muted";

const DEFAULT_TRUSTEES = [
  { slot: 1, label: "Organiser" },
  { slot: 2, label: "Independent auditor" },
  { slot: 3, label: "Community signer" },
];

/** S14: create a draft auction, then hand out the trustee invites. */
export function CreateAuctionForm({ defaultNetwork }: { defaultNetwork: Network }) {
  const { state } = useSigner("organiser");
  const [tip, setTip] = useState<number | null>(null);
  const [network, setNetwork] = useState<Network>(defaultNetwork);
  const [mode, setMode] = useState<AuctionMode>("uniform_price");
  const [sealed, setSealed] = useState(false);
  const [revealHours, setRevealHours] = useState("6");
  const [revealHours, setRevealHours] = useState("6");
  const [hours, setHours] = useState("24");
  const [manualClose, setManualClose] = useState("");
  const [trustees, setTrustees] = useState(DEFAULT_TRUSTEES);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [created, setCreated] = useState<CreatedAuction | null>(null);

  useEffect(() => {
    api<ChainView>("GET", "/v1/chain", null)
      .then((c) => setTip(c.tip))
      .catch(() => setTip(null));
  }, []);

  const windowHours = Number(hours);
  const computedClose =
    tip !== null && Number.isFinite(windowHours) && windowHours > 0
      ? tip + SETUP_BLOCKS + Math.round(windowHours * BLOCKS_PER_HOUR)
      : null;
  const closeHeight = tip === null ? Number(manualClose) : computedClose;

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (state.kind !== "ready") return;
    const form = new FormData(e.currentTarget);
    const reserve = parseZec(String(form.get("reserve") ?? "0") || "0");
    if (reserve === null) return setError("Enter the reserve in ZEC, for example 0.01.");
    if (tip !== null && windowHours < MIN_WINDOW_HOURS)
      return setError(`Bidding must stay open for at least ${MIN_WINDOW_HOURS} hours.`);
    if (!closeHeight || closeHeight <= 0)
      return setError("Enter the block height bidding closes at.");
    const revealBlocks = Math.round(Number(revealHours) * BLOCKS_PER_HOUR);
    if (sealed && !(revealBlocks >= MIN_WINDOW_HOURS * BLOCKS_PER_HOUR))
      return setError(`The reveal window must be at least ${MIN_WINDOW_HOURS} hours.`);
    const body: CreateAuction = {
      title: String(form.get("title") ?? "").trim(),
      description: String(form.get("description") ?? "").trim(),
      lot_image_url: null,
      network,
      mode,
      bid_mode: sealed ? "commit_reveal" : "deposit",
      reveal_close_height: sealed ? closeHeight + revealBlocks : null,
      quantity: Number(form.get("quantity")),
      reserve_zat: Number(reserve),
      close_height: closeHeight,
      payout_ua: String(form.get("payout_ua") ?? "").trim(),
      trustees: trustees.map((t) => ({ slot: t.slot, label: t.label.trim() })),
    };
    setBusy(true);
    setError(null);
    try {
      setCreated(await api<CreatedAuction>("POST", "/v1/auctions", state.signer, body));
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 403
          ? "Organiser key not recognised. Load the key that `sealforge init` registered."
          : err instanceof Error
            ? err.message
            : "Couldn't create the auction.",
      );
    } finally {
      setBusy(false);
    }
  }

  if (state.kind === "loading") return <p className="text-muted">Loading…</p>;
  if (state.kind === "none")
    return (
      <p className="text-muted">
        Load your organiser key on the{" "}
        <Link href="/console" className="text-text underline">
          console
        </Link>{" "}
        first.
      </p>
    );

  if (created) return <Invites created={created} />;

  return (
    <form onSubmit={submit} className="max-w-[640px] space-y-8" noValidate>
      <fieldset className="space-y-4">
        <legend className="mb-2 text-[18px] leading-6 font-semibold">Lot</legend>
        <div className="space-y-1.5">
          <label htmlFor="title" className={label}>
            Title
          </label>
          <input id="title" name="title" required maxLength={120} className={field} />
        </div>
        <div className="space-y-1.5">
          <label htmlFor="description" className={label}>
            Description <span className="font-normal text-muted">(optional)</span>
          </label>
          <textarea
            id="description"
            name="description"
            rows={3}
            maxLength={2000}
            className={`${field} h-auto py-2`}
          />
        </div>
      </fieldset>

      <fieldset className="space-y-4">
        <legend className="mb-2 text-[18px] leading-6 font-semibold">Rules</legend>
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <label htmlFor="mode" className={label}>
              Format
            </label>
            <select
              id="mode"
              value={mode}
              onChange={(e) => setMode(e.target.value as AuctionMode)}
              className={field}
            >
              <option value="uniform_price">Uniform price</option>
              <option value="first_price">First price</option>
            </select>
            <p className={hint}>
              {mode === "uniform_price"
                ? "Every winner pays the lowest winning bid."
                : "Each winner pays what they bid."}
            </p>
          </div>
          <div className="space-y-1.5">
            <label htmlFor="quantity" className={label}>
              Lots
            </label>
            <input
              id="quantity"
              name="quantity"
              type="number"
              min={1}
              max={10000}
              defaultValue={1}
              required
              className={`${field} num`}
            />
          </div>
          <div className="space-y-1.5">
            <label htmlFor="reserve" className={label}>
              Reserve (ZEC)
            </label>
            <input
              id="reserve"
              name="reserve"
              inputMode="decimal"
              placeholder="0.01"
              defaultValue="0.01"
              className={`${field} num`}
            />
            <p className={hint}>Bids below this are refunded in full.</p>
          </div>
          <div className="space-y-1.5">
            <label htmlFor="network" className={label}>
              Network
            </label>
            <select
              id="network"
              value={network}
              onChange={(e) => setNetwork(e.target.value as Network)}
              className={field}
            >
              <option value="mainnet">Mainnet</option>
              <option value="testnet">Testnet</option>
              <option value="regtest">Regtest (local)</option>
            </select>
          </div>
        </div>

        <div className="space-y-2">
          <label className="flex items-start gap-3">
            <input
              type="checkbox"
              checked={sealed}
              onChange={(e) => setSealed(e.target.checked)}
              className="mt-1 size-4 accent-[var(--primary)]"
            />
            <span>
              <span className={label}>Sealed reveal</span>
              <span className={`block ${hint}`}>
                Bidders deposit a budget and reveal their bid after the close, so nobody (trustees
                included) sees bid amounts while bidding is open. Bidders need one extra step.
              </span>
            </span>
          </label>
          {sealed ? (
            <div className="space-y-1.5 pl-7">
              <label htmlFor="reveal-hours" className={label}>
                Reveal window (hours)
              </label>
              <input
                id="reveal-hours"
                inputMode="decimal"
                value={revealHours}
                onChange={(e) => setRevealHours(e.target.value)}
                className={`${field} num max-w-[200px]`}
              />
              <p className={hint}>Bids that aren&rsquo;t revealed by then don&rsquo;t count.</p>
            </div>
          ) : null}
        </div>

        {tip !== null ? (
          <div className="space-y-1.5">
            <label htmlFor="hours" className={label}>
              Bidding window (hours)
            </label>
            <input
              id="hours"
              inputMode="decimal"
              value={hours}
              onChange={(e) => setHours(e.target.value)}
              className={`${field} num max-w-[200px]`}
            />
            <p className={hint}>
              Closes at block{" "}
              <span className="num text-text">{computedClose?.toLocaleString("en-US") ?? "…"}</span>
              : current height {tip.toLocaleString("en-US")}, plus about an hour for the trustees to
              set up the escrow.
            </p>
          </div>
        ) : (
          <div className="space-y-1.5">
            <label htmlFor="close" className={label}>
              Close height
            </label>
            <input
              id="close"
              inputMode="numeric"
              value={manualClose}
              onChange={(e) => setManualClose(e.target.value)}
              className={`${field} num max-w-[200px]`}
            />
            <p className={hint}>
              The chain height isn't available right now. Bidding closes when this block is mined
              (about 48 blocks per hour).
            </p>
          </div>
        )}
      </fieldset>

      <fieldset className="space-y-4">
        <legend className="mb-2 text-[18px] leading-6 font-semibold">Payout and trustees</legend>
        <div className="space-y-1.5">
          <label htmlFor="payout_ua" className={label}>
            Payout address
          </label>
          <input
            id="payout_ua"
            name="payout_ua"
            required
            spellCheck={false}
            autoComplete="off"
            placeholder={
              network === "mainnet" ? "u1…" : network === "testnet" ? "utest1…" : "uregtest1…"
            }
            className={`${field} num`}
          />
          <p className={hint}>
            A Unified Address with a shielded receiver. Winners' payments arrive here after refunds.
          </p>
        </div>
        <div className="space-y-3">
          <p className={label}>Trustees</p>
          <p className={hint}>
            Any two of the three can move escrow funds; one alone can't. Each gets a one-time invite
            after you create the auction.
          </p>
          {trustees.map((t) => (
            <div key={t.slot} className="flex items-center gap-3">
              <span className="num w-6 text-muted">{t.slot}</span>
              <label htmlFor={`trustee-${t.slot}`} className="sr-only">
                Trustee {t.slot} label
              </label>
              <input
                id={`trustee-${t.slot}`}
                value={t.label}
                maxLength={80}
                onChange={(e) =>
                  setTrustees(
                    trustees.map((v) => (v.slot === t.slot ? { ...v, label: e.target.value } : v)),
                  )
                }
                className={field}
              />
            </div>
          ))}
        </div>
      </fieldset>

      {error ? (
        <p role="alert" className="flex items-start gap-2 text-sm text-danger">
          <TriangleAlert size={16} aria-hidden="true" className="mt-0.5 shrink-0" />
          {error}
        </p>
      ) : null}

      <Button type="submit" disabled={busy}>
        {busy ? "Creating…" : "Create draft"}
      </Button>
    </form>
  );
}

function Invites({ created }: { created: CreatedAuction }) {
  const code = created.auction.code;
  return (
    <div className="max-w-[640px] space-y-6">
      <div className="rounded-[var(--radius-card)] border border-border bg-surface p-6">
        <p className="font-medium">Draft {code} created</p>
        <p className="mt-1 text-sm text-muted">
          Send each trustee their invite privately. They're shown only once.
        </p>
      </div>
      <ol className="space-y-3">
        {created.trustee_invites.map((inv) => (
          <li
            key={inv.slot}
            className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-card)] border border-border p-4"
          >
            <span>
              <span className="num text-muted">{inv.slot}</span>{" "}
              <span className="font-medium">{inv.label}</span>
            </span>
            <HashChip
              value={inv.invite_code}
              label={`invite for ${inv.label}`}
              head={12}
              tail={6}
            />
          </li>
        ))}
      </ol>
      <div className="space-y-2 text-sm text-muted">
        <p>Each trustee runs, on their own machine:</p>
        <pre className="num break-all whitespace-pre-wrap rounded-[var(--radius-control)] bg-surface-2 p-3 text-text">
          {`sealforge trustee join <invite>\nsealforge trustee dkg ${code} --wait`}
        </pre>
        <p>
          Once the escrow is ready, open the auction from its page. Refund and payout fees come out
          of the escrow and reduce your payout.
        </p>
      </div>
      <ButtonLink href={`/console/${code}`}>Go to the auction</ButtonLink>
    </div>
  );
}
