// Agent Office's connector, for Windows and macOS: a small window that connects you to a lobby (an
// office on your Umbrel or a server, or one it starts on this computer), signs you in there with
// your own account, and signs you in to your own Claude and GitHub for your workers. The office
// itself opens in your normal browser, signed in as you (see controller.cjs). The office this app
// starts on this computer runs with the node it ships, exactly like the `agent-office` command, with
// Claude Code on the Claude subscription login, so no worker needs an Anthropic API key.
const { app, BrowserWindow, Menu, Tray, clipboard, dialog, ipcMain, nativeImage, safeStorage, shell } = require('electron');
const { spawn, execFile, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const netSocket = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { DOCS } = require('./lobby.cjs');
const { createController } = require('./controller.cjs');
const { MAC, MAC_DIRS, installMacGit, loginShellPath, macGit, openTerminal, scriptName } = require('./platform.cjs');
const { inviteText, newPassword, shareAddresses } = require('./hosting.cjs');

const WIN = process.platform === 'win32';

// One connector per computer: starting the app again brings its window back.
const primary = app.requestSingleInstanceLock();
if (!primary) {
  app.quit();
} else {
  app.on('second-instance', () => showWindow());
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
  /** 'local': the office on this computer. 'lobby': the online lobby `lobby`. Null until one's picked. */
  mode: null,
  /** The online lobby last joined: its origin. */
  lobby: null,
  /** Online lobbies joined before, newest first: { name, url }. */
  lobbies: [],
  /** Your session in each lobby (its cookie), by origin, sealed with the system's keychain where there is one. */
  sessions: {},
  /** The office on this computer is open to the network (a lobby hosted here, see hosting.cjs), behind `sharePassword`. */
  share: false,
  sharePassword: null,
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
let tray = null;
let office = null;
let officeInfo = null;
let quitting = false;
/** The app is on its way out, so closing the window closes it instead of keeping it in the tray. */
let leaving = false;
let restarting = false;
const logLines = [];
const signInWaiters = new Map();
/** The office on this computer on its way up: asked for twice meanwhile, it still starts once. */
let starting = null;

function log(line) {
  logLines.push(line);
  if (logLines.length > 2000) logLines.splice(0, logLines.length - 2000);
}

function freePort(from, host) {
  return new Promise((resolve, reject) => {
    const tryPort = (port) => {
      if (port > from + 100) return reject(new Error(`no free port from ${from}`));
      const probe = netSocket.createServer();
      probe.once('error', () => tryPort(port + 1));
      probe.listen(port, host, () => probe.close(() => resolve(port)));
    };
    tryPort(from);
  });
}

async function startOffice() {
  // This computer only, or (a lobby hosted here) every network it's on, behind the lobby's password.
  const host = settings.share ? '0.0.0.0' : '127.0.0.1';
  const port = await freePort(Number(settings.port) || DEFAULTS.port, host);
  fs.mkdirSync(settings.home, { recursive: true });
  const args = [entry, '--home', settings.home, '--host', host, '--port', String(port), '--no-open'];
  const env = childEnv();
  // Through the environment, not the command line, where anyone on this computer could read it. The
  // office keeps its own password as it was, for when the lobby is closed again.
  if (settings.share) env.AGENT_OFFICE_PASSWORD = settings.sharePassword;
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
        officeInfo = null;
        const why = `Das Office auf diesem Computer wurde unerwartet beendet (Code ${code}).`;
        dialog.showErrorBox('Agent Office', `${why}\n\n${logLines.slice(-15).join('\n')}`);
        if (controller.isHere()) controller.leave(`${why} „Auf diesem Computer starten“ versucht es noch einmal; das Protokoll steht im Menü „Agent Office“.`);
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

/** Starts the office on this computer again, when it's the one in use. `first` runs once it's back. */
async function restartOffice(first) {
  if (restarting || quitting || settings.mode !== 'local') return;
  restarting = true;
  try {
    await stopOffice();
    officeInfo = null;
    // Something may have been installed meanwhile (Git, gh, Claude Code): look for it afresh.
    loginShellPath(true);
  } finally {
    restarting = false;
  }
  await controller.reconnectHere();
  await first?.();
}

/** The office on this computer, as the connector uses it (see controller.cjs). */
const here = {
  running: () => !!office,
  async start() {
    if (!office || !officeInfo) await (starting ??= startOffice().finally(() => (starting = null)));
    // What it needs from this computer, asked about once the window shows it (see checkTools).
    setTimeout(checkTools, 1500);
    return { origin: officeInfo.url, link: await freshSignIn() };
  },
  async stop() {
    restarting = true;
    await stopOffice().finally(() => (restarting = false));
    officeInfo = null;
  },
  /** Where friends reach it: nowhere until it's opened to the network (toggleShare). */
  inviteBase: () => (settings.share && officeInfo ? shareAddresses(Number(new URL(officeInfo.url).port))[0]?.url : undefined),
  claude: () => openClaudeCode(['auth', 'login', '--claudeai']),
  github: () => githubLogin(),
};

// --- A lobby hosted on this computer ------------------------------------------------------------------

/**
 * Opens the office on this computer to the network for friends to join (see hosting.cjs), or closes
 * it again: either way the office starts again, on 0.0.0.0 with the lobby's password or on
 * 127.0.0.1 as before. Opening it asks first, since whoever joins runs workers on this computer.
 */
async function toggleShare(on) {
  if (on) {
    const firewall = WIN
      ? 'Windows fragt gleich, ob „Node.js“ im Netzwerk erreichbar sein darf: für private Netzwerke zulassen (und dein WLAN in Windows als privates Netzwerk einstellen).'
      : 'Fragt macOS, ob eingehende Verbindungen erlaubt sind: „Erlauben“.';
    const { response } = await dialog.showMessageBox(win, {
      type: 'warning',
      title: 'Lobby für andere öffnen',
      message: 'Das Office auf diesem Computer für deine Mitspieler öffnen?',
      detail:
        'Wer die Adresse und das Passwort hat, kommt in dein Office und kann Worker einstellen, die auf diesem Computer Befehle ausführen und deine Projekte ändern. Gib beides nur Leuten, denen du das zutraust.\n\n' +
        `${firewall}\n\n` +
        'Die Worker aller Spieler laufen dann auf diesem Computer. Soll er geschont werden, hoste die Lobby auf einem Server (Umbrel-App).',
      buttons: ['Lobby öffnen', 'Abbrechen'],
      defaultId: 0,
      cancelId: 1,
    });
    if (response !== 0) return buildMenu();
    saveSettings({ share: true, sharePassword: settings.sharePassword || newPassword() });
  } else {
    saveSettings({ share: false });
  }
  buildMenu();
  await restartOffice(on ? showInvite : undefined);
}

/** Who to tell what: the addresses this computer has on its networks, and the lobby's password. */
async function showInvite() {
  if (!officeInfo || !settings.share) return;
  const text = inviteText(shareAddresses(Number(new URL(officeInfo.url).port)), settings.sharePassword);
  const { response } = await dialog.showMessageBox(win, {
    type: 'info',
    title: 'Mitspieler einladen',
    message: 'Deine Lobby ist offen',
    detail: `${text}\n\nMit dem Passwort kommen sie als Admin herein. Besser: ein Einladungslink für ein eigenes Konto, mit eigenem Claude und GitHub (im Connector unter „Mitspieler einladen“).`,
    buttons: ['Einladung kopieren', 'Schließen'],
    defaultId: 0,
    cancelId: 1,
  });
  if (response === 0) clipboard.writeText(text);
}


// --- The connector -------------------------------------------------------------------------------

/** A session cookie at rest: sealed with the system's keychain (Windows' DPAPI, the Mac's Keychain) where there is one. */
const seal = (cookie) => (safeStorage.isEncryptionAvailable() ? { e: safeStorage.encryptString(cookie).toString('base64') } : { p: cookie });
const unseal = (v) => (v?.e ? safeStorage.decryptString(Buffer.from(v.e, 'base64')) : (v?.p ?? null));

let menuFor = '';
const controller = createController({
  settings: () => settings,
  save: saveSettings,
  seal,
  unseal,
  openExternal: (url) => shell.openExternal(url),
  copy: (text) => clipboard.writeText(text),
  here,
  onChange(state) {
    win?.webContents.send('connector:state', state);
    // The menus say which lobby you're in: built again when that changes.
    const now = JSON.stringify(controller.current());
    if (now !== menuFor) buildMenu();
  },
});

/** Only the connector's own page (ui/) may ask the app for anything. */
const fromWindow = (e) => !!win && e.sender === win.webContents && !!e.senderFrame?.url.startsWith('file:');
ipcMain.handle('connector:state', (e) => (fromWindow(e) ? controller.state() : null));
ipcMain.handle('connector:do', (e, action, args) => (fromWindow(e) ? controller.do(String(action), args) : { error: 'Nicht erlaubt' }));

async function openInBrowser() {
  const r = await controller.do('office');
  if (r.error) dialog.showErrorBox('Agent Office', r.error);
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
 * restart of the office (an install or sign-in finishing restarts it), unless declined. Never for
 * an online lobby: its workers run on its own machine.
 */
let checking = false;
async function checkTools() {
  if (checking || !win || settings.mode !== 'local') return;
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
        'Oder tritt im Connector einer Online-Lobby bei: Dort laufen die Worker auf dem Server, nicht auf diesem Computer.',
      'Installieren und anmelden',
    );
    if (yes) openClaudeCode();
  }
}


// --- Window, tray and menus ----------------------------------------------------------------------

function createWindow() {
  win = new BrowserWindow({
    width: 600,
    height: 820,
    minWidth: 440,
    minHeight: 560,
    title: 'Agent Office',
    backgroundColor: '#1b1d2a',
    autoHideMenuBar: !MAC,
    webPreferences: { contextIsolation: true, sandbox: true, nodeIntegration: false, preload: path.join(__dirname, 'preload.cjs') },
  });
  // The window never leaves its own page: links (the guide, GitHub's token page) open in the browser.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/i.test(url)) shell.openExternal(url);
    return { action: 'deny' };
  });
  win.webContents.on('will-navigate', (e, url) => {
    e.preventDefault();
    if (/^https?:/i.test(url)) shell.openExternal(url);
  });
  // The office in the browser goes through this app (the passthrough, or the office running here),
  // so on Windows closing the window keeps it going in the tray. A Mac app keeps going in the Dock.
  win.on('close', (e) => {
    if (leaving || MAC || !(controller.passthroughRunning() || office)) return;
    e.preventDefault();
    win.hide();
    showTray(true);
  });
  win.on('closed', () => (win = null));
  win.loadFile(path.join(__dirname, 'ui', 'index.html'));
}

