// The connector's side of the server: the office on your Umbrel (the app „Agent Office Vermittler“)
// or another server. Signs in there with your name and the server's password (your own account,
// made the first time), and asks it for what the connector shows: who you are, your own Claude and
// GitHub sign-ins, and a one-time link that signs the normal browser in as you (see
// src/server/http/routes/connector.ts). Plain fetch, so it runs in Electron's main process and in
// the tests alike; the session is the office's own cookie, kept by the app between starts.

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
   * `origin` is the server's (http://100.78.107.162:4600), `cookie` a session from before (name=value).
   * Every request says it comes from the server's own origin, which the office's same-origin checks
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
      throw new LobbyError(`Der Server ${this.origin} ist nicht erreichbar (${why}).`, 0);
    }
    const set = sessionCookie(res.headers);
    if (set !== undefined) this.cookie = set;
    const json = await res.json().catch(() => ({}));
    return { status: res.status, ok: res.ok, json: json && typeof json === 'object' ? json : {} };
  }

  async must(path, body, ms) {
    const r = await this.call(path, body, ms);
    if (!r.ok) throw new LobbyError(r.json.error ?? `HTTP ${r.status}`, r.status);
    return r.json;
  }

  /** Your name and the server's password: your own account there, made the first time. */
  login(name, password) {
    return this.must('/api/login', { name, password });
  }

  /** Who the session signs in ({ account?, admin }), or null when it doesn't (any more). */
  async whoami() {
    if (!this.cookie) return null;
    const r = await this.call('/api/whoami');
    if (r.status === 401) {
      this.cookie = null;
      return null;
    }
    if (!r.ok) throw new LobbyError(r.json.error ?? `HTTP ${r.status}`, r.status);
    return r.json.me;
  }

  /** Your own Claude and GitHub sign-ins (SignInsState), or null on the shared password, which has none. */
  async signins() {
    const r = await this.call('/api/signins');
    if (r.status === 409) return null;
    if (!r.ok) throw new LobbyError(r.json.error ?? `HTTP ${r.status}`, r.status);
    return r.json.state;
  }

  /**
   * Does what one of 🔐 Your sign-ins' buttons does: `what` is start, code, token, office, cancel or
   * signout. Resolves to { state, error? }: what didn't take is said, not thrown.
   */
  async signin(what, which, more = {}) {
    const r = await this.call('/api/signins', { do: what, which, ...more }, 60_000);
    if (r.json.state) return { state: r.json.state, error: r.json.error };
    throw new LobbyError(r.json.error ?? `HTTP ${r.status}`, r.status);
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

module.exports = { Lobby, LobbyError, sessionCookie };
