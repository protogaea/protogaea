// The core in a worker, for the time machine: resumes a snapshot and steps it to an epoch without
// holding up the page. The module (`core.wasm`, built from `viewer/wasm`) has a plain C ABI.

interface Core {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  dealloc(ptr: number, len: number): void;
  load(ptr: number, len: number): bigint;
  step(n: number): bigint;
  map(): void;
  out_ptr(): number;
  out_len(): number;
}

export type Request =
  | { kind: 'load'; snapshot: ArrayBuffer; target: number }
  | { kind: 'step'; target: number };

export type Reply =
  | { kind: 'progress'; epoch: number }
  | { kind: 'done'; epoch: number; map: string }
  | { kind: 'error'; message: string };

let core: Core | undefined;
let epoch = -1;

async function instance(): Promise<Core> {
  if (!core) {
    const url = new URL(`${import.meta.env.BASE_URL}core.wasm`, self.location.origin);
    const bytes = await (await fetch(url)).arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    core = instance.exports as unknown as Core;
  }
  return core;
}

function output(c: Core): string {
  return new TextDecoder().decode(new Uint8Array(c.memory.buffer, c.out_ptr(), c.out_len()));
}

const post = (r: Reply) => (self as unknown as Worker).postMessage(r);

self.onmessage = async (ev: MessageEvent<Request>) => {
  try {
    const c = await instance();
    const req = ev.data;
    if (req.kind === 'load') {
      const bytes = new Uint8Array(req.snapshot);
      const ptr = c.alloc(bytes.length);
      new Uint8Array(c.memory.buffer, ptr, bytes.length).set(bytes);
      const at = Number(c.load(ptr, bytes.length));
      c.dealloc(ptr, bytes.length);
      if (at < 0) return post({ kind: 'error', message: output(c) });
      epoch = at;
    }
    if (epoch < 0) return post({ kind: 'error', message: 'no world loaded' });
    while (epoch < ev.data.target) {
      epoch = Number(c.step(1));
      post({ kind: 'progress', epoch });
    }
    c.map();
    post({ kind: 'done', epoch, map: output(c) });
  } catch (e) {
    post({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
  }
};
