"use client";

import { KeyRound, Upload } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { ed25519Supported, KeyFileError } from "@/lib/sfsig";

/**
 * Load a key file into this browser. The parsed key is stored non-extractable,
 * so it can sign requests but not be read back by any script.
 */
export function KeyLoader({
  title,
  fileHint,
  accept,
  onLoad,
}: {
  title: string;
  /** Where the file lives, e.g. `~/.sealforge/organiser.key`. */
  fileHint: string;
  accept: string;
  onLoad: (text: string) => Promise<void>;
}) {
  const inputId = useId();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [supported, setSupported] = useState(true);

  useEffect(() => {
    ed25519Supported().then(setSupported);
  }, []);

  async function load(file: File | undefined) {
    if (!file) return;
    setBusy(true);
    setError(null);
    try {
      await onLoad(await file.text());
      window.dispatchEvent(new Event("sf-signer-changed"));
    } catch (e) {
      setError(e instanceof KeyFileError ? e.message : "Couldn't load that file.");
    } finally {
      setBusy(false);
    }
  }

  if (!supported)
    return (
      <div
        role="alert"
        className="rounded-[var(--radius-card)] border border-border bg-surface p-6"
      >
        <p className="font-medium">This browser can't sign requests</p>
        <p className="mt-1 text-sm text-muted">
          The console needs Ed25519 in WebCrypto: Chrome 137+, Firefox 129+ or Safari 17+. The{" "}
          <code className="num">sealforge</code> CLI does everything the console does.
        </p>
      </div>
    );

  return (
    <section
      aria-labelledby={`${inputId}-title`}
      className="max-w-[560px] space-y-4 rounded-[var(--radius-card)] border border-border bg-surface p-6"
    >
      <div className="flex items-center gap-3">
        <KeyRound size={20} aria-hidden="true" className="text-primary" />
        <h2 id={`${inputId}-title`} className="text-[18px] leading-6 font-semibold">
          {title}
        </h2>
      </div>
      <label
        htmlFor={inputId}
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragging(false);
          load(e.dataTransfer.files[0]);
        }}
        className={`flex cursor-pointer flex-col items-center gap-2 rounded-[var(--radius-control)] border border-dashed px-4 py-8 text-center transition-colors duration-150 ${
          dragging ? "border-primary bg-surface-2" : "border-border hover:border-muted"
        }`}
      >
        <Upload size={20} aria-hidden="true" className="text-muted" />
        <span className="font-medium">{busy ? "Loading…" : "Choose or drop the key file"}</span>
        <span className="num text-[13px] text-muted">{fileHint}</span>
      </label>
      <input
        id={inputId}
        type="file"
        accept={accept}
        className="sr-only"
        onChange={(e) => load(e.target.files?.[0])}
      />
      {error ? (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      ) : null}
      <p className="text-sm text-muted">
        The key stays in this browser, stored so that it can sign requests but can't be read back.
        Nothing is uploaded. Use <span className="font-medium text-text">Forget key</span> on shared
        computers.
      </p>
    </section>
  );
}
