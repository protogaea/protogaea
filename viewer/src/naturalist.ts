// The naturalist in the browser (stage C): a key kept in this browser, wishes made with it, and
// sparks kindled for them on several workers. Every receipt from the server is checked against
// the operator's key with the same core.

import type { Job, MinerReply, MinerRequest } from './miner.worker';

const KEY = 'protogaea.naturalist';

interface Core {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  dealloc(ptr: number, len: number): void;
  spark_api(ptr: number, len: number): number;
  out_ptr(): number;
  out_len(): number;
}

let core: Promise<Core> | undefined;

/** A spark client operation in the core of this page (keys, wishes, receipts). */
async function api(req: unknown): Promise<any> {
  core ??= (async () => {
    const res = await fetch(`${import.meta.env.BASE_URL}core.wasm`);
    const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {});
    return instance.exports as unknown as Core;
  })();
  const c = await core;
  const bytes = new TextEncoder().encode(JSON.stringify(req));
  const ptr = c.alloc(bytes.length);
  new Uint8Array(c.memory.buffer, ptr, bytes.length).set(bytes);
  const r = c.spark_api(ptr, bytes.length);
  c.dealloc(ptr, bytes.length);
  const text = new TextDecoder().decode(new Uint8Array(c.memory.buffer, c.out_ptr(), c.out_len()));
  if (r !== 0) throw new Error(text);
  return JSON.parse(text);
}

const toHex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
const fromHex = (h: string) => new Uint8Array(h.match(/../g)!.map((x) => parseInt(x, 16)));

function secret(): string {
  let s: string | null = null;
  try {
    s = localStorage.getItem(KEY);
  } catch {
    /* no storage: a key for this visit only */
  }
  if (!s || s.length !== 64) {
    s = toHex(crypto.getRandomValues(new Uint8Array(32)));
    try {
      localStorage.setItem(KEY, s);
    } catch {
      /* ignored */
    }
  }
  return s;
}

let publicKey: string | undefined;
export async function me(): Promise<string> {
  publicKey ??= (await api({ op: 'public', secret: secret() })).public as string;
  return publicKey;
}

async function getJson(path: string) {
  const res = await fetch(path);
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(body.error ? `${body.error} ${body.message ?? ''}`.trim() : `${res.status}`);
  return body;
}

async function postBody(path: string, body: BodyInit, type: string) {
  const res = await fetch(path, { method: 'POST', headers: { 'content-type': type }, body });
  const v = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(v.error ? `${v.error} ${v.message ?? ''}`.trim() : `${res.status}`);
  return v;
}

export type Action =
  | { weather: { x: number; y: number; rain: boolean } }
  | { migrate: { clade_id: number; from: [number, number]; to: [number, number] } }
  | { revive: { museum: boolean; entry_id: number; steps: [number, number][]; at: [number, number] } };

export interface Progress {
  proposal: string;
  accepted: number;
  work: bigint;
  rate: number;
  epoch: number;
  error?: string;
  /** A receipt that did not check against the operator's key stops everything. */
  forged?: boolean;
}

let workers: Worker[] = [];
let session: { proposal: string; started: number; hashes: number; accepted: number; work: bigint; job: Job; weight: bigint } | undefined;
let pending: { epoch: number; spark: string }[] = [];
let firstSpark: ((spark: string) => void) | undefined;
let timers: number[] = [];
let listener: (p: Progress) => void = () => {};

export function onProgress(f: (p: Progress) => void) {
  listener = f;
}

export function mining(): string | undefined {
  return session?.proposal;
}

async function windowJob(proposal: string): Promise<{ job: Job; weight: bigint; raw: any }> {
  const w = await getJson('/v0/window');
  if (!w.open) throw new Error('E_WINDOW_CLOSED');
  const job: Job = { world_id: w.world_id, epoch: w.epoch, challenge: w.challenge, target: w.target, proposal_id: proposal, miner: await me() };
  return { job, weight: BigInt(w.weight), raw: w };
}

export function threads(): number {
  return Math.max(1, Math.min(8, Math.floor((navigator.hardwareConcurrency || 2) / 2)));
}

function report(extra: Partial<Progress> = {}) {
  if (!session) return;
  const secs = (performance.now() - session.started) / 1000;
  listener({
    proposal: session.proposal,
    accepted: session.accepted,
    work: session.work,
    rate: secs > 0 ? session.hashes / secs : 0,
    epoch: session.job.epoch,
    ...extra,
  });
}

