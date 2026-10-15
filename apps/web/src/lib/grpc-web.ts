"use client";

/**
 * Minimal gRPC-web client for the two lightwalletd calls the browser audit
 * needs (task 3.5): GetBlockRange (server stream of CompactBlock) and
 * GetTransaction. Messages are tiny, so they're encoded and decoded by hand;
 * CompactBlock bytes go straight to the WebAssembly scanner, which decodes them.
 */

const SERVICE = "cash.z.wallet.sdk.rpc.CompactTxStreamer";

function varint(n: bigint): number[] {
  const out: number[] = [];
  let v = n;
  do {
    let byte = Number(v & 0x7fn);
    v >>= 7n;
    if (v > 0n) byte |= 0x80;
    out.push(byte);
  } while (v > 0n);
  return out;
}

function field(num: number, wire: number): number[] {
  return varint(BigInt((num << 3) | wire));
}

function bytesField(num: number, data: Uint8Array | number[]): number[] {
  return [...field(num, 2), ...varint(BigInt(data.length)), ...data];
}

/** BlockId { height = 1 } */
function blockId(height: number): number[] {
  return [...field(1, 0), ...varint(BigInt(height))];
}

function frame(message: number[]): Uint8Array {
  const out = new Uint8Array(5 + message.length);
  new DataView(out.buffer).setUint32(1, message.length);
  out.set(message, 5);
  return out;
}

export class GrpcError extends Error {}

async function call(endpoint: string, method: string, message: number[]): Promise<Response> {
  let res: Response;
  try {
    res = await fetch(`${endpoint.replace(/\/$/, "")}/${SERVICE}/${method}`, {
      method: "POST",
      headers: { "content-type": "application/grpc-web+proto", "x-grpc-web": "1" },
      body: frame(message) as BodyInit,
    });
  } catch {
    throw new GrpcError(`Can't reach ${endpoint}. Is it a gRPC-web lightwallet endpoint?`);
  }
  if (!res.ok || !res.body) throw new GrpcError(`${endpoint} answered HTTP ${res.status}.`);
  return res;
}

/** Read gRPC-web frames as they arrive; data frames go to `onMessage`. */
async function readFrames(res: Response, onMessage: (m: Uint8Array) => void): Promise<void> {
  const reader = (res.body as ReadableStream<Uint8Array>).getReader();
  let buf = new Uint8Array(0);
  for (;;) {
    const { value, done } = await reader.read();
    if (value) {
      const merged = new Uint8Array(buf.length + value.length);
      merged.set(buf);
      merged.set(value, buf.length);
      buf = merged;
    }
    while (buf.length >= 5) {
      const len = new DataView(buf.buffer, buf.byteOffset).getUint32(1);
      if (buf.length < 5 + len) break;
      const flag = buf[0] ?? 0;
      const payload = buf.slice(5, 5 + len);
      buf = buf.slice(5 + len);
      if (flag & 0x80) {
        const trailers = new TextDecoder().decode(payload);
        const status = /grpc-status:\s*(\d+)/i.exec(trailers)?.[1];
        if (status && status !== "0") {
          const msg = /grpc-message:\s*(.*)/i.exec(trailers)?.[1] ?? "";
          throw new GrpcError(`Lightwallet server error ${status}: ${decodeURIComponent(msg)}`);
        }
      } else {
        onMessage(payload);
      }
    }
    if (done) return;
  }
}

/** Stream compact blocks `from..=to` (raw CompactBlock protobuf bytes). */
export async function getBlockRange(
  endpoint: string,
  from: number,
  to: number,
  onBlock: (block: Uint8Array) => void,
): Promise<void> {
  const msg = [...bytesField(1, blockId(from)), ...bytesField(2, blockId(to))];
  await readFrames(await call(endpoint, "GetBlockRange", msg), onBlock);
}

/** Field 1 (`data`) of a RawTransaction message. */
function rawTransactionData(m: Uint8Array): Uint8Array {
  let i = 0;
  const readVarint = (): bigint => {
    let shift = 0n;
    let out = 0n;
    for (;;) {
      const b = m[i++] ?? 0;
      out |= BigInt(b & 0x7f) << shift;
      if (!(b & 0x80)) return out;
      shift += 7n;
    }
  };
  while (i < m.length) {
    const key = Number(readVarint());
    const wire = key & 7;
    if (wire === 2) {
      const len = Number(readVarint());
      if (key >> 3 === 1) return m.slice(i, i + len);
      i += len;
    } else if (wire === 0) {
      readVarint();
    } else {
      throw new GrpcError("Unexpected RawTransaction encoding.");
    }
  }
  throw new GrpcError("Transaction not found on that server.");
}

/** A transaction by txid (internal byte order, hex). Returns the raw bytes. */
export async function getTransaction(endpoint: string, txidHex: string): Promise<Uint8Array> {
  const hash = txidHex.match(/../g)?.map((h) => Number.parseInt(h, 16)) ?? [];
  let data: Uint8Array | null = null;
  await readFrames(await call(endpoint, "GetTransaction", bytesField(3, hash)), (m) => {
    data = rawTransactionData(m);
  });
  if (!data) throw new GrpcError("Transaction not found on that server.");
  return data;
}
