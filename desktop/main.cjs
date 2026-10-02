// Agent Office as a Windows app: starts the office (with the node.exe the app ships, exactly like the
// `agent-office` command) on this computer only, signs its own window in, and offers Claude Code with
// the Claude subscription login, so neither the office's workers nor the "Claude Code" shortcut need
// an Anthropic API key.
const { app, BrowserWindow, Menu, dialog, desktopCapturer, session, shell } = require('electron');
const { spawn, execFile, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');

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
 * Packaged: resources/{office,node,claude,gh,tools}. From a checkout: the repository root, the
 * PATH's node and gh, and the scripts in desktop/.
 */
const paths = app.isPackaged
  ? {
      root: path.join(process.resourcesPath, 'office'),
      node: path.join(process.resourcesPath, 'node', WIN ? 'node.exe' : 'node'),
      claudeCmd: path.join(process.resourcesPath, 'claude', 'claude-code.cmd'),
      ghDir: path.join(process.resourcesPath, 'gh'),
      tools: path.join(process.resourcesPath, 'tools'),
    }
  : {
      root: path.join(__dirname, '..'),
      node: process.env.AGENT_OFFICE_NODE || 'node',
      claudeCmd: path.join(__dirname, 'claude', 'claude-code.cmd'),
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
};
function readSettings() {
  try {
    return { ...DEFAULTS, ...JSON.parse(fs.readFileSync(settingsFile, 'utf8')) };
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
 * Its PATH is the app's, plus what's been installed since (registryPath), plus the usual homes of
 * Claude Code, Git and the GitHub CLI, and last the gh.exe the app ships: a gh you installed
 * yourself comes first.
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
  for (const dir of registryPath()) add(dir);
  if (WIN) {
    const programFiles = process.env.ProgramFiles || 'C:\\Program Files';
    const local = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
    if (process.env.APPDATA) add(path.join(process.env.APPDATA, 'npm'));
    for (const dir of [path.join(programFiles, 'Git', 'cmd'), path.join(local, 'Programs', 'Git', 'cmd'), path.join(programFiles, 'GitHub CLI'), path.join(local, 'Programs', 'GitHub CLI')]) add(dir);
  }
  add(paths.ghDir);
  env[pathKey] = dirs.join(path.delimiter);
  return env;
}

/** Whether `cmd` (claude, git, gh) can be found the way the office will look for it. */
function installed(cmd) {
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

// --- The office ----------------------------------------------------------------------------------

let win = null;
let office = null;
let officeInfo = null;
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
      const probe = net.createServer();
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
  restarting = true;
  try {
    await stopOffice();
    showLoading('Agent Office wird neu gestartet …');
    await startOffice();
    await openOffice();
    checkTools();
  } catch (err) {
    showFailure(err);
  } finally {
    restarting = false;
  }
}

// --- Claude Code without an API key --------------------------------------------------------------

/**
 * Opens a console window running one of the app's .cmd scripts with the office's environment.
 * With `onExit`, that's called once the window is closed (start /wait).
 */
function openConsole(title, script, args = [], onExit) {
  if (!WIN) {
    dialog.showMessageBox(win, { message: `„${title}“ gibt es nur unter Windows.` });
    return;
  }
  const cwd = fs.existsSync(settings.home) ? settings.home : os.homedir();
  // cmd /s strips the outer quotes and keeps the rest verbatim: start "<title>" [/wait] "<script>" args…
  const line = `"start "${title}" ${onExit ? '/wait ' : ''}"${script}"${args.length ? ' ' + args.join(' ') : ''}"`;
  const child = spawn(process.env.COMSPEC || 'cmd.exe', ['/d', '/s', '/c', line], { cwd, env: childEnv(), detached: !onExit, stdio: 'ignore', windowsHide: true, windowsVerbatimArguments: true });
  if (onExit) child.on('exit', onExit);
  else child.unref();
}

/**
 * Claude Code through claude-code.cmd, which drops any API key for that window, installs Claude
 * Code if it's missing, and passes `args` on to claude.
 */
function openClaudeCode(args = []) {
  openConsole('Claude Code', paths.claudeCmd, args);
}

/** Signs gh in to GitHub in a console, then restarts the office so it sees the new sign-in. */
function githubLogin() {
  openConsole('Bei GitHub anmelden', path.join(paths.tools, 'github-login.cmd'), [], () => restartOffice());
}

/** Installs Git for Windows with winget in a console, then restarts the office so it finds git. */
function installGit() {
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
 * restart of the office (an install or sign-in finishing restarts it), unless declined.
 */
let checking = false;
async function checkTools() {
  if (checking || !win) return;
  checking = true;
  try {
    await askForTools();
  } finally {
    checking = false;
  }
}
async function askForTools() {
  if (!installed('git')) {
    const yes = await ask(
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
      'Damit sieht das Office deine Repositorys (im Aufzug: Projekt hinzufügen), klont sie und zeigt Issues und Pull Requests.\n\nJetzt anmelden? Es öffnet sich ein Konsolenfenster und GitHub im Browser; danach startet das Office neu.',
      'Bei GitHub anmelden',
    );
    if (yes) return githubLogin();
  }
  if (!installed('claude')) {
    const yes = await ask(
      'Claude Code fehlt',
      'Claude Code ist auf diesem Computer noch nicht installiert.',
      'Die Worker im Office laufen mit Claude Code. Du brauchst dafür keinen API-Key: Claude Code meldet sich mit deinem Claude-Abo (Pro oder Max) an.\n\n' +
        'Jetzt installieren und anmelden? Es öffnet sich ein Konsolenfenster mit dem offiziellen Installer; danach wählst du „Claude account with subscription“.',
      'Installieren und anmelden',
    );
    if (yes) openClaudeCode();
  }
}

// --- Window --------------------------------------------------------------------------------------

function page(title, body) {
  const html = `<!doctype html><meta charset="utf-8"><title>${title}</title>
<style>body{margin:0;height:100vh;display:grid;place-items:center;background:#1b1d2a;color:#e8e8f0;font:16px system-ui,sans-serif}
main{max-width:640px;padding:24px;text-align:center}h1{font-size:22px}pre{text-align:left;white-space:pre-wrap;font-size:12px;background:#0f1018;padding:12px;border-radius:8px;max-height:40vh;overflow:auto}
.dot{width:12px;height:12px;border-radius:50%;background:${ORANGE};display:inline-block;animation:p 1s infinite alternate}@keyframes p{to{opacity:.2}}</style>
<main>${body}</main>`;
  return `data:text/html;charset=utf-8,${encodeURIComponent(html)}`;
}
const escapeHtml = (s) => String(s).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);

function showLoading(text) {
  win?.loadURL(page('Agent Office', `<h1>🏢 ${escapeHtml(text)}</h1><span class="dot"></span>`));
}

function showFailure(err) {
  win?.loadURL(
    page(
      'Agent Office',
      `<h1>Agent Office konnte nicht starten</h1><p>${escapeHtml(err.message)}</p><pre>${escapeHtml(logLines.slice(-40).join('\n'))}</pre>` +
        '<p>Menü „Agent Office → Neu starten“ versucht es noch einmal.</p>',
    ),
  );
}

async function openOffice() {
  const url = await freshSignIn();
  if (url && win) await win.loadURL(url);
}

const isOffice = (url) => !!officeInfo && url.startsWith(officeInfo.url + '/');

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
    if (url.startsWith('data:') || isOffice(url)) return;
    e.preventDefault();
    if (/^https?:/i.test(url)) shell.openExternal(url);
  });
  win.on('closed', () => (win = null));
  buildMenu();
}