/** Starts kindling sparks for a wish on `threads()` workers. */
async function start(proposal: string, onFirst?: (spark: string) => void) {
  stop();
  firstSpark = onFirst;
  const { job, weight } = await windowJob(proposal);
  session = { proposal, started: performance.now(), hashes: 0, accepted: 0, work: 0n, job, weight };
  const base = BigInt('0x' + toHex(crypto.getRandomValues(new Uint8Array(6))));
  workers = Array.from({ length: threads() }, (_, i) => {
    const w = new Worker(new URL('./miner.worker.ts', import.meta.url), { type: 'module' });
    w.onmessage = (ev: MessageEvent<MinerReply>) => {
      const r = ev.data;
      if (!session) return;
      if (r.kind === 'error') return report({ error: r.message });
      session.hashes += r.hashes;
      for (const s of r.sparks) {
        if (firstSpark) {
          const f = firstSpark;
          firstSpark = undefined;
          f(s);
        } else if (r.epoch === session.job.epoch) pending.push({ epoch: r.epoch, spark: s });
      }
    };
    const req: MinerRequest = { kind: 'start', job, nonce: (base + (BigInt(i) << 40n)).toString() };
    w.postMessage(req);
    return w;
  });
  // Batches every 3 s; the window re-read every 5 s.
  timers.push(window.setInterval(() => void flush(), 3000));
  timers.push(window.setInterval(() => void refresh(), 5000));
}

async function refresh() {
  if (!session) return;
  try {
    const { job, weight } = await windowJob(session.proposal);
    if (job.epoch !== session.job.epoch || job.challenge !== session.job.challenge) {
      session.job = job;
      session.weight = weight;
      pending = pending.filter((p) => p.epoch === job.epoch);
      for (const w of workers) w.postMessage({ kind: 'window', job } satisfies MinerRequest);
    }
  } catch {
    /* a window between epochs: try again */
  }
}

async function flush() {
  if (!session || pending.length === 0) return report();
  const batch = pending.splice(0, 64).filter((p) => p.epoch === session!.job.epoch);
  if (batch.length === 0) return;
  const body = fromHex(batch.map((p) => p.spark).join(''));
  try {
    const [res, operator] = await Promise.all([postBody('/v0/sparks', body, 'application/octet-stream'), operatorKey()]);
    const results: any[] = res.results ?? [];
    for (let i = 0; i < batch.length; i++) {
      const r = results[i];
      if (r?.receipt) {
        const ok = (await api({ op: 'receipt', receipt: r.receipt, spark: batch[i].spark, operator })).ok;
        if (!ok) {
          stop();
          return listener({ proposal: '', accepted: 0, work: 0n, rate: 0, epoch: 0, forged: true });
        }
        session.accepted += 1;
        session.work += session.weight;
      } else if (r?.error === 'E_PROPOSAL_CLOSED') {
        report({ error: r.error });
        return stop();
      }
    }
    report();
  } catch (e) {
    report({ error: e instanceof Error ? e.message : String(e) });
  }
}

let operator: string | undefined;
async function operatorKey(): Promise<string> {
  operator ??= (await getJson('/v0/operator')).operator as string;
  return operator;
}

export function stop() {
  for (const w of workers) {
    w.postMessage({ kind: 'stop' } satisfies MinerRequest);
    w.terminate();
  }
  workers = [];
  for (const t of timers) clearInterval(t);
  timers = [];
  pending = [];
  firstSpark = undefined;
  session = undefined;
}

/** Supports an existing wish with sparks. */
export function support(proposal: string) {
  return start(proposal);
}

/**
 * Makes a wish: signs it, mines its first spark (a wish is created only with one), sends both,
 * then keeps kindling sparks for it. Returns the proposal id.
 */
export async function wish(action: Action): Promise<string> {
  const author = await me();
  const w = (await getJson('/v0/window')) as any;
  const made = await api({
    op: 'wish',
    secret: secret(),
    author,
    world_id: w.world_id,
    ruleset_id: w.ruleset_id,
    created_epoch: w.epoch,
    expires_epoch: w.epoch + 288,
    action,
  });
  const spark = await new Promise<string>((resolve, reject) => {
    start(made.id, resolve).catch(reject);
  });
  const res = await postBody('/v0/proposals', JSON.stringify({ wish: made.bytes, signature: made.signature, spark }), 'application/json');
  const ok = (await api({ op: 'receipt', receipt: res.receipt, spark, operator: await operatorKey() })).ok;
  if (!ok) {
    stop();
    throw new Error('a receipt that does not verify against the operator key');
  }
  if (session) {
    session.accepted += 1;
    session.work += session.weight;
  }
  return made.id as string;
}

export interface WishRow {
  id: string;
  action: string;
  status: string;
  work: string;
  reason: string | null;
  executed_epoch: number | null;
  price_mult: number;
  author: string;
  created_epoch: number;
}

/** The wishes made with this browser's key. */
export async function mine(): Promise<WishRow[]> {
  const [author, rows] = await Promise.all([me(), getJson('/v0/proposals?limit=1000')]);
  return (rows.proposals as WishRow[]).filter((r) => r.author === author);
}

/** Whether a signed epoch header checks against the operator's key (hash and signature). */
export async function checkHeader(header: unknown): Promise<boolean> {
  return (await api({ op: 'header', header, operator: await operatorKey() })).ok as boolean;
}

export async function ledger(): Promise<{ price: bigint; per_epoch: number }> {
  const l = await getJson('/v0/ledger');
  return { price: BigInt(l.price), per_epoch: l.per_epoch };
}
