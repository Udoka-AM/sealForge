"use client";

/**
 * SF-Sig request signing in the browser (crates/sealforge-api/src/auth.rs).
 *
 * The organiser key file (`~/.sealforge/organiser.key`) or a trustee's
 * `identity.json` holds a 32-byte Ed25519 seed. It is imported once as a
 * non-extractable WebCrypto key and kept in IndexedDB, so page scripts can
 * sign with it but can never read it back. Requests go through the
 * same-origin proxy at `/api/coordinator`, which forwards them unchanged.
 */

import { del, get, set } from "idb-keyval";

export type Role = "organiser" | `trustee:${string}`;

export interface StoredSigner {
  /** Non-extractable Ed25519 private key. */
  key: CryptoKey;
  /** Public key, standard base64 (the SF-Sig keyId). */
  keyId: string;
  /** Trustee slot and label, from identity.json. */
  slot?: number;
  label?: string;
}

// PKCS#8 wrapper for a raw Ed25519 seed (RFC 8410).
const PKCS8_PREFIX = Uint8Array.from([
  0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
]);

const storeKey = (role: Role) => `sf-signer:${role}`;

function hexToBytes(hex: string): Uint8Array | null {
  const clean = hex.trim();
  if (!/^[0-9a-fA-F]*$/.test(clean) || clean.length % 2 !== 0) return null;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = Number.parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function toBase64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

function base64urlToBase64(v: string): string {
  const s = v.replace(/-/g, "+").replace(/_/g, "/");
  return s + "=".repeat((4 - (s.length % 4)) % 4);
}

function toHex(bytes: ArrayBuffer): string {
  return Array.from(new Uint8Array(bytes), (b) => b.toString(16).padStart(2, "0")).join("");
}

export class KeyFileError extends Error {}

/** True when this browser can sign with Ed25519 through WebCrypto. */
export async function ed25519Supported(): Promise<boolean> {
  try {
    await crypto.subtle.generateKey({ name: "Ed25519" }, false, ["sign"]);
    return true;
  } catch {
    return false;
  }
}

async function importSeed(seed: Uint8Array): Promise<{ key: CryptoKey; keyId: string }> {
  if (seed.length !== 32) throw new KeyFileError("Expected a 32-byte Ed25519 key.");
  const pkcs8 = new Uint8Array(PKCS8_PREFIX.length + 32);
  pkcs8.set(PKCS8_PREFIX);
  pkcs8.set(seed, PKCS8_PREFIX.length);
  // Import once as extractable only to learn the public key, then keep a non-extractable copy.
  const temp = await crypto.subtle.importKey("pkcs8", pkcs8, { name: "Ed25519" }, true, ["sign"]);
  const jwk = await crypto.subtle.exportKey("jwk", temp);
  if (!jwk.x) throw new KeyFileError("Couldn't derive the public key.");
  const key = await crypto.subtle.importKey("pkcs8", pkcs8, { name: "Ed25519" }, false, ["sign"]);
  pkcs8.fill(0);
  return { key, keyId: base64urlToBase64(jwk.x) };
}

/** Load `organiser.key` (64 hex characters). */
export async function loadOrganiserKey(text: string): Promise<StoredSigner> {
  const seed = hexToBytes(text);
  if (seed?.length !== 32)
    throw new KeyFileError("This isn't an organiser key file. It should be 64 hex characters.");
  const signer = await importSeed(seed);
  seed.fill(0);
  await set(storeKey("organiser"), signer);
  return signer;
}

/** Load a trustee's `identity.json` for one auction. Returns the auction code. */
export async function loadTrusteeIdentity(
  text: string,
): Promise<{ code: string; signer: StoredSigner }> {
  let parsed: { auction_code?: string; slot?: number; label?: string; signing_key?: string };
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new KeyFileError("This isn't a trustee identity file (identity.json).");
  }
  const seed = parsed.signing_key ? hexToBytes(parsed.signing_key) : null;
  if (!parsed.auction_code || !seed || seed.length !== 32)
    throw new KeyFileError("This isn't a trustee identity file (identity.json).");
  const signer: StoredSigner = {
    ...(await importSeed(seed)),
    slot: parsed.slot,
    label: parsed.label,
  };
  seed.fill(0);
  await set(storeKey(`trustee:${parsed.auction_code}`), signer);
  return { code: parsed.auction_code, signer };
}

export async function getSigner(role: Role): Promise<StoredSigner | null> {
  try {
    return (await get<StoredSigner>(storeKey(role))) ?? null;
  } catch {
    return null; // IndexedDB unavailable (private mode)
  }
}

export async function forgetSigner(role: Role): Promise<void> {
  await del(storeKey(role));
}

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code?: string,
  ) {
    super(message);
  }
}

/** Call the coordinator through the same-origin proxy, signed when `signer` is given. */
export async function api<T>(
  method: "GET" | "POST" | "PATCH",
  path: `/v1/${string}`,
  signer: StoredSigner | null,
  body?: unknown,
): Promise<T> {
  const bytes =
    body === undefined ? new Uint8Array() : new TextEncoder().encode(JSON.stringify(body));
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["content-type"] = "application/json";
  if (signer) {
    const ts = Math.floor(Date.now() / 1000);
    const nonce = toBase64(crypto.getRandomValues(new Uint8Array(16)));
    const bodyHash = toHex(await crypto.subtle.digest("SHA-256", bytes));
    const message = `SF-Sig-v1\n${method}\n${path}\n${ts}\n${nonce}\n${bodyHash}`;
    const sig = await crypto.subtle.sign(
      { name: "Ed25519" },
      signer.key,
      new TextEncoder().encode(message),
    );
    headers.authorization = `SF-Sig keyId=${signer.keyId}, ts=${ts}, nonce=${nonce}, sig=${toBase64(new Uint8Array(sig))}`;
  }
  let res: Response;
  try {
    res = await fetch(`/api/coordinator${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : bytes,
      cache: "no-store",
    });
  } catch {
    throw new ApiError("The auction server can't be reached right now.", 0);
  }
  if (!res.ok) {
    let err: { error?: string; code?: string } = {};
    try {
      err = await res.json();
    } catch {
      // not JSON
    }
    throw new ApiError(
      err.error ?? `The auction server returned ${res.status}.`,
      res.status,
      err.code,
    );
  }
  return (await res.json()) as T;
}
