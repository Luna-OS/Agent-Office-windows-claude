// Agent Office as a desktop app, for Windows and macOS: starts the office (with the node it ships,
// exactly like the `agent-office` command) on this computer only and signs its own window in, or
// joins an online lobby, an office running on another machine (an Umbrel, a server), so the workers
// run there instead (see lobby.cjs). Either way it offers Claude Code with the Claude subscription
// login, so neither the office's workers nor the "Claude Code" shortcut need an Anthropic API key.
const { app, BrowserWindow, Menu, clipboard, dialog, desktopCapturer, net, session, shell, systemPreferences } = require('electron');
const { spawn, execFile, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const netSocket = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { APP_LINK, DOCS, escapeHtml, lobbyName, lobbyOrigin, needsSecureFlag, startPage, unreachable } = require('./lobby.cjs');
const { MAC, MAC_DIRS, installMacGit, loginShellPath, macGit, openTerminal, scriptName } = require('./platform.cjs');

const WIN = process.platform === 'win32';
const ORANGE = '#d97757';

// One office per computer: starting the app again brings its window back.
const primary = app.requestSingleInstanceLock();
if (!primary) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (!win) return;
    if (win.isMinimized()) win.restore();
    win.focus();
  });
}

// --- Where things are ------------------------------------------------------------------------------

/**
 * Packaged: resources/{office,node,claude,gh,tools} (Contents/Resources in the Mac app). From a
 * checkout: the repository root, the PATH's node and gh, and the scripts in desktop/.
 */
const paths = app.isPackaged
  ? {
      root: path.join(process.resourcesPath, 'office'),
      node: path.join(process.resourcesPath, 'node', WIN ? 'node.exe' : 'node'),
      claudeCmd: path.join(process.resourcesPath, 'claude', scriptName('claude-code')),
      ghDir: path.join(process.resourcesPath, 'gh'),
      tools: path.join(process.resourcesPath, 'tools'),
    }
  : {
      root: path.join(__dirname, '..'),
      node: process.env.AGENT_OFFICE_NODE || 'node',
      claudeCmd: path.join(__dirname, 'claude', scriptName('claude-code')),
      ghDir: null,
      tools: path.join(__dirname, 'tools'),
    };
const entry = app.isPackaged ? path.join(paths.root, 'office-entry.mjs') : path.join(__dirname, 'office-entry.mjs');

// --- Settings ------------------------------------------------------------------------------------

const settingsFile = path.join(app.getPath('userData'), 'settings.json');
const DEFAULTS = {
  /** First port tried; the next free one is taken when it's busy. */
  port: 4600,
  /** The office's data, as `agent-office --home`. */
  home: path.join(os.homedir(), 'agent-office'),
  /** Workers sign in to Claude with the subscription (Pro/Max) login, never an ANTHROPIC_API_KEY. */
  subscriptionOnly: true,
  /** 'local': the office runs on this computer. 'lobby': the window joins `lobby`. Null until picked on the start page. */
  mode: null,
  /** The online lobby last joined: its origin. */
  lobby: null,
  /** Online lobbies joined before, newest first: { name, url }. */
  lobbies: [],
};
function readSettings() {
  try {
    // An app from before lobbies always ran the office here, so that's where it carries on.
    return { ...DEFAULTS, mode: 'local', ...JSON.parse(fs.readFileSync(settingsFile, 'utf8')) };
  } catch {
    return { ...DEFAULTS };
  }
}
let settings = readSettings();
function saveSettings(patch) {
  settings = { ...settings, ...patch };
  fs.mkdirSync(path.dirname(settingsFile), { recursive: true });
  fs.writeFileSync(settingsFile, JSON.stringify(settings, null, 2));
}

// Voice and screen sharing only work on a secure origin. A lobby over plain http (an Umbrel at home
// or on Tailscale) is told to count as one, and that can only be said before the app is ready: a
// new one takes a restart of the app (see joinLobby).
const secureAtStart = new Set(settings.lobbies.map((l) => l.url).filter(needsSecureFlag));
if (secureAtStart.size) app.commandLine.appendSwitch('unsafely-treat-insecure-origin-as-secure', [...secureAtStart].join(','));

// --- Environment ---------------------------------------------------------------------------------

/** Windows environment names ignore case ("Path" is PATH); a plain copy of process.env doesn't. */
function envKey(env, name) {
  return Object.keys(env).find((k) => k.toUpperCase() === name) ?? name;
}

