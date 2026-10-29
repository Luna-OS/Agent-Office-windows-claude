// Der Arbeitsplatz eines Spielers (src/server/arbeitsplatz/): zwei Offices in einem Test, das eine als
// Gastgeber, das andere als Kims Arbeitsplatz. Kim stellt beim Gastgeber einen Arbeiter ein, und der
// läuft auf ihrem Arbeitsplatz; beim Gastgeber sitzt er trotzdem am Schreibtisch, sein Terminal geht
// hin und zurück, und nach Hause geschickt verschwindet er dort wie hier.
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import net from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import WebSocket from 'ws';
import { loadConfig } from '../src/server/config.js';
import { Pausen } from '../src/server/arbeitsplatz/pause.js';
import { scheinBrowser } from '../src/server/arbeitsplatz/virtuell.js';
import { startServer } from '../src/server/server.js';

let tmp = '';
let base = '';
let gastgeber: Awaited<ReturnType<typeof startServer>>;
let platz: Awaited<ReturnType<typeof startServer>> | undefined;
let kimCookie = '';
const PASSWORT = 'gastgeber-pw';

function freePort(): Promise<number> {
  return new Promise((resolve) => {
    const s = net.createServer();
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address() as net.AddressInfo;
      s.close(() => resolve(port));
    });
  });
}

/** Ein Projekt mit dem GitHub-Repo luna-test/werkstatt als origin (ohne dass je etwas geholt wird). */
function projekt(dir: string) {
  mkdirSync(dir, { recursive: true });
  const git = (...a: string[]) => execFileSync('git', ['-C', dir, ...a], { stdio: 'ignore' });
  git('init', '-q');
  git('-c', 'user.email=t@t', '-c', 'user.name=t', 'commit', '-q', '--allow-empty', '-m', 'init');
  git('remote', 'add', 'origin', 'https://github.com/luna-test/werkstatt.git');
}

async function office(name: string, extra: string[] = []) {
  const home = path.join(tmp, name);
  const publicDir = path.join(tmp, `${name}-public`);
  mkdirSync(path.join(publicDir, 'assets'), { recursive: true });
  for (const page of ['index', 'login']) writeFileSync(path.join(publicDir, `${page}.html`), `<!doctype html><title>Agent Office ${page}</title>`);
  // Noch einmal gestartet (derselbe PC, wieder an): sein Projekt ist schon da.
  if (!existsSync(path.join(home, 'werkstatt', '.git'))) projekt(path.join(home, 'werkstatt'));
  const port = await freePort();
  const cfg = loadConfig([path.join(home, 'werkstatt'), '--home', home, '--projects', path.join(home, 'projects'), '--port', String(port), '--password', PASSWORT, '--no-open', '--weather', 'clear', '--agent', path.join(tmp, 'schein-agent'), ...extra]);
  return { cfg, publicDir, port };
}

/** Ein Browser beim Gastgeber: was er bekommt, und worauf man warten kann. */
async function browser(cookie: string) {
  const got: any[] = [];
  const waiters: { pick: (m: any) => boolean; resolve: (m: any) => void }[] = [];
  const ws = new WebSocket(`${base.replace('http', 'ws')}/ws`, { headers: { cookie, origin: base } });
  ws.on('message', (d) => {
    const m = JSON.parse(String(d));
    got.push(m);
    for (const w of [...waiters]) if (w.pick(m)) (waiters.splice(waiters.indexOf(w), 1), w.resolve(m));
  });
  const until = (pick: (m: any) => boolean, ms = 15000) =>
    new Promise<any>((resolve, reject) => {
      const seen = got.find(pick);
      if (seen) return resolve(seen);
      const t = setTimeout(() => reject(new Error(`nicht gekommen; zuletzt: ${got.slice(-5).map((m) => m.t).join(', ')}`)), ms);
      waiters.push({ pick, resolve: (m) => (clearTimeout(t), resolve(m)) });
    });
  const welcome = await until((m) => m.t === 'welcome');
  return { ws, got, until, welcome, send: (m: object) => ws.send(JSON.stringify(m)) };
}

async function login(name: string | undefined, password: string) {
  const r = await fetch(`${base}/api/login`, { method: 'POST', headers: { 'content-type': 'application/json', origin: base }, body: JSON.stringify(name ? { name, password } : { password }) });
  assert.equal(r.status, 200);
  return (r.headers.get('set-cookie') ?? '').split(';')[0];
}

