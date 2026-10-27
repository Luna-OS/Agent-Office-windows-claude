// What the app does differently on a Mac: where programs are (an app started from the Dock or
// Finder gets a bare PATH), how Git gets installed (Apple's Command Line Tools), and how a script
// opens in a window you can type in (Terminal, through a .command file).
const { execFile, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const MAC = process.platform === 'darwin';

/** Single-quoted for a POSIX shell. */
const sq = (s) => `'${String(s).replace(/'/g, `'\\''`)}'`;

let shellPath = null;
/**
 * The PATH your login shell sets up (Homebrew, nvm, ~/.local/bin…), which an app started from the
 * Dock doesn't get: /usr/bin:/bin:/usr/sbin:/sbin is all it has. Read once, and again on `fresh`
 * (after an install). Empty off a Mac, or when the shell doesn't answer.
 */
function loginShellPath(fresh = false) {
  if (!MAC) return [];
  if (shellPath && !fresh) return shellPath;
  try {
    const out = execFileSync(process.env.SHELL || '/bin/zsh', ['-ilc', 'printf "\\n__PATH__%s\\n" "$PATH"'], { encoding: 'utf8', timeout: 5000, stdio: ['ignore', 'pipe', 'ignore'] });
    shellPath = (/__PATH__(.*)/.exec(out)?.[1] ?? '').split(':').filter(Boolean);
  } catch {
    shellPath = [];
  }
  return shellPath;
}

/** Where a Mac's programs usually are, though the Dock's PATH leaves them out: Homebrew's, on Apple silicon and on Intel. */
const MAC_DIRS = ['/opt/homebrew/bin', '/usr/local/bin'];

/**
 * Whether Git really works on this Mac: /usr/bin/git is only a stand-in until Apple's Command Line
 * Tools are installed (running it asks to install them), so it's those, or a git from elsewhere.
 */
function macGit(env) {
  try {
    execFileSync('xcode-select', ['-p'], { stdio: 'ignore', timeout: 5000 });
    return true;
  } catch {
    // no Command Line Tools: maybe Homebrew's git
  }
  try {
    const all = execFileSync('which', ['-a', 'git'], { env, encoding: 'utf8', timeout: 5000 });
    return all.split('\n').some((p) => p.trim() && p.trim() !== '/usr/bin/git');
  } catch {
    return false;
  }
}

/**
 * Installs Apple's Command Line Tools (Git among them): macOS asks in a window of its own and
 * installs them. `done` is called once Git works, for the office to start again and find it.
 */
function installMacGit(env, done) {
  execFile('xcode-select', ['--install'], () => {});
  const since = Date.now();
  const timer = setInterval(() => {
    if (macGit(env)) {
      clearInterval(timer);
      done();
    } else if (Date.now() - since > 60 * 60_000) clearInterval(timer);
  }, 5000);
}

/**
 * Opens `script` with `args` in a Terminal window, with the app's PATH, in `cwd`. With `onExit`,
 * that's called once the script has finished (Terminal keeps its window, so the script leaves a
 * mark the app waits for).
 */
function openTerminal({ title, script, args = [], cwd, env, dir, onExit }) {
  fs.mkdirSync(dir, { recursive: true });
  const id = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  const file = path.join(dir, `${id}.command`);
  const mark = onExit ? path.join(dir, `${id}.done`) : null;
  const lines = [
    '#!/bin/bash',
    // Written by Agent Office for one window, and deleted as it starts.
    `rm -f ${sq(file)}`,
    `printf '\\033]0;%s\\007' ${sq(title)}`,
    `export PATH=${sq(env.PATH || '/usr/bin:/bin:/usr/sbin:/sbin')}`,
    `cd ${sq(cwd)} 2>/dev/null || cd "$HOME"`,
    `${sq(script)}${args.length ? ' ' + args.map(sq).join(' ') : ''}`,
    'status=$?',
    mark ? `: > ${sq(mark)}` : '',
    'exit $status',
  ];
  fs.writeFileSync(file, lines.filter(Boolean).join('\n') + '\n', { mode: 0o755 });
  execFile('open', ['-a', 'Terminal', file], (err) => err && console.error(`agent-office: couldn't open Terminal: ${err.message}`));
  if (!mark) return;
  const since = Date.now();
  const timer = setInterval(() => {
    if (fs.existsSync(mark)) {
      clearInterval(timer);
      fs.rmSync(mark, { force: true });
      onExit();
    } else if (Date.now() - since > 60 * 60_000) clearInterval(timer);
  }, 1500);
}

/** The app's own scripts, by what they do: .cmd on Windows, .command (bash, in Terminal) on a Mac. */
function scriptName(base) {
  return process.platform === 'win32' ? `${base}.cmd` : `${base}.command`;
}

module.exports = { MAC, MAC_DIRS, loginShellPath, macGit, installMacGit, openTerminal, scriptName };
