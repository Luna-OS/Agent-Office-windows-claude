// Beim Gastgeber: die Arbeitsplätze der Spieler (siehe protokoll.ts). Wer einen angemeldet hat, dessen
// Arbeiter stellt der Gastgeber dort ein, auf dem Computer des Spielers; er zeigt sie auf seinen Etagen
// wie seine eigenen und reicht weiter, was Browser ihnen schicken und was sie zurückgeben. Ist ein
// Arbeitsplatz weg, machen seine Arbeiter an ihren Plätzen Pause, bis er wieder da ist (pause.ts).
import path from 'node:path';
import { WebSocket } from 'ws';
import type { ClientMsg, WorkerInfo } from '../../shared/protocol.js';
import type { Account } from '../accounts.js';
import type { Floor } from '../floor.js';
import type { Client } from '../office/client.js';
import type { Ctx } from '../office/context.js';
import type { FeatureHooks } from '../ws/handlers/types.js';
import { Pausen } from './pause.js';
import { FUER_DIE_ETAGE, FUER_EINEN, type FernEtage, type VomArbeitsplatz, type ZumArbeitsplatz } from './protokoll.js';

/** Alle so oft ein Ping an jeden Arbeitsplatz; kommt bis zum nächsten keine Antwort, ist sein PC weg. */
const HERZ_MS = 20_000;

interface Platz {
  account: string;
  name: string;
  ws: WebSocket;
  /** Seine Arbeiter: auf welcher Etage des Gastgebers, und wie sie zuletzt aussahen. */
  arbeiter: Map<string, { etage: string; info: WorkerInfo }>;
  /** Die Browser, für die er etwas tut: Gehen sie, erfährt er es (sie lassen ihre Terminals los). */
  browser: Set<string>;
}

interface Stand {
  /** Nach Konto: höchstens ein Arbeitsplatz je Konto, der neueste. */
  plaetze: Map<string, Platz>;
  /** Wo jeder Arbeiter von außen sitzt, nach seiner ID. */
  fern: Map<string, Platz>;
  /** Die Arbeiter der Arbeitsplätze, die gerade weg sind. */
  pausen: Pausen;
}

const staende = new WeakMap<object, Stand>();
function stand(ctx: Ctx): Stand {
  let s = staende.get(ctx);
  if (!s) {
    const datei = ctx.cfg?.dataDir ? path.join(ctx.cfg.dataDir, 'arbeitsplatz-pausen.json') : undefined;
    staende.set(ctx, (s = { plaetze: new Map(), fern: new Map(), pausen: new Pausen(datei) }));
  }
  return s;
}
/** Der Stand, wenn es einen gibt oder einer gemerkt ist (ein Office mit Daten), sonst keiner. */
const standDa = (ctx: Ctx): Stand | undefined => staende.get(ctx) ?? (ctx.cfg?.dataDir ? stand(ctx) : undefined);

/** Eine Etage, wie ein Arbeitsplatz sie findet: nur mit GitHub-Repo, sonst kann er sie nicht klonen. */
export const fernEtage = (floor: Floor): FernEtage | undefined => (floor.def.repo ? { id: floor.id, name: floor.def.name, repo: floor.def.repo, wing: floor.plan.wing } : undefined);

function senden(p: Platz, m: ZumArbeitsplatz) {
  if (p.ws.readyState === WebSocket.OPEN) p.ws.send(JSON.stringify(m));
}

/** Ein Arbeitsplatz hat sich angemeldet (ws/upgrade.ts), mit dem Konto `account`. */
export function arbeitsplatzAnnehmen(ctx: Ctx, ws: WebSocket, account: Account) {
  const s = stand(ctx);
  const p: Platz = { account: account.id, name: account.name, ws, arbeiter: new Map(), browser: new Set() };
  // Derselbe Spieler noch einmal (seine Verbindung war kurz weg): Seine Arbeiter gehören gleich dem neuen.
  const alt = s.plaetze.get(account.id);
  if (alt) {
    for (const [wid, a] of alt.arbeiter) (p.arbeiter.set(wid, a), s.fern.set(wid, p));
    alt.arbeiter.clear();
    alt.ws.close(4000, 'Ein neuerer Arbeitsplatz hat sich angemeldet');
  }
  s.plaetze.set(account.id, p);
  senden(p, { t: 'etagen', etagen: [...ctx.floors.values()].map(fernEtage).filter((e): e is FernEtage => !!e) });
  console.log(`  arbeitsplatz: ${account.name} ist da: Arbeiter von ${account.name} laufen ab jetzt auf dem eigenen Computer`);
  ws.on('message', (d: Buffer, binaer: boolean) => {
    if (binaer) return;
    let m: VomArbeitsplatz;
    try {
      m = JSON.parse(d.toString('utf8'));
    } catch {
      return;
    }
    vomPlatz(ctx, p, m);
  });
  // Geht sein PC einfach aus (Strom, Netz weg), sagt das keiner: Ohne Antwort auf den Ping ist er weg.
  let lebt = true;
  ws.on('pong', () => (lebt = true));
  const herz = setInterval(() => {
    if (!lebt) return ws.terminate();
    lebt = false;
    ws.ping();
  }, HERZ_MS);
  herz.unref();
  ws.on('close', () => {
    clearInterval(herz);
    abmelden(ctx, p);
  });
  ws.on('error', () => ws.close());
}