before(async () => {
  tmp = mkdtempSync(path.join(tmpdir(), 'agent-office-arbeitsplatz-'));
  // Ein Agent zum Schein: gibt zurück, was man ihm sagt (die Arbeiter "agent" in den Tests zum Reden).
  writeFileSync(path.join(tmp, 'schein-agent'), '#!/bin/sh\nexec cat\n', { mode: 0o755 });
  for (const k of Object.keys(process.env)) if (k.startsWith('AGENT_OFFICE_')) delete process.env[k];
  const g = await office('gastgeber');
  gastgeber = await startServer(g.cfg, { publicDir: g.publicDir });
  base = `http://127.0.0.1:${g.port}`;
  // Kim hat ein eigenes Konto in der Lobby.
  const invite = gastgeber.accounts.invite('test', 'member', 'Kim');
  assert.equal(typeof invite, 'object');
  await gastgeber.accounts.join((invite as { token: string }).token, 'Kim', 'kim-passwort-1');
  kimCookie = await login('Kim', 'kim-passwort-1');
  platz = await kimsPc('kims-pc');
});

/** Kims PC, mit ihrem Arbeitsplatz in der Lobby des Gastgebers, verbunden. */
async function kimsPc(name: string) {
  const p = await office(name);
  p.cfg.arbeitsplatz = { url: base, sitzung: kimCookie };
  const pc = await startServer(p.cfg, { publicDir: p.publicDir });
  for (let i = 0; i < 100 && pc.arbeitsplatz()?.stand !== 'verbunden'; i++) await new Promise((r) => setTimeout(r, 50));
  assert.deepEqual(pc.arbeitsplatz(), { stand: 'verbunden' });
  return pc;
}

after(async () => {
  platz?.shutdown();
  gastgeber?.shutdown();
  // Was beim Herunterfahren noch geschrieben wird (beendete Shells ihre History, workers.json), fertig
  // werden lassen: so lange wieder versuchen, bis der Ordner weg ist.
  for (let i = 0; tmp && i < 40; i++) {
    try {
      rmSync(tmp, { recursive: true, force: true });
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 250));
    }
  }
});

test('Kims Arbeiter läuft auf ihrem Arbeitsplatz und sitzt trotzdem beim Gastgeber', async () => {
  const kim = await browser(kimCookie);
  const gast = await browser(await login(undefined, PASSWORT));
  const etage = kim.welcome.floor;
  assert.ok(etage, 'Kim ist auf einer Etage');
  kim.send({ t: 'worker.spawn', deskId: 'desk-3', kind: 'shell' });

  const update = await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-3');
  const wid = update.worker.id;
  assert.equal(update.worker.kind, 'shell');
  assert.equal(update.worker.createdBy, 'Kim');
  // Er läuft auf Kims Arbeitsplatz, nicht beim Gastgeber, und alle auf der Etage sehen ihn.
  assert.equal(gastgeber.floors()[0].workers.get(wid), undefined);
  assert.ok(platz!.floors()[0].workers.get(wid));
  await gast.until((m) => m.t === 'worker.update' && m.worker.id === wid);

  // Wer jetzt dazukommt, sieht ihn auch.
  const spaeter = await browser(await login(undefined, PASSWORT));
  assert.ok(spaeter.welcome.workers.some((w: { id: string }) => w.id === wid));

  // Sein Terminal: anhängen, tippen, die Ausgabe kommt zurück.
  kim.send({ t: 'worker.attach', workerId: wid });
  await kim.until((m) => m.t === 'term.snapshot' && m.workerId === wid);
  kim.send({ t: 'term.input', workerId: wid, data: 'echo HALLO_VOM_ARBEITSPLATZ\r' });
  await kim.until((m) => m.t === 'term.data' && m.workerId === wid && String(m.data).includes('HALLO_VOM_ARBEITSPLATZ'));

  // Beim Gastgeber ist das Hinterzimmer ausgebaut: Kims nächster Arbeiter kann auch dorthin.
  gastgeber.floors()[0].plan.expand();
  kim.send({ t: 'worker.spawn', deskId: 'desk-13', kind: 'shell' });
  const hinten = await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-13');
  assert.equal(platz!.floors()[0].plan.wing, 1);
  kim.send({ t: 'worker.kill', workerId: hinten.worker.id });
  await kim.until((m) => m.t === 'worker.remove' && m.workerId === hinten.worker.id);

  // Ein anderer Arbeiter kann nicht an denselben Platz.
  gast.send({ t: 'worker.spawn', deskId: 'desk-3', kind: 'shell' });
  await gast.until((m) => m.t === 'toast' && /An diesem Platz sitzt schon/.test(m.text));

  // Nach Hause: dort weg, und hier auch.
  kim.send({ t: 'worker.kill', workerId: wid });
  await gast.until((m) => m.t === 'worker.remove' && m.workerId === wid);
  for (let i = 0; i < 100 && platz!.floors()[0].workers.get(wid); i++) await new Promise((r) => setTimeout(r, 50));
  assert.equal(platz!.floors()[0].workers.get(wid), undefined);
  for (const b of [kim, gast, spaeter]) b.ws.close();
});

