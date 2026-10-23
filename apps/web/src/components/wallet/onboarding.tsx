"use client";

import {
  ArrowLeft,
  ArrowRight,
  Eye,
  FileDown,
  KeyRound,
  LoaderCircle,
  QrCode,
  Search,
  ShieldCheck,
} from "lucide-react";
import { AnimatePresence, m } from "motion/react";
import { type ReactNode, useEffect, useId, useMemo, useState } from "react";
import { Button, ButtonLink } from "@/components/button";
import { blockSeconds } from "@/lib/format";
import { getLatestHeight } from "@/lib/grpc-web";
import { walletTools } from "@/lib/wallet/session";
import { CreateIcon, LinkIcon, QrIcon } from "./wallet-icons";
import { useWallet } from "./wallet-provider";

const MIN_PASSPHRASE = 10;

type Mode = "choose" | "create" | "restore" | "external";

const EASE = [0.22, 1, 0.36, 1] as const;

/** First screen when no wallet is stored in this browser. Steps slide between each other. */
export function Onboarding() {
  const [mode, setMode] = useState<Mode>("choose");
  const back = () => setMode("choose");
  return (
    <AnimatePresence mode="wait" initial={false}>
      <m.div
        key={mode}
        initial={{ opacity: 0, x: mode === "choose" ? -16 : 16 }}
        animate={{ opacity: 1, x: 0 }}
        exit={{ opacity: 0, x: mode === "choose" ? 16 : -16 }}
        transition={{ duration: 0.3, ease: EASE }}
      >
        {mode === "create" ? (
          <CreateWallet onBack={back} />
        ) : mode === "restore" ? (
          <RestoreWallet onBack={back} />
        ) : mode === "external" ? (
          <ExternalWallet onBack={back} />
        ) : (
          <div className="space-y-8">
            <m.ul
              initial="hidden"
              animate="shown"
              variants={{ shown: { transition: { staggerChildren: 0.07 } } }}
              className="grid gap-3 md:grid-cols-3"
            >
              <Way
                icon={<CreateIcon />}
                title="Create a wallet"
                body="A new 24-word recovery phrase, made in this browser."
                meta="About 2 minutes"
                onClick={() => setMode("create")}
              />
              <Way
                icon={<LinkIcon />}
                title="Link an existing wallet"
                body="Import the 24-word recovery phrase from Zashi, Zodl or another Zcash wallet."
                meta="Your phrase stays here"
                onClick={() => setMode("restore")}
              />
              <Way
                icon={<QrIcon />}
                title="Use your own wallet"
                body="No import: pay each bid by scanning its QR code with the wallet you already use."
                meta="Nothing to set up"
                onClick={() => setMode("external")}
              />
            </m.ul>
            <PrivacyNotes />
          </div>
        )}
      </m.div>
    </AnimatePresence>
  );
}

function Way({
  icon,
  title,
  body,
  meta,
  onClick,
}: {
  icon: ReactNode;
  title: string;
  body: string;
  meta: string;
  onClick: () => void;
}) {
  return (
    <m.li
      variants={{ hidden: { opacity: 0, y: 12 }, shown: { opacity: 1, y: 0 } }}
      transition={{ duration: 0.45, ease: EASE }}
    >
      <button
        type="button"
        onClick={onClick}
        className="lift group flex h-full w-full flex-col gap-5 rounded-[var(--radius-card)] border border-border bg-surface p-5 text-left"
      >
        <span className="inline-flex size-16 items-center justify-center rounded-[14px] border border-border bg-bg text-primary transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:-rotate-3 group-hover:scale-105">
          {icon}
        </span>
        <span className="flex-1 space-y-1.5">
          <span className="block text-[17px] font-semibold tracking-tight">{title}</span>
          <span className="block text-sm leading-6 text-muted">{body}</span>
        </span>
        <span className="flex items-center justify-between border-t border-border pt-3 font-mono text-[11px] tracking-[0.12em] text-muted uppercase">
          {meta}
          <ArrowRight
            size={15}
            aria-hidden="true"
            className="text-primary transition-transform duration-200 group-hover:translate-x-0.5"
          />
        </span>
      </button>
    </m.li>
  );
}

