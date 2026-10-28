// Beim Gastgeber: die Arbeitsplätze der Spieler (siehe protokoll.ts). Wer einen angemeldet hat, dessen
// Arbeiter stellt der Gastgeber dort ein, auf dem Computer des Spielers; er zeigt sie auf seinen Etagen
// wie seine eigenen und reicht weiter, was Browser ihnen schicken und was sie zurückgeben.
import { WebSocket } from 'ws';
import type { ClientMsg, WorkerInfo } from '../../shared/protocol.js';
import type { Account } from '../accounts.js';
import type { Floor } from '../floor.js';
import type { Client } from '../office/client.js';
import type { Ctx } from '../office/context.js';
import type { FeatureHooks } from '../ws/handlers/types.js';
import { FUER_DIE_ETAGE, FUER_EINEN, type FernEtage, type VomArbeitsplatz, type ZumArbeitsplatz } from './protokoll.js';

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
}

const staende = new WeakMap<object, Stand>();
function stand(ctx: Ctx): Stand {
  let s = staende.get(ctx);
  if (!s) staende.set(ctx, (s = { plaetze: new Map(), fern: new Map() }));
  return s;
}

/** Eine Etage, wie ein Arbeitsplatz sie findet: nur mit GitHub-Repo, sonst kann er sie nicht klonen. */
export const fernEtage = (floor: Floor): FernEtage | undefined => (floor.def.repo ? { id: floor.id, name: floor.def.name, repo: floor.def.repo, wing: floor.plan.wing } : undefined);

function senden(p: Platz, m: ZumArbeitsplatz) {
  if (p.ws.readyState === WebSocket.OPEN) p.ws.send(JSON.stringify(m));
}

/** Ein Arbeitsplatz hat sich angemeldet (ws/upgrade.ts), mit dem Konto `account`. */
export function arbeitsplatzAnnehmen(ctx: Ctx, ws: WebSocket, account: Account) {
  const s = stand(ctx);
  s.plaetze.get(account.id)?.ws.close(4000, 'Ein neuerer Arbeitsplatz hat sich angemeldet');
  const p: Platz = { account: account.id, name: account.name, ws, arbeiter: new Map(), browser: new Set() };
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
  ws.on('close', () => abmelden(ctx, p));
  ws.on('error', () => ws.close());
}

/** Was ein Arbeitsplatz schickt: für eine ganze Etage, oder für einen Browser. */
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
      if (floor.workers.get(info.id) || (da && da !== p)) return;
      p.arbeiter.set(info.id, { etage: floor.id, info });
      s.fern.set(info.id, p);
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
  }
}

/** Der Arbeitsplatz ist weg: Seine Arbeiter verschwinden von den Etagen (auf seinem Computer laufen sie weiter). */
function abmelden(ctx: Ctx, p: Platz) {
  const s = stand(ctx);
  if (s.plaetze.get(p.account) === p) s.plaetze.delete(p.account);
  for (const [wid, a] of p.arbeiter) {
    s.fern.delete(wid);
    const floor = ctx.floors.get(a.etage);
    if (floor) ctx.toFloor(floor, { t: 'worker.remove', workerId: wid });
  }
  p.arbeiter.clear();
  console.log(`  arbeitsplatz: Der Arbeitsplatz von ${p.name} ist weg`);
}

function weitergeben(p: Platz, etage: FernEtage, c: Client, msg: ClientMsg) {
  p.browser.add(c.id);
  senden(p, { t: 'nachricht', etage, wer: { id: c.id, name: c.peer.name }, msg });
}

/**
 * Vor den Handlern (ws/dispatch.ts): Nachrichten für einen Arbeiter von außen gehen zu seinem
 * Arbeitsplatz, und wer einen eigenen hat, dessen neuer Arbeiter wird dort eingestellt. true: erledigt.
 */
export function fernAbfangen(ctx: Ctx, c: Client, msg: ClientMsg): boolean {
  const s = staende.get(ctx);
  if (!s || (!s.plaetze.size && !s.fern.size)) return false;
  if (msg.t === 'worker.spawn') {
    const floor = ctx.floorOf(c);
    if (!floor) return false;
    const desk = String(msg.deskId ?? '');
    for (const p of s.plaetze.values()) {
      for (const a of p.arbeiter.values()) {
        if (a.etage === floor.id && a.info.deskId === desk) {
          ctx.warn(c, `An diesem Platz sitzt schon ${a.info.name} (von ${p.name})`);
          return true;
        }
      }
    }
    const p = c.accountId ? s.plaetze.get(c.accountId) : undefined;
    const etage = fernEtage(floor);
    // Ohne Arbeitsplatz, auf einer Etage ohne GitHub-Repo, oder über mehrere Repos: beim Gastgeber.
    if (!p || !etage || p.ws.readyState !== WebSocket.OPEN || msg.repos?.length) return false;
    weitergeben(p, etage, c, msg);
    return true;
  }
  const wid = 'workerId' in msg && typeof msg.workerId === 'string' ? msg.workerId : undefined;
  const p = wid ? s.fern.get(wid) : undefined;
  const a = wid && p?.arbeiter.get(wid);
  const floor = a ? ctx.floors.get(a.etage) : undefined;
  const etage = floor && fernEtage(floor);
  if (!p || !etage) return false;
  weitergeben(p, etage, c, msg);
  return true;
}

/** Die Arbeiter von außen auf `floor`, für das, was jemand beim Ankommen sieht (FloorView.workers). */
export function fernArbeiter(ctx: Ctx, floor: Floor | undefined): WorkerInfo[] {
  const s = staende.get(ctx);
  if (!s || !floor) return [];
  const out: WorkerInfo[] = [];
  for (const p of s.plaetze.values()) for (const a of p.arbeiter.values()) if (a.etage === floor.id) out.push(a.info);
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