/**
 * The PATH Windows gives programs started from now on: the machine's and the user's from the
 * registry, so git, gh or claude installed while the app was running are found on the office's
 * next start. Empty off Windows, or when the registry can't be read.
 */
function registryPath() {
  if (!WIN) return [];
  const read = (key) => {
    try {
      const out = execFileSync('reg.exe', ['query', key, '/v', 'Path'], { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] });
      return out.match(/^\s*Path\s+REG_(?:EXPAND_)?SZ\s+(.*)$/im)?.[1].trim() ?? '';
    } catch {
      return '';
    }
  };
  // %SystemRoot%\..., %USERPROFILE%\...: process.env looks names up without regard to case on Windows.
  const expand = (p) => p.replace(/%([^%]+)%/g, (all, name) => process.env[name] ?? all);
  return [read('HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment'), read('HKCU\\Environment')]
    .flatMap((list) => list.split(';'))
    .map((p) => expand(p.trim()))
    .filter(Boolean);
}

/**
 * The environment the office (and so every worker it starts) runs with. Claude Code prefers an
 * ANTHROPIC_API_KEY over the subscription login, so with subscriptionOnly the API credentials are
 * left out and workers use whatever `claude` is signed in to.
 *
 * Its PATH is the app's, plus what's been installed since (registryPath on Windows, your login
 * shell's on a Mac), plus the usual homes of Claude Code, Git and the GitHub CLI, and last the gh
 * the app ships: a gh you installed yourself comes first.
 */
