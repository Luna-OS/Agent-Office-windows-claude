// The desktop app's connector (desktop/connector.cjs, desktop/passthrough.cjs) against a real
// office: signing in with the office password, making your own account, your own sign-ins and
// invites over HTTP (src/server/http/routes/connector.ts), and handing the session to the normal
// browser, through the passthrough that gives it voice on a plain-http lobby.
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import http from 'node:http';
import net from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import WebSocket from 'ws';
import { loadConfig } from '../src/server/config.js';
import { startServer } from '../src/server/server.js';

const require = createRequire(import.meta.url);
const { Lobby, linkKeyOf, sessionCookie } = require('../desktop/connector.cjs');
const { WELL_KNOWN, startPassthrough } = require('../desktop/passthrough.cjs');
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
  for (const page of ['index', 'login', 'claim', 'join', 'lite']) writeFileSync(path.join(publicDir, `${page}.html`), `<!doctype html><title>Agent Office ${page}</title>`);
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
  assert.equal(linkKeyOf('http://127.0.0.1:4600/login#key=Ab_c-12'), 'Ab_c-12');
  assert.equal(linkKeyOf('http://127.0.0.1:4600/'), undefined);
});

test('which lobbies the browser gets through the passthrough, and what counts as an invite link', () => {
  const { inviteOf, lobbyOrigin, needsPassthrough } = require('../desktop/lobby.cjs');
  assert.equal(needsPassthrough('http://100.78.107.162:4600'), true);
  assert.equal(needsPassthrough('http://umbrel.local:4600'), true);
  assert.equal(needsPassthrough('https://office.example.com'), false);
  assert.equal(needsPassthrough('http://127.0.0.1:4600'), false);
  assert.equal(needsPassthrough('http://localhost:4600'), false);
  assert.equal(lobbyOrigin('100.78.107.162:4600/join#abcdefghijklmnopqrstuv'), 'http://100.78.107.162:4600');
  assert.equal(inviteOf('http://100.78.107.162:4600/join#abcdefghijklmnopqrstuv'), '/join#abcdefghijklmnopqrstuv');
  assert.equal(inviteOf('http://100.78.107.162:4600/'), undefined);
});

test('the owner signs in with the office password, makes an account of their own, and invites a friend', async () => {
  const owner = new Lobby(base);
  assert.deepEqual(await owner.options(), { accounts: false, shared: true });
  assert.equal(await owner.whoami(), null);
  await assert.rejects(owner.login('', 'wrong'), /Wrong password/);
  await owner.login('', PASSWORD);
  assert.match(owner.cookie, /^ao_session_\d+=/);
  assert.deepEqual(await owner.whoami(), { admin: true });
  // On the shared password there are no sign-ins of your own.
  assert.equal(await owner.signins(), null);

  // Their own account: an admin invite, used at once.
  const own = await owner.invite('admin', 'Luna', 'Luna');
  assert.equal(own.role, 'admin');
  await owner.join(own.token, 'Luna', 'lunas-own-password');
  assert.deepEqual(await owner.whoami(), { account: { name: 'Luna', role: 'admin' }, admin: true });
  const mine = await owner.signins();
  assert.ok(['ok', 'none', 'busy'].includes(mine.claude.status));
  assert.deepEqual(mine.office, { claude: true, github: true });

  // A friend: a member invite, joined from their own connector.
  const invite = await owner.invite('member', undefined, 'Luna');
  const friend = new Lobby(base);
  const peek = await friend.peek(invite.token);
  assert.equal(peek.role, 'member');
  assert.equal(peek.by, 'Luna');
  await friend.join(invite.token, 'Nico', 'nicos-password');
  assert.deepEqual(await friend.whoami(), { account: { name: 'Nico', role: 'member' }, admin: false });
  await assert.rejects(friend.invite('member'), /Only admins/);
  const theirs = await friend.signins();
  assert.deepEqual(theirs.office, { claude: false, github: false });
  // What doesn't take is said, with the state as it is.
  const nope = await friend.signin('office', 'claude');
  assert.match(nope.error, /Only admins/);
  assert.ok(nope.state.claude);
  const code = await friend.signin('code', 'claude', { code: 'whatever-code' });
  assert.match(code.error, /Start signing in to Claude first/);

  // Signing back in later, by name.
  const again = new Lobby(base);
  await again.login('Nico', 'nicos-password');
  assert.equal((await again.whoami()).account.name, 'Nico');
  await again.logout();
  assert.equal(again.cookie, null);
});

