// Predictions for tomorrow (a first form of the hypothesis journal of spec §7): the viewer bets
// whether a clade will still be alive, or larger, one world day later, and the answer is checked
// the next day. Kept in this browser only; there are no accounts yet.

import { api } from './api';

export type Question = 'survive' | 'grow';

export interface Prediction {
  clade: number;
  question: Question;
  answer: boolean;
  made: number;
  due: number;
  /** The clade's size when the prediction was made, for "grow". */
  living: number;
  correct?: boolean;
  /** The size or fate that decided it. */
  outcome?: number | 'extinct';
}

const KEY = 'protogaea.predictions';

export function load(): Prediction[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '[]');
  } catch {
    return [];
  }
}

function save(list: Prediction[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list.slice(-200)));
  } catch {
    /* storage can be blocked; predictions then last for this visit only */
  }
}

export function pending(list: Prediction[], clade: number, question: Question): Prediction | undefined {
  return list.find((p) => p.clade === clade && p.question === question && p.correct === undefined);
}

export function make(clade: number, question: Question, answer: boolean, epoch: number, epochsPerDay: number, living: number) {
  const list = load();
  if (pending(list, clade, question)) return;
  list.push({ clade, question, answer, made: epoch, due: epoch + epochsPerDay, living });
  save(list);
}

/** Checks the predictions that are due against the clade's history. Returns how many were settled. */
export async function resolve(now: number): Promise<number> {
  const list = load();
  let settled = 0;
  for (const p of list) {
    if (p.correct !== undefined || now < p.due) continue;
    try {
      const c = await api.clade(p.clade);
      const extinct = c.extinct_epoch !== null && c.extinct_epoch <= p.due;
      // The clade's size at the due time, from its hourly history.
      let size = 0;
      for (const [epoch, living] of c.history ?? []) if (epoch <= p.due) size = living;
      if (p.question === 'survive') {
        p.correct = p.answer === !extinct;
        p.outcome = extinct ? 'extinct' : size;
      } else {
        const grew = !extinct && size > p.living;
        p.correct = p.answer === grew;
        p.outcome = extinct ? 'extinct' : size;
      }
      settled++;
    } catch {
      /* try again later */
    }
  }
  if (settled) save(list);
  return settled;
}
