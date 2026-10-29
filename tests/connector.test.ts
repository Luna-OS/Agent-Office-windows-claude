// The desktop app's connector (desktop/connector.cjs, desktop/controller.cjs, desktop/passthrough.cjs)
// against a real office, as on the server: signing in with your name and the server's password
// (your own account, made the first time), your own sign-ins over HTTP
// (src/server/http/routes/connector.ts), coming back by itself, ending the connection, your workers
// on this computer, and handing the session to the normal browser, through the passthrough that
// gives it voice on a plain-http server.
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import net from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import WebSocket from 'ws';
import { loadConfig } from '../src/server/config.js';
import { startServer } from '../src/server/server.js';

const require = createRequire(import.meta.url);
const { Lobby, sessionCookie } = require('../desktop/connector.cjs');
const { startPassthrough } = require('../desktop/passthrough.cjs');
const { createController } = require('../desktop/controller.cjs');
const { githubToken, lookClaude, lookGithub } = require('../desktop/local-signins.cjs');

const PASSWORD = 'connector-test-pw';
let tmp = '';
let base = '';
let office: Awaited<ReturnType<typeof startServer>>;

function freePort(): Promise<number> {
  return new Promise((resolve) => {
    const s = net.createServer();
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address() as net.AddressInfo;
      s.close(() => resolve(port));
    });
  });
}

before(async () => {
  tmp = mkdtempSync(path.join(tmpdir(), 'agent-office-connector-'));
  const home = path.join(tmp, 'home');
  const publicDir = path.join(tmp, 'public');
  const bin = path.join(tmp, 'bin');
  for (const d of [home, publicDir, path.join(publicDir, 'assets'), bin, path.join(tmp, 'projects')]) mkdirSync(d, { recursive: true });
  // The connector checks the sign-in page says it's Agent Office's.
  for (const page of ['index', 'login', 'claim', 'lite']) writeFileSync(path.join(publicDir, `${page}.html`), `<!doctype html><title>Agent Office ${page}</title>`);
  const claude = path.join(bin, 'claude');
  writeFileSync(claude, '#!/bin/sh\nexit 0\n');
  chmodSync(claude, 0o755);
  for (const k of Object.keys(process.env)) if (k.startsWith('AGENT_OFFICE_')) delete process.env[k];
  const port = await freePort();
  const cfg = loadConfig(['--home', home, '--projects', path.join(tmp, 'projects'), '--port', String(port), '--password', PASSWORD, '--no-open', '--weather', 'clear', '--agent', claude]);
  office = await startServer(cfg, { publicDir });
  base = `http://127.0.0.1:${port}`;
});

after(() => {
  office?.shutdown();
  if (tmp) rmSync(tmp, { recursive: true, force: true });
});

test('reads the session cookie a sign-in sets, and the one a sign-out clears', () => {
  const h = (...lines: string[]) => ({ getSetCookie: () => lines, get: () => null });
  assert.equal(sessionCookie(h('ao_session_4600=abc.def; Path=/; HttpOnly')), 'ao_session_4600=abc.def');
  assert.equal(sessionCookie(h('other=1', 'ao_session=x; Path=/')), 'ao_session=x');
  assert.equal(sessionCookie(h('ao_session_4600=; Path=/; Max-Age=0')), null);
  assert.equal(sessionCookie(h('other=1')), undefined);
});

test('the server as typed, which ones the browser gets through the passthrough, and your name', () => {
  const { cleanName, lobbyOrigin, needsPassthrough } = require('../desktop/lobby.cjs');
  assert.equal(lobbyOrigin('100.78.107.162:4600'), 'http://100.78.107.162:4600');
  assert.equal(lobbyOrigin(' https://office.example.com/ '), 'https://office.example.com');
  assert.equal(lobbyOrigin('nope://x'), null);
  assert.equal(lobbyOrigin(''), null);
  assert.equal(needsPassthrough('http://100.78.107.162:4600'), true);
  assert.equal(needsPassthrough('http://umbrel.local:4600'), true);
  assert.equal(needsPassthrough('https://office.example.com'), false);
  assert.equal(needsPassthrough('http://127.0.0.1:4600'), false);
  assert.equal(needsPassthrough('http://localhost:4600'), false);
  assert.equal(cleanName('  Luna ​  Lovegood '), 'Luna Lovegood');
  assert.equal(cleanName('x'.repeat(40)).length, 24);
});