/** Was ein Arbeitsplatz schickt: für eine ganze Etage, für einen Browser, oder wer bei ihm sitzt. */
function vomPlatz(ctx: Ctx, p: Platz, m: VomArbeitsplatz) {
  const s = stand(ctx);
  if (m?.t === 'etage') {
    const floor = ctx.floors.get(String(m.etage));
    const msg = m.msg;
    if (!floor || !msg || !FUER_DIE_ETAGE.has(msg.t)) return;
    if (msg.t === 'worker.update') {
      const info = msg.worker;
      if (!info || typeof info.id !== 'string') return;
      // Ein Arbeiter des Gastgebers, oder einer eines anderen Arbeitsplatzes, bleibt, wie er ist.
      const da = s.fern.get(info.id);
      const ruht = s.pausen.get(info.id);
      if (floor.workers.get(info.id) || (da && da !== p) || (ruht && ruht.account !== p.account)) return;
      // Aus der Pause zurück.
      if (ruht && s.pausen.delete(info.id) && ruht.etage !== floor.id) aufEtage(ctx, ruht.etage, { t: 'worker.remove', workerId: info.id });
      const wach = { ...info, pause: undefined };
      p.arbeiter.set(info.id, { etage: floor.id, info: wach });
      s.fern.set(info.id, p);
      return ctx.toFloor(floor, { t: 'worker.update', worker: wach });
    } else if (msg.t === 'worker.remove' || msg.t === 'screen') {
      if (s.fern.get(msg.workerId) !== p) return;
      if (msg.t === 'screen') return ctx.toFloor(floor, msg, true);
      p.arbeiter.delete(msg.workerId);
      s.fern.delete(msg.workerId);
    }
    ctx.toFloor(floor, msg);
  } else if (m?.t === 'an') {
    const c = ctx.clients.get(String(m.an));
    const msg = m.msg;
    if (!c || !msg || !FUER_EINEN.has(msg.t)) return;
    if ('workerId' in msg && s.fern.get(msg.workerId) !== p) return;
    ctx.sendTo(c, msg);
  } else if (m?.t === 'bestand' && Array.isArray(m.arbeiter)) {
    const da = new Set(m.arbeiter.map(String));
    // Was bei ihm nicht mehr ist (dort nach Hause geschickt, solange er nicht hier war), macht auch keine Pause mehr.
    for (const r of s.pausen.ofAccount(p.account)) if (!da.has(r.info.id)) pauseVorbei(ctx, r.info.id);
    // Was er von einem anderen PC desselben Spielers übernommen hat, aber nicht hat: Das läuft dort, also Pause.
    for (const [wid, a] of p.arbeiter) {
      if (da.has(wid)) continue;
      p.arbeiter.delete(wid);
      s.fern.delete(wid);
      aufEtage(ctx, a.etage, { t: 'worker.update', worker: s.pausen.add(p.account, p.name, a.etage, a.info) });
    }
  }
}

function aufEtage(ctx: Ctx, etage: string, msg: { t: 'worker.remove'; workerId: string } | { t: 'worker.update'; worker: WorkerInfo }) {
  const floor = ctx.floors.get(etage);
  if (floor) ctx.toFloor(floor, msg);
}

/** Ein Arbeiter in der Pause geht aus der Lobby (auf dem PC seines Spielers ist er, wie er ist). */
function pauseVorbei(ctx: Ctx, wid: string) {
  const r = stand(ctx).pausen.delete(wid);
  if (r) aufEtage(ctx, r.etage, { t: 'worker.remove', workerId: wid });
}

/** Der Arbeitsplatz ist weg: Seine Arbeiter machen an ihren Plätzen Pause, bis er wieder da ist. */
function abmelden(ctx: Ctx, p: Platz) {
  const s = stand(ctx);
  if (s.plaetze.get(p.account) === p) s.plaetze.delete(p.account);
  for (const [wid, a] of p.arbeiter) {
    // Ein neuerer Arbeitsplatz desselben Spielers hat ihn schon übernommen.
    if (s.fern.get(wid) !== p) continue;
    s.fern.delete(wid);
    if (ctx.floors.get(a.etage)) aufEtage(ctx, a.etage, { t: 'worker.update', worker: s.pausen.add(p.account, p.name, a.etage, a.info) });
  }
  p.arbeiter.clear();
  console.log(`  arbeitsplatz: Der Arbeitsplatz von ${p.name} ist weg; die Arbeiter dort machen Pause, bis er wieder da ist`);
}

function weitergeben(p: Platz, etage: FernEtage, c: Client, msg: ClientMsg) {
  p.browser.add(c.id);
  senden(p, { t: 'nachricht', etage, wer: { id: c.id, name: c.peer.name }, msg });
}

