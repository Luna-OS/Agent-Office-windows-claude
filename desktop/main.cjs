// Agent Office's connector, for Windows and macOS: a small window that connects you to your server
// (the Umbrel app „Agent Office Vermittler“, where the office runs) with its address, its password
// and your name, and signs you in to your own Claude and GitHub for your workers. The office itself
// opens in your normal browser, signed in as you (see controller.cjs). The workers you hire run on
// this computer, in your workstation: an office in the background (src/server/arbeitsplatz) with
// the node this app ships, Claude Code on the Claude subscription login, so no worker needs an
// Anthropic API key. Closing the window (or minimizing it on a Mac) keeps the app going in the menu
// bar (macOS) or the tray (Windows), where it opens the office or the settings, or ends the connection.
const { app, BrowserWindow, Menu, Tray, clipboard, dialog, ipcMain, nativeImage, safeStorage, shell } = require('electron');
const { spawn, exec, execFile, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const netSocket = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { DOCS } = require('./lobby.cjs');
const { createController } = require('./controller.cjs');
const { githubToken, lookClaude, lookGithub } = require('./local-signins.cjs');
const { MAC, MAC_DIRS, installMacGit, loginShellPath, macGit, openTerminal, scriptName } = require('./platform.cjs');

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
  /** Your workstation's data, as `agent-office --home` (in <home>/arbeitsplatz). */
  home: path.join(os.homedir(), 'agent-office'),
  /** Workers sign in to Claude with the subscription (Pro/Max) login, never an ANTHROPIC_API_KEY. */
  subscriptionOnly: true,
  /** Der Server: { url (Origin), name (dein Name dort) }. Null, bis du dich zum ersten Mal verbindest. */
  server: null,
  /** Das Passwort des Servers, versiegelt mit dem Schlüsselbund des Systems, wo es einen gibt. */
  password: null,
  /** Deine Sitzung auf dem Server (ihr Cookie), ebenso versiegelt. */
  session: null,
  /** Beim Start von selbst verbinden; aus, nachdem du die Verbindung getrennt hast. */
  connect: true,
  /** Deine Arbeiter laufen auf diesem Computer (dein Arbeitsplatz), nicht auf dem Server. */
  ownWorkers: true,
};
function readSettings() {
  try {
    const saved = JSON.parse(fs.readFileSync(settingsFile, 'utf8'));
    // Aus der Zeit mit Lobbys: deren letzte ist jetzt der Server (Passwort und Name fragt das Fenster).
    if (!saved.server && typeof saved.lobby === 'string') saved.server = { url: saved.lobby, name: '' };
    for (const old of ['mode', 'lobby', 'lobbies', 'sessions', 'share', 'sharePassword', 'relay', 'port']) delete saved[old];
    return { ...DEFAULTS, ...saved };
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
 * registry, so git, gh or claude installed while the app was running are found on the workstation's
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
 * The environment your workstation (and so every worker it starts) runs with. Claude Code prefers
 * an ANTHROPIC_API_KEY over the subscription login, so with subscriptionOnly the API credentials
 * are left out and workers use whatever `claude` is signed in to.
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

/** Whether `cmd` (claude, git, gh) can be found the way your workstation will look for it. */
function installed(cmd) {
  if (MAC && cmd === 'git') return macGit(childEnv());
  try {
    execFileSync(WIN ? 'where.exe' : 'which', [cmd], { env: childEnv(), stdio: 'ignore', windowsHide: true });
    return true;
  } catch {
    return false;
  }
}

/**
 * Runs claude or gh the way your workstation would find them, for what they say: { code, out, missing }.
 * Through the shell on Windows, where an npm-installed claude is a .cmd (the arguments are the app's own).
 */
function runTool(cmd, args) {
  if (!installed(cmd)) return Promise.resolve({ code: 127, out: '', missing: true });
  const opts = { env: childEnv(), timeout: 20_000, windowsHide: true };
  const done = (resolve) => (err, out) => resolve({ code: err ? 1 : 0, out: String(out ?? '') });
  return new Promise((resolve) => (WIN ? exec([cmd, ...args].join(' '), opts, done(resolve)) : execFile(cmd, args, opts, done(resolve))));
}

/** Whether gh is signed in to GitHub (`gh auth status` succeeds). */
function ghSignedIn() {
  return new Promise((resolve) => {
    execFile('gh', ['auth', 'status', '--hostname', 'github.com'], { env: childEnv(), timeout: 20_000, windowsHide: true }, (err) => resolve(!err));
  });
}

// --- Your workstation ------------------------------------------------------------------------------

let win = null;
let tray = null;
/** The app is on its way out, so closing the window closes it instead of keeping it in the menu bar. */
let leaving = false;
let quitting = false;
const logLines = [];

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

/** Das Office im Hintergrund, in dem die Arbeiter laufen, die du auf dem Server einstellst (src/server/arbeitsplatz). */
let workstation = null;
/** Für welchen Server, mit welcher Sitzung, und wie es dort steht: { lobby, cookie, stand }. */
let workstationState = null;

/**
 * Startet deinen Arbeitsplatz für den Server `origin`, angemeldet mit deiner Sitzung dort (`cookie`):
 * ein eigenes Office nur auf diesem Computer (127.0.0.1), mit eigenen Daten unter <home>/arbeitsplatz.
 */
async function startWorkstation(origin, cookie) {
  if (workstation && workstationState?.lobby === origin && workstationState?.cookie === cookie) return;
  await stopWorkstation();
  const home = path.join(settings.home, 'arbeitsplatz');
  fs.mkdirSync(home, { recursive: true });
  const port = await freePort(4700, '127.0.0.1');
  const args = [entry, '--home', home, '--host', '127.0.0.1', '--port', String(port), '--no-open', '--arbeitsplatz', origin];
  const env = childEnv();
  // Die Sitzung über die Umgebung, nicht über die Kommandozeile, wo jeder auf diesem Computer sie lesen könnte.
  env.AGENT_OFFICE_ARBEITSPLATZ_SITZUNG = cookie;
  if (!app.isPackaged) env.AGENT_OFFICE_ROOT = paths.root;
  log(`arbeitsplatz: ${paths.node} ${args.join(' ')}`);
  const child = spawn(paths.node, args, { cwd: home, env, stdio: ['ignore', 'pipe', 'pipe', 'ipc'], windowsHide: true });
  workstation = child;
  workstationState = { lobby: origin, cookie, stand: { stand: 'verbinde' } };
  const prefixed = (d) => d.split(/\r?\n/).filter(Boolean).forEach((l) => log(`arbeitsplatz: ${l}`));
  child.stdout.setEncoding('utf8').on('data', prefixed);
  child.stderr.setEncoding('utf8').on('data', prefixed);
  child.on('message', (msg) => {
    if (msg?.t !== 'arbeitsplatz' || workstation !== child) return;
    workstationState = { lobby: origin, cookie, stand: msg.stand ?? { stand: 'verbinde' } };
    controller.changed();
  });
  child.on('exit', (code) => {
    if (workstation !== child) return;
    workstation = null;
    workstationState = { lobby: origin, cookie, stand: { stand: 'getrennt', grund: `Dein Arbeitsplatz wurde beendet (Code ${code})` } };
    controller.changed();
  });
  controller.changed();
  // What your workers need from this computer, asked about once the window shows it (see checkTools).
  setTimeout(checkTools, 1500);
}

function stopWorkstation() {
  const child = workstation;
  workstation = null;
  workstationState = null;
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

/** Startet deinen Arbeitsplatz neu, wenn er läuft: Er findet dann, was inzwischen installiert oder angemeldet wurde. */
async function restartWorkstation() {
  const was = workstationState;
  if (!was) return;
  await stopWorkstation();
  // Something may have been installed meanwhile (Git, gh, Claude Code): look for it afresh.
  loginShellPath(true);
  await startWorkstation(was.lobby, was.cookie);
}

/** This computer, as the connector uses it (see controller.cjs). */
const here = {
  /** Dein Arbeitsplatz auf dem Server (siehe startWorkstation). */
  workstation: { start: startWorkstation, stop: stopWorkstation, state: () => workstationState },
  /** This computer's own sign-ins (see local-signins.cjs), and signing in in a console window. */
  look: (which) => (which === 'github' ? lookGithub(runTool) : lookClaude(runTool)),
  claude: () => openClaudeCode(['auth', 'login', '--claudeai']),
  github: () => githubLogin(),
  githubToken: () => githubToken(runTool),
};

// --- The connector -------------------------------------------------------------------------------

/** A password or session cookie at rest: sealed with the system's keychain (Windows' DPAPI, the Mac's Keychain) where there is one. */
const seal = (secret) => (safeStorage.isEncryptionAvailable() ? { e: safeStorage.encryptString(secret).toString('base64') } : { p: secret });
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
    // The menus and the menu bar say whether you're connected, and where: built again when that changes.
    const now = JSON.stringify([controller.current(), controller.canConnect()]);
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

/** Die Einstellungen: das Fenster, mit dem Formular des Servers. */
function openSettings() {
  showWindow();
  void controller.do('settings');
}

// --- Claude Code without an API key --------------------------------------------------------------

/**
 * Opens a console window (Windows) or a Terminal window (Mac) running one of the app's scripts with
 * your workstation's environment. With `onExit`, that's called once the script has finished.
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

/** Signs gh in to GitHub in a console, then restarts your workstation so it sees the new sign-in. */
function githubLogin() {
  openConsole('Bei GitHub anmelden', path.join(paths.tools, scriptName('github-login')), [], () => restartWorkstation());
}

/**
 * Installs Git, then restarts your workstation so it finds it: Git for Windows with winget in a
 * console, or on a Mac Apple's Command Line Tools, which macOS asks about and installs by itself.
 */
function installGit() {
  if (MAC) return installMacGit(childEnv(), () => restartWorkstation());
  openConsole('Git installieren', path.join(paths.tools, 'install-git.cmd'), [], () => restartWorkstation());
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
 * What your workers need from this computer, asked about once your workstation runs: Git (their
 * checkouts, and Claude Code's Git Bash), a GitHub sign-in for gh (cloning, pushing, pull requests)
 * and Claude Code itself. One at a time, each skippable; asked again after every restart of the
 * workstation (an install or sign-in finishing restarts it), unless declined.
 */
let checking = false;
async function checkTools() {
  if (checking || !win || !win.isVisible() || !workstation) return;
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
          'Deine Arbeiter brauchen Git für ihre Arbeitskopien. Es kommt mit Apples Befehlszeilentools (Command Line Tools).\n\nJetzt installieren? macOS fragt nach und installiert sie; danach startet dein Arbeitsplatz neu.',
          'Git installieren',
        )
      : await ask(
          'Git fehlt',
          'Git for Windows ist auf diesem Computer noch nicht installiert.',
          'Deine Arbeiter brauchen Git für ihre Arbeitskopien, und Claude Code braucht es unter Windows auch.\n\nJetzt mit winget installieren? Danach startet dein Arbeitsplatz neu.',
          'Git installieren',
        );
    if (yes) return installGit();
  }
  if (installed('gh') && !(await ghSignedIn())) {
    const yes = await ask(
      'Bei GitHub anmelden',
      'Die GitHub CLI ist noch nicht bei GitHub angemeldet.',
      `Damit klonen deine Arbeiter die Projekte und machen Pull Requests.\n\nJetzt anmelden? Es öffnet sich ein ${MAC ? 'Terminal-Fenster' : 'Konsolenfenster'} und GitHub im Browser; danach startet dein Arbeitsplatz neu.`,
      'Bei GitHub anmelden',
    );
    if (yes) return githubLogin();
  }
  if (!installed('claude')) {
    const yes = await ask(
      'Claude Code fehlt',
      'Claude Code ist auf diesem Computer noch nicht installiert.',
      'Die Arbeiter, die du einstellst, laufen auf diesem Computer mit Claude Code. Du brauchst dafür keinen API-Key: Claude Code meldet sich mit deinem Claude-Abo (Pro oder Max) an.\n\n' +
        `Jetzt installieren und anmelden? Es öffnet sich ein ${MAC ? 'Terminal-Fenster' : 'Konsolenfenster'} mit dem offiziellen Installer; danach wählst du „Claude account with subscription“.\n\n` +
        'Oder lass deine Arbeiter im Connector auf dem Server laufen („Lieber auf dem Server laufen lassen“).',
      'Installieren und anmelden',
    );
    if (yes) openClaudeCode();
  }
}

// --- Window, menu bar and menus ------------------------------------------------------------------

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
  // Schließen (und auf dem Mac auch Minimieren) räumt nur das Fenster weg: Der Connector lebt in der
  // Menüleiste bzw. im Infobereich weiter, mit deinen Arbeitern und dem Office im Browser.
  win.on('close', (e) => {
    if (leaving) return;
    e.preventDefault();
    hideWindow();
  });
  if (MAC) win.on('minimize', () => !leaving && hideWindow());
  win.on('closed', () => (win = null));
  win.loadFile(path.join(__dirname, 'ui', 'index.html'));
}

let toldTray = false;
/** Das Fenster weg, der Connector bleibt: oben in der Menüleiste (Mac, ohne Symbol im Dock) bzw. unten im Infobereich. */
function hideWindow() {
  win?.hide();
  if (MAC) app.dock?.hide();
  if (WIN && tray && !toldTray) {
    toldTray = true;
    tray.displayBalloon({ iconType: 'info', title: 'Agent Office läuft weiter', content: 'Deine Arbeiter und das Office im Browser brauchen den Connector. Hier unten im Infobereich holst du ihn zurück, änderst die Einstellungen oder trennst die Verbindung.' });
  }
}

function showWindow() {
  if (MAC) void app.dock?.show();
  if (!win) return createWindow();
  if (win.isMinimized()) win.restore();
  win.show();
  win.focus();
}

/** Where the app's symbol in the menu bar (Mac, a template macOS colours itself) or the tray (Windows) comes from. */
function trayImage() {
  const img = nativeImage.createFromPath(path.join(__dirname, 'ui', MAC ? 'trayTemplate.png' : 'tray.png'));
  if (MAC) img.setTemplateImage(true);
  return img;
}

/** Das Symbol in der Menüleiste (Mac) bzw. im Infobereich (Windows), mit dem, was von dort geht. */
function buildTray(now) {
  try {
    if (!tray) {
      tray = new Tray(trayImage());
      tray.setToolTip('Agent Office');
      // Windows: ein Klick holt das Fenster; das Menü kommt mit der rechten Maustaste. Auf dem Mac öffnet ein Klick das Menü.
      if (!MAC) tray.on('click', () => showWindow());
    }
  } catch (err) {
    log(`no tray icon: ${err.message}`);
    return;
  }
  const where = now ? `${new URL(now.origin).host} als ${now.name}` : '';
  tray.setToolTip(now ? `Agent Office: verbunden mit ${where}` : 'Agent Office: nicht verbunden');
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: now ? `✅ Verbunden: ${where}` : '⚪ Nicht verbunden', enabled: false },
      { type: 'separator' },
      { label: 'Office im Browser öffnen', enabled: !!now, click: () => openInBrowser() },
      { label: 'Connector öffnen', click: () => showWindow() },
      { label: 'Einstellungen …', click: () => openSettings() },
      now ? { label: 'Verbindung trennen', click: () => void controller.disconnect() } : { label: 'Verbinden', enabled: controller.canConnect(), click: () => void controller.reconnect() },
      { type: 'separator' },
      { label: 'Agent Office beenden', click: () => app.quit() },
    ]),
  );
}