test('your name and the server password: your own account, made the first time, and the same one after', async () => {
  const luna = new Lobby(base);
  assert.equal(await luna.whoami(), null);
  await assert.rejects(luna.login('Luna', 'wrong'), /Das Passwort stimmt nicht/);
  await luna.login('Luna', PASSWORD);
  assert.match(luna.cookie, /^ao_session_\d+=/);
  assert.deepEqual(await luna.whoami(), { account: { name: 'Luna', role: 'admin' }, admin: true });
  const mine = await luna.signins();
  assert.ok(['ok', 'none', 'busy'].includes(mine.claude.status));
  assert.deepEqual(mine.office, { claude: true, github: true });
  // What doesn't take is said, with the state as it is.
  const code = await luna.signin('code', 'claude', { code: 'whatever-code' });
  assert.match(code.error, /Start signing in to Claude first/);
  assert.ok(code.state.claude);

  // Again, from another computer: the same account.
  const again = new Lobby(base);
  await again.login(' luna ', PASSWORD);
  assert.equal((await again.whoami()).account.name, 'Luna');
  assert.equal(office.accounts.state(new Set()).accounts.filter((a) => a.name === 'Luna').length, 1);
  await again.logout();
  assert.equal(again.cookie, null);
});

test('only the office itself, or the connector saying it is, may use the connector routes', async () => {
  const lobby = new Lobby(base);
  await lobby.login('Mo', PASSWORD);
  const elsewhere = (p: string) => fetch(base + p, { method: 'POST', headers: { cookie: lobby.cookie, origin: 'https://evil.example', 'content-type': 'application/json' }, body: '{}' });
  for (const p of ['/api/link/new', '/api/signins']) assert.equal((await elsewhere(p)).status, 403, p);
  assert.equal((await fetch(base + '/api/link/new', { method: 'POST', headers: { origin: base }, body: '{}' })).status, 401);
  // Invite links are gone.
  assert.equal((await fetch(base + '/api/join', { method: 'POST', headers: { origin: base, 'content-type': 'application/json' }, body: '{"token":"x"}' })).status, 401);
});

test('a handed-off link signs the browser in as you, once, through the passthrough', async () => {
  const lobby = new Lobby(base);
  await lobby.login('Luna', PASSWORD);
  const pass = await startPassthrough(base, await freePort());
  try {
    // What the browser does with /login#key=…: trade the key for a session, on the passthrough's address.
    const key = await lobby.handoff();
    const traded = await fetch(`${pass.origin}/api/link`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ key }) });
    assert.equal(traded.status, 200);
    const cookie = sessionCookie(traded.headers);
    assert.match(cookie, new RegExp(`^ao_session_${new URL(pass.origin).port}=`));
    const who = await (await fetch(`${pass.origin}/api/whoami`, { headers: { cookie } })).json();
    assert.deepEqual(who.me, { account: { name: 'Luna', role: 'admin' }, admin: true });
    // Once only.
    assert.equal((await fetch(`${pass.origin}/api/link`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ key }) })).status, 410);

    // The office's WebSocket, as the browser opens it there.
    const ws = new WebSocket(`${pass.origin.replace('http', 'ws')}/ws`, { headers: { cookie, origin: pass.origin } });
    const welcome = await new Promise<{ t: string; me: unknown }>((resolve, reject) => {
      ws.on('message', (raw) => {
        const m = JSON.parse(raw.toString());
        if (m.t === 'welcome') resolve(m);
      });
      ws.on('error', reject);
    });
    assert.deepEqual(welcome.me, { account: { name: 'Luna', role: 'admin' }, admin: true });
    ws.close();

    // The server isn't answering: the browser is told so, in a page.
    pass.retarget('http://127.0.0.1:9');
    const down = await fetch(pass.origin + '/');
    assert.equal(down.status, 502);
    assert.match(await down.text(), /Der Server antwortet nicht/);
  } finally {
    await pass.close();
  }
});

/** A connector as the app makes it, with its settings in memory and the browser and clipboard written down. */
function connector(start: Record<string, unknown> = {}, here: Record<string, unknown> = {}) {
  let settings: Record<string, any> = { server: null, password: null, session: null, connect: true, ownWorkers: true, ...start };
  const opened: string[] = [];
  const copied: string[] = [];
  const states: any[] = [];
  const c = createController({
    settings: () => settings,
    save: (patch: Record<string, unknown>) => void (settings = { ...settings, ...patch }),
    openExternal: (url: string) => void opened.push(url),
    copy: (text: string) => void copied.push(text),
    here: { look: async () => ({ status: 'none' }), ...here },
    onChange: (s: unknown) => void states.push(s),
  });
  const last = () => states[states.length - 1];
  const run = async (action: string, args: Record<string, unknown> = {}) => {
    const r = await c.do(action, args);
    if (r.error) throw new Error(r.error);
    return last();
  };
  return { c, run, last, opened, copied, settings: () => settings };
}