test('only the office itself, or the connector saying it is, may use the connector routes', async () => {
  const lobby = new Lobby(base);
  await lobby.login('', PASSWORD);
  const elsewhere = (p: string) => fetch(base + p, { method: 'POST', headers: { cookie: lobby.cookie, origin: 'https://evil.example', 'content-type': 'application/json' }, body: '{}' });
  for (const p of ['/api/link/new', '/api/signins', '/api/accounts/invite']) assert.equal((await elsewhere(p)).status, 403, p);
  assert.equal((await fetch(base + '/api/link/new', { method: 'POST', headers: { origin: base }, body: '{}' })).status, 401);
});

test('a handed-off link signs the browser in as you, once, through the passthrough', async () => {
  const lobby = new Lobby(base);
  await lobby.login('Luna', 'lunas-own-password');
  const pass = await startPassthrough(base, await freePort());
  try {
    assert.deepEqual(await (await fetch(pass.origin + WELL_KNOWN)).json(), { lobby: base });
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

    // The lobby isn't answering: the browser is told so, in a page.
    pass.retarget('http://127.0.0.1:9');
    const down = await fetch(pass.origin + '/');
    assert.equal(down.status, 502);
    assert.match(await down.text(), /Die Lobby antwortet nicht/);
  } finally {
    await pass.close();
  }
});

test('the office on this computer hands the app a link that signs it in with the office password', async () => {
  const key = linkKeyOf(`${base}${office.signInLink()}`);
  const lobby = new Lobby(base);
  await lobby.useLink(key);
  assert.deepEqual(await lobby.whoami(), { admin: true });
  await assert.rejects(new Lobby(base).useLink(key), /already used/);
});

