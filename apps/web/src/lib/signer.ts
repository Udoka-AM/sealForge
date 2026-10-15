"use client";

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
