// Puts together what the desktop app ships next to Electron in its resources folder (desktop/staging,
// copied there by scripts/after-pack.cjs):
//
//   office/   the built office (bin/, dist/, package.json) with its production dependencies, exactly
//             the versions in package-lock.json, and office-entry.mjs
//   node/     the node (node.exe) it runs on: the office starts its terminal host, hooks and MCP
//             server with process.execPath, so it needs a real Node.js, not Electron's
//   claude/   claude-code.cmd (claude-code.command on a Mac), behind "Claude Code (ohne API-Key)"
//   gh/       the GitHub CLI, which the office lists, clones and opens PRs with; the app puts it on
//             the PATH after any gh the user installed
//   tools/    github-login.cmd and install-git.cmd (github-login.command on a Mac), behind the app's
//             "GitHub" menu
//
// Run it on the platform being packaged (CI: windows-latest, macos-latest), after `npm ci` in the
// repository root has built dist/. NODE_VERSION picks the node (default: the Node.js running this
// script), GH_VERSION the GitHub CLI (default: its newest release). STAGE_ARCH stages for another
// architecture than this machine's, into desktop/staging-<arch> (a Mac builds the arm64 and the x64
// app one after the other: `STAGE_ARCH=arm64 npm run stage && STAGE_ARCH=x64 npm run stage`).
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const desktop = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const root = path.dirname(desktop);
const WIN = process.platform === 'win32';
const MAC = process.platform === 'darwin';
const ARCH = process.env.STAGE_ARCH || process.arch;
if (!['x64', 'arm64'].includes(ARCH)) throw new Error(`STAGE_ARCH must be x64 or arm64, not ${ARCH}`);
const staging = path.join(desktop, process.env.STAGE_ARCH ? `staging-${ARCH}` : 'staging');
const office = path.join(staging, 'office');

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

step(`Installing its production dependencies (${process.platform}-${ARCH})`);
// --os and --cpu pick the native modules' builds (node-pty's) for the app's platform, not this machine's.
execFileSync(WIN ? 'npm.cmd' : 'npm', ['ci', '--omit=dev', '--no-audit', '--no-fund', '--loglevel=error', `--os=${process.platform}`, `--cpu=${ARCH}`], { cwd: office, stdio: 'inherit', shell: WIN });

const nodeDir = path.join(staging, 'node');
mkdirSync(nodeDir, { recursive: true });
const version = (process.env.NODE_VERSION || process.versions.node).replace(/^v/, '');
const get = async (url, headers = {}) => {
  const res = await fetch(url, { headers });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
};
/** The SHA-256 a checksums file (`<hex>  <name>` lines) lists for `name`, checked against `data`. */
function verify(data, sums, name) {
  const want = sums.split('\n').find((l) => l.trim().endsWith(name))?.split(/\s+/)[0];
  const got = createHash('sha256').update(data).digest('hex');
  if (!want || want !== got) throw new Error(`${name}: checksum mismatch (expected ${want}, got ${got})`);
}

if (WIN) {
  const arch = process.env.NODE_ARCH || ARCH;
  const base = `https://nodejs.org/dist/v${version}`;
  step(`Downloading node.exe ${version} (win-${arch})`);
  const exe = await get(`${base}/win-${arch}/node.exe`);
  verify(exe, (await get(`${base}/SHASUMS256.txt`)).toString('utf8'), ` win-${arch}/node.exe`);
  writeFileSync(path.join(nodeDir, 'node.exe'), exe);
} else if (MAC) {
  const name = `node-v${version}-darwin-${ARCH}`;
  const base = `https://nodejs.org/dist/v${version}`;
  step(`Downloading node ${version} (darwin-${ARCH})`);
  const tarball = await get(`${base}/${name}.tar.gz`);
  verify(tarball, (await get(`${base}/SHASUMS256.txt`)).toString('utf8'), ` ${name}.tar.gz`);
  const unpack = path.join(staging, 'node-unpack');
  mkdirSync(unpack, { recursive: true });
  writeFileSync(path.join(unpack, `${name}.tar.gz`), tarball);
  execFileSync('tar', ['-xzf', `${name}.tar.gz`, `${name}/bin/node`, `${name}/LICENSE`], { cwd: unpack, stdio: 'inherit' });
  copyFileSync(path.join(unpack, name, 'bin', 'node'), path.join(nodeDir, 'node'));
  copyFileSync(path.join(unpack, name, 'LICENSE'), path.join(nodeDir, 'LICENSE'));
  chmodSync(path.join(nodeDir, 'node'), 0o755);
  rmSync(unpack, { recursive: true, force: true });
} else {
  // Packaging for this machine (a local test build): the Node.js running this script.
  step(`Copying this machine's node ${process.version}`);
  copyFileSync(process.execPath, path.join(nodeDir, 'node'));
  chmodSync(path.join(nodeDir, 'node'), 0o755);
}