/** A connector as the app makes it, with its settings in memory and the browser and clipboard written down. */
function connector(start: Record<string, unknown> = {}, here: Record<string, unknown> = {}) {
  let settings: Record<string, any> = { mode: null, lobby: null, lobbies: [], sessions: {}, share: false, ...start };
  const opened: string[] = [];
  const copied: string[] = [];
  const states: any[] = [];
  const c = createController({
    settings: () => settings,
    save: (patch: Record<string, unknown>) => void (settings = { ...settings, ...patch }),
    openExternal: (url: string) => void opened.push(url),
    copy: (text: string) => void copied.push(text),
    here: { running: () => false, stop: async () => {}, inviteBase: () => undefined, look: async () => ({ status: 'none' }), ...here },
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

test('the connector: office password, then an account of your own, an invite, and the office in the browser', async (t) => {
  const owner = connector();
  t.after(() => owner.c.close());
  assert.equal((await owner.run('connect', { address: 'nope://x' })).view, 'lobbies');
  assert.match(owner.last().error, /keine Adresse/);
  let s = await owner.run('connect', { address: base.replace('http://', ''), name: 'Umbrel' });
  assert.equal(s.view, 'signin');
  assert.equal(s.options.shared, true);
  assert.deepEqual(owner.settings().lobbies, [{ name: 'Umbrel', url: base }]);
  await assert.rejects(owner.run('login', { name: '', password: 'wrong' }), /Wrong password/);
  s = await owner.run('login', { name: '', password: PASSWORD });
  assert.equal(s.view, 'home');
  assert.deepEqual(s.me, { admin: true });
  assert.equal(s.signins, null);

  await assert.rejects(owner.run('account', { name: 'Mara', password: 'maras-password', again: 'typo' }), /nicht gleich/);
  s = await owner.run('account', { name: 'Mara', password: 'maras-password', again: 'maras-password' });
  assert.deepEqual(s.me, { account: { name: 'Mara', role: 'admin' }, admin: true });
  assert.ok(s.signins.claude && s.signins.github);
  assert.ok(owner.settings().sessions[base], 'the session is kept for next time');

  s = await owner.run('invite', { role: 'member' });
  assert.match(s.invited.link, new RegExp(`^${base}/join#[\\w-]{16,}$`));
  await owner.run('office');
  assert.match(owner.opened.at(-1)!, new RegExp(`^${base}/login#key=[\\w-]+$`), 'a lobby on this computer opens straight in the browser');

  // A friend pastes the invite link into their own connector, and makes their account.
  const friend = connector();
  t.after(() => friend.c.close());
  s = await friend.run('connect', { address: s.invited.link });
  assert.equal(s.view, 'signin');
  assert.equal(s.pending.by, 'Mara');
  assert.equal(s.pending.token, undefined, "the window never sees the invite's token");
  s = await friend.run('join', { name: 'Kim', password: 'kims-password', again: 'kims-password' });
  assert.deepEqual(s.me, { account: { name: 'Kim', role: 'member' }, admin: false });
  assert.deepEqual(s.signins.office, { claude: false, github: false });
  await assert.rejects(friend.run('signin', { what: 'office', which: 'claude' }), /Only admins/);

  // The next start: straight back in.
  const later = connector(friend.settings());
  t.after(() => later.c.close());
  await later.c.start();
  assert.equal(later.last().view, 'home');
  assert.equal(later.last().me.account.name, 'Kim');
  s = await later.run('logout');
  assert.equal(s.view, 'signin');
  assert.equal(later.settings().sessions[base], undefined);

  await later.run('forget', { url: base });
  assert.deepEqual(later.settings().lobbies, []);
});

test('the connector: the office on this computer signs it in with its own link', async (t) => {
  const c = connector({}, { start: async () => ({ origin: base, link: `${base}${office.signInLink()}` }) });
  t.after(() => c.c.close());
  const s = await c.run('here');
  assert.equal(s.view, 'home');
  assert.equal(s.lobby.here, true);
  assert.deepEqual(s.me, { admin: true });
  assert.equal(c.settings().mode, 'local');
  assert.deepEqual(c.settings().sessions, {}, "the office here hands the app a new link each time: nothing's kept");
  await assert.rejects(c.run('invite', { role: 'member' }), /nur für dich offen/);
  const down = connector({}, { start: async () => Promise.reject(new Error('no node')) });
  assert.match((await down.c.do('here')).error ?? down.last().error, /startet nicht: no node/);
});

test('the connector: hosting through a Vermittler starts the office here with it, and stops again', async (t) => {
  // Ein Vermittler zum Schein: sagt unter /vermittler/stand, dass er einer ist.
  const relay = http.createServer((req, res) => {
    if (req.url !== '/vermittler/stand') return void res.writeHead(404).end();
    res.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify({ dienst: 'agent-office-vermittler', gastgeber: null }));
  });
  await new Promise<void>((r) => relay.listen(0, '127.0.0.1', r));
  t.after(() => relay.close());
  const relayUrl = `http://127.0.0.1:${(relay.address() as net.AddressInfo).port}`;
  let running = false;
  let stops = 0;
  const c = connector(
    {},
    {
      running: () => running,
      stop: async () => void ((running = false), stops++),
      start: async () => ((running = true), { origin: base, link: `${base}${office.signInLink()}` }),
      relayStand: () => ({ stand: 'gastgeber' }),
      ensureSharePassword: () => 'pw',
    },
  );
  t.after(() => c.c.close());
  await assert.rejects(c.run('relayHost', { url: 'nope://x', password: 'x' }), /keine Adresse/);
  await assert.rejects(c.run('relayHost', { url: relayUrl, password: '' }), /Lobby-Passwort/);
  await c.run('relayHost', { url: base, password: 'geheim' });
  assert.match(c.last().error, /kein Vermittler/, 'the office itself is no Vermittler');
  let s = await c.run('relayHost', { url: relayUrl.replace('http://', ''), password: 'geheim' });
  assert.equal(s.view, 'home');
  assert.equal(s.lobby.here, true);
  assert.deepEqual(s.relay, { url: relayUrl, stand: { stand: 'gastgeber' } });
  assert.deepEqual(c.settings().relay, { url: relayUrl, password: { p: 'geheim' } });
  s = await c.run('relayStop');
  assert.equal(s.relay, null);
  assert.equal(c.settings().relay, null);
  assert.equal(stops, 1, 'the office here starts again without the Vermittler');
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

test("the connector: the office here shows this computer's sign-ins first, and sees a console sign-in finish", async (t) => {
  const found: Record<string, { status: string; who?: string }> = { claude: { status: 'none' }, github: { status: 'ok', who: '@luna-os' } };
  const opened: string[] = [];
  const c = connector({}, {
    start: async () => ({ origin: base, link: `${base}${office.signInLink()}` }),
    look: async (which: string) => found[which],
    claude: () => void opened.push('claude'),
  });
  t.after(() => c.c.close());
  let s = await c.run('here');
  assert.equal(s.view, 'home');
  assert.equal(s.signins, null);
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

test("the connector: a lobby account without GitHub is offered this computer's", async (t) => {
  const c = connector({}, { look: async (which: string) => (which === 'github' ? { status: 'ok', who: '@luna-os' } : { status: 'none' }), githubToken: async () => undefined });
  t.after(() => c.c.close());
  await c.run('connect', { address: base });
  const s = await c.run('login', { name: 'Mara', password: 'maras-password' });
  assert.equal(s.signins.github.status === 'ok', false);
  assert.deepEqual(s.mine, { github: { status: 'ok', who: '@luna-os' } });
  await assert.rejects(c.run('githubFromHere'), /nicht angemeldet/);
});
