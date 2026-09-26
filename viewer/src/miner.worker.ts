// A spark miner in a worker: its own copy of the core (8 MiB of yespower memory), trying nonces
// in small slices so that it stays responsive to a new window or a stop.

interface Core {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  dealloc(ptr: number, len: number): void;
  spark_api(ptr: number, len: number): number;
  out_ptr(): number;
  out_len(): number;
}

export interface Job {
  world_id: string;
  epoch: number;
  challenge: string;
  target: string;
  proposal_id: string;
  miner: string;
}

export type MinerRequest = { kind: 'start'; job: Job; nonce: string } | { kind: 'window'; job: Job } | { kind: 'stop' };
export type MinerReply = { kind: 'found'; epoch: number; sparks: string[]; hashes: number } | { kind: 'error'; message: string };

let core: Core | undefined;
let job: Job | undefined;
let nonce = '0';
let running = false;

async function instance(): Promise<Core> {
  if (!core) {
    const url = new URL(`${import.meta.env.BASE_URL}core.wasm`, self.location.origin);
    const { instance } = await WebAssembly.instantiate(await (await fetch(url)).arrayBuffer(), {});
    core = instance.exports as unknown as Core;
  }
  return core;
}

function call(c: Core, req: unknown): { ok: boolean; value: any } {
  const bytes = new TextEncoder().encode(JSON.stringify(req));
  const ptr = c.alloc(bytes.length);
  new Uint8Array(c.memory.buffer, ptr, bytes.length).set(bytes);
  const r = c.spark_api(ptr, bytes.length);
  c.dealloc(ptr, bytes.length);
  const text = new TextDecoder().decode(new Uint8Array(c.memory.buffer, c.out_ptr(), c.out_len()));
  return r === 0 ? { ok: true, value: JSON.parse(text) } : { ok: false, value: text };
}

const post = (r: MinerReply) => (self as unknown as Worker).postMessage(r);
/** Nonces per slice: a few hashes, a few dozen milliseconds. */
const SLICE = 6;

async function loop() {
  const c = await instance();
  while (running && job) {
    const j = job;
    const r = call(c, { op: 'mine', ...j, nonce, count: SLICE });
    if (!r.ok) {
      post({ kind: 'error', message: r.value });
      running = false;
      return;
    }
    nonce = r.value.next;
    post({ kind: 'found', epoch: j.epoch, sparks: r.value.found, hashes: SLICE });
    // Let messages in: a new window or a stop.
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

self.onmessage = (ev: MessageEvent<MinerRequest>) => {
  const m = ev.data;
  if (m.kind === 'start') {
    job = m.job;
    nonce = m.nonce;
    if (!running) {
      running = true;
      loop();
    }
  } else if (m.kind === 'window') {
    job = m.job;
  } else {
    running = false;
  }
};
