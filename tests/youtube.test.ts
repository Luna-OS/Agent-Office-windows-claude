// YouTube-Adressen für den DJ auf dem Dach (src/shared/youtube.ts): alle Formen, Startzeiten, Fehler.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseYoutube, readTime } from '../src/shared/youtube.js';

const ID = 'dQw4w9WgXcQ';

test('alle üblichen Formen einer YouTube-Adresse', () => {
  for (const url of [
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
    'http://youtube.com/watch?v=dQw4w9WgXcQ&list=PL123&index=2',
    'youtube.com/watch?feature=share&v=dQw4w9WgXcQ',
    'https://m.youtube.com/watch?v=dQw4w9WgXcQ',
    'https://music.youtube.com/watch?v=dQw4w9WgXcQ&si=abc',
    'https://youtu.be/dQw4w9WgXcQ',
    'https://youtu.be/dQw4w9WgXcQ?si=xyz',
    'https://www.youtube.com/shorts/dQw4w9WgXcQ',
    'https://youtube.com/shorts/dQw4w9WgXcQ?feature=share',
    'https://www.youtube.com/embed/dQw4w9WgXcQ?rel=0',
    'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ',
    'https://www.youtube.com/live/dQw4w9WgXcQ?feature=shared',
    'https://www.youtube.com/v/dQw4w9WgXcQ',
    '  https://WWW.YouTube.com/watch?v=dQw4w9WgXcQ  ',
    'dQw4w9WgXcQ',
  ]) {
    assert.deepEqual(parseYoutube(url), { id: ID, start: 0 }, url);
  }
});

test('Startzeiten aus t= und start=', () => {
  assert.equal(parseYoutube('https://youtu.be/dQw4w9WgXcQ?t=42').start, 42);
  assert.equal(parseYoutube('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m30s').start, 90);
  assert.equal(parseYoutube('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1h2m3s').start, 3723);
  assert.equal(parseYoutube('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90s').start, 90);
  assert.equal(parseYoutube('https://www.youtube.com/embed/dQw4w9WgXcQ?start=15').start, 15);
  assert.equal(parseYoutube('https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=2m').start, 120);
  assert.equal(parseYoutube('https://youtube.com/shorts/dQw4w9WgXcQ?t=7').start, 7);
  assert.equal(readTime('1:02:03'), 3723);
  assert.equal(readTime('abc'), undefined);
});

test('keine YouTube-Adresse: eine deutsche Meldung', () => {
  for (const url of [
    '',
    'https://vimeo.com/12345',
    'https://www.youtube.com/watch?v=kurz',
    'https://www.youtube.com/channel/UC123',
    'https://youtu.be/',
    'https://evil.com/watch?v=dQw4w9WgXcQ',
    'https://youtube.com.evil.com/watch?v=dQw4w9WgXcQ',
  ]) {
    assert.throws(() => parseYoutube(url), /YouTube|Video-ID/, url);
  }
});

test('attribution_link', () => {
  assert.equal(parseYoutube('https://www.youtube.com/attribution_link?a=x&u=%2Fwatch%3Fv%3DdQw4w9WgXcQ%26feature%3Dshare').id, ID);
});