/** Bidding without importing anything: what happens, step by step. */
function ExternalWallet({ onBack }: { onBack: () => void }) {
  const steps = [
    {
      icon: <Search size={16} />,
      title: "Open an auction",
      body: "Enter your bid, and paste a fresh Unified Address from your wallet for refunds.",
    },
    {
      icon: <QrCode size={16} />,
      title: "Scan one QR code",
      body: "It's a ZIP-321 payment request: any Zcash wallet with shielded payments can pay it, memo included.",
    },
    {
      icon: <FileDown size={16} />,
      title: "Keep your receipt",
      body: "It's made in this browser. Download it: it proves your bid and where your refund goes.",
    },
  ];
  return (
    <div className="space-y-6">
      <BackButton onBack={onBack} />
      <div className="space-y-2">
        <h2 className="text-[22px] font-semibold tracking-tight">Use your own wallet</h2>
        <p className="text-muted">
          Nothing to import or install. Your wallet sends the bid, and refunds come back to a fresh
          address from that wallet, used for this bid only.
        </p>
      </div>
      <ol className="relative space-y-5 border-l border-border pl-6">
        {steps.map((st, i) => (
          <m.li
            key={st.title}
            initial={{ opacity: 0, x: -8 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.4, ease: EASE, delay: 0.1 + i * 0.1 }}
            className="relative"
          >
            <span
              aria-hidden="true"
              className="absolute top-0 -left-[37px] inline-flex size-[26px] items-center justify-center rounded-full border border-border bg-bg text-primary"
            >
              {st.icon}
            </span>
            <span className="block font-medium">{st.title}</span>
            <span className="text-sm text-muted">{st.body}</span>
          </m.li>
        ))}
      </ol>
      <p className="rounded-[var(--radius-control)] border border-border bg-surface p-3 text-sm text-muted">
        Paying from your own wallet, this site can&rsquo;t see your payment until the result is
        published. My bids then confirms it from the chain.
      </p>
      <ButtonLink href="/#live" size="lg" className="group">
        Find an auction
        <ArrowRight
          size={18}
          aria-hidden="true"
          className="transition-transform duration-200 group-hover:translate-x-0.5"
        />
      </ButtonLink>
    </div>
  );
}

export function PrivacyNotes() {
  return (
    <ul className="space-y-2 rounded-[var(--radius-card)] border border-border p-5 text-sm text-muted">
      <li className="flex gap-2.5">
        <KeyRound size={16} aria-hidden="true" className="mt-0.5 shrink-0 text-primary" />
        <span>
          Your recovery phrase and keys never leave this browser. sealForge&rsquo;s server never
          sees them, and can&rsquo;t recover them for you.
        </span>
      </li>
      <li className="flex gap-2.5">
        <ShieldCheck size={16} aria-hidden="true" className="mt-0.5 shrink-0 text-primary" />
        <span>
          Each bid gets a fresh refund address, so your refunds can&rsquo;t be linked to each other
          or to the address you fund the wallet from.
        </span>
      </li>
      <li className="flex gap-2.5">
        <Eye size={16} aria-hidden="true" className="mt-0.5 shrink-0 text-muted" />
        <span>
          The lightwallet server this wallet syncs from sees your IP address and when you sync and
          send. Funding amounts and timing can still hint at who you are: fund well before you bid,
          in round amounts.
        </span>
      </li>
    </ul>
  );
}

function BackButton({ onBack }: { onBack: () => void }) {
  return (
    <button
      type="button"
      onClick={onBack}
      className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-text"
    >
      <ArrowLeft size={16} aria-hidden="true" /> Back
    </button>
  );
}

/** Three distinct word positions to confirm, chosen once per phrase. */
function pickPositions(): number[] {
  const out = new Set<number>();
  const rnd = crypto.getRandomValues(new Uint32Array(8));
  for (const r of rnd) {
    out.add(r % 24);
    if (out.size === 3) break;
  }
  return [...out].sort((a, b) => a - b);
}

