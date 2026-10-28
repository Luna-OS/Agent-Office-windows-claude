// Der DJ auf dem Dach (server/dj.ts): einen YouTube-Song auflegen, eine falsche Adresse, runternehmen.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DjBooth, djBooth } from '../src/server/dj.js';

test('ein Song wird aufgelegt, mit Startzeit, und wieder runtergenommen', () => {
  const dj = new DjBooth();
  assert.equal(dj.state(), null);
  assert.equal(dj.stop(), false, 'nichts lief');
  const r = dj.play('https://youtu.be/dQw4w9WgXcQ?t=1m', 'Luna', 5000);
  assert.deepEqual(r, { state: { video: 'dQw4w9WgXcQ', start: 60, startedAt: 5000, by: 'Luna' } });
  assert.deepEqual(dj.state(), { video: 'dQw4w9WgXcQ', start: 60, startedAt: 5000, by: 'Luna' });
  assert.equal(dj.stop(), true);
  assert.equal(dj.state(), null);
});

test('keine YouTube-Adresse: eine deutsche Meldung, und es läuft weiter, was lief', () => {
  const dj = new DjBooth();
  dj.play('dQw4w9WgXcQ', 'Kim', 1);
  assert.deepEqual(dj.play('https://vimeo.com/123', 'Mo'), { error: 'Das ist keine YouTube-Adresse' });
  assert.deepEqual(dj.play(undefined, 'Mo'), { error: 'Bitte eine YouTube-Adresse eingeben' });
  assert.equal(dj.state()?.by, 'Kim');
});

test('ein DJ-Pult pro Office', () => {
  const a = {};
  const b = {};
  assert.equal(djBooth(a), djBooth(a));
  assert.notEqual(djBooth(a), djBooth(b));
});
