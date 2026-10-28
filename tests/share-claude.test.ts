// The host's AI for everyone: while an admin shares the office machine's own Claude (🔑 Accounts),
// a member may run their workers on it instead of their own plan, and is back on their own once it
// isn't shared any more. GitHub stays the admins' alone.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { Accounts } from '../src/server/accounts.js';
import { SignIns } from '../src/server/signins.js';

const ADMIN = 'admin0000001';
const MEMBER = 'member000001';

function office() {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'share-claude-'));
  let shared = false;
  const signins = new SignIns(
    dir,
    null,
    null,
    () => ({ ANTHROPIC_API_KEY: 'sk-ant-api03-the-hosts-own', GH_TOKEN: 'ghp_the-hosts-own' }),
    (id, which) => id === ADMIN || (which === 'claude' && shared),
    () => {},
  );
  return { dir, signins, share: (on: boolean) => void (shared = on) };
}

test('a member runs on their own Claude until the host shares theirs', async (t) => {
  const { dir, signins, share } = office();
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  assert.deepEqual(signins.state(MEMBER).office, { claude: false, github: false });
  assert.deepEqual(signins.state(ADMIN).office, { claude: true, github: true });
  assert.match(signins.useOffice(MEMBER, 'claude') ?? '', /Only admins can use the office's own Claude/);

  share(true);
  assert.deepEqual(signins.state(MEMBER).office, { claude: true, github: false });
  assert.equal(signins.useOffice(MEMBER, 'claude'), undefined);
  assert.equal(signins.state(MEMBER).claude.how, 'office');
  assert.equal(signins.claudeReady(MEMBER), true);
  assert.equal(signins.claudeKey(MEMBER), undefined, "the host's plan limits, not a plan of their own");
  // Their workers keep the host's Claude, and still act on GitHub as themselves.
  const env = signins.apply(MEMBER, { ANTHROPIC_API_KEY: 'sk-ant-api03-the-hosts-own', GH_TOKEN: 'ghp_the-hosts-own' });
  assert.equal(env.ANTHROPIC_API_KEY, 'sk-ant-api03-the-hosts-own');
  assert.equal(env.GH_TOKEN, undefined);
  assert.equal(env.GH_CONFIG_DIR, path.join(dir, 'homes', MEMBER, 'gh'));
  assert.match(signins.useOffice(MEMBER, 'github') ?? '', /Only admins/);

  // No longer shared: back on their own Claude, which still has to be signed in.
  share(false);
  assert.equal(signins.state(MEMBER).claude.how, 'login');
  assert.equal(signins.claudeReady(MEMBER), false);
  assert.equal(signins.apply(MEMBER, { ANTHROPIC_API_KEY: 'sk-ant-api03-the-hosts-own' }).ANTHROPIC_API_KEY, undefined);
  signins.shutdown();
});

test('sharing the office’s Claude is kept with the accounts, off until an admin turns it on', (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'share-claude-accounts-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const accounts = new Accounts(dir);
  assert.equal(accounts.shareClaude, false);
  assert.equal(accounts.state(new Set()).shareClaude, false);
  accounts.setShareClaude(true);
  assert.equal(new Accounts(dir).shareClaude, true, 'read back from accounts.json');
  assert.equal(new Accounts(dir).state(new Set()).shareClaude, true);
  accounts.setShareClaude(false);
  assert.equal(new Accounts(dir).shareClaude, false);
});
