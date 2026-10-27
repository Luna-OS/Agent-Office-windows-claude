// What the connector's window shows and does (ui/connector.js draws it): which lobby you're in,
// signing in there, your own Claude and GitHub sign-ins, invite links, and the office in your normal
// browser. main.cjs hands it what it needs from Electron and from the office on this computer, so
// the tests drive it with plain stand-ins.
const { Lobby, linkKeyOf } = require('./connector.cjs');
const { inviteOf, lobbyName, lobbyOrigin, needsPassthrough, unreachable } = require('./lobby.cjs');
const { startPassthrough } = require('./passthrough.cjs');

/** Only https:// pages are opened for a lobby (Claude's and GitHub's sign-in pages). */
const httpsUrl = (url) => (typeof url === 'string' && /^https:\/\/[^\s]+$/.test(url) ? url : undefined);
const KINDS = ['claude', 'github'];

/**
 * `settings()` and `save(patch)` are the app's settings (lobbies, mode, sessions), `seal`/`unseal`
 * keep a session cookie encrypted at rest, `openExternal` and `copy` are the browser and the
 * clipboard, `onChange(state)` redraws the window, and `here` is this computer: its office
 * ({ running(), start() → { origin, link }, stop(), inviteBase() → where friends reach it }) and its own
 * sign-ins ({ look(which) → { status, who?, missing? }, claude(), github() to sign in in a console
 * window, githubToken() }).
 */