if (WIN || MAC) {
  // A GitHub token (Actions' GITHUB_TOKEN) only lifts the API's rate limit for finding the newest release.
  const auth = process.env.GITHUB_TOKEN ? { authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {};
  let ghVersion = (process.env.GH_VERSION || '').replace(/^v/, '');
  if (!ghVersion) ghVersion = JSON.parse((await get('https://api.github.com/repos/cli/cli/releases/latest', auth)).toString('utf8')).tag_name.replace(/^v/, '');
  const ghArch = ARCH === 'arm64' ? 'arm64' : 'amd64';
  const zipName = WIN ? `gh_${ghVersion}_windows_${ghArch}.zip` : `gh_${ghVersion}_macOS_${ghArch}.zip`;
  const exeName = WIN ? 'gh.exe' : 'gh';
  const base = `https://github.com/cli/cli/releases/download/v${ghVersion}`;
  step(`Downloading the GitHub CLI ${ghVersion}`);
  const zip = await get(`${base}/${zipName}`);
  verify(zip, (await get(`${base}/gh_${ghVersion}_checksums.txt`)).toString('utf8'), ` ${zipName}`);
  const unpack = path.join(staging, 'gh-unpack');
  mkdirSync(unpack, { recursive: true });
  writeFileSync(path.join(unpack, zipName), zip);
  // Windows' own tar and the Mac's (both bsdtar) read zip files.
  execFileSync(WIN ? path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe') : 'tar', ['-xf', zipName], { cwd: unpack, stdio: 'inherit' });
  const find = (dir) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) {
        const found = find(p);
        if (found) return found;
      } else if (e.name.toLowerCase() === exeName && (WIN || path.basename(dir) === 'bin')) return p;
    }
    return undefined;
  };
  const ghExe = find(unpack);
  if (!ghExe) throw new Error(`${zipName} has no bin/${exeName}`);
  mkdirSync(path.join(staging, 'gh'), { recursive: true });
  copyFileSync(ghExe, path.join(staging, 'gh', exeName));
  if (!WIN) chmodSync(path.join(staging, 'gh', exeName), 0o755);
  const license = path.join(path.dirname(path.dirname(ghExe)), 'LICENSE');
  if (existsSync(license)) copyFileSync(license, path.join(staging, 'gh', 'LICENSE'));
  rmSync(unpack, { recursive: true, force: true });
} else {
  // A local test build uses this machine's gh, if any.
  mkdirSync(path.join(staging, 'gh'), { recursive: true });
}

step('Copying the Claude Code shortcut and the GitHub helpers');
// Each platform's own: .cmd (and the shortcut's icon) on Windows, .command elsewhere, kept executable.
const ours = (file) => (WIN ? !file.endsWith('.command') : !/\.(cmd|ico)$/.test(file));
for (const dir of ['claude', 'tools']) {
  mkdirSync(path.join(staging, dir), { recursive: true });
  for (const file of readdirSync(path.join(desktop, dir)).filter(ours)) {
    copyFileSync(path.join(desktop, dir, file), path.join(staging, dir, file));
    if (file.endsWith('.command')) chmodSync(path.join(staging, dir, file), 0o755);
  }
}

step(`Staged in ${staging}`);
