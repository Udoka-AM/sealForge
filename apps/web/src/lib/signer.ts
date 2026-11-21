"use client";

import type { VerifiedBatch } from "./types";

/**
 * Trustee batch signing, compiled from crates/sealforge-signer-wasm
 * (scripts/build-signer-wasm.sh): the same FROST code the CLI runs. The key
 * share and nonces passed in here live only in this page's memory.
 */
interface SignerModule {
  default: (init: { module_or_path: string }) => Promise<unknown>;
  key_info: (keyPackageHex: string) => string;
  commit_batch: (keyPackageHex: string, spends: number) => string;
  sign_batch: (
    keyPackageHex: string,
    sighashHex: string,
    alphasJson: string,
    signerCommitmentsJson: string,
    noncesJson: string,
  ) => string;
  verify_batch: (
    keyPackageHex: string,
    encSecretHex: string,
    sealedViewSecretHex: string,
    escrowUa: string,
    network: string,
    nowSecs: number,
    batchJson: string,
    refundTotalDueZat: number,
  ) => string;
}

let loading: Promise<SignerModule> | null = null;

function load(): Promise<SignerModule> {
  loading ??= (async () => {
    const url = "/signer/sealforge_signer_wasm.js";
    const mod = (await import(
      /* webpackIgnore: true */ /* turbopackIgnore: true */ url
    )) as SignerModule;
    await mod.default({ module_or_path: "/signer/sealforge_signer_wasm_bg.wasm" });
    return mod;
  })();
  return loading;
}

export interface KeyInfo {
  slot: number;
  group_key: string;
}

export interface Round1 {
  nonces: string[];
  commitments: string[];
}

export async function keyInfo(keyPackageHex: string): Promise<KeyInfo> {
  return JSON.parse((await load()).key_info(keyPackageHex)) as KeyInfo;
}

export async function commitBatch(keyPackageHex: string, spends: number): Promise<Round1> {
  return JSON.parse((await load()).commit_batch(keyPackageHex, spends)) as Round1;
}

export async function signBatch(
  keyPackageHex: string,
  sighash: string,
  alphas: string[],
  signerCommitments: Record<string, string[]>,
  nonces: string[],
): Promise<string[]> {
  return JSON.parse(
    (await load()).sign_batch(
      keyPackageHex,
      sighash,
      JSON.stringify(alphas),
      JSON.stringify(signerCommitments),
      JSON.stringify(nonces),
    ),
  ) as string[];
}

/**
 * Digest of everything a round 2 share depends on besides the key and the
 * nonces (hex), computed by the same Rust code as `sealforge trustee sign`.
 */
export async function round2InputsDigest(
  sighash: string,
  alphas: string[],
  signerCommitments: Record<string, string[]>,
): Promise<string> {
  return (await load()).round2_inputs_digest(
    sighash,
    JSON.stringify(alphas),
    JSON.stringify(signerCommitments),
  );
}

/**
 * Decode and check a batch's transaction before taking part in signing it
 * (security review D1): the same checks as `sealforge trustee sign`. Throws
 * with the refusal when the transaction isn't one this trustee may sign.
 */
export async function verifyBatch(args: {
  keyPackageHex: string;
  encSecretHex: string;
  sealedViewSecretHex: string;
  escrowUa: string;
  network: string;
  batch: unknown;
  refundTotalDueZat: number | null;
}): Promise<VerifiedBatch> {
  return JSON.parse(
    (await load()).verify_batch(
      args.keyPackageHex,
      args.encSecretHex,
      args.sealedViewSecretHex,
      args.escrowUa,
      args.network,
      Math.floor(Date.now() / 1000),
      JSON.stringify(args.batch),
      args.refundTotalDueZat ?? -1,
    ),
  ) as VerifiedBatch;
}
