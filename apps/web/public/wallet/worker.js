// The bidder's wallet (crates/sealforge-wallet, WebAssembly) in a Web Worker,
// so syncing and proving never block the page. The page sends compact blocks
// and tree states it fetched over gRPC-web; nothing here touches the network.
// The recovery phrase arrives in a message and stays in this worker's memory.

import init, * as wasm from "./sealforge_wallet.js";

let ready = null;
let handle = null;

function need() {
  if (!handle) throw new Error("No wallet is open.");
  return handle;
}

const ops = {
  async init() {
    ready ??= init({ module_or_path: new URL("./sealforge_wallet_bg.wasm", import.meta.url) });
    await ready;
    return true;
  },
  newPhrase() {
    return wasm.new_phrase(crypto.getRandomValues(new Uint8Array(32)));
  },
  phraseIsValid({ phrase }) {
    return wasm.phrase_is_valid(phrase);
  },
  create({ network, phrase, birthday, treeState }) {
    handle?.free();
    handle = wasm.WalletHandle.create(network, phrase, birthday, treeState);
    return handle.state_json();
  },
  open({ network, phrase, state }) {
    handle?.free();
    handle = wasm.WalletHandle.open(network, phrase, state);
    return true;
  },
  close() {
    handle?.free();
    handle = null;
    return true;
  },
  // Scan blocks in order. Stops at the first error and reports how far it got.
  scanBlocks({ blocks }) {
    const w = need();
    const outcomes = [];
    for (const block of blocks) {
      try {
        outcomes.push(JSON.parse(w.scan_block(block)));
      } catch (e) {
        return { outcomes, error: String(e?.message ?? e), height: w.scanned_height() };
      }
    }
    return { outcomes, error: null, height: w.scanned_height() };
  },
  reset() {
    need().reset();
    return need().scanned_height();
  },
  summary() {
    const w = need();
    return {
      state: w.state_json(),
      scannedHeight: w.scanned_height(),
      birthday: w.birthday(),
      balance: JSON.parse(w.balance()),
      notes: JSON.parse(w.notes()),
      sent: JSON.parse(w.sent()),
      fundingAddress: w.funding_address(),
    };
  },
  freshAddress() {
    return JSON.parse(need().fresh_address());
  },
  address({ index }) {
    return need().address(index);
  },
  quote({ amountZat }) {
    return Number(need().quote(BigInt(amountZat)));
  },
  prepareProver() {
    wasm.prepare_prover();
    return true;
  },
  pay({ request }) {
    return JSON.parse(need().pay(JSON.stringify(request)));
  },
  forgetPayment({ txid }) {
    need().forget_payment(txid);
    return true;
  },
};

self.onmessage = async (event) => {
  const { id, op, args } = event.data;
  try {
    const fn = ops[op];
    if (!fn) throw new Error(`unknown operation ${op}`);
    if (op !== "init") await ops.init();
    const result = await fn(args ?? {});
    self.postMessage({ id, ok: true, result });
  } catch (e) {
    self.postMessage({ id, ok: false, error: String(e?.message ?? e) });
  }
};
