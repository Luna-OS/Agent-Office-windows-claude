// This computer's own Claude Code and GitHub CLI sign-ins: the ones the office on this computer runs
// its workers with, and GitHub's the connector can hand on to a lobby. Read with the tools
// themselves; `run(cmd, args)` (main.cjs, or a stand-in in the tests) resolves to { code, out, missing }.

const cap = (s) => s[0].toUpperCase() + s.slice(1);

/** Who Claude Code is signed in as here: { status: 'ok', who } or { status: 'none', missing? }. */
async function lookClaude(run) {
  const r = await run('claude', ['auth', 'status', '--json']);
  if (r.missing) return { status: 'none', missing: true };
  let s = {};
  try {
    s = JSON.parse(r.out);
  } catch {
    // an older Claude Code, or it failed: nothing to go on
  }
  if (!s?.loggedIn) return { status: 'none' };
  const plan = typeof s.subscriptionType === 'string' && s.subscriptionType ? ` · ${cap(s.subscriptionType)}` : '';
  const who = typeof s.email === 'string' && s.email ? s.email : s.authMethod === 'api_key' ? 'mit API-Key' : 'angemeldet';
  return { status: 'ok', who: `${who}${plan}` };
}

/** Who the GitHub CLI is signed in as here: { status: 'ok', who: '@login' } or { status: 'none', missing? }. */
async function lookGithub(run) {
  const r = await run('gh', ['api', 'user', '--jq', '.login']);
  if (r.missing) return { status: 'none', missing: true };
  const login = r.code === 0 ? r.out.trim() : '';
  return /^[A-Za-z0-9-]{1,39}$/.test(login) ? { status: 'ok', who: `@${login}` } : { status: 'none' };
}

/** The GitHub CLI's token here, to sign a lobby in to the same GitHub account; undefined without one. */
async function githubToken(run) {
  const r = await run('gh', ['auth', 'token', '--hostname', 'github.com']);
  const token = r.code === 0 ? r.out.trim() : '';
  return /^(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})$/.test(token) ? token : undefined;
}

module.exports = { lookClaude, lookGithub, githubToken };