function CreateWallet({ onBack }: { onBack: () => void }) {
  const [phrase, setPhrase] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [step, setStep] = useState<"show" | "confirm" | "protect">("show");
  const [revealed, setRevealed] = useState(false);
  const words = useMemo(() => phrase?.split(" ") ?? [], [phrase]);
  const positions = useMemo(() => (phrase ? pickPositions() : []), [phrase]);

  useEffect(() => {
    walletTools
      .newPhrase()
      .then(setPhrase)
      .catch((e: Error) => setError(e.message));
    // Warm the prover up while the user writes the phrase down.
    void walletTools.prepareProver().catch(() => {});
  }, []);

  if (error)
    return (
      <p role="alert" className="text-danger">
        {error}
      </p>
    );
  if (!phrase)
    return (
      <p className="flex items-center gap-2 text-muted">
        <LoaderCircle size={16} className="animate-spin" aria-hidden="true" /> Loading the wallet
        (about 6 MB, only the first time) and making your recovery phrase…
      </p>
    );

  if (step === "show")
    return (
      <div className="space-y-5 fade-in">
        <BackButton onBack={onBack} />
        <h2 className="text-[20px] font-semibold tracking-tight">
          Write down your recovery phrase
        </h2>
        <p className="text-sm text-muted">
          These 24 words are the only way to recover this wallet and any refunds or winnings in it.
          Write them on paper, in order. Don&rsquo;t screenshot them or paste them anywhere online.
        </p>
        <div className="relative">
          <ol
            aria-label="Recovery phrase"
            className={`grid grid-cols-2 gap-2 rounded-[var(--radius-card)] border border-border bg-surface p-4 sm:grid-cols-3 ${
              revealed ? "" : "select-none blur-md"
            }`}
            aria-hidden={!revealed}
          >
            {words.map((w, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a phrase word is identified by its position
              <li key={`${i}-${w}`} className="num flex gap-2 text-[15px]">
                <span className="w-6 text-right text-muted">{i + 1}</span>
                <span>{w}</span>
              </li>
            ))}
          </ol>
          {revealed ? null : (
            <div className="absolute inset-0 flex items-center justify-center">
              <Button type="button" variant="secondary" onClick={() => setRevealed(true)}>
                <Eye size={16} aria-hidden="true" /> Show phrase
              </Button>
            </div>
          )}
        </div>
        <Button
          type="button"
          size="lg"
          className="w-full"
          disabled={!revealed}
          onClick={() => setStep("confirm")}
        >
          I&rsquo;ve written it down
        </Button>
      </div>
    );

  if (step === "confirm")
    return (
      <ConfirmWords
        words={words}
        positions={positions}
        onBack={() => setStep("show")}
        onConfirmed={() => setStep("protect")}
      />
    );

  return <ProtectStep phrase={phrase} onBack={() => setStep("confirm")} />;
}

function ConfirmWords({
  words,
  positions,
  onBack,
  onConfirmed,
}: {
  words: string[];
  positions: number[];
  onBack: () => void;
  onConfirmed: () => void;
}) {
  const [answers, setAnswers] = useState<Record<number, string>>({});
  const [error, setError] = useState<string | null>(null);
  const baseId = useId();
  return (
    <form
      className="space-y-5 fade-in"
      onSubmit={(e) => {
        e.preventDefault();
        const ok = positions.every((p) => (answers[p] ?? "").trim().toLowerCase() === words[p]);
        if (ok) onConfirmed();
        else setError("One of those words doesn't match. Check your written copy.");
      }}
    >
      <BackButton onBack={onBack} />
      <h2 className="text-[20px] font-semibold tracking-tight">Check your copy</h2>
      <p className="text-sm text-muted">Type these words from the phrase you wrote down.</p>
      {positions.map((p) => (
        <div key={p} className="space-y-1.5">
          <label htmlFor={`${baseId}-${p}`} className="block text-sm font-medium">
            Word {p + 1}
          </label>
          <input
            id={`${baseId}-${p}`}
            autoComplete="off"
            autoCapitalize="off"
            spellCheck={false}
            value={answers[p] ?? ""}
            onChange={(e) => {
              setAnswers((a) => ({ ...a, [p]: e.target.value }));
              setError(null);
            }}
            className="num h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
          />
        </div>
      ))}
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
      <Button type="submit" size="lg" className="w-full">
        Continue
      </Button>
    </form>
  );
}

