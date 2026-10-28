// electron-builder's afterPack hook: copies desktop/staging (see stage.mjs) into the packed app's
// resources folder (Contents/Resources in the Mac app). Not done with "extraResources", which always
// leaves node_modules folders out, and the office can't start without its dependencies.
//
// A Mac packs the arm64 and the x64 app in one go, each from its own desktop/staging-<arch>; any
// other build takes desktop/staging. On a Mac it runs before the app is signed, so what it copies is
// signed with it.
const { cpSync, existsSync } = require('node:fs');
const path = require('node:path');
const { Arch } = require('electron-builder');

exports.default = async function afterPack(context) {
  const arch = Arch[context.arch];
  const perArch = path.join(__dirname, '..', `staging-${arch}`);
  const staging = existsSync(perArch) ? perArch : path.join(__dirname, '..', 'staging');
  const resources = context.packager.getResourcesDir(context.appOutDir);
  for (const dir of ['office', 'node', 'claude', 'gh', 'tools']) {
    const from = path.join(staging, dir);
    if (!existsSync(from)) throw new Error(`${from} is missing: run \`npm run stage\` first`);
    cpSync(from, path.join(resources, dir), { recursive: true, verbatimSymlinks: true });
  }
  if (!existsSync(path.join(resources, 'office', 'node_modules', '@lydell', 'node-pty'))) {
    throw new Error('the office was packed without its dependencies');
  }
  // The terminals' native module, built for the app's platform and architecture (not the build machine's).
  const pty = `node-pty-${context.electronPlatformName}-${arch}`;
  if (!existsSync(path.join(resources, 'office', 'node_modules', '@lydell', pty))) {
    throw new Error(`the office was packed without @lydell/${pty}: stage it with STAGE_ARCH=${arch}`);
  }
};
