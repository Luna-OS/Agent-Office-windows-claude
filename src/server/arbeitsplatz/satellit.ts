// Auf dem Computer eines Spielers: Dieses Office ist sein Arbeitsplatz in der Lobby eines Gastgebers
// (--arbeitsplatz, siehe protokoll.ts). Es meldet sich dort mit dem Konto des Spielers an, und die
// Arbeiter, die er einstellt, laufen hier, mit dem Claude und GitHub dieses Computers. Jede Etage des
// Gastgebers ist hier eine eigene, mit demselben Repo (beim ersten Arbeiter geklont).
import { randomBytes } from 'node:crypto';
import { WebSocket } from 'ws';
import type { ServerMsg } from '../../shared/protocol.js';
import type { Client } from '../office/client.js';
import type { Ctx } from '../office/context.js';
import { dispatch } from '../ws/dispatch.js';
import { FUER_DIE_ETAGE, FUER_EINEN, type FernBrowser, type FernEtage, type VomArbeitsplatz, type ZumArbeitsplatz } from './protokoll.js';
import { scheinBrowser } from './virtuell.js';

export interface ArbeitsplatzOpts {
  /** Das Office auf dem Server, z. B. http://umbrel.local:4600 (die Umbrel-App „Agent Office Vermittler“). */
  url: string;
  /** Die Sitzung des Spielers dort, als Cookie (`name=wert`) oder nur ihr Wert. */
  sitzung: string;
  log?: (text: string) => void;
}

export type ArbeitsplatzStand = { stand: 'verbinde' } | { stand: 'verbunden' } | { stand: 'getrennt'; grund: string } | { stand: 'abgelehnt'; grund: string };

/** Alle so oft zeigt jeder Laptop hier beim Gastgeber, was auf ihm steht (für wer gerade dazukam). */
const BILDSCHIRME_MS = 20_000;
/** Kommt so lange kein Ping vom Gastgeber (er schickt alle 20 s einen), ist die Verbindung tot. */
const STILL_MS = 60_000;
/** So lange darf das Klonen eines Repos dauern. */
const KLONEN_MS = 10 * 60_000;

/** Ein Arbeitsplatz, für die Arbeiter darauf, die mit denen in der Lobby reden (reden.ts). */
export interface Satellit {
  /** Die Etage des Gastgebers, zu der die Etage `hier` gehört. */
  dort(hier: string): string | undefined;
  /** Fragt den Gastgeber (`wer`, `sagen`) und wartet auf seine Antwort; undefined, wenn keine kommt. */
  fragen(m: Extract<VomArbeitsplatz, { t: 'wer' } | { t: 'sagen' }>): Promise<ZumArbeitsplatz | undefined>;
}
const satelliten = new WeakMap<object, Satellit>();
/** Der Arbeitsplatz, der dieses Office ist, wenn es einer ist. */
export const satellitVon = (ctx: Ctx): Satellit | undefined => satelliten.get(ctx);
/** So lange wartet ein Arbeiter höchstens auf die Antwort des Gastgebers. */
const ANTWORT_MS = 8000;