function createController({ settings, save, seal = (s) => ({ p: s }), unseal = (v) => v?.p ?? null, openExternal, copy, here, onChange, passthroughPort, get = fetch }) {
  let lobby = null;
  /** The lobby is the office on this computer. */
  let local = false;
  let me = null;
  /** Your own sign-ins; null on the shared office password, which has none. */
  let signins = null;
  let options = null;
  /** The invite link being joined: { token, name?, role, by, project }. */
  let pending = null;
  let invited = null;
  let view = 'lobbies';
  let busy = '';
  let error = '';
  let address = '';
  let passthrough = null;
  let timer = null;
  /** Claude's sign-in pages opened in the browser already: each opens by itself once. */
  const opened = new Set();
  /**
   * This computer's own sign-ins, { claude?, github? }: what the office here runs on (on its
   * password), and GitHub's, to hand on to a lobby that doesn't have yours yet.
   */
  let mine = {};
  /** Sign-ins under way in a console window on this computer: since when, by kind. */
  const waiting = {};
  /** How long a console sign-in is waited for before the window stops looking every few seconds. */
  const WAIT_MS = 10 * 60_000;

  const state = () => {
    const s = settings();
    return {
      view,
      busy,
      error,
      address,
      lobbies: s.lobbies,
      lobby: lobby && { origin: lobby.origin, here: local, name: local ? 'Dieser Computer' : (s.lobbies.find((l) => l.url === lobby.origin)?.name ?? new URL(lobby.origin).host) },
      options,
      pending: pending && { name: pending.name, role: pending.role, by: pending.by, project: pending.project },
      me,
      signins,
      mine,
      waiting: { claude: !!waiting.claude, github: !!waiting.github },
      invited,
      viaPassthrough: !!lobby && needsPassthrough(lobby.origin),
      passthrough: passthrough?.origin ?? null,
      share: !!s.share,
    };
  };
  const push = () => onChange(state());
  function show(next, extra = {}) {
    view = next;
    error = extra.error ?? '';
    busy = extra.busy ?? '';
    address = extra.address ?? '';
    if (next !== 'home') clearTimeout(timer);
    push();
  }

  /** The session cookie, kept for the next start (not for the office here: that one hands the app a fresh link). */
  function keepSession() {
    if (!lobby || local) return;
    const sessions = { ...settings().sessions };
    if (lobby.cookie) sessions[lobby.origin] = seal(lobby.cookie);
    else delete sessions[lobby.origin];
    save({ sessions });
  }

  async function connect(input, name) {
    const origin = lobbyOrigin(input);
    if (!origin) return show('lobbies', { error: `„${input ?? ''}“ ist keine Adresse, die der Connector versteht. Zum Beispiel: 100.78.107.162:4600, umbrel.local:4600 oder https://office.example.com`, address: input ?? '' });
    const token = inviteOf(input)?.slice('/join#'.length);
    show('busy', { busy: `Verbinde mit ${origin} …` });
    const why = await unreachable(origin, get);
    if (why) return show('lobbies', { error: why, address: input });
    // The lobby's workers are the ones that run now.
    if (here.running()) await here.stop();
    const s = settings();
    const before = s.lobbies.find((l) => l.url === origin);
    save({ mode: 'lobby', lobby: origin, lobbies: [{ name: lobbyName(name || before?.name, origin), url: origin }, ...s.lobbies.filter((l) => l.url !== origin)].slice(0, 12) });
    let cookie = null;
    try {
      cookie = unseal(s.sessions?.[origin]);
    } catch {
      // sealed on another computer, or by another user: sign in again
    }
    return enter(new Lobby(origin, cookie, get), false, token);
  }

  /** The office on this computer, signed in with the link it hands the app. */
  async function connectHere() {
    save({ mode: 'local' });
    show('busy', { busy: 'Das Office startet auf diesem Computer …' });
    try {
      const { origin, link } = await here.start();
      const l = new Lobby(origin, null, get);
      const key = linkKeyOf(link);
      if (key) await l.useLink(key).catch(() => {});
      await enter(l, true);
    } catch (err) {
      show('lobbies', { error: `Das Office auf diesem Computer startet nicht: ${err.message}` });
    }
  }

  async function enter(l, isHere, token) {
    clearTimeout(timer);
    lobby = l;
    local = isHere;
    me = null;
    signins = null;
    options = null;
    pending = null;
    invited = null;
    opened.clear();
    try {
      options = await l.options();
      if (token) {
        pending = { token, ...(await l.peek(token)) };
        return show('signin');
      }
      me = await l.whoami();
      keepSession();
      if (!me) return show('signin');
      await home();
    } catch (err) {
      show(options ? 'signin' : 'lobbies', { error: err.message, address: l.origin });
    }
  }

  async function signedIn() {
    keepSession();
    me = await lobby.whoami();
    if (!me) throw new Error('Die Anmeldung hat nicht geklappt. Versuch es noch einmal.');
    await home();
  }

  async function home() {
    signins = me.account ? await lobby.signins() : null;
    await lookHere();
    show('home');
    afterLook();
  }

  /**
   * This computer's sign-ins, as far as they matter here: both, for the office here on its password
   * (they're the ones its workers use), and GitHub's, while a lobby account has none of its own yet.
   */
  async function lookHere() {
    const kinds = local && !me?.account ? KINDS : !local && me?.account && signins?.github?.status !== 'ok' ? ['github'] : [];
    const found = {};
    for (const k of kinds) found[k] = await here.look(k).catch(() => ({ status: 'none' }));
    mine = found;
    for (const k of KINDS) if (mine[k]?.status === 'ok' || Date.now() - (waiting[k] ?? 0) > WAIT_MS) delete waiting[k];
  }

  /** After a look at your sign-ins: Claude's sign-in page opens in the browser once, then the window's redrawn. */
  function afterLook() {
    const url = httpsUrl(signins?.claude?.pending?.url);
    if (url && !opened.has(url)) {
      opened.add(url);
      openExternal(url);
    }
    push();
    schedule();
  }

  /** Looks again soon: every few seconds while a sign-in is under way, else now and then. */
  function schedule() {
    clearTimeout(timer);
    if (view !== 'home' || !lobby) return;
    const live = signins && KINDS.some((k) => signins[k].pending || signins[k].status === 'busy');
    const inConsole = KINDS.some((k) => waiting[k]);
    timer = setTimeout(refresh, live ? 1500 : inConsole ? 3000 : local ? 30_000 : 15_000);
    timer.unref?.();
  }

  async function refresh() {
    if (view !== 'home' || !lobby) return;
    try {
      me = await lobby.whoami();
      if (!me) {
        keepSession();
        return show('signin', { error: 'Deine Anmeldung in dieser Lobby gilt nicht mehr. Melde dich wieder an.' });
      }
      signins = me.account ? await lobby.signins() : null;
      await lookHere();
      error = '';
    } catch (err) {
      error = err.message;
    }
    afterLook();
  }

  function leave(why) {
    clearTimeout(timer);
    lobby = null;
    local = false;
    me = null;
    show('lobbies', { error: why });
  }

  function signInHere(which) {
    waiting[which] = Date.now();
    here[which]();
    push();
    schedule();
  }

  const samePassword = (password, again) => {
    if (again !== undefined && password !== again) throw new Error('Die beiden Passwörter sind nicht gleich.');
  };

  /** The office in the browser, signed in as you: plain http elsewhere goes through the passthrough, for voice. */
  async function openOffice() {
    if (!lobby || !me) throw new Error('Melde dich zuerst in einer Lobby an.');
    const key = await lobby.handoff();
    let origin = lobby.origin;
    if (needsPassthrough(origin)) {
      if (passthrough) passthrough.retarget(origin);
      else passthrough = await startPassthrough(origin, passthroughPort);
      origin = passthrough.origin;
    }
    await openExternal(`${origin}/login#key=${key}`);
    push();
  }

  const text = (v, max = 1024) => String(v ?? '').slice(0, max);

  const actions = {
    connect: ({ address: a, name }) => connect(text(a, 2048), text(name, 40)),
    here: () => connectHere(),
    lobbies: () => leave(),
    forget({ url }) {
      const s = settings();
      const sessions = { ...s.sessions };
      delete sessions[url];
      save({ lobbies: s.lobbies.filter((l) => l.url !== url), sessions, ...(s.lobby === url ? { lobby: null, mode: null } : {}) });
      push();
    },
    async login({ name, password }) {
      await lobby.login(text(name, 64).trim(), text(password));
      await signedIn();
    },
    async join({ name, password, again }) {
      samePassword(password, again);
      if (!pending) throw new Error('Füge den Einladungslink noch einmal ein.');
      await lobby.join(pending.token, text(name, 64).trim() || pending.name, text(password));
      pending = null;
      await signedIn();
    },
    /** Your own admin account, made from the office password: an invite for you, used at once. */
    async account({ name, password, again }) {
      samePassword(password, again);
      const n = text(name, 24).trim();
      if (!n) throw new Error('Wie heißt du?');
      const v = await lobby.invite('admin', n, n);
      await lobby.join(v.token, n, text(password));
      await signedIn();
    },
    async signin({ what, which, code, token }) {
      const r = await lobby.signin(text(what, 16), which === 'github' ? 'github' : 'claude', { code: text(code, 4096), token: text(token, 4096) });
      signins = r.state;
      afterLook();
      if (r.error) throw new Error(r.error);
    },
    claudePage() {
      const url = httpsUrl(signins?.claude?.pending?.url);
      if (url) openExternal(url);
    },
    /** GitHub's one-time code onto the clipboard, and its page in the browser, to paste it there. */
    githubPage() {
      const p = signins?.github?.pending;
      if (p?.code) copy(p.code);
      const url = httpsUrl(p?.url);
      if (url) openExternal(url);
    },
    office: () => openOffice(),
    async invite({ role, name }) {
      const base = local ? here.inviteBase() : lobby.origin;
      if (!base) throw new Error('Das Office auf diesem Computer ist nur für dich offen. Öffne es zuerst für andere: Menü „Dieser Computer → Lobby für andere öffnen“.');
      const v = await lobby.invite(role === 'admin' ? 'admin' : 'member', text(name, 24).trim() || undefined, me?.account?.name);
      invited = { link: `${base}/join#${v.token}`, name: v.name, role: v.role };
      push();
    },
    copy: ({ text: t }) => copy(text(t, 4096)),
    async logout() {
      await lobby.logout();
      keepSession();
      me = null;
      signins = null;
      show('signin');
    },
    /** This computer's own sign-in, in a console window: the window looks every few seconds until it's done. */
    hereClaude: () => signInHere('claude'),
    hereGithub: () => signInHere('github'),
    /** The GitHub CLI's sign-in on this computer, for your account in the lobby too. */
    async githubFromHere() {
      const token = await here.githubToken();
      if (!token) throw new Error('Die GitHub CLI ist auf diesem Computer nicht angemeldet.');
      const r = await lobby.signin('token', 'github', { token });
      signins = r.state;
      await lookHere();
      afterLook();
      if (r.error) throw new Error(r.error);
    },
    refresh: () => refresh(),
  };

  return {
    state,
    /** Runs one of the window's actions: resolves to {} or { error }, never throws. */
    async do(action, args) {
      const fn = Object.hasOwn(actions, action) ? actions[action] : undefined;
      if (!fn) return { error: `Unbekannte Aktion: ${action}` };
      try {
        await fn(args && typeof args === 'object' ? args : {});
        return {};
      } catch (err) {
        push();
        return { error: err?.message ?? String(err) };
      }
    },
    /** Where the app was last: back in that lobby, or the office on this computer, or the list of lobbies. */
    start() {
      const s = settings();
      if (s.mode === 'lobby' && s.lobby) return connect(s.lobby);
      if (s.mode === 'local') return connectHere();
      show('lobbies');
    },
    /** Signed in to the office on this computer again (after it restarted). */
    reconnectHere: () => connectHere(),
    leave,
    /** In a lobby, signed in: { origin, here }, for the menus. */
    current: () => (lobby && me ? { origin: lobby.origin, here: local } : null),
    isHere: () => !!lobby && local,
    passthroughRunning: () => !!passthrough,
    openOffice: () => actions.office(),
    close: () => {
      clearTimeout(timer);
      return passthrough?.close();
    },
  };
}

module.exports = { createController };