test('the connector: address, password and name, then the office in the browser', async (t) => {
  const c = connector();
  t.after(() => c.c.close());
  await c.c.start();
  assert.equal(c.last().view, 'server', 'nothing saved: the form');
  let s = await c.run('connect', { address: 'nope://x', password: PASSWORD, name: 'Mara' });
  assert.equal(s.view, 'server');
  assert.match(s.error, /keine Adresse/);
  s = await c.run('connect', { address: base, password: PASSWORD, name: '  ' });
  assert.match(s.error, /Wie heißt du/);
  s = await c.run('connect', { address: base, password: '', name: 'Mara' });
  assert.match(s.error, /Passwort des Servers fehlt/);
  s = await c.run('connect', { address: base.replace('http://', ''), password: 'wrong', name: 'Mara' });
  assert.equal(s.view, 'server');
  assert.equal(s.error, 'Das Passwort des Servers stimmt nicht.');
  assert.equal(s.connected, false);

  s = await c.run('connect', { address: base.replace('http://', ''), password: PASSWORD, name: 'Mara' });
  assert.equal(s.view, 'home');
  assert.equal(s.connected, true);
  assert.deepEqual(s.me, { account: { name: 'Mara', role: 'admin' }, admin: true });
  assert.ok(s.signins.claude && s.signins.github);
  assert.deepEqual(s.server, { url: base, name: 'Mara', password: true });
  assert.deepEqual(c.settings().server, { url: base, name: 'Mara' });
  assert.deepEqual(c.settings().password, { p: PASSWORD }, 'the password is kept, sealed');
  assert.ok(c.settings().session?.p, 'and the session');
  assert.deepEqual(c.c.current(), { origin: base, name: 'Mara' });

  await c.run('office');
  assert.match(c.opened.at(-1)!, new RegExp(`^${base}/login#key=[\\w-]+$`), 'a server on this computer opens straight in the browser');
});

test('the connector: back by itself on the next start, and not after the connection was ended', async (t) => {
  const first = connector();
  t.after(() => first.c.close());
  await first.run('connect', { address: base, password: PASSWORD, name: 'Kim' });
  const session = first.settings().session;

  const later = connector(first.settings());
  t.after(() => later.c.close());
  await later.c.start();
  assert.equal(later.last().view, 'home');
  assert.equal(later.last().me.account.name, 'Kim');
  assert.deepEqual(later.settings().session, session, 'with the session from before');

  // Verbindung trennen: back at the form, and the next start stays there.
  let s = await later.run('disconnect');
  assert.equal(s.view, 'server');
  assert.equal(s.connected, false);
  assert.equal(later.c.current(), null);
  assert.equal(later.settings().connect, false);
  const next = connector(later.settings());
  t.after(() => next.c.close());
  await next.c.start();
  assert.equal(next.last().view, 'server');
  assert.deepEqual(next.last().server, { url: base, name: 'Kim', password: true });
  // Verbinden again (the menu bar's): what's saved is enough.
  assert.equal(next.c.canConnect(), true);
  await next.c.reconnect();
  assert.equal(next.last().view, 'home');
  assert.equal(next.settings().connect, true);

  // A session that stopped counting: signed in again with the saved password.
  const stale = connector({ ...first.settings(), session: { p: 'ao_session_1=nope' } });
  t.after(() => stale.c.close());
  await stale.c.start();
  assert.equal(stale.last().view, 'home');
  assert.equal(stale.last().me.account.name, 'Kim');
  assert.notDeepEqual(stale.settings().session, { p: 'ao_session_1=nope' });
});

test('the connector: the settings, another name, and the saved password', async (t) => {
  const c = connector();
  t.after(() => c.c.close());
  await c.run('connect', { address: base, password: PASSWORD, name: 'Lea' });
  let s = await c.run('settings');
  assert.equal(s.view, 'server');
  assert.equal(s.editing, true);
  s = await c.run('back');
  assert.equal(s.view, 'home');
  assert.equal(s.editing, false);
  // Another name, the password field left empty: the saved one, and a new session for the new name.
  s = await c.run('connect', { address: base, password: '', name: 'Lea Zwei' });
  assert.equal(s.view, 'home');
  assert.equal(s.me.account.name, 'Lea Zwei');
  // Another server, without its password: asked for, not the saved one of the other.
  s = await c.run('connect', { address: 'http://127.0.0.1:9', password: '', name: 'Lea' });
  assert.match(s.error, /Passwort des Servers fehlt/);
});