export function arbeitsplatzSein(ctx: Ctx, opts: ArbeitsplatzOpts) {
  const log = opts.log ?? ((t: string) => console.log(`arbeitsplatz: ${t}`));
  const lobbyUrl = new URL(/^[a-z]+:\/\//i.test(opts.url) ? opts.url : `http://${opts.url}`);
  const lobby = lobbyUrl.origin;
  // Das Office nennt sein Cookie nach dem Port, unter dem man es erreicht (siehe auth.ts cookieName).
  const sitzung = opts.sitzung.includes('=') ? opts.sitzung.slice(opts.sitzung.indexOf('=') + 1) : opts.sitzung;
  const cookie = `${lobbyUrl.port ? `ao_session_${lobbyUrl.port}` : 'ao_session'}=${sitzung.trim()}`;
  let stand: ArbeitsplatzStand = { stand: 'verbinde' };
  let ws: WebSocket | undefined;
  let gestoppt = false;
  let pause = 2000;
  let wieder: NodeJS.Timeout | undefined;
  /** Etage des Gastgebers → Etage hier. */
  const zuordnung = new Map<string, string>();
  /** Je Etage hier eine Verbindung zum Schein, die alles hört, was dort für alle ist. */
  const spiegel = new Map<string, Client>();
  /** Je Browser beim Gastgeber eine Verbindung zum Schein (für seine Terminals und Antworten). */
  const browser = new Map<string, Client>();
  /** Repos, die gerade geklont werden, nach owner/name. */
  const klonen = new Map<string, Promise<string | undefined>>();

  const senden = (m: VomArbeitsplatz) => {
    if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify(m));
  };
  const etageDort = (hier: string) => [...zuordnung].find(([, l]) => l === hier)?.[0];
  /** Welche Namen auf jeder Etage des Gastgebers schon vergeben sind (zuletzt gehört): Ein neuer hier heißt anders. */
  const vergeben = new Map<string, string[]>();
  /** Was die Arbeiter hier den Gastgeber gefragt haben, nach Nummer: wer die Antwort bekommt. */
  const offen = new Map<number, (a: ZumArbeitsplatz | undefined) => void>();
  let fragen = 0;
  satelliten.set(ctx, {
    dort: etageDort,
    fragen: (m) =>
      new Promise((resolve) => {
        if (ws?.readyState !== WebSocket.OPEN) return resolve(undefined);
        const id = ++fragen;
        const zuLange = setTimeout(() => (offen.delete(id), resolve(undefined)), ANTWORT_MS);
        offen.set(id, (a) => (clearTimeout(zuLange), offen.delete(id), resolve(a)));
        senden({ ...m, id });
      }),
  });
  const etageHier = (repo: string) => [...ctx.floors.values()].find((f) => f.def.repo?.toLowerCase() === repo.toLowerCase())?.id;

  /** Was auf der Etage `hier` sitzt, dem Gastgeber zeigen: die Arbeiter und ihre Laptops. */
  function melden(hier: string) {
    const floor = ctx.floors.get(hier);
    const dort = etageDort(hier);
    if (!floor || !dort) return;
    for (const worker of floor.workers.list()) senden({ t: 'etage', etage: dort, msg: { t: 'worker.update', worker } });
    for (const { workerId, frame } of floor.workers.fullScreens()) senden({ t: 'etage', etage: dort, msg: { t: 'screen', workerId, ...frame, full: true } });
  }

  /** Die Etage `hier` gehört zur Etage `dort` des Gastgebers: Alles für alle dort geht an ihn. */
  function zuordnen(hier: string, dort: string) {
    zuordnung.set(dort, hier);
    const floor = ctx.floors.get(hier);
    if (floor) floor.workers.taken = () => vergeben.get(etageDort(hier) ?? '') ?? [];
    if (!spiegel.has(hier)) {
      const c = scheinBrowser(`spiegel-${hier}`, 'Arbeitsplatz', hier, (msg: ServerMsg) => {
        const d = etageDort(hier);
        if (d && FUER_DIE_ETAGE.has(msg.t)) senden({ t: 'etage', etage: d, msg });
      });
      spiegel.set(hier, c);
      ctx.clients.set(c.id, c);
    }
    melden(hier);
  }

  /** Die Etage hier für die Etage `e` des Gastgebers: da, oder ihr Repo wird jetzt geklont. */
  function etageFuer(e: FernEtage, wer: FernBrowser): Promise<string | undefined> {
    const da = etageHier(e.repo);
    if (da) {
      zuordnen(da, e.id);
      return Promise.resolve(da);
    }
    const key = e.repo.toLowerCase();
    let laeuft = klonen.get(key);
    if (!laeuft) {
      senden({ t: 'an', an: wer.id, msg: { t: 'toast', text: `📦 ${e.repo} wird auf deinen Computer geklont, dann fängt dein Arbeiter an …`, level: 'info' } });
      laeuft = new Promise<string | undefined>((resolve) => {
        const id = `haus-${randomBytes(4).toString('hex')}`;
        const fertig = (floor: string | undefined) => {
          ctx.clients.delete(id);
          klonen.delete(key);
          clearTimeout(zuLange);
          resolve(floor);
        };
        const zuLange = setTimeout(() => fertig(undefined), KLONEN_MS);
        const c = scheinBrowser(id, wer.name, undefined, (msg) => {
          if (msg.t !== 'floor.added' || msg.repo !== e.repo) return;
          if (msg.error) senden({ t: 'an', an: wer.id, msg: { t: 'toast', text: `${e.repo} ließ sich auf deinem Computer nicht klonen: ${msg.error}`, level: 'warn' } });
          fertig(msg.floor);
        });
        ctx.clients.set(id, c);
        dispatch(ctx, c, { t: 'floor.add', repo: e.repo });
      });
      klonen.set(key, laeuft);
    }
    return laeuft.then((hier) => {
      if (hier) zuordnen(hier, e.id);
      return hier;
    });
  }

  /** Der Browser `wer` beim Gastgeber, hier als Verbindung zum Schein. */
  function browserFuer(wer: FernBrowser): Client {
    let c = browser.get(wer.id);
    if (!c) {
      c = scheinBrowser(`fern-${wer.id}`, wer.name, undefined, (msg) => {
        if (FUER_EINEN.has(msg.t)) senden({ t: 'an', an: wer.id, msg });
      });
      browser.set(wer.id, c);
      ctx.clients.set(c.id, c);
    }
    c.peer.name = wer.name;
    return c;
  }

  /** Der Browser `id` ist beim Gastgeber gegangen: Seine Terminals lassen los. */
  function weg(id: string) {
    const c = browser.get(id);
    if (!c) return;
    browser.delete(id);
    ctx.clients.delete(c.id);
    for (const wid of c.attached) ctx.workerFloor(wid)?.workers.detach(wid, c.id);
  }

  async function nachricht(m: Extract<ZumArbeitsplatz, { t: 'nachricht' }>) {
    if (Array.isArray(m.vergeben)) vergeben.set(m.etage.id, m.vergeben.map(String));
    const c = browserFuer(m.wer);
    const msg = m.msg;
    if (msg.t !== 'worker.spawn') return dispatch(ctx, c, msg);
    const hier = await etageFuer(m.etage, m.wer);
    const floor = hier ? ctx.floors.get(hier) : undefined;
    if (!hier || !floor) return;
    // Beim Gastgeber ist das Hinterzimmer ausgebaut: hier auch, damit es dessen Schreibtische gibt.
    while (floor.plan.wing < Number(m.etage.wing) && Array.isArray(floor.plan.expand()));
    // Für den Augenblick auf der Etage: Der Handler stellt dort ein, wo er sie findet.
    c.peer.floor = hier;
    try {
      dispatch(ctx, c, { ...msg, repos: undefined });
    } finally {
      delete c.peer.floor;
    }
  }

  const verbinden = () => {
    if (gestoppt) return;
    const sock = new WebSocket(`${lobby.replace(/^http/, 'ws')}/arbeitsplatz`, { headers: { cookie, origin: lobby }, maxPayload: 8 * 1024 * 1024 });
    ws = sock;
    // Hängt die Verbindung, ohne dass sie zugeht (Netz weg), merkt das nur, wer auf die Pings achtet.
    let zuletzt = Date.now();
    const wache = setInterval(() => Date.now() - zuletzt > STILL_MS && sock.terminate(), STILL_MS / 4);
    wache.unref();
    sock.on('ping', () => (zuletzt = Date.now()));
    sock.on('open', () => {
      zuletzt = Date.now();
      pause = 2000;
      stand = { stand: 'verbunden' };
      log(`verbunden mit ${lobby}: Deine Arbeiter dort laufen jetzt auf diesem Computer`);
    });
    sock.on('unexpected-response', (_req, res) => {
      stand = { stand: 'abgelehnt', grund: res.statusCode === 401 ? 'Die Lobby kennt diese Anmeldung nicht (mit eigenem Konto anmelden)' : `Die Lobby antwortet mit HTTP ${res.statusCode}` };
      log(stand.grund);
    });
    sock.on('message', (d: Buffer, binaer: boolean) => {
      if (binaer) return;
      let m: ZumArbeitsplatz;
      try {
        m = JSON.parse(d.toString('utf8'));
      } catch {
        return;
      }
      if (m.t === 'etagen') {
        for (const e of m.etagen) {
          const hier = etageHier(e.repo);
          if (hier) zuordnen(hier, e.id);
        }
        // Alle gemeldet: Wer beim Gastgeber noch in der Pause sitzt und hier nicht mehr ist, geht dort.
        senden({ t: 'bestand', arbeiter: [...new Set(zuordnung.values())].flatMap((hier) => ctx.floors.get(hier)?.workers.list().map((w) => w.id) ?? []) });
      } else if (m.t === 'nachricht') void nachricht(m);
      else if (m.t === 'die' || m.t === 'gesagt') offen.get(Number(m.id))?.(m);
      else if (m.t === 'weg') weg(String(m.id));
    });
    sock.on('error', (e) => {
      if (stand.stand !== 'abgelehnt') stand = { stand: 'getrennt', grund: e.message };
    });
    sock.on('close', (code: number) => {
      clearInterval(wache);
      for (const antworten of [...offen.values()]) antworten(undefined);
      for (const id of [...browser.keys()]) weg(id);
      if (gestoppt) return;
      // Ein anderer Computer mit demselben Konto ist jetzt der Arbeitsplatz: nicht zurückdrängeln (bis zum nächsten Start).
      if (code === 4000) {
        stand = { stand: 'abgelehnt', grund: 'Ein anderer Computer mit deinem Konto ist jetzt dein Arbeitsplatz' };
        return log(stand.grund);
      }
      if (stand.stand === 'verbunden') stand = { stand: 'getrennt', grund: 'Verbindung zur Lobby unterbrochen' };
      wieder = setTimeout(verbinden, pause);
      pause = Math.min(pause * 2, 30_000);
    });
  };

  const bildschirme = setInterval(() => {
    for (const hier of spiegel.keys()) {
      const floor = ctx.floors.get(hier);
      const dort = etageDort(hier);
      if (!floor || !dort) continue;
      for (const { workerId, frame } of floor.workers.fullScreens()) senden({ t: 'etage', etage: dort, msg: { t: 'screen', workerId, ...frame, full: true } });
    }
  }, BILDSCHIRME_MS);
  bildschirme.unref();

  verbinden();
  return {
    stand: (): ArbeitsplatzStand => stand,
    stop() {
      gestoppt = true;
      satelliten.delete(ctx);
      clearTimeout(wieder);
      clearInterval(bildschirme);
      ws?.close();
    },
  };
}
