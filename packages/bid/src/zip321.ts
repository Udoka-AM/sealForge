import { utf8ToBytes } from "@noble/hashes/utils";
import { formatZec } from "./amount";

/** Base64url without padding, as ZIP-321 requires for memos. */
export function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Single-payment ZIP-321 URI. Mirrors sealforge-core::zip321::payment_uri. */
export function paymentUri(address: string, zat: bigint, memo: string): string {
  return `zcash:${address}?amount=${formatZec(zat)}&memo=${base64url(utf8ToBytes(memo))}`;
}