test('the connector: with your account on the server, your workers run on this computer', async (t) => {
  const calls: string[] = [];
  const workstation = {
    start: async (origin: string, cookie: string) => void calls.push(`start ${origin} ${cookie.split('=')[0]}`),
    stop: async () => void calls.push('stop'),
    state: () => ({ lobby: base, stand: { stand: 'verbunden' } }),
  };
  const c = connector({}, { workstation, look: async () => ({ status: 'ok', who: 'lea' }) });
  t.after(() => c.c.close());
  let s = await c.run('connect', { address: base, password: PASSWORD, name: 'Lea' });
  assert.equal(s.view, 'home');
  assert.deepEqual(s.workstation, { on: true, stand: { stand: 'verbunden' } });
  assert.match(calls.at(-1)!, new RegExp(`^start ${base} ao_session_\\d+$`), 'started with her session there');
  assert.deepEqual(s.mine, { claude: { status: 'ok', who: 'lea' }, github: { status: 'ok', who: 'lea' } }, "this computer's sign-ins are the ones that count");
  s = await c.run('ownWorkers', { on: false });
  assert.equal(calls.at(-1), 'stop');
  assert.equal(s.workstation.on, false);
  assert.equal(c.settings().ownWorkers, false);
  await c.run('ownWorkers', { on: true });
  assert.match(calls.at(-1)!, /^start /);
  await c.run('disconnect');
  assert.equal(calls.at(-1), 'stop', 'not connected: no workstation');
});

test("this computer's own sign-ins, as claude and gh tell them", async () => {
  const says = (out: string, code = 0) => async () => ({ code, out });
  assert.deepEqual(await lookClaude(says('{"loggedIn":true,"email":"luna@example.com","subscriptionType":"max"}')), { status: 'ok', who: 'luna@example.com · Max' });
  assert.deepEqual(await lookClaude(says('{"loggedIn":false}', 1)), { status: 'none' });
  assert.deepEqual(await lookClaude(says('not json', 1)), { status: 'none' });
  assert.deepEqual(await lookClaude(async () => ({ code: 127, out: '', missing: true })), { status: 'none', missing: true });
  assert.deepEqual(await lookGithub(says('luna-os\n')), { status: 'ok', who: '@luna-os' });
  assert.deepEqual(await lookGithub(says('', 1)), { status: 'none' });
  assert.equal(await githubToken(says('gho_abcdefghijklmnopqrstuvwxyz0123456789\n')), 'gho_abcdefghijklmnopqrstuvwxyz0123456789');
  assert.equal(await githubToken(says('no oauth token found', 1)), undefined);
});

test("the connector: this computer's sign-ins for your workers, and a console sign-in finishing", async (t) => {
  const found: Record<string, { status: string; who?: string }> = { claude: { status: 'none' }, github: { status: 'ok', who: '@luna-os' } };
  const opened: string[] = [];
  const workstation = { start: async () => {}, stop: async () => {}, state: () => null };
  const c = connector({}, { workstation, look: async (which: string) => found[which], claude: () => void opened.push('claude') });
  t.after(() => c.c.close());
  let s = await c.run('connect', { address: base, password: PASSWORD, name: 'Ida' });
  assert.equal(s.view, 'home');
  assert.deepEqual(s.mine, found);
  s = await c.run('hereClaude');
  assert.deepEqual(opened, ['claude']);
  assert.equal(s.waiting.claude, true);
  // Signed in in the console window: the next look says so.
  found.claude = { status: 'ok', who: 'luna@example.com · Max' };
  s = await c.run('refresh');
  assert.equal(s.mine.claude.who, 'luna@example.com · Max');
  assert.equal(s.waiting.claude, false);
});

test("the connector: an account without GitHub on the server is offered this computer's", async (t) => {
  const c = connector({ ownWorkers: false }, { look: async (which: string) => (which === 'github' ? { status: 'ok', who: '@luna-os' } : { status: 'none' }), githubToken: async () => undefined });
  t.after(() => c.c.close());
  const s = await c.run('connect', { address: base, password: PASSWORD, name: 'Nico' });
  assert.equal(s.signins.github.status === 'ok', false);
  assert.deepEqual(s.mine, { github: { status: 'ok', who: '@luna-os' } });
  await assert.rejects(c.run('githubFromHere'), /nicht angemeldet/);
});
