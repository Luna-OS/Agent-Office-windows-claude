// Die Jukebox (server/jukebox.ts, shared/jukebox.ts) mit einem YouTube-Link: Sie nimmt ihn wie einen
// Stream, sagt, dass es YouTube ist, und der Browser spielt nur seinen Ton (features/jukebox/youtube.ts).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { Jukebox } from '../src/server/jukebox.js';
import { STREAM, checkStreamUrl, trackTitle, youtubeVideo } from '../src/shared/jukebox.js';

test('ein YouTube-Link läuft auf der Jukebox wie ein Stream, und bleibt über einen Neustart', (t) => {
  const dir = mkdtempSync(path.join(tmpdir(), 'agent-office-jukebox-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const box = new Jukebox(dir);
  assert.deepEqual(box.play({ url: 'https://youtu.be/dQw4w9WgXcQ?t=1m30s' }, 'Luna'), { changed: true });
  const s = box.state();
  assert.equal(s.on, true);
  assert.equal(s.track, STREAM);
  assert.equal(s.url, 'https://youtu.be/dQw4w9WgXcQ?t=1m30s');
  assert.equal(box.title(), 'YouTube · dQw4w9WgXcQ');
  assert.equal(new Jukebox(dir).state().url, s.url, 'gespeichert');
});

test('welcher Link YouTube ist, und wo es losgeht', () => {
  assert.deepEqual(youtubeVideo('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90'), { id: 'dQw4w9WgXcQ', start: 90 });
  assert.deepEqual(youtubeVideo('https://music.youtube.com/watch?v=dQw4w9WgXcQ'), { id: 'dQw4w9WgXcQ', start: 0 });
  assert.equal(youtubeVideo('https://radio.example.com/live.mp3'), undefined);
  assert.equal(youtubeVideo(undefined), undefined);
  assert.equal(trackTitle({ track: STREAM, url: 'https://radio.example.com/jazz/live.mp3' }), 'radio.example.com · live.mp3');
  assert.deepEqual(checkStreamUrl(' '), { error: 'Füge einen Link ein: YouTube, Internetradio oder eine Audiodatei' });
  assert.deepEqual(checkStreamUrl('https://youtu.be/dQw4w9WgXcQ'), { url: 'https://youtu.be/dQw4w9WgXcQ' });
});