/** Microphone (voice), screen sharing, pointer lock (mouse-look), notifications: for the office only. */
function allowOfficePermissions() {
  const allowed = new Set(['media', 'pointerLock', 'fullscreen', 'notifications', 'clipboard-read', 'clipboard-sanitized-write', 'display-capture']);
  session.defaultSession.setPermissionRequestHandler((wc, permission, callback) => {
    callback(allowed.has(permission) && isOffice(wc.getURL()));
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
  const template = [
    {
      label: 'Agent Office',
      submenu: [
        { label: 'Neu laden', accelerator: 'F5', click: () => openOffice() },
        {
          label: 'Im Browser öffnen',
          click: async () => {
            const url = await freshSignIn();
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
        { role: 'quit', label: 'Beenden' },
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
        { label: 'Git for Windows installieren …', click: () => installGit() },
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
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

// --- Lifecycle -----------------------------------------------------------------------------------

app.whenReady().then(async () => {
  if (!primary) return;
  allowOfficePermissions();
  createWindow();
  showLoading('Agent Office startet …');
  try {
    await startOffice();
    await openOffice();
    checkTools();
  } catch (err) {
    showFailure(err);
  }
});

app.on('window-all-closed', () => app.quit());

app.on('before-quit', (e) => {
  if (quitting || !office) return;
  e.preventDefault();
  quitting = true;
  stopOffice().finally(() => app.quit());
});
