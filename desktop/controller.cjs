// What the connector's window shows and does (ui/connector.js draws it): the server you're connected
// to (its address, its password and your name), your workers on this computer with its Claude and
// GitHub, your sign-ins on the server, and the office in your normal browser. There are no lobbies
// and no office on this computer: the office runs on the server (the Umbrel app „Agent Office
// Vermittler“), and every player connects to it. main.cjs hands it what it needs from Electron and
// from this computer, so the tests drive it with plain stand-ins.
const { Lobby } = require('./connector.cjs');
const { cleanName, lobbyOrigin, needsPassthrough, unreachable } = require('./lobby.cjs');
const { startPassthrough } = require('./passthrough.cjs');

/** Only https:// pages are opened for the server (Claude's and GitHub's sign-in pages). */
const httpsUrl = (url) => (typeof url === 'string' && /^https:\/\/[^\s]+$/.test(url) ? url : undefined);
const KINDS = ['claude', 'github'];

/**
 * `settings()` and `save(patch)` are the app's settings ({ server: { url, name }, password, session,
 * connect, ownWorkers }), `seal`/`unseal` keep the password and the session cookie encrypted at
 * rest, `openExternal` and `copy` are the browser and the clipboard, `onChange(state)` redraws the
 * window, and `here` is this computer: your workstation ({ start(origin, cookie), stop(), state() },
 * where your workers run) and its own sign-ins ({ look(which) → { status, who?, missing? },
 * claude(), github() to sign in in a console window, githubToken() }).
 */