function childEnv() {
  const env = { ...process.env };
  for (const name of ['ELECTRON_RUN_AS_NODE', 'ELECTRON_NO_ATTACH_CONSOLE']) delete env[envKey(env, name)];
  if (settings.subscriptionOnly) {
    for (const name of ['ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'ANTHROPIC_BASE_URL', 'CLAUDE_CODE_USE_BEDROCK', 'CLAUDE_CODE_USE_VERTEX']) {
      delete env[envKey(env, name)];
    }
  }
  const pathKey = envKey(env, 'PATH');
  const dirs = (env[pathKey] || '').split(path.delimiter).filter(Boolean);
  const add = (dir, first = false) => {
    if (!dir || !fs.existsSync(dir) || dirs.some((d) => d.toLowerCase() === dir.toLowerCase())) return;
    if (first) dirs.unshift(dir);
    else dirs.push(dir);
  };
  add(path.join(os.homedir(), '.local', 'bin'), true);
  for (const dir of WIN ? registryPath() : loginShellPath()) add(dir);
  if (WIN) {
    const programFiles = process.env.ProgramFiles || 'C:\\Program Files';
    const local = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
    if (process.env.APPDATA) add(path.join(process.env.APPDATA, 'npm'));
    for (const dir of [path.join(programFiles, 'Git', 'cmd'), path.join(local, 'Programs', 'Git', 'cmd'), path.join(programFiles, 'GitHub CLI'), path.join(local, 'Programs', 'GitHub CLI')]) add(dir);
  }
  if (MAC) for (const dir of MAC_DIRS) add(dir);
  add(paths.ghDir);
  env[pathKey] = dirs.join(path.delimiter);
  return env;
}

/** Whether `cmd` (claude, git, gh) can be found the way the office will look for it. */
function installed(cmd) {
  if (MAC && cmd === 'git') return macGit(childEnv());
  try {
    execFileSync(WIN ? 'where.exe' : 'which', [cmd], { env: childEnv(), stdio: 'ignore', windowsHide: true });
    return true;
  } catch {
    return false;
  }
}

/** Whether gh is signed in to GitHub (`gh auth status` succeeds). */
function ghSignedIn() {
  return new Promise((resolve) => {
    execFile('gh', ['auth', 'status', '--hostname', 'github.com'], { env: childEnv(), timeout: 20_000, windowsHide: true }, (err) => resolve(!err));
  });
}

// --- The office on this computer -----------------------------------------------------------------

let win = null;
let office = null;
let officeInfo = null;
/** The online lobby the window is in (its origin), or null for the office on this computer. */
let lobby = null;
let quitting = false;
let restarting = false;
const logLines = [];
const signInWaiters = new Map();

function log(line) {
  logLines.push(line);
  if (logLines.length > 2000) logLines.splice(0, logLines.length - 2000);
}

function freePort(from) {
  return new Promise((resolve, reject) => {
    const tryPort = (port) => {
      if (port > from + 100) return reject(new Error(`no free port from ${from}`));
      const probe = netSocket.createServer();
      probe.once('error', () => tryPort(port + 1));
      probe.listen(port, '127.0.0.1', () => probe.close(() => resolve(port)));
    };
    tryPort(from);
  });
}

async function startOffice() {
  const port = await freePort(Number(settings.port) || DEFAULTS.port);
  fs.mkdirSync(settings.home, { recursive: true });
  const args = [entry, '--home', settings.home, '--host', '127.0.0.1', '--port', String(port), '--no-open'];
  const env = childEnv();
  if (!app.isPackaged) env.AGENT_OFFICE_ROOT = paths.root;
  log(`starting: ${paths.node} ${args.join(' ')}`);
  const child = spawn(paths.node, args, { cwd: settings.home, env, stdio: ['ignore', 'pipe', 'pipe', 'ipc'], windowsHide: true });
  office = child;
  officeInfo = null;
  child.stdout.setEncoding('utf8').on('data', (d) => d.split(/\r?\n/).filter(Boolean).forEach(log));
  child.stderr.setEncoding('utf8').on('data', (d) => d.split(/\r?\n/).filter(Boolean).forEach(log));
  return new Promise((resolve, reject) => {
    child.on('message', (msg) => {
      if (msg?.t === 'ready') {
        officeInfo = msg;
        resolve(msg);
      } else if (msg?.t === 'signIn') {
        signInWaiters.get(msg.id)?.(msg.url);
        signInWaiters.delete(msg.id);
      } else if (msg?.t === 'error') {
        reject(new Error(msg.message));
      }
    });
    child.on('error', reject);
    child.on('exit', (code) => {
      log(`office exited (${code})`);
      if (office === child) office = null;
      reject(new Error(`the office stopped (exit code ${code})`));
      if (!quitting && !restarting && officeInfo) {
        dialog.showErrorBox('Agent Office', `Das Office wurde unerwartet beendet (Code ${code}).\n\n${logLines.slice(-15).join('\n')}`);
        app.quit();
      }
    });
  });
}

/** A fresh one-time sign-in link from the running office. */
function freshSignIn() {
  if (!office || !officeInfo) return Promise.resolve(null);
  const id = Math.random().toString(36).slice(2);
  return new Promise((resolve) => {
    signInWaiters.set(id, resolve);
    office.send({ t: 'signIn', id });
    setTimeout(() => signInWaiters.delete(id) && resolve(officeInfo.url + '/'), 3000);
  });
}

function stopOffice() {
  const child = office;
  if (!child) return Promise.resolve();
  return new Promise((resolve) => {
    const done = setTimeout(() => {
      child.kill();
      resolve();
    }, 5000);
    child.once('exit', () => {
      clearTimeout(done);
      resolve();
    });
    try {
      child.send({ t: 'quit', keep: false });
    } catch {
      child.kill();
    }
  });
}

async function restartOffice() {
  if (restarting || quitting) return;
  if (lobby) return reloadOffice();
  restarting = true;
  try {
    await stopOffice();
    showLoading('Agent Office wird neu gestartet …');
    // Something may have been installed meanwhile (Git, gh, Claude Code): look for it afresh.
    loginShellPath(true);
    await startOffice();
    await openOffice();
    checkTools();
  } catch (err) {
    showFailure(err);
  } finally {
    restarting = false;
  }
}

/** Runs the office on this computer (again), and shows it. */
async function startLocal() {
  lobby = null;
  saveSettings({ mode: 'local' });
  buildMenu();
  if (office && officeInfo) return openOffice();
  showLoading('Agent Office startet …');
  try {
    await startOffice();
    await openOffice();
    checkTools();
  } catch (err) {
    showFailure(err);
  }
}

// --- Online lobbies --------------------------------------------------------------------------------

/**
 * Joins the online lobby at `input` (an address as typed, see lobbyOrigin): checks it's an office
 * that answers, keeps it in the list, stops the office on this computer (it's the lobby's workers
 * that run now) and shows the lobby, where you sign in with its password or your account.
 */
async function joinLobby(input, name) {
  const origin = lobbyOrigin(input);
  if (!origin) return showStart({ error: `„${input ?? ''}“ ist keine Adresse, die die App versteht. Zum Beispiel: umbrel.local:4600 oder https://office.example.com`, address: input ?? '' });
  showLoading(`Verbinde mit ${origin} …`);
  const why = await unreachable(origin, net.fetch);
  if (why) return showStart({ error: why, address: input });
  const before = settings.lobbies.find((l) => l.url === origin);
  const lobbies = [{ name: lobbyName(name || before?.name, origin), url: origin }, ...settings.lobbies.filter((l) => l.url !== origin)].slice(0, 12);
  saveSettings({ mode: 'lobby', lobby: origin, lobbies });
  if (office) {
    restarting = true;
    await stopOffice().finally(() => (restarting = false));
  }
  officeInfo = null;
  if (needsSecureFlag(origin) && !secureAtStart.has(origin)) {
    // So voice and screen sharing work there (see secureAtStart): back in a moment, in the lobby.
    showLoading('Agent Office startet für die neue Lobby neu …');
    app.relaunch();
    return app.exit(0);
  }
  lobby = origin;
  buildMenu();
  await win?.loadURL(`${origin}/`).catch(() => {});
}

function forgetLobby(url) {
  saveSettings({ lobbies: settings.lobbies.filter((l) => l.url !== url), ...(settings.lobby === url ? { lobby: null } : {}) });
  buildMenu();
}

/** Where to go: this computer or an online lobby (see lobby.cjs). */
function showStart(extra = {}) {
  buildMenu();
  win?.loadURL(page('Agent Office', startPage({ lobbies: settings.lobbies, current: lobby ?? settings.lobby, here: settings.mode !== 'lobby', ...extra })));
}

/** The start page's links and form (see lobby.cjs): they never load anything. */
function handleAppLink(raw) {
  const u = new URL(raw);
  const action = u.pathname.replace(/^\/+/, '');
  if (action === 'local') return startLocal();
  if (action === 'join') return joinLobby(u.searchParams.get('url'), u.searchParams.get('name'));
  if (action === 'forget') forgetLobby(u.searchParams.get('url'));
  showStart();
}

// --- Claude Code without an API key --------------------------------------------------------------

/**
 * Opens a console window (Windows) or a Terminal window (Mac) running one of the app's scripts with
 * the office's environment. With `onExit`, that's called once the script has finished.
 */
function openConsole(title, script, args = [], onExit) {
  const cwd = fs.existsSync(settings.home) ? settings.home : os.homedir();
  if (MAC) return openTerminal({ title, script, args, cwd, env: childEnv(), dir: path.join(app.getPath('userData'), 'terminal'), onExit });
  if (!WIN) {
    dialog.showMessageBox(win, { message: `„${title}“ gibt es nur unter Windows und macOS.` });
    return;
  }
  // cmd /s strips the outer quotes and keeps the rest verbatim: start "<title>" [/wait] "<script>" args…
  const line = `"start "${title}" ${onExit ? '/wait ' : ''}"${script}"${args.length ? ' ' + args.join(' ') : ''}"`;
  const child = spawn(process.env.COMSPEC || 'cmd.exe', ['/d', '/s', '/c', line], { cwd, env: childEnv(), detached: !onExit, stdio: 'ignore', windowsHide: true, windowsVerbatimArguments: true });
  if (onExit) child.on('exit', onExit);
  else child.unref();
}

/**
 * Claude Code through claude-code.cmd (claude-code.command on a Mac), which drops any API key for
 * that window, installs Claude Code if it's missing, and passes `args` on to claude.
 */
function openClaudeCode(args = []) {
  openConsole('Claude Code', paths.claudeCmd, args);
}

/** Signs gh in to GitHub in a console, then restarts the office so it sees the new sign-in. */
function githubLogin() {
  openConsole('Bei GitHub anmelden', path.join(paths.tools, scriptName('github-login')), [], () => restartOffice());
}

/**
 * Installs Git, then restarts the office so it finds it: Git for Windows with winget in a console,
 * or on a Mac Apple's Command Line Tools, which macOS asks about and installs by itself.
 */
function installGit() {
  if (MAC) return installMacGit(childEnv(), () => restartOffice());
  openConsole('Git installieren', path.join(paths.tools, 'install-git.cmd'), [], () => restartOffice());
}

/** Questions answered with "Später": not asked again until the app is started again. */
const declined = new Set();
async function ask(title, message, detail, yes) {
  if (declined.has(title)) return false;
  const { response } = await dialog.showMessageBox(win, { type: 'question', title, message, detail, buttons: [yes, 'Später'], defaultId: 0, cancelId: 1 });
  if (response !== 0) declined.add(title);
  return response === 0;
}

/**
 * What the office needs from this computer, asked about once it's open: Git (projects, worktrees,
 * and Claude Code's Git Bash), a GitHub sign-in for gh (the elevator's repository list, cloning,
 * issues and PRs) and Claude Code itself. One at a time, each skippable; asked again after every
 * restart of the office (an install or sign-in finishing restarts it), unless declined. Never in
 * an online lobby: its workers run on its own machine.
 */
let checking = false;
async function checkTools() {
  if (checking || !win || lobby) return;
  checking = true;
  try {
    await askForTools();
  } finally {
    checking = false;
  }
}
async function askForTools() {
  if (!installed('git')) {
    const yes = MAC
      ? await ask(
          'Git fehlt',
          'Git ist auf diesem Mac noch nicht installiert.',
          'Agent Office braucht Git für Projekte und die Arbeitskopien der Worker. Es kommt mit Apples Befehlszeilentools (Command Line Tools).\n\nJetzt installieren? macOS fragt nach und installiert sie; danach startet das Office neu.',
          'Git installieren',
        )
      : await ask(
          'Git fehlt',
          'Git for Windows ist auf diesem Computer noch nicht installiert.',
          'Agent Office braucht Git für Projekte und die Arbeitskopien der Worker, und Claude Code braucht es unter Windows auch.\n\nJetzt mit winget installieren? Danach startet das Office neu.',
          'Git installieren',
        );
    if (yes) return installGit();
  }
  if (installed('gh') && !(await ghSignedIn())) {
    const yes = await ask(
      'Bei GitHub anmelden',
      'Die GitHub CLI ist noch nicht bei GitHub angemeldet.',
      `Damit sieht das Office deine Repositorys (im Aufzug: Projekt hinzufügen), klont sie und zeigt Issues und Pull Requests.\n\nJetzt anmelden? Es öffnet sich ein ${MAC ? 'Terminal-Fenster' : 'Konsolenfenster'} und GitHub im Browser; danach startet das Office neu.`,
      'Bei GitHub anmelden',
    );
    if (yes) return githubLogin();
  }
  if (!installed('claude')) {
    const yes = await ask(
      'Claude Code fehlt',
      'Claude Code ist auf diesem Computer noch nicht installiert.',
      'Die Worker im Office laufen mit Claude Code. Du brauchst dafür keinen API-Key: Claude Code meldet sich mit deinem Claude-Abo (Pro oder Max) an.\n\n' +
        `Jetzt installieren und anmelden? Es öffnet sich ein ${MAC ? 'Terminal-Fenster' : 'Konsolenfenster'} mit dem offiziellen Installer; danach wählst du „Claude account with subscription“.\n\n` +
        'Oder spiel in einer Online-Lobby (Menü „Online-Lobby“): Dort laufen die Worker auf dem Server, nicht auf diesem Computer.',
      'Installieren und anmelden',
    );
    if (yes) openClaudeCode();
  }
}

// --- Window --------------------------------------------------------------------------------------

function page(title, body) {
  const html = `<!doctype html><meta charset="utf-8"><title>${title}</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#1b1d2a;color:#e8e8f0;font:16px system-ui,sans-serif}
main{max-width:640px;padding:24px;text-align:center}h1{font-size:22px}pre{text-align:left;white-space:pre-wrap;font-size:12px;background:#0f1018;padding:12px;border-radius:8px;max-height:40vh;overflow:auto}
.dot{width:12px;height:12px;border-radius:50%;background:${ORANGE};display:inline-block;animation:p 1s infinite alternate}@keyframes p{to{opacity:.2}}</style>
<main>${body}</main>`;
  return `data:text/html;charset=utf-8,${encodeURIComponent(html)}`;
}

function showLoading(text) {
  win?.loadURL(page('Agent Office', `<h1>🏢 ${escapeHtml(text)}</h1><span class="dot"></span>`));
}

function showFailure(err) {
  win?.loadURL(
    page(
      'Agent Office',
      `<h1>Agent Office konnte nicht starten</h1><p>${escapeHtml(err.message)}</p><pre>${escapeHtml(logLines.slice(-40).join('\n'))}</pre>` +
        `<p>Menü „Agent Office → Neu starten“ versucht es noch einmal, oder <a style="color:#f0a487" href="${APP_LINK}start">tritt einer Online-Lobby bei</a>.</p>`,
    ),
  );
}

/** Shows the office: this computer's, signed in with a fresh link, or the lobby's. */
async function openOffice() {
  if (lobby) return reloadOffice();
  const url = await freshSignIn();
  if (url && win) await win.loadURL(url);
}

function reloadOffice() {
  if (lobby) return win?.loadURL(`${lobby}/`).catch(() => {});
  return openOffice();
}

/** The office the window shows: the lobby's origin, or this computer's. */
const officeBase = () => lobby ?? officeInfo?.url ?? null;
const isOffice = (url) => {
  const base = officeBase();
  return !!base && (url === base || url.startsWith(base + '/'));
};

function createWindow() {
  win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 900,
    minHeight: 600,
    title: 'Agent Office',
    backgroundColor: '#1b1d2a',
    autoHideMenuBar: false,
    webPreferences: { contextIsolation: true, sandbox: true, nodeIntegration: false },
  });
  // Links out of the office (GitHub, PRs, docs) open in the normal browser; the office's own pages
  // (a doc or a terminal popped out) get a window of their own.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (isOffice(url)) return { action: 'allow' };
    if (/^https?:/i.test(url)) shell.openExternal(url);
    return { action: 'deny' };
  });
  win.webContents.on('will-navigate', (e, url) => {
    if (url.startsWith(APP_LINK)) {
      e.preventDefault();
      // Only from the app's own pages: a lobby's page never gets to switch lobbies or start the office here.
      if (win.webContents.getURL().startsWith('data:')) handleAppLink(url);
      return;
    }
    if (url.startsWith('data:') || isOffice(url)) return;
    e.preventDefault();
    if (/^https?:/i.test(url)) shell.openExternal(url);
  });
  // The lobby went away (the Umbrel is off, the network's gone): back to where to go, saying so.
  win.webContents.on('did-fail-load', (e, code, description, url, mainFrame) => {
    if (!mainFrame || !lobby || code === -3 || !url.startsWith(lobby)) return;
    showStart({ error: `Die Lobby ${lobby} ist nicht erreichbar (${description}).`, address: lobby });
  });
  win.on('closed', () => (win = null));
  buildMenu();
}

