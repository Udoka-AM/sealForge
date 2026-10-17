"use client";

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { Network } from "@/lib/types";
import {
  type Built,
  type PaymentRequest,
  type SyncProgress,
  WalletSession,
  type WalletSummary,
} from "@/lib/wallet/session";
import { deleteVault, hasVault, vaultInfo } from "@/lib/wallet/vault";

export type WalletStatus = "loading" | "unavailable" | "none" | "locked" | "unlocked";

export interface SyncState {
  running: boolean;
  progress: SyncProgress | null;
  lastSynced: number | null;
  error: string | null;
}

interface WalletContext {
  status: WalletStatus;
  /** Why the wallet can't run here (no lightwallet endpoint, no IndexedDB…). */
  unavailableReason: string | null;
  network: Network | null;
  endpoint: string | null;
  createdAt: string | null;
  summary: WalletSummary | null;
  sync: SyncState;
  create: (args: { phrase: string; passphrase: string; birthday?: number }) => Promise<void>;
  unlock: (passphrase: string) => Promise<void>;
  lock: () => void;
  remove: () => Promise<void>;
  syncNow: () => Promise<WalletSummary | null>;
  freshAddress: () => Promise<{ index: number; address: string }>;
  address: (index: number) => Promise<string>;
  quote: (amountZat: number) => Promise<number>;
  pay: (request: PaymentRequest) => Promise<Built>;
  revealPhrase: (passphrase: string) => Promise<string>;
}

const Ctx = createContext<WalletContext | null>(null);

const SYNC_EVERY_MS = 30_000;

export function WalletProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<WalletStatus>("loading");
  const [unavailableReason, setUnavailableReason] = useState<string | null>(null);
  const [network, setNetwork] = useState<Network | null>(null);
  const [endpoint, setEndpoint] = useState<string | null>(null);
  const [createdAt, setCreatedAt] = useState<string | null>(null);
  const [summary, setSummary] = useState<WalletSummary | null>(null);
  const [sync, setSync] = useState<SyncState>({
    running: false,
    progress: null,
    lastSynced: null,
    error: null,
  });
  const session = useRef<WalletSession | null>(null);

  // Which network and lightwallet server this deployment uses, and whether a wallet is stored.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/wallet/config", { cache: "no-store" });
        const config = (await res.json()) as { network: Network; grpcWeb: string | null };
        if (cancelled) return;
        setNetwork(config.network);
        if (!config.grpcWeb) {
          setUnavailableReason(
            "This site has no lightwallet server configured, so the wallet can't sync.",
          );
          setStatus("unavailable");
          return;
        }
        if (typeof Worker === "undefined" || typeof indexedDB === "undefined") {
          setUnavailableReason(
            "This browser can't run the wallet (it needs Web Workers and IndexedDB).",
          );
          setStatus("unavailable");
          return;
        }
        setEndpoint(config.grpcWeb);
        const info = await vaultInfo(config.network);
        if (cancelled) return;
        setCreatedAt(info?.created_at ?? null);
        setStatus(info ? "locked" : "none");
      } catch {
        if (!cancelled) {
          setUnavailableReason(
            "The wallet couldn't start: this browser blocks storage (private mode?) or the site is unreachable.",
          );
          setStatus("unavailable");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const syncNow = useCallback(async () => {
    const s = session.current;
    if (!s) return null;
    setSync((x) => ({ ...x, running: true, error: null }));
    try {
      const result = await s.sync((progress) => setSync((x) => ({ ...x, progress })));
      setSummary(result);
      setSync((x) => ({ ...x, running: false, lastSynced: Date.now(), error: null }));
      return result;
    } catch (e) {
      setSync((x) => ({ ...x, running: false, error: (e as Error).message }));
      return null;
    }
  }, []);

  // Keep syncing while unlocked and the tab is visible.
  useEffect(() => {
    if (status !== "unlocked") return;
    const tick = () => {
      if (document.visibilityState === "visible") void syncNow();
    };
    const timer = window.setInterval(tick, SYNC_EVERY_MS);
    document.addEventListener("visibilitychange", tick);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", tick);
    };
  }, [status, syncNow]);

  const opened = useCallback(
    async (s: WalletSession) => {
      session.current = s;
      setSummary(await s.summary());
      setStatus("unlocked");
      void syncNow();
    },
    [syncNow],
  );

  const create = useCallback(
    async (args: { phrase: string; passphrase: string; birthday?: number }) => {
      if (!network || !endpoint) throw new Error("The wallet isn't available here.");
      const s = await WalletSession.create({ network, endpoint, ...args });
      setCreatedAt(new Date().toISOString());
      await opened(s);
    },
    [network, endpoint, opened],
  );

  const unlock = useCallback(
    async (passphrase: string) => {
      if (!network || !endpoint) throw new Error("The wallet isn't available here.");
      await opened(await WalletSession.unlock(network, endpoint, passphrase));
    },
    [network, endpoint, opened],
  );

  const lock = useCallback(() => {
    session.current?.lock();
    session.current = null;
    setSummary(null);
    setSync({ running: false, progress: null, lastSynced: null, error: null });
    setStatus((s) => (s === "unlocked" ? "locked" : s));
  }, []);

  const remove = useCallback(async () => {
    if (!network) return;
    session.current?.lock();
    session.current = null;
    await deleteVault(network);
    setSummary(null);
    setCreatedAt(null);
    setStatus((await hasVault(network)) ? "locked" : "none");
  }, [network]);

  const need = useCallback(() => {
    if (!session.current) throw new Error("Unlock your wallet first.");
    return session.current;
  }, []);

  const value = useMemo<WalletContext>(
    () => ({
      status,
      unavailableReason,
      network,
      endpoint,
      createdAt,
      summary,
      sync,
      create,
      unlock,
      lock,
      remove,
      syncNow,
      freshAddress: async () => need().freshAddress(),
      address: async (i) => need().address(i),
      quote: async (z) => need().quote(z),
      pay: async (req) => {
        const built = await need().pay(req);
        setSummary(await need().summary());
        return built;
      },
      revealPhrase: async (p) => need().revealPhrase(p),
    }),
    [
      status,
      unavailableReason,
      network,
      endpoint,
      createdAt,
      summary,
      sync,
      create,
      unlock,
      lock,
      remove,
      syncNow,
      need,
    ],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useWallet(): WalletContext {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useWallet must be used inside <WalletProvider>");
  return ctx;
}
