// Der Arbeitsplatz eines Spielers (src/server/arbeitsplatz/): zwei Offices in einem Test, das eine als
// Gastgeber, das andere als Kims Arbeitsplatz. Kim stellt beim Gastgeber einen Arbeiter ein, und der
// läuft auf ihrem Arbeitsplatz; beim Gastgeber sitzt er trotzdem am Schreibtisch, sein Terminal geht
// hin und zurück, und nach Hause geschickt verschwindet er dort wie hier.
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import net from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import WebSocket from 'ws';
import { loadConfig } from '../src/server/config.js';
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
  projekt(path.join(home, 'werkstatt'));
  const port = await freePort();
  const cfg = loadConfig([path.join(home, 'werkstatt'), '--home', home, '--projects', path.join(home, 'projects'), '--port', String(port), '--password', PASSWORT, '--no-open', '--weather', 'clear', ...extra]);
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
  for (const k of Object.keys(process.env)) if (k.startsWith('AGENT_OFFICE_')) delete process.env[k];
  const g = await office('gastgeber');
  gastgeber = await startServer(g.cfg, { publicDir: g.publicDir });
  base = `http://127.0.0.1:${g.port}`;
  // Kim hat ein eigenes Konto in der Lobby.
  const invite = gastgeber.accounts.invite('test', 'member', 'Kim');
  assert.equal(typeof invite, 'object');
  await gastgeber.accounts.join((invite as { token: string }).token, 'Kim', 'kim-passwort-1');
  kimCookie = await login('Kim', 'kim-passwort-1');
  const p = await office('kims-pc');
  p.cfg.arbeitsplatz = { url: base, sitzung: kimCookie };
  platz = await startServer(p.cfg, { publicDir: p.publicDir });
  for (let i = 0; i < 100 && platz.arbeitsplatz()?.stand !== 'verbunden'; i++) await new Promise((r) => setTimeout(r, 50));
  assert.deepEqual(platz.arbeitsplatz(), { stand: 'verbunden' });
});

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

test('geht der Arbeitsplatz, verschwinden seine Arbeiter beim Gastgeber', async () => {
  const kim = await browser(kimCookie);
  kim.send({ t: 'worker.spawn', deskId: 'desk-4', kind: 'shell' });
  const update = await kim.until((m) => m.t === 'worker.update' && m.worker.deskId === 'desk-4');
  platz!.shutdown();
  platz = undefined;
  await kim.until((m) => m.t === 'worker.remove' && m.workerId === update.worker.id);
  kim.ws.close();
});
