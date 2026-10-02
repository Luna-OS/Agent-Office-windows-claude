// Puts together what the Windows app ships next to Electron (desktop/staging, see "extraResources"
// in desktop/package.json):
//
//   office/   the built office (bin/, dist/, package.json) with its production dependencies, exactly
//             the versions in package-lock.json, and office-entry.mjs
//   node/     the node.exe it runs on: the office starts its terminal host, hooks and MCP server with
//             process.execPath, so it needs a real Node.js, not Electron's
//   claude/   claude-code.cmd, behind the "Claude Code (ohne API-Key)" shortcut
//
// Run it on the platform being packaged (CI: windows-latest), after `npm ci` in the repository root
// has built dist/. NODE_VERSION picks the node.exe (default: the Node.js running this script).
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync, chmodSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const desktop = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const root = path.dirname(desktop);
const staging = path.join(desktop, 'staging');
const office = path.join(staging, 'office');
const WIN = process.platform === 'win32';

const step = (msg) => console.log(`==> ${msg}`);

if (!existsSync(path.join(root, 'dist', 'server', 'server', 'cli.js')) || !existsSync(path.join(root, 'dist', 'public'))) {
  throw new Error('the office isn\'t built: run `npm ci` (or `npm run build`) in the repository root first');
}

rmSync(staging, { recursive: true, force: true });
mkdirSync(office, { recursive: true });

step('Copying the built office');
for (const item of ['bin', 'dist', 'package.json', 'package-lock.json', 'LICENSE', 'README.md']) {
  cpSync(path.join(root, item), path.join(office, item), { recursive: true });
}
copyFileSync(path.join(desktop, 'office-entry.mjs'), path.join(office, 'office-entry.mjs'));

// It's built already, and the app has no build tools: as release.yml does for the npm package.
const pkgFile = path.join(office, 'package.json');
const pkg = JSON.parse(readFileSync(pkgFile, 'utf8'));
delete pkg.scripts.prepare;
writeFileSync(pkgFile, JSON.stringify(pkg, null, 2) + '\n');

step('Installing its production dependencies');
execFileSync(WIN ? 'npm.cmd' : 'npm', ['ci', '--omit=dev', '--no-audit', '--no-fund', '--loglevel=error'], { cwd: office, stdio: 'inherit', shell: WIN });

const nodeDir = path.join(staging, 'node');
mkdirSync(nodeDir, { recursive: true });
const version = (process.env.NODE_VERSION || process.versions.node).replace(/^v/, '');
if (WIN) {
  const arch = process.env.NODE_ARCH || 'x64';
  const base = `https://nodejs.org/dist/v${version}`;
  step(`Downloading node.exe ${version} (win-${arch})`);
  const get = async (url) => {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
    return Buffer.from(await res.arrayBuffer());
  };
  const exe = await get(`${base}/win-${arch}/node.exe`);
  const sums = (await get(`${base}/SHASUMS256.txt`)).toString('utf8');
  const want = sums.split('\n').find((l) => l.trim().endsWith(` win-${arch}/node.exe`))?.split(/\s+/)[0];
  const got = createHash('sha256').update(exe).digest('hex');
  if (!want || want !== got) throw new Error(`node.exe checksum mismatch (expected ${want}, got ${got})`);
  writeFileSync(path.join(nodeDir, 'node.exe'), exe);
} else {
  // Packaging for this machine (a local test build): the Node.js running this script.
  step(`Copying this machine's node ${process.version}`);
  copyFileSync(process.execPath, path.join(nodeDir, 'node'));
  chmodSync(path.join(nodeDir, 'node'), 0o755);
}

step('Copying the Claude Code shortcut');
cpSync(path.join(desktop, 'claude'), path.join(staging, 'claude'), { recursive: true });

step(`Staged in ${staging}`);
