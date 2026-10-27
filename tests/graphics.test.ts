// How hard the 3D office works the computer: the frame pace (ctx.pace) and the Graphics presets.
import test from 'node:test';
import assert from 'node:assert/strict';
import { Pace } from '../src/client/core/registry.js';
import { DEFAULT_GRAPHICS, GRAPHICS, fpsFor, loadGraphics, onGraphics, pixelRatioFor, setGraphics } from '../src/client/features/graphics/quality.js';

/** How many of `frames` screen frames, `hz` a second, `pace` draws. */
function drawn(pace: Pace, hz: number, frames: number, from = 1000): number {
  let n = 0;
  for (let i = 0; i < frames; i++) if (pace.due(from + (i * 1000) / hz)) n++;
  return n;
}

test('with no cap, every screen frame is drawn', () => {
  const pace = new Pace();
  assert.equal(pace.fps(), 0);
  assert.equal(drawn(pace, 144, 144), 144);
});

test('a cap draws that many frames a second, whatever the screen’s own rate', () => {
  for (const [hz, fps] of [
    [60, 60],
    [60, 30],
    [144, 60],
    [120, 60],
    [144, 30],
    [165, 15],
  ] as const) {
    const pace = new Pace();
    pace.cap(() => fps);
    const n = drawn(pace, hz, hz * 10);
    assert.ok(Math.abs(n / 10 - fps) <= fps * 0.05, `${fps} a second on a ${hz} Hz screen drew ${n / 10} a second`);
  }
});

test('a 60 Hz screen capped at 60 still draws every frame, early or late by a little', () => {
  const pace = new Pace();
  pace.cap(() => 60);
  let n = 0;
  for (let i = 0; i < 600; i++) if (pace.due(1000 + i * (1000 / 60) + (i % 2 ? 1.5 : -1.5))) n++;
  assert.equal(n, 600);
});

test('the lowest cap wins, a cap of 0 is none, and taking a cap out lifts it', () => {
  const pace = new Pace();
  const offA = pace.cap(() => 60);
  pace.cap(() => 0);
  let quiet = 0;
  pace.cap(() => quiet);
  assert.equal(pace.fps(), 60);
  quiet = 30;
  assert.equal(pace.fps(), 30);
  quiet = 0;
  offA();
  assert.equal(pace.fps(), 0);
});

test('after a stall the pace starts over instead of drawing a burst to catch up', () => {
  const pace = new Pace();
  pace.cap(() => 30);
  assert.equal(drawn(pace, 60, 60, 1000), 30);
  // Two seconds with no frames (a hidden tab), then the screen comes back.
  assert.equal(drawn(pace, 60, 6, 4000), 3);
});

test('the presets: Balanced by default, Eco lightest, High with no limits', () => {
  assert.equal(DEFAULT_GRAPHICS, 'balanced');
  assert.equal(loadGraphics(), 'balanced', 'nothing saved (no localStorage under node)');
  assert.equal(fpsFor('high', false), 0);
  assert.equal(fpsFor('high', true), 0);
  assert.equal(fpsFor('balanced', false), 60);
  assert.equal(fpsFor('balanced', true), 30);
  assert.equal(fpsFor('eco', false), 30);
  assert.equal(fpsFor('eco', true), 15);
  assert.ok(GRAPHICS.eco.shadowMap < GRAPHICS.balanced.shadowMap);
  assert.equal(GRAPHICS.balanced.shadowMap, GRAPHICS.high.shadowMap, 'Balanced keeps the shadows as sharp as High');
  // Never sharper than the screen, and on a Retina screen a little under it.
  assert.equal(pixelRatioFor('balanced', 1), 1);
  assert.equal(pixelRatioFor('balanced', 1.25), 1.25);
  assert.equal(pixelRatioFor('balanced', 2), 1.5);
  assert.equal(pixelRatioFor('high', 3), 2);
  assert.equal(pixelRatioFor('eco', 2), 1);
});

test('picking a preset tells the renderer', () => {
  const heard: string[] = [];
  const off = onGraphics((q) => heard.push(q));
  setGraphics('eco');
  setGraphics('high');
  off();
  setGraphics('balanced');
  assert.deepEqual(heard, ['eco', 'high']);
});
