// Arbeiter reden über die PCs der Spieler hinweg (wie, steht in gespraech.ts): list_workers zeigt die
// ganze Etage der Lobby, die Arbeiter des Gastgebers und die auf den PCs der Spieler, und tell_worker
// erreicht jeden davon. Beim Gastgeber geht eine Nachricht an einen Arbeiter von außen an dessen
// Arbeitsplatz; auf einem Arbeitsplatz fragt das Office den Gastgeber (`wer`, `sagen`), der weiterreicht.
import type { WorkerInfo } from '../../shared/protocol.js';
import type { Floor } from '../floor.js';
import { mitAbsender, zeigen } from '../gespraech.js';
import { findWorker, workerRow, type PullsView, type WorkerRow } from '../office-workers.js';
import type { Ctx } from '../office/context.js';
import { fernListe, type FernArbeiter } from './gastgeber.js';
import type { VomArbeitsplatz, ZumArbeitsplatz } from './protokoll.js';
import { satellitVon } from './satellit.js';

type Ich = Pick<WorkerInfo, 'id' | 'name' | 'color'>;
type Gesagt = { worker?: { id: string; name: string }; error?: string };
/** Wo die Arbeiter des Gastgebers laufen, für die, die von außen fragen. */
const GASTGEBER = 'Gastgeber';

const pullsView = (ctx: Ctx, floor: Floor): PullsView => ({ pulls: floor.github.pulls.items, tasks: floor.queue.state().tasks, pullsOf: (id) => ctx.floors.get(id)?.github.pulls.items });

/** Für list_workers: wer sonst auf dieser Etage der Lobby sitzt und nicht in diesem Office läuft. */
export async function andere(ctx: Ctx, floor: Floor, me: Ich): Promise<WorkerRow[]> {
  const sat = satellitVon(ctx);
  if (sat) {
    const dort = sat.dort(floor.id);
    const a = dort ? await sat.fragen({ t: 'wer', id: 0, etage: dort }) : undefined;
    return a?.t === 'die' && Array.isArray(a.arbeiter) ? a.arbeiter : [];
  }
  const view = pullsView(ctx, floor);
  return fernListe(ctx, floor).map((f) => ({ ...workerRow(f.info, view, me.id), pc: f.pc }));
}

/** tell_worker an einen, der nicht in diesem Office läuft. undefined: Auf der Etage heißt keiner so. */
export async function fernSagen(ctx: Ctx, floor: Floor, me: Ich, an: string, text: string): Promise<Gesagt | undefined> {
  const sat = satellitVon(ctx);
  if (sat) {
    const dort = sat.dort(floor.id);
    if (!dort) return undefined;
    const a = await sat.fragen({ t: 'sagen', id: 0, etage: dort, von: { id: me.id, name: me.name, color: me.color }, an, text });
    if (a?.t !== 'gesagt') return { error: 'Der Gastgeber der Lobby antwortet gerade nicht: Versuch es gleich noch einmal' };
    return a.fehlt ? undefined : { worker: a.worker, error: a.error };
  }
  const fern = fernListe(ctx, floor);
  const w = findWorker(
    fern.map((f) => f.info),
    an,
  );
  if (typeof w === 'string') return undefined;
  return zuFern(ctx, floor, fern.find((f) => f.info === w)!, me, text);
}

/** Beim Gastgeber: einem Arbeiter von außen etwas sagen, über seinen Arbeitsplatz. `vonPc`: wo der Absender läuft. */
function zuFern(ctx: Ctx, floor: Floor, f: FernArbeiter, von: Ich, text: string, vonPc?: string): Gesagt {
  if (!f.schicken) return { error: `${f.info.name} macht Pause: Der PC von ${f.pc} ist gerade aus` };
  if (f.info.kind !== 'agent') return { error: `${f.info.name} is a shell, not an agent` };
  if (f.info.status === 'offline' || f.info.status === 'exited') return { error: `${f.info.name} läuft gerade nicht (auf dem PC von ${f.pc}): Wer dort ist, weckt ihn mit R an seinem Platz` };
  f.schicken({ t: 'worker.prompt', workerId: f.info.id, prompt: mitAbsender(von, text, vonPc) }, { id: `arbeiter-${von.id}`, name: von.name });
  zeigen(ctx, floor, von, f.info, text);
  return { worker: { id: f.info.id, name: f.info.name } };
}

/** Wer beim Gastgeber fragt: ein Arbeitsplatz, mit seinem Konto und den Arbeitern, die bei ihm laufen. */
export interface Frager {
  account: string;
  name: string;
  /** Einer seiner Arbeiter, nach der ID (nur seine eigenen). */
  arbeiter(id: string): WorkerInfo | undefined;
  antworten(a: ZumArbeitsplatz): void;
}

/** Beim Gastgeber: was ein Arbeitsplatz für seine Arbeiter wissen (`wer`) oder sagen (`sagen`) will. */
export function beantworten(ctx: Ctx, floor: Floor, frager: Frager, m: Extract<VomArbeitsplatz, { t: 'wer' } | { t: 'sagen' }>) {
  const id = Number(m.id);
  const hier = floor.workers.list();
  // Seine eigenen kennt er selbst.
  const fern = fernListe(ctx, floor).filter((f) => f.account !== frager.account);
  if (m.t === 'wer') {
    const view = pullsView(ctx, floor);
    return frager.antworten({ t: 'die', id, arbeiter: [...hier.map((w) => ({ ...workerRow(w, view), pc: GASTGEBER })), ...fern.map((f) => ({ ...workerRow(f.info, view), pc: f.pc }))] });
  }
  const antwort = (g: Gesagt & { fehlt?: true }) => frager.antworten({ t: 'gesagt', id, ...g });
  // Der Absender ist einer seiner Arbeiter, so wie der Gastgeber ihn kennt: Kein Arbeitsplatz spricht für einen anderen.
  const von = frager.arbeiter(String(m.von?.id ?? ''));
  const text = String(m.text ?? '')
    .slice(0, 20000)
    .replace(/\r\n?/g, '\n')
    .trim();
  if (!von) return antwort({ error: 'Diesen Arbeiter kennt die Lobby nicht' });
  if (!text) return antwort({ error: 'Say what to tell it: prompt' });
  const w = findWorker([...hier, ...fern.map((f) => f.info)], String(m.an ?? '').slice(0, 64));
  if (typeof w === 'string') return antwort({ error: w, ...(/^No worker here/.test(w) ? { fehlt: true as const } : {}) });
  const f = fern.find((x) => x.info === w);
  if (f) return antwort(zuFern(ctx, floor, f, von, text, frager.name));
  if (w.kind !== 'agent') return antwort({ error: `${w.name} is a shell, not an agent` });
  const prompt = mitAbsender(von, text, frager.name);
  let err = floor.workers.prompt(w.id, prompt, von.name);
  // Gestoppt oder eingeschlafen: Er wacht mit der Nachricht auf.
  if (err === 'Worker is not running') err = floor.workers.resume(w.id, prompt);
  if (err) return antwort({ error: err });
  zeigen(ctx, floor, von, w, text);
  antwort({ worker: { id: w.id, name: w.name } });
}