/** Microphone (voice), screen sharing, pointer lock (mouse-look), notifications: for the office only. */
function allowOfficePermissions() {
  const allowed = new Set(['media', 'pointerLock', 'fullscreen', 'notifications', 'clipboard-read', 'clipboard-sanitized-write', 'display-capture']);
  session.defaultSession.setPermissionRequestHandler((wc, permission, callback) => {
    if (!allowed.has(permission) || !isOffice(wc.getURL())) return callback(false);
    // A Mac asks once whether the app may use the microphone, for voice.
    if (MAC && permission === 'media' && systemPreferences.getMediaAccessStatus('microphone') !== 'granted') {
      return systemPreferences.askForMediaAccess('microphone').then(callback, () => callback(false));
    }
    callback(true);
  });
  session.defaultSession.setPermissionCheckHandler((wc, permission) => allowed.has(permission) && !!wc && isOffice(wc.getURL()));
  session.defaultSession.setDisplayMediaRequestHandler(
    (request, callback) => {
      desktopCapturer
        .getSources({ types: ['screen'] })
        .then((sources) => callback(sources.length ? { video: sources[0] } : {}))
        .catch(() => callback({}));
    },
    { useSystemPicker: true },
  );
}

function buildMenu() {
  const inLobby = settings.mode === 'lobby' && !!settings.lobby;
  const template = [
    {
      label: 'Agent Office',
      submenu: [
        ...(MAC ? [{ role: 'about', label: 'Über Agent Office' }, { type: 'separator' }] : []),
        { label: 'Neu laden', accelerator: 'F5', click: () => reloadOffice() },
        {
          label: 'Im Browser öffnen',
          click: async () => {
            const url = lobby ? `${lobby}/` : await freshSignIn();
            if (url) shell.openExternal(url);
          },
        },
        { type: 'separator' },
        { label: 'Daten-Ordner öffnen', click: () => shell.openPath(settings.home) },
        {
          label: 'Daten-Ordner ändern …',
          click: async () => {
            const r = await dialog.showOpenDialog(win, { title: 'Wo soll Agent Office seine Daten speichern?', defaultPath: settings.home, properties: ['openDirectory', 'createDirectory'] });
            if (r.canceled || !r.filePaths[0]) return;
            saveSettings({ home: r.filePaths[0] });
            restartOffice();
          },
        },
        {
          label: 'Protokoll anzeigen',
          click: () => {
            const file = path.join(app.getPath('userData'), 'office.log');
            fs.writeFileSync(file, logLines.join(os.EOL));
            shell.openPath(file);
          },
        },
        { label: 'Neu starten', click: () => restartOffice() },
        { type: 'separator' },
        ...(MAC ? [{ role: 'hide', label: 'Agent Office ausblenden' }, { role: 'hideOthers', label: 'Andere ausblenden' }, { type: 'separator' }] : []),
        { role: 'quit', label: 'Beenden' },
      ],
    },
    // Copy and paste with ⌘C / ⌘V need an Edit menu on a Mac. Not on Windows, where Ctrl+C belongs to the terminals.
    ...(MAC ? [{ role: 'editMenu', label: 'Bearbeiten' }] : []),
    {
      label: 'Online-Lobby',
      submenu: [
        { label: 'Lobby beitreten oder wechseln …', accelerator: 'CmdOrCtrl+Shift+L', click: () => showStart() },
        ...(settings.lobbies.length ? [{ type: 'separator' }] : []),
        ...settings.lobbies.map((l) => ({ label: `${l.name}  (${l.url})`, type: 'checkbox', checked: inLobby && settings.lobby === l.url, click: () => joinLobby(l.url) })),
        { type: 'separator' },
        { label: 'Auf diesem Computer (lokales Office)', type: 'checkbox', checked: !inLobby, click: () => startLocal() },
        { type: 'separator' },
        { label: 'Adresse der Lobby kopieren', enabled: inLobby, click: () => clipboard.writeText(settings.lobby) },
        { label: 'Eine Lobby hosten (Umbrel-App) …', click: () => shell.openExternal(`${DOCS}online-lobby.md`) },
      ],
    },
    {
      label: 'Claude Code',
      submenu: [
        { label: 'Claude Code öffnen (ohne API-Key)', accelerator: 'CmdOrCtrl+Shift+C', click: () => openClaudeCode() },
        { label: 'Mit Claude-Abo anmelden …', click: () => openClaudeCode(['auth', 'login', '--claudeai']) },
        { label: 'Anmeldestatus prüfen', click: () => openClaudeCode(['auth', 'status']) },
        { type: 'separator' },
        {
          label: 'Worker nur mit Claude-Abo (API-Key ignorieren)',
          type: 'checkbox',
          checked: settings.subscriptionOnly,
          click: (item) => {
            saveSettings({ subscriptionOnly: item.checked });
            restartOffice();
          },
        },
        { type: 'separator' },
        { label: 'Hilfe: Claude Code mit Abo nutzen', click: () => shell.openExternal('https://code.claude.com/docs/en/setup') },
      ],
    },
    {
      label: 'GitHub',
      submenu: [
        { label: 'Bei GitHub anmelden …', click: () => githubLogin() },
        { label: MAC ? 'Git installieren (Apple-Befehlszeilentools) …' : 'Git for Windows installieren …', click: () => installGit() },
      ],
    },
    {
      label: 'Ansicht',
      submenu: [
        { role: 'zoomIn', label: 'Vergrößern' },
        { role: 'zoomOut', label: 'Verkleinern' },
        { role: 'resetZoom', label: 'Originalgröße' },
        { type: 'separator' },
        { role: 'togglefullscreen', label: 'Vollbild' },
        { role: 'toggleDevTools', label: 'Entwicklertools' },
      ],
    },
    ...(MAC ? [{ role: 'windowMenu', label: 'Fenster' }] : []),
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

// --- Lifecycle -----------------------------------------------------------------------------------

app.whenReady().then(async () => {
  if (!primary) return;
  allowOfficePermissions();
  createWindow();
  if (settings.mode === 'lobby' && settings.lobby) return joinLobby(settings.lobby);
  if (settings.mode === 'local') return startLocal();
  // First start: where should the office run?
  showStart();
});

app.on('window-all-closed', () => app.quit());

app.on('before-quit', (e) => {
  if (quitting || !office) return;
  e.preventDefault();
  quitting = true;
  stopOffice().finally(() => app.quit());
});
