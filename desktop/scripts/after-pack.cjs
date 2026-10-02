// electron-builder's afterPack hook: copies desktop/staging (see stage.mjs) into the packed app's
// resources folder. Not done with "extraResources", which always leaves node_modules folders out,
// and the office can't start without its dependencies.
const { cpSync, existsSync } = require('node:fs');
const path = require('node:path');

exports.default = async function afterPack(context) {
  const staging = path.join(__dirname, '..', 'staging');
  const resources = path.join(context.appOutDir, 'resources');
  for (const dir of ['office', 'node', 'claude', 'gh', 'tools']) {
    const from = path.join(staging, dir);
    if (!existsSync(from)) throw new Error(`${from} is missing: run \`npm run stage\` first`);
    cpSync(from, path.join(resources, dir), { recursive: true, verbatimSymlinks: true });
  }
  if (!existsSync(path.join(resources, 'office', 'node_modules', '@lydell', 'node-pty'))) {
    throw new Error('the office was packed without its dependencies');
  }
};