function createController({ settings, save, seal = (s) => ({ p: s }), unseal = (v) => v?.p ?? null, openExternal, copy, here, onChange, passthroughPort, get = fetch }) {
  /** The server, signed in: a Lobby with your session. Null while not connected. */
  let lobby = null;
  let me = null;
  /** Your sign-ins on the server. */
  let signins = null;
  /** 'server' (the form), 'busy' or 'home'. */
  let view = 'server';
  /** The form is open while connected (⚙️ Einstellungen): Abbrechen goes back home. */
  let editing = false;
  let busy = '';
  let error = '';
  let passthrough = null;
  let timer = null;
  /** Claude's sign-in pages opened in the browser already: each opens by itself once. */
  const opened = new Set();
  /** This computer's own sign-ins, { claude?, github? }: the ones your workers here run with. */
  let mine = {};
  /** Sign-ins under way in a console window on this computer: since when, by kind. */
  const waiting = {};
  /** How long a console sign-in is waited for before the window stops looking every few seconds. */
  const WAIT_MS = 10 * 60_000;

  const saved = () => settings().server ?? null;
  const state = () => ({
    view,
    editing,
    busy,
    error,
    /** The server as saved: { url, name }, and whether its password is kept. */
    server: saved() && { url: saved().url, name: saved().name, password: !!settings().password },
    connected: !!lobby && !!me,
    me,
    signins,
    mine,
    waiting: { claude: !!waiting.claude, github: !!waiting.github },
    viaPassthrough: !!lobby && needsPassthrough(lobby.origin),
    passthrough: passthrough?.origin ?? null,
    // Deine Arbeiter: Mit deinem Konto auf dem Server laufen sie auf diesem Computer.
    workstation: here.workstation && lobby && me?.account ? { on: settings().ownWorkers !== false, stand: here.workstation.state()?.stand ?? null } : null,
  });
  const push = () => onChange(state());
  function show(next, extra = {}) {
    view = next;
    error = extra.error ?? '';
    busy = extra.busy ?? '';
    if (next !== 'home') clearTimeout(timer);
    if (next !== 'server') editing = false;
    push();
  }

  const open = (v) => {
    try {
      return unseal(v);
    } catch {
      // sealed on another computer, or by another user
      return null;
    }
  };

  /** The session cookie, kept for the next start. */
  const keepSession = () => save({ session: lobby?.cookie ? seal(lobby.cookie) : null });

  /**
   * Connects to the server at `origin` as `name`, with the session from before if it still counts,
   * else signing in with `password`. Resolves once home, or back at the form with what went wrong.
   */
  async function connect(origin, name, password, cookie = null) {
    clearTimeout(timer);
    show('busy', { busy: `Verbinde mit ${origin} …` });
    const why = await unreachable(origin, get);
    if (why) return leave(why);
    const l = new Lobby(origin, cookie, get);
    try {
      let who = cookie ? await l.whoami() : null;
      // The session from before is somebody else's (the name was changed meanwhile): sign in again.
      if (who && who.account?.name !== name) who = null;
      if (!who) {
        if (!password) return leave('Das Passwort des Servers fehlt.');
        await l.login(name, password);
        who = await l.whoami();
      }
      if (!who?.account) throw new Error('Die Anmeldung hat nicht geklappt. Versuch es noch einmal.');
      lobby = l;
      me = who;
      keepSession();
      save({ connect: true });
      await home();
    } catch (err) {
      leave(err.status === 401 ? 'Das Passwort des Servers stimmt nicht.' : err.message);
    }
  }

  async function home() {
    opened.clear();
    signins = await lobby.signins();
    syncWorkstation();
    await lookHere();
    show('home');
    afterLook();
  }

  /** Deine Arbeiter sollen hier laufen: verbunden, mit deinem Konto, und wenn du es nicht ausgeschaltet hast. */
  const workstationWanted = () => !!here.workstation && !!lobby && !!me?.account && !!lobby.cookie && settings().ownWorkers !== false;

  /** Startet oder beendet deinen Arbeitsplatz, wie es gerade sein soll (here.workstation, siehe main.cjs). */
  function syncWorkstation() {
    const ws = here.workstation;
    if (!ws) return;
    if (workstationWanted()) {
      ws.start(lobby.origin, lobby.cookie).catch((err) => {
        error = `Dein Arbeitsplatz startet nicht: ${err?.message ?? err}`;
        push();
      });
    } else void ws.stop();
  }

  /** This computer's sign-ins, as far as they matter: both while your workers run here, else GitHub's while the server has none of yours. */
  async function lookHere() {
    const kinds = workstationWanted() ? KINDS : signins?.github?.status !== 'ok' ? ['github'] : [];
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
    timer = setTimeout(refresh, live ? 1500 : inConsole ? 3000 : 15_000);
    timer.unref?.();
  }

  async function refresh() {
    if (view !== 'home' || !lobby) return;
    try {
      me = await lobby.whoami();
      if (!me?.account) {
        // The session stopped counting (the server restarted with a new key, say): sign in again with what's kept.
        const s = settings();
        return connect(lobby.origin, s.server?.name, open(s.password));
      }
      signins = await lobby.signins();
      await lookHere();
      error = '';
    } catch (err) {
      error = err.message;
    }
    afterLook();
  }

  /** Not connected (any more): your workers here stop, and the window shows the form, with `why`. */
  function leave(why) {
    clearTimeout(timer);
    lobby = null;
    me = null;
    signins = null;
    syncWorkstation();
    show('server', { error: why });
  }

  function signInHere(which) {
    waiting[which] = Date.now();
    here[which]();
    push();
    schedule();
  }

  /** The office in the browser, signed in as you: plain http elsewhere goes through the passthrough, for voice. */
  async function openOffice() {
    if (!lobby || !me) throw new Error('Verbinde dich zuerst mit dem Server.');
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
    /**
     * Mit dem Server verbinden: seine Adresse, sein Passwort (leer: das gespeicherte, solange es derselbe
     * Server ist) und dein Name. Beim ersten Mal legt der Server dabei dein Konto an.
     */
    async connect({ address, password, name }) {
      const origin = lobbyOrigin(text(address, 2048));
      if (!origin) return void show('server', { error: `„${text(address, 200)}“ ist keine Adresse, die der Connector versteht. Zum Beispiel: 100.78.107.162:4600 oder umbrel.local:4600` });
      const n = cleanName(name);
      if (!n) return void show('server', { error: 'Wie heißt du? Unter diesem Namen bist du im Office.' });
      const before = saved();
      const same = before?.url === origin;
      const pw = text(password, 512) || (same ? open(settings().password) : '');
      if (!pw) return void show('server', { error: 'Das Passwort des Servers fehlt (bei der Umbrel-App steht es auf ihrer Seite im App Store).' });
      // Another server or another name: whoever was connected before goes first, workers and all.
      if (lobby) leave();
      save({ server: { url: origin, name: n }, password: seal(pw), ...(same && before.name === n ? {} : { session: null }) });
      await connect(origin, n, pw, same && before.name === n ? open(settings().session) : null);
    },
    /** ⚙️ Einstellungen: das Formular, mit dem, was gespeichert ist. */
    settings() {
      editing = !!lobby;
      show('server');
    },
    /** Zurück aus den Einstellungen, ohne etwas zu ändern. */
    back() {
      if (!lobby || !me) return void show('server');
      show('home');
      schedule();
    },
    /** Verbindung trennen: Deine Arbeiter hier stoppen, und beim nächsten Start verbindet der Connector nicht von selbst. */
    disconnect() {
      save({ connect: false });
      leave();
      return passthrough?.close().then(() => (passthrough = null));
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
    /** Deine Arbeiter auf diesem Computer (an), oder auf dem Server (aus). */
    async ownWorkers({ on }) {
      save({ ownWorkers: on !== false });
      syncWorkstation();
      await lookHere();
      push();
    },
    /** This computer's own sign-in, in a console window: the window looks every few seconds until it's done. */
    hereClaude: () => signInHere('claude'),
    hereGithub: () => signInHere('github'),
    /** The GitHub CLI's sign-in on this computer, for your account on the server too. */
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
    /** Beim Start: wieder mit dem Server verbinden, wenn einer gespeichert ist und die Verbindung nicht getrennt wurde. */
    start() {
      const s = settings();
      if (s.server?.url && s.server.name && s.connect !== false && s.password) return connect(s.server.url, s.server.name, open(s.password), open(s.session));
      show('server');
    },
    /** Verbunden: { origin, name }, für die Menüs und das Symbol in der Menüleiste. */
    current: () => (lobby && me ? { origin: lobby.origin, name: me.account?.name ?? '' } : null),
    /** Ob ein Server gespeichert ist, mit dem „Verbinden“ gleich wieder geht. */
    canConnect: () => !!settings().server?.url && !!settings().password,
    /** Wieder verbinden, mit dem, was gespeichert ist (aus dem Menü). */
    reconnect() {
      const s = settings();
      if (!s.server?.url || !s.password) return show('server');
      return connect(s.server.url, s.server.name, open(s.password), open(s.session));
    },
    disconnect: () => actions.disconnect(),
    openOffice: () => actions.office(),
    /** Beim Arbeitsplatz hat sich etwas getan (siehe main.cjs): die Ansicht neu zeichnen. */
    changed: () => push(),
    close: () => {
      clearTimeout(timer);
      return passthrough?.close();
    },
  };
}

module.exports = { createController };