function PassphraseFields({
  passphrase,
  confirm,
  setPassphrase,
  setConfirm,
}: {
  passphrase: string;
  confirm: string;
  setPassphrase: (s: string) => void;
  setConfirm: (s: string) => void;
}) {
  const id = useId();
  const strength =
    passphrase.length === 0
      ? null
      : passphrase.length < MIN_PASSPHRASE
        ? `At least ${MIN_PASSPHRASE} characters.`
        : passphrase.length < 16
          ? "OK. Longer is stronger: a few unrelated words work well."
          : "Strong length.";
  return (
    <>
      <div className="space-y-1.5">
        <label htmlFor={`${id}-p`} className="block text-sm font-medium">
          Passphrase for this browser
        </label>
        <input
          id={`${id}-p`}
          type="password"
          autoComplete="new-password"
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
          aria-describedby={`${id}-help`}
          className="h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
        />
        <p id={`${id}-help`} className="text-sm text-muted">
          {strength ??
            "Encrypts the wallet on this device. You'll type it each time you open the wallet."}
        </p>
      </div>
      <div className="space-y-1.5">
        <label htmlFor={`${id}-c`} className="block text-sm font-medium">
          Type it again
        </label>
        <input
          id={`${id}-c`}
          type="password"
          autoComplete="new-password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          className="h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
        />
      </div>
    </>
  );
}

function passphraseProblem(passphrase: string, confirm: string): string | null {
  if (passphrase.length < MIN_PASSPHRASE) return `Use at least ${MIN_PASSPHRASE} characters.`;
  if (passphrase !== confirm) return "The two passphrases don't match.";
  return null;
}

function ProtectStep({ phrase, onBack }: { phrase: string; onBack: () => void }) {
  const wallet = useWallet();
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <form
      className="space-y-5 fade-in"
      onSubmit={async (e) => {
        e.preventDefault();
        const problem = passphraseProblem(passphrase, confirm);
        if (problem) return setError(problem);
        setBusy(true);
        setError(null);
        try {
          await wallet.create({ phrase, passphrase });
        } catch (err) {
          setError((err as Error).message);
          setBusy(false);
        }
      }}
    >
      <BackButton onBack={onBack} />
      <h2 className="text-[20px] font-semibold tracking-tight">Protect it on this device</h2>
      <PassphraseFields
        passphrase={passphrase}
        confirm={confirm}
        setPassphrase={setPassphrase}
        setConfirm={setConfirm}
      />
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
      <Button type="submit" size="lg" className="w-full" disabled={busy}>
        {busy ? <LoaderCircle size={18} className="animate-spin" aria-hidden="true" /> : null}
        {busy ? "Creating your wallet…" : "Create wallet"}
      </Button>
    </form>
  );
}

const BIRTHDAYS = [
  { key: "day", label: "In the last day", days: 1 },
  { key: "week", label: "In the last week", days: 7 },
  { key: "month", label: "In the last month", days: 30 },
  { key: "exact", label: "I know the block height", days: 0 },
] as const;