function buildMenu() {
  const now = controller.current();
  menuFor = JSON.stringify([now, controller.canConnect()]);
  buildTray(now);
  const template = [
    {
      label: 'Agent Office',
      submenu: [
        ...(MAC ? [{ role: 'about', label: 'Über Agent Office' }, { type: 'separator' }] : []),
        { label: 'Office im Browser öffnen', accelerator: 'CmdOrCtrl+O', enabled: !!now, click: () => openInBrowser() },
        { label: 'Einstellungen …', accelerator: 'CmdOrCtrl+,', click: () => openSettings() },
        now ? { label: 'Verbindung trennen', click: () => void controller.disconnect() } : { label: 'Verbinden', enabled: controller.canConnect(), click: () => void controller.reconnect() },
        { label: 'Adresse des Servers kopieren', enabled: !!now, click: () => clipboard.writeText(now.origin) },
        { type: 'separator' },
        { label: 'Den Server einrichten (Umbrel-App) …', click: () => shell.openExternal(`${DOCS}vermittler.md`) },
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
      label: 'Deine Arbeiter',
      submenu: [
        { label: 'Arbeitsplatz neu starten', enabled: !!workstation, click: () => restartWorkstation() },
        { label: 'Daten-Ordner öffnen', click: () => shell.openPath(settings.home) },
        {
          label: 'Daten-Ordner ändern …',
          click: async () => {
            const r = await dialog.showOpenDialog(win, { title: 'Wo soll dein Arbeitsplatz seine Daten speichern?', defaultPath: settings.home, properties: ['openDirectory', 'createDirectory'] });
            if (r.canceled || !r.filePaths[0]) return;
            saveSettings({ home: r.filePaths[0] });
            restartWorkstation();
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
            restartWorkstation();
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

// The window only goes when the app does (closing it hides it, see createWindow): the menu bar or
// the tray keeps the connector going.
app.on('window-all-closed', () => {});
app.on('activate', () => showWindow());

app.on('before-quit', (e) => {
  leaving = true;
  if (quitting) return;
  e.preventDefault();
  quitting = true;
  Promise.all([stopWorkstation(), controller.close()]).finally(() => app.quit());
});