function showWindow() {
  if (!win) return createWindow();
  if (win.isMinimized()) win.restore();
  win.show();
  win.focus();
}

let toldTray = false;
/** The tray icon (Windows, Linux): the connector back, the office in the browser, or quit. `closed`: the window was just closed. */
function showTray(closed) {
  if (MAC) return;
  try {
    if (!tray) {
      tray = new Tray(nativeImage.createFromPath(path.join(__dirname, 'ui', 'tray.png')));
      tray.setToolTip('Agent Office');
      tray.on('click', () => showWindow());
    }
  } catch (err) {
    log(`no tray icon: ${err.message}`);
    return;
  }
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Connector anzeigen', click: () => showWindow() },
      { label: 'Office im Browser öffnen', enabled: !!controller.current(), click: () => openInBrowser() },
      { type: 'separator' },
      { label: 'Beenden', click: () => app.quit() },
    ]),
  );
  if (closed && WIN && !toldTray) {
    toldTray = true;
    tray.displayBalloon({ iconType: 'info', title: 'Agent Office läuft weiter', content: 'Dein Browser braucht den Connector fürs Office. Hier unten im Infobereich holst du ihn zurück oder beendest ihn.' });
  }
}

function buildMenu() {
  const now = controller.current();
  menuFor = JSON.stringify(now);
  const atHere = settings.mode === 'local';
  if (tray) showTray(false);
  const template = [
    {
      label: 'Agent Office',
      submenu: [
        ...(MAC ? [{ role: 'about', label: 'Über Agent Office' }, { type: 'separator' }] : []),
        { label: 'Office im Browser öffnen', accelerator: 'CmdOrCtrl+O', enabled: !!now, click: () => openInBrowser() },
        {
          label: 'Lobby wechseln …',
          accelerator: 'CmdOrCtrl+Shift+L',
          click: () => {
            showWindow();
            controller.do('lobbies');
          },
        },
        { label: 'Adresse der Lobby kopieren', enabled: !!now && !now.here, click: () => clipboard.writeText(now.origin) },
        { type: 'separator' },
        { label: 'Eine Lobby hosten (Umbrel-App) …', click: () => shell.openExternal(`${DOCS}online-lobby.md`) },
        {
          label: 'Protokoll anzeigen',
          click: () => {
            const file = path.join(app.getPath('userData'), 'office.log');
            fs.writeFileSync(file, logLines.join(os.EOL));
            shell.openPath(file);
          },
        },
        { type: 'separator' },
        ...(MAC ? [{ role: 'hide', label: 'Agent Office ausblenden' }, { role: 'hideOthers', label: 'Andere ausblenden' }, { type: 'separator' }] : []),
        { role: 'quit', label: 'Beenden' },
      ],
    },
    // Copy and paste with ⌘C / ⌘V need an Edit menu on a Mac.
    ...(MAC ? [{ role: 'editMenu', label: 'Bearbeiten' }] : []),
    {
      label: 'Dieser Computer',
      submenu: [
        {
          label: 'Office auf diesem Computer starten',
          type: 'checkbox',
          checked: atHere,
          click: () => {
            showWindow();
            controller.do('here');
          },
        },
        { label: 'Lobby auf diesem Computer für andere öffnen', type: 'checkbox', checked: settings.share, enabled: atHere, click: (item) => toggleShare(item.checked) },
        { label: 'Mitspieler einladen (Adresse und Passwort) …', enabled: atHere && settings.share, click: () => showInvite() },
        { label: 'Office neu starten', enabled: atHere, click: () => restartOffice() },
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
        { type: 'separator' },
        { label: 'Claude Code öffnen (ohne API-Key)', accelerator: 'CmdOrCtrl+Shift+C', click: () => openClaudeCode() },
        { label: 'Claude Code mit Claude-Abo anmelden …', click: () => openClaudeCode(['auth', 'login', '--claudeai']) },
        { label: 'Anmeldestatus prüfen', click: () => openClaudeCode(['auth', 'status']) },
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
        { label: 'GitHub CLI anmelden …', click: () => githubLogin() },
        { label: MAC ? 'Git installieren (Apple-Befehlszeilentools) …' : 'Git for Windows installieren …', click: () => installGit() },
        { label: 'Hilfe: Claude Code mit Abo nutzen', click: () => shell.openExternal('https://code.claude.com/docs/en/setup') },
      ],
    },
    {
      label: 'Ansicht',
      submenu: [
        { role: 'zoomIn', label: 'Vergrößern' },
        { role: 'zoomOut', label: 'Verkleinern' },
        { role: 'resetZoom', label: 'Originalgröße' },
        { type: 'separator' },
        { role: 'toggleDevTools', label: 'Entwicklertools' },
      ],
    },
    ...(MAC ? [{ role: 'windowMenu', label: 'Fenster' }] : []),
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

// --- Lifecycle -----------------------------------------------------------------------------------

app.whenReady().then(() => {
  if (!primary) return;
  buildMenu();
  createWindow();
  controller.start();
});

// Windows: closing the last window quits (unless the tray keeps it, see createWindow). A Mac app
// keeps going without one, and its Dock icon brings the window back.
app.on('window-all-closed', () => {
  if (!MAC) app.quit();
});
app.on('activate', () => showWindow());

app.on('before-quit', (e) => {
  leaving = true;
  if (quitting) return;
  if (!office) return void controller.close();
  e.preventDefault();
  quitting = true;
  Promise.all([stopOffice(), controller.close()]).finally(() => app.quit());
});