function RestoreWallet({ onBack }: { onBack: () => void }) {
  const wallet = useWallet();
  const id = useId();
  const [phrase, setPhrase] = useState("");
  const [when, setWhen] = useState<(typeof BIRTHDAYS)[number]["key"]>("week");
  const [height, setHeight] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  return (
    <form
      className="space-y-5 fade-in"
      onSubmit={async (e) => {
        e.preventDefault();
        setError(null);
        const words = phrase.trim().split(/\s+/);
        if (words.length !== 24) return setError("A recovery phrase has 24 words.");
        if (!(await walletTools.phraseIsValid(phrase)))
          return setError("That phrase isn't valid: check each word and the order.");
        const problem = passphraseProblem(passphrase, confirm);
        if (problem) return setError(problem);
        setBusy(true);
        try {
          let birthday: number | undefined;
          if (when === "exact") {
            birthday = Number(height);
            if (!Number.isInteger(birthday) || birthday <= 0)
              throw new Error("Enter a block height.");
          } else if (wallet.endpoint && wallet.network) {
            const tip = await getLatestHeight(wallet.endpoint);
            const days = BIRTHDAYS.find((b) => b.key === when)?.days ?? 7;
            birthday = Math.max(1, tip - Math.ceil((days * 86_400) / blockSeconds(wallet.network)));
          }
          await wallet.create({ phrase: words.join(" ").toLowerCase(), passphrase, birthday });
        } catch (err) {
          setError((err as Error).message);
          setBusy(false);
        }
      }}
    >
      <BackButton onBack={onBack} />
      <h2 className="text-[20px] font-semibold tracking-tight">Link an existing wallet</h2>
      <p className="rounded-[var(--radius-control)] border border-border bg-surface p-3 text-sm text-muted">
        Works with wallets that use a 24-word phrase and the standard first account (ZIP 32 account
        0). Here you&rsquo;ll see Orchard and Ironwood funds and can spend Ironwood funds; Sapling
        and transparent funds stay in your other wallet. The phrase never leaves this browser.
      </p>
      <div className="space-y-1.5">
        <label htmlFor={`${id}-phrase`} className="block text-sm font-medium">
          Recovery phrase
        </label>
        <textarea
          id={`${id}-phrase`}
          rows={4}
          spellCheck={false}
          autoComplete="off"
          autoCapitalize="off"
          value={phrase}
          onChange={(e) => setPhrase(e.target.value)}
          className="num w-full resize-none rounded-[var(--radius-control)] border border-border bg-surface-2 p-3 text-[14px] outline-none focus:border-primary"
        />
        <p className="text-sm text-muted">
          24 words, separated by spaces. Checked here, never sent.
        </p>
      </div>
      <fieldset className="space-y-2">
        <legend className="text-sm font-medium">When did you create it?</legend>
        <div className="grid gap-2 sm:grid-cols-2">
          {BIRTHDAYS.map((b) => (
            <label
              key={b.key}
              className={`flex cursor-pointer items-center gap-2 rounded-[var(--radius-control)] border px-3 py-2.5 text-sm transition-colors duration-150 has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-primary ${
                when === b.key
                  ? "border-primary text-text"
                  : "border-border text-muted hover:border-muted"
              }`}
            >
              <input
                type="radio"
                name="birthday"
                className="sr-only"
                checked={when === b.key}
                onChange={() => setWhen(b.key)}
              />
              {b.label}
            </label>
          ))}
        </div>
        {when === "exact" ? (
          <input
            inputMode="numeric"
            aria-label="Block height"
            placeholder="Block height"
            value={height}
            onChange={(e) => setHeight(e.target.value.replace(/\D/g, ""))}
            className="num h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
          />
        ) : null}
        <p className="text-sm text-muted">
          The wallet scans from then on. Earlier is safer; later is faster.
        </p>
      </fieldset>
      <PassphraseFields
        passphrase={passphrase}
        confirm={confirm}
        setPassphrase={setPassphrase}
        setConfirm={setConfirm}
      />
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
      <Button type="submit" size="lg" className="w-full" disabled={busy}>
        {busy ? <LoaderCircle size={18} className="animate-spin" aria-hidden="true" /> : null}
        {busy ? "Linking…" : "Link wallet"}
      </Button>
    </form>
  );
}

/** Passphrase prompt for a stored wallet. */
export function UnlockForm({ onForgot }: { onForgot: () => void }) {
  const wallet = useWallet();
  const id = useId();
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <form
      className="space-y-4 rounded-[var(--radius-card)] border border-border bg-surface p-5 fade-in"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        setError(null);
        try {
          await wallet.unlock(passphrase);
        } catch (err) {
          setError((err as Error).message);
          setBusy(false);
        }
      }}
    >
      <div className="space-y-1.5">
        <label htmlFor={id} className="block text-sm font-medium">
          Passphrase
        </label>
        <input
          id={id}
          type="password"
          autoComplete="current-password"
          value={passphrase}
          onChange={(e) => {
            setPassphrase(e.target.value);
            setError(null);
          }}
          aria-invalid={Boolean(error)}
          className="h-11 w-full rounded-[var(--radius-control)] border border-border bg-surface-2 px-3 outline-none focus:border-primary"
        />
      </div>
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
      <Button type="submit" className="w-full" disabled={busy || passphrase.length === 0}>
        {busy ? <LoaderCircle size={16} className="animate-spin" aria-hidden="true" /> : null}
        {busy ? "Unlocking…" : "Unlock wallet"}
      </Button>
      <button
        type="button"
        onClick={onForgot}
        className="w-full text-center text-sm text-muted hover:text-text"
      >
        Forgot it? Restore from your recovery phrase
      </button>
    </form>
  );
}

export { RestoreWallet };