/** Was ein Arbeiter (`id`) eines Office über sein office-workers sieht oder tut: list_workers und tell_worker. */
async function arbeiterFragt(o: Awaited<ReturnType<typeof startServer>>, id: string, tell?: { worker: string; prompt: string }) {
  const token = (o.floors()[0].workers as any).workers.get(id).hookToken;
  const r = await fetch(`http://127.0.0.1:${o.hookPort}/office/workers${tell ? '/tell' : ''}?worker=${id}`, {
    method: tell ? 'POST' : 'GET',
    headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
    body: tell ? JSON.stringify(tell) : undefined,
  });
  return { status: r.status, body: (await r.json()) as any };
}

test('Arbeiter reden miteinander, auch über die PCs hinweg, und alle sehen es', async () => {
  const kim = await browser(kimCookie);
  const gast = await browser(await login(undefined, PASSWORT));
  // Hanna arbeitet beim Gastgeber, zwei von Kim auf ihrem PC.
  gast.send({ t: 'worker.spawn', deskId: 'desk-8', kind: 'agent' });
  const h = (await gast.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-8')).worker;
  kim.send({ t: 'worker.spawn', deskId: 'desk-9', kind: 'agent' });
  const k = (await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-9')).worker;
  kim.send({ t: 'worker.spawn', deskId: 'desk-10', kind: 'agent' });
  const k2 = (await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-10')).worker;
  assert.ok(gastgeber.floors()[0].workers.get(h.id), 'Hanna beim Gastgeber');
  assert.ok(platz!.floors()[0].workers.get(k.id), 'der von Kim bei ihr');
  // Jeder hat in der Lobby seinen eigenen Namen, auch wenn Kims PC die Namen selbst vergibt.
  assert.equal(new Set([h.name, k.name, k2.name]).size, 3, `${h.name}, ${k.name}, ${k2.name}`);
  // Und wer danach beim Gastgeber anfängt, heißt auch nicht wie die von Kim.
  gast.send({ t: 'worker.spawn', deskId: 'desk-11', kind: 'shell' });
  const h2 = (await gast.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-11')).worker;
  assert.ok(![k.name, k2.name].includes(h2.name.replace(/ 🐚$/u, '')), h2.name);
  const warte = async (ob: () => unknown) => {
    for (let i = 0; i < 100 && !ob(); i++) await new Promise((r) => setTimeout(r, 50));
    assert.ok(ob());
  };

  // Beim Gastgeber: list_workers zeigt auch die auf Kims PC.
  const liste = await arbeiterFragt(gastgeber, h.id);
  assert.equal(liste.status, 200);
  assert.equal(liste.body.workers.find((w: any) => w.id === k.id)?.pc, 'Kim');

  // Hanna sagt dem von Kim etwas: Es kommt auf Kims PC an, mit Absender, und alle sehen es.
  const an = await arbeiterFragt(gastgeber, h.id, { worker: k.name, prompt: 'HALLO_VON_HANNA' });
  assert.equal(an.status, 200, JSON.stringify(an.body));
  assert.equal(an.body.worker.id, k.id);
  await warte(() => platz!.floors()[0].workers.get(k.id)?.lastInput?.by === h.name);
  assert.match(String(platz!.floors()[0].workers.get(k.id)?.activity), new RegExp(`^💬 Nachricht von ${h.name}`), 'mit Absender, zum Antworten');
  await gast.until((m) => m.t === 'worker.says' && m.from === h.id && m.to === k.id && m.text === 'HALLO_VON_HANNA');
  await gast.until((m) => m.t === 'chat' && m.name === `🤖 ${h.name} → ${k.name}` && m.text === 'HALLO_VON_HANNA');

  // Auf Kims PC: list_workers zeigt Hanna beim Gastgeber, und die Antwort geht zu ihr zurück.
  const dort = await arbeiterFragt(platz!, k.id);
  assert.equal(dort.body.workers.find((w: any) => w.id === h.id)?.pc, 'Gastgeber');
  assert.equal(dort.body.workers.filter((w: any) => w.id === k2.id).length, 1, 'die eigenen nur einmal');
  const zurueck = await arbeiterFragt(platz!, k.id, { worker: h.name, prompt: 'ANTWORT_VON_KIMS_ARBEITER' });
  assert.equal(zurueck.status, 200, JSON.stringify(zurueck.body));
  await warte(() => gastgeber.floors()[0].workers.get(h.id)?.lastInput?.by === k.name);
  await gast.until((m) => m.t === 'worker.says' && m.from === k.id && m.to === h.id);

  // Zwei auf Kims PC reden miteinander: Das sehen auch alle in der Lobby.
  const unter = await arbeiterFragt(platz!, k.id, { worker: k2.name, prompt: 'UNTER_UNS' });
  assert.equal(unter.status, 200, JSON.stringify(unter.body));
  await gast.until((m) => m.t === 'worker.says' && m.from === k.id && m.to === k2.id && m.text === 'UNTER_UNS');

  // Wen es nicht gibt, sagt die Lobby auch.
  const niemand = await arbeiterFragt(platz!, k.id, { worker: 'Niemand', prompt: 'hallo?' });
  assert.equal(niemand.status, 404);
  for (const id of [k.id, k2.id]) kim.send({ t: 'worker.kill', workerId: id });
  for (const id of [h.id, h2.id]) gast.send({ t: 'worker.kill', workerId: id });
  await gast.until((m) => m.t === 'worker.remove' && m.workerId === k2.id);
  for (const b of [kim, gast]) b.ws.close();
});

test('eine Verbindung zum Schein überlebt den Herzschlag und reicht weiter, was sie bekommt', () => {
  const bekommen: string[] = [];
  const c = scheinBrowser('x1', 'Kim', undefined, (m) => bekommen.push(m.t));
  // Wie office/timers.ts: erst auf false, dann ping; beim nächsten Schlag muss es wieder true sein.
  c.isAlive = false;
  c.ws.ping();
  assert.equal(c.isAlive, true);
  assert.equal(c.ws.readyState, WebSocket.OPEN);
  c.ws.send(JSON.stringify({ t: 'term.data', workerId: 'w', data: 'hallo' }));
  assert.deepEqual(bekommen, ['term.data']);
});

test('ohne eigenes Konto gibt es keinen Arbeitsplatz', async () => {
  const gast = await login(undefined, PASSWORT);
  const ws = new WebSocket(`${base.replace('http', 'ws')}/arbeitsplatz`, { headers: { cookie: gast, origin: base } });
  const status = await new Promise<number>((resolve) => {
    ws.on('unexpected-response', (_req, res) => resolve(res.statusCode ?? 0));
    ws.on('open', () => resolve(101));
  });
  assert.equal(status, 401);
});

test('geht der Arbeitsplatz, machen seine Arbeiter Pause, und ist er wieder da, geht es weiter', async () => {
  const kim = await browser(kimCookie);
  kim.send({ t: 'worker.spawn', deskId: 'desk-4', kind: 'shell' });
  const a = (await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-4')).worker.id;
  kim.send({ t: 'worker.spawn', deskId: 'desk-5', kind: 'shell' });
  const b = (await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-5')).worker.id;

  // Kims PC geht aus: Ihre Arbeiter bleiben an ihren Plätzen sitzen und machen Pause.
  platz!.shutdown();
  platz = undefined;
  const inPause = (id: string) => (m: any) => m.t === 'worker.update' && m.worker.id === id && m.worker.pause === 'Kim';
  assert.equal((await kim.until(inPause(a))).worker.status, 'offline');
  await kim.until(inPause(b));
  const gast = await browser(await login(undefined, PASSWORT));
  assert.ok(gast.welcome.workers.some((w: { id: string; pause?: string }) => w.id === a && w.pause === 'Kim'), 'wer dazukommt, sieht ihn in der Pause');
  // Sein Platz bleibt belegt, und wer ihn anspricht, erfährt, warum er nichts tut.
  gast.send({ t: 'worker.spawn', deskId: 'desk-4', kind: 'shell' });
  await gast.until((m) => m.t === 'toast' && /gerade in der Pause/.test(m.text));
  gast.send({ t: 'worker.attach', workerId: a });
  await gast.until((m) => m.t === 'toast' && /macht Pause: Der PC von Kim ist gerade aus/.test(m.text));
  // Nach Hause geschickt, geht einer aus der Pause aus der Lobby.
  gast.send({ t: 'worker.kill', workerId: b });
  await gast.until((m) => m.t === 'worker.remove' && m.workerId === b);
  await gast.until((m) => m.t === 'toast' && /geht aus der Pause nach Hause/.test(m.text));

  // Kims PC ist wieder an: Die Pause ist vorbei, und auch der andere ist wieder da (auf ihrem PC war er noch).
  const seit = gast.got.length;
  platz = await kimsPc('kims-pc');
  const zurueck = (id: string) => (m: any) => gast.got.indexOf(m) >= seit && m.t === 'worker.update' && m.worker.id === id && !m.worker.pause;
  await gast.until(zurueck(a));
  await gast.until(zurueck(b));

  // Und von einem neuen PC aus, auf dem sie nicht sind: Sie gehen.
  platz.shutdown();
  await kim.until((m) => kim.got.indexOf(m) > kim.got.findIndex(zurueck(b)) && inPause(a)(m));
  platz = await kimsPc('kims-neuer-pc');
  await gast.until((m) => m.t === 'worker.remove' && m.workerId === a && gast.got.indexOf(m) >= seit);
  await gast.until((m) => m.t === 'worker.remove' && m.workerId === b && gast.got.indexOf(m) >= seit);
  for (const x of [kim, gast]) x.ws.close();
});

test('ein zweiter PC mit demselben Konto wird der Arbeitsplatz; was nur auf dem ersten läuft, macht Pause', async () => {
  const kim = await browser(kimCookie);
  kim.send({ t: 'worker.spawn', deskId: 'desk-6', kind: 'shell' });
  const x = (await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-6')).worker.id;
  const erster = platz!;
  platz = await kimsPc('kims-zweiter-pc');
  await kim.until((m) => m.t === 'worker.update' && m.worker.id === x && m.worker.pause === 'Kim');
  // Der erste drängelt sich nicht zurück.
  for (let i = 0; i < 60 && erster.arbeitsplatz()?.stand !== 'abgelehnt'; i++) await new Promise((r) => setTimeout(r, 50));
  assert.equal(erster.arbeitsplatz()?.stand, 'abgelehnt');
  // Wer einem in der Pause etwas tippt, erfährt es einmal, nicht bei jeder Taste.
  const hinweis = (m: any) => m.t === 'toast' && /macht Pause/.test(m.text);
  for (const ch of 'hallo') kim.send({ t: 'term.input', workerId: x, data: ch });
  kim.send({ t: 'term.resize', workerId: x, cols: 80, rows: 24 });
  await kim.until(hinweis);
  await new Promise((r) => setTimeout(r, 300));
  assert.equal(kim.got.filter(hinweis).length, 1);
  erster.shutdown();
  kim.ws.close();
});

test('die Pausen bleiben gemerkt, auch über einen Neustart', () => {
  const datei = path.join(tmp, 'pausen.json');
  const info = { id: 'w9', name: 'Ada', deskId: 'desk-7', status: 'working', viewers: ['Mo'], viewerIds: ['c1'] } as any;
  const ruht = new Pausen(datei).add('konto-kim', 'Kim', 'etage-1', info);
  assert.equal(ruht.pause, 'Kim');
  assert.equal(ruht.status, 'offline');
  assert.deepEqual(ruht.viewers, []);
  const wieder = new Pausen(datei);
  assert.equal(wieder.get('w9')?.info.pause, 'Kim');
  assert.equal(wieder.ofAccount('konto-kim').length, 1);
  wieder.delete('w9');
  assert.equal(new Pausen(datei).size, 0);
});
