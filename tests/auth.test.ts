import { test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { Auth, type Session } from '../src/server/auth.js';
import type { Accounts } from '../src/server/accounts.js';

const auth = () => new Auth(randomBytes(32), randomBytes(16), 'secret', {} as Accounts);

test('a sign-in link works once', () => {
  const a = auth();
  const key = a.linkKey();
  assert.match(key, /^[\w-]{32}$/);
  assert.deepEqual(a.useLinkKey(key), { accountId: undefined });
  assert.equal(a.useLinkKey(key), undefined);
});

test("a made-up or empty key isn't a sign-in link, and another office's isn't either", () => {
  const a = auth();
  a.linkKey();
  assert.equal(a.useLinkKey(''), undefined);
  assert.equal(a.useLinkKey('x'.repeat(32)), undefined);
  assert.equal(a.useLinkKey(auth().linkKey()), undefined);
});

test('only the newest sign-in links are kept', () => {
  const a = auth();
  const first = a.linkKey();
  const later = Array.from({ length: 32 }, () => a.linkKey());
  assert.equal(a.useLinkKey(first), undefined);
  for (const key of later) assert.ok(a.useLinkKey(key));
});

test('a handed-off link signs in the same account, for a few minutes only', (t) => {
  const a = auth();
  const account = { id: 'acct00000001', name: 'Nico', role: 'member' } as Session['account'];
  assert.deepEqual(a.useLinkKey(a.linkKey({ account })), { accountId: 'acct00000001' });
  assert.deepEqual(a.useLinkKey(a.linkKey({})), { accountId: undefined });
  const stale = a.linkKey({ account });
  t.mock.timers.enable({ apis: ['Date'], now: Date.now() + 5 * 60_000 + 1 });
  assert.equal(a.useLinkKey(stale), undefined);
  // The office's own terminal links don't go stale.
  const terminal = a.linkKey();
  t.mock.timers.tick(24 * 60 * 60_000);
  assert.ok(a.useLinkKey(terminal));
});
