// The connector's side of a lobby: an office on an Umbrel, a server, or this computer. Signs in
// there (your own account, an invite link, the office password, or the local office's one-time
// link), and asks it for what the connector shows: who you are, your own Claude and GitHub
// sign-ins, invite links, and a one-time link that signs the normal browser in as you (see
// src/server/http/routes/connector.ts). Plain fetch, so it runs in Electron's main process and in
// the tests alike; the session is the office's own cookie, kept by the app between starts.

const { closedLobby } = require('./lobby.cjs');

const COOKIE = /^(ao_session(?:_\d+)?)=([^;]*)/;

class LobbyError extends Error {
  constructor(message, status) {
    super(message);
    this.status = status;
  }
}

/** The session cookie a response sets (name=value), null when it signs out, undefined when it sets none. */
function sessionCookie(headers) {
  const all = typeof headers.getSetCookie === 'function' ? headers.getSetCookie() : [headers.get('set-cookie')].filter(Boolean);
  for (const line of all) {
    const m = COOKIE.exec(line);
    if (m) return m[2] ? `${m[1]}=${m[2]}` : null;
  }
  return undefined;
}

class Lobby {
  /**
   * `origin` is the lobby's (http://100.78.107.162:4600), `cookie` a session from before (name=value).
   * Every request says it comes from the lobby's own origin, which the office's same-origin checks
   * want: the connector acts for whoever signed in here, like the office's own page would.
   */
  constructor(origin, cookie = null, get = fetch) {
    this.origin = origin;
    this.cookie = cookie;
    this.get = get;
  }

  async call(path, body, ms = 20_000) {
    const headers = { origin: this.origin, accept: 'application/json' };
    if (body !== undefined) headers['content-type'] = 'application/json';
    if (this.cookie) headers.cookie = this.cookie;
    let res;
    try {
      res = await this.get(`${this.origin}${path}`, {
        method: body === undefined ? 'GET' : 'POST',
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        redirect: 'manual',
        signal: AbortSignal.timeout(ms),
      });
    } catch (err) {
      const why = err?.name === 'TimeoutError' ? 'keine Antwort' : (err?.cause?.code ?? err?.message ?? String(err));
      throw new LobbyError(`Die Lobby ${this.origin} ist nicht erreichbar (${why}).`, 0);
    }
    const set = sessionCookie(res.headers);
    if (set !== undefined) this.cookie = set;
    const raw = await res.text().catch(() => '');
    let json = {};
    try {
      json = JSON.parse(raw);
    } catch {
      // not JSON: a Vermittler's "nobody hosts here", say (see closedLobby)
    }
    return { status: res.status, ok: res.ok, json: json && typeof json === 'object' ? json : {}, raw };
  }

  async must(path, body, ms) {
    const r = await this.call(path, body, ms);
    if (!r.ok) throw new LobbyError(r.json.error ?? closedLobby(r.status, r.raw) ?? `HTTP ${r.status}`, r.status);
    return r.json;
  }

  /** Which ways in the lobby's sign-in takes: { accounts, shared }. */
  options() {
    return this.must('/api/login');
  }

  /** With a name, that account; without one, the shared office password. */
  login(name, password) {
    return this.must('/api/login', { name: name ?? '', password });
  }

  /** Who an invite link (its token) is for: { name?, role, by, project }. */
  peek(token) {
    return this.must('/api/join', { token, peek: true });
  }

  /** Makes the invite's account, and signs in with it. */
  join(token, name, password) {
    return this.must('/api/join', { token, name, password });
  }

  /** Signs in with a one-time sign-in link's key (the office on this computer hands the app those). */
  useLink(key) {
    return this.must('/api/link', { key });
  }

  /** Who the session signs in ({ account?, admin }), or null when it doesn't (any more). */
  async whoami() {
    if (!this.cookie) return null;
    const r = await this.call('/api/whoami');
    if (r.status === 401) {
      this.cookie = null;
      return null;
    }
    if (!r.ok) throw new LobbyError(r.json.error ?? closedLobby(r.status, r.raw) ?? `HTTP ${r.status}`, r.status);
    return r.json.me;
  }

  /** Your own Claude and GitHub sign-ins (SignInsState), or null on the shared password, which has none. */
  async signins() {
    const r = await this.call('/api/signins');
    if (r.status === 409) return null;
    if (!r.ok) throw new LobbyError(r.json.error ?? closedLobby(r.status, r.raw) ?? `HTTP ${r.status}`, r.status);
    return r.json.state;
  }

  /**
   * Does what one of 🔐 Your sign-ins' buttons does: `what` is start, code, token, office, cancel or
   * signout. Resolves to { state, error? }: what didn't take is said, not thrown.
   */
  async signin(what, which, more = {}) {
    const r = await this.call('/api/signins', { do: what, which, ...more }, 60_000);
    if (r.json.state) return { state: r.json.state, error: r.json.error };
    throw new LobbyError(r.json.error ?? closedLobby(r.status, r.raw) ?? `HTTP ${r.status}`, r.status);
  }

  /** An invite link's token (admins): for `name` (or whoever opens it), as a member or an admin. */
  async invite(role, name, by) {
    const { invite } = await this.must('/api/accounts/invite', { role, name, by });
    return invite;
  }

  /** The key of a link that signs another browser in as you, once, within a few minutes. */
  async handoff() {
    const { key } = await this.must('/api/link/new', {});
    return key;
  }

  async logout() {
    await this.call('/api/logout', {}).catch(() => {});
    this.cookie = null;
  }
}

/** The key in a sign-in link (…/login#key=…), as the office on this computer hands it over. */
const linkKeyOf = (url) => /#key=([\w-]+)$/.exec(String(url ?? ''))?.[1];

module.exports = { Lobby, LobbyError, linkKeyOf, sessionCookie };
