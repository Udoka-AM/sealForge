"use client";

import { createStore, del, get, set } from "idb-keyval";
import type { Network } from "@/lib/types";

/**
 * The wallet's recovery phrase and state, encrypted under the user's
 * passphrase and kept in this browser's IndexedDB. WebCrypto does the work:
 * PBKDF2-SHA-256 (600,000 iterations) derives an AES-GCM-256 key, which stays
 * non-extractable in memory while the wallet is unlocked. Nothing here is
 * ever sent to a server.
 */

const ITERATIONS = 600_000;

export interface VaultRecord {
  v: 1;
  network: Network;
  created_at: string;
  birthday: number;
  iterations: number;
  salt: string;
  iv: string;
  ciphertext: string;
}

export interface VaultSecrets {
  phrase: string;
  /** The wallet state from WebAssembly (`WalletState` JSON). */
  state: string;
}

const store = () => createStore("sealforge-wallet", "vault");

const b64 = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes));
const unb64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

async function deriveKey(passphrase: string, salt: Uint8Array, iterations: number) {
  const material = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(passphrase),
    "PBKDF2",
    false,
    ["deriveKey"],
  );
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", hash: "SHA-256", salt: salt as BufferSource, iterations },
    material,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

/** An unlocked vault: the derived key and the record's fixed fields. */
export interface OpenVault {
  key: CryptoKey;
  record: Omit<VaultRecord, "iv" | "ciphertext">;
}

export async function hasVault(network: Network): Promise<boolean> {
  return (await get<VaultRecord>(network, store())) !== undefined;
}

export async function vaultInfo(
  network: Network,
): Promise<Pick<VaultRecord, "created_at" | "birthday"> | null> {
  const r = await get<VaultRecord>(network, store());
  return r ? { created_at: r.created_at, birthday: r.birthday } : null;
}

async function write(vault: OpenVault, secrets: VaultSecrets): Promise<void> {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: iv as BufferSource },
      vault.key,
      new TextEncoder().encode(JSON.stringify(secrets)),
    ),
  );
  await set(
    vault.record.network,
    { ...vault.record, iv: b64(iv), ciphertext: b64(ciphertext) } satisfies VaultRecord,
    store(),
  );
}

/** Encrypt a new wallet under `passphrase`, replacing any wallet for this network. */
export async function createVault(
  network: Network,
  passphrase: string,
  birthday: number,
  secrets: VaultSecrets,
): Promise<OpenVault> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const vault: OpenVault = {
    key: await deriveKey(passphrase, salt, ITERATIONS),
    record: {
      v: 1,
      network,
      created_at: new Date().toISOString(),
      birthday,
      iterations: ITERATIONS,
      salt: b64(salt),
    },
  };
  await write(vault, secrets);
  return vault;
}

export class WrongPassphrase extends Error {}

/** Decrypt the stored wallet. Throws [`WrongPassphrase`] if the passphrase is wrong. */
export async function unlockVault(
  network: Network,
  passphrase: string,
): Promise<{ vault: OpenVault; secrets: VaultSecrets }> {
  const record = await get<VaultRecord>(network, store());
  if (!record) throw new Error("No wallet is stored in this browser.");
  const key = await deriveKey(passphrase, unb64(record.salt), record.iterations);
  let plain: ArrayBuffer;
  try {
    plain = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: unb64(record.iv) as BufferSource },
      key,
      unb64(record.ciphertext) as BufferSource,
    );
  } catch {
    throw new WrongPassphrase("That passphrase doesn't open this wallet.");
  }
  const { iv: _iv, ciphertext: _c, ...fixed } = record;
  return {
    vault: { key, record: fixed },
    secrets: JSON.parse(new TextDecoder().decode(plain)) as VaultSecrets,
  };
}

/** Save the latest wallet state (fresh IV each time). */
export function saveVault(vault: OpenVault, secrets: VaultSecrets): Promise<void> {
  return write(vault, secrets);
}

export function deleteVault(network: Network): Promise<void> {
  return del(network, store());
}