/** Wer an `desk` auf `floor` sitzt, von einem Arbeitsplatz (auch in der Pause): sein Name und von wem. */
function besetzt(s: Stand, floor: Floor, desk: string): string | undefined {
  for (const p of s.plaetze.values()) for (const a of p.arbeiter.values()) if (a.etage === floor.id && a.info.deskId === desk) return `${a.info.name} (von ${p.name})`;
  for (const r of s.pausen.all()) if (r.etage === floor.id && r.info.deskId === desk) return `${r.info.name} (von ${r.name}, gerade in der Pause)`;
  return undefined;
}

/** Was ein Browser von sich aus schickt (ein Fenster geht zu, ändert seine Größe): dafür sagt einer in der Pause nichts. */
const NEBENBEI: ReadonlySet<string> = new Set(['term.resize', 'term.typing', 'worker.detach', 'changes.unwatch']);
/** Wem ein Arbeiter in der Pause zuletzt gesagt hat, warum er nichts tut (Browser|Arbeiter → wann), damit nicht jede Taste es tut. */
const gesagt = new Map<string, number>();
const SAGEN_MS = 5000;

/** Für einen Arbeiter in der Pause: Er sagt, warum er nichts tut; nach Hause geschickt geht er aus der Lobby. */
function inDerPause(ctx: Ctx, c: Client, wid: string, msg: ClientMsg): boolean {
  const r = stand(ctx).pausen.get(wid);
  if (!r) return false;
  if (msg.t === 'worker.kill') {
    pauseVorbei(ctx, wid);
    ctx.toastFloor(ctx.floors.get(r.etage), `🏠 ${r.info.name} geht aus der Pause nach Hause. Auf dem PC von ${r.name} ist er wieder da, wenn der an ist.`, 'info');
    return true;
  }
  const key = `${c.id}|${wid}`;
  const now = Date.now();
  if (NEBENBEI.has(msg.t) || now - (gesagt.get(key) ?? 0) < SAGEN_MS) return true;
  for (const [k, at] of gesagt) if (now - at >= SAGEN_MS) gesagt.delete(k);
  gesagt.set(key, now);
  ctx.sendTo(c, { t: 'toast', text: `☕ ${r.info.name} macht Pause: Der PC von ${r.name} ist gerade aus. Ist er wieder an, geht es weiter.`, level: 'info' });
  return true;
}

/**
 * Vor den Handlern (ws/dispatch.ts): Nachrichten für einen Arbeiter von außen gehen zu seinem
 * Arbeitsplatz, und wer einen eigenen hat, dessen neuer Arbeiter wird dort eingestellt. true: erledigt.
 */
export function fernAbfangen(ctx: Ctx, c: Client, msg: ClientMsg): boolean {
  const s = standDa(ctx);
  if (!s || (!s.plaetze.size && !s.fern.size && !s.pausen.size)) return false;
  if (msg.t === 'worker.spawn') {
    const floor = ctx.floorOf(c);
    if (!floor) return false;
    const sitzt = besetzt(s, floor, String(msg.deskId ?? ''));
    if (sitzt) {
      ctx.warn(c, `An diesem Platz sitzt schon ${sitzt}`);
      return true;
    }
    const p = c.accountId ? s.plaetze.get(c.accountId) : undefined;
    const etage = fernEtage(floor);
    // Ohne Arbeitsplatz, auf einer Etage ohne GitHub-Repo, oder über mehrere Repos: beim Gastgeber.
    if (!p || !etage || p.ws.readyState !== WebSocket.OPEN || msg.repos?.length) return false;
    weitergeben(p, etage, c, msg);
    return true;
  }
  const wid = 'workerId' in msg && typeof msg.workerId === 'string' ? msg.workerId : undefined;
  if (wid && inDerPause(ctx, c, wid, msg)) return true;
  const p = wid ? s.fern.get(wid) : undefined;
  const a = wid && p?.arbeiter.get(wid);
  const floor = a ? ctx.floors.get(a.etage) : undefined;
  const etage = floor && fernEtage(floor);
  if (!p || !etage) return false;
  weitergeben(p, etage, c, msg);
  return true;
}

/** Die Arbeiter von außen auf `floor`, auch die in der Pause, für das, was jemand beim Ankommen sieht (FloorView.workers). */
export function fernArbeiter(ctx: Ctx, floor: Floor | undefined): WorkerInfo[] {
  const s = standDa(ctx);
  if (!s || !floor) return [];
  const out: WorkerInfo[] = [];
  for (const p of s.plaetze.values()) for (const a of p.arbeiter.values()) if (a.etage === floor.id) out.push(a.info);
  for (const r of s.pausen.all()) if (r.etage === floor.id) out.push(r.info);
  return out;
}

/** Geht ein Browser (oder auf eine andere Etage), lassen seine Terminals bei den Arbeitsplätzen los. */
const loslassen = (ctx: Ctx, c: Client) => {
  const s = staende.get(ctx);
  if (!s) return;
  for (const p of s.plaetze.values()) if (p.browser.delete(c.id)) senden(p, { t: 'weg', id: c.id });
};
export const arbeitsplatzHooks: FeatureHooks = {
  leaving: (ctx, c) => loslassen(ctx, c),
  closed: (ctx, c) => loslassen(ctx, c),
};
