import * as THREE from 'three';
import type { Ctx } from '../../core/context';
import { modalOpen, onModalChange } from '../../ui/dom';
import { GRAPHICS, IDLE_AFTER, fpsFor, loadGraphics, onGraphics, pixelRatioFor, shadowDue } from './quality';

export interface GraphicsDeps {
  /** The sun, whose shadow map is the one that costs (see core/scene.ts). */
  sun: THREE.DirectionalLight;
}

/**
 * The renderer works as hard as ⚙️ Settings → You → Graphics says (see quality.ts): how sharp it
 * draws on a high-resolution screen, how many frames a second, and the sun's shadows. Fewer frames
 * while you stand and watch, since the office around you moves slowly then, and fewer still while a
 * window of the office's own is open over it or another app is in front, since nobody is walking
 * around then; none at all in a hidden tab, as ever. The sun's shadows are drawn afresh only so
 * often a second: most of what casts them stands still.
 */
export function installGraphics(ctx: Ctx, deps: GraphicsDeps) {
  let quality = loadGraphics();
  /** Another app (or window) is in front of the office's. */
  let away = !document.hasFocus();
  /** A window of the office's own is open over the 3D. */
  let covered = modalOpen();
  /** When you last pressed a key, moved the mouse or the view moved (performance.now()). */
  let busyAt = performance.now();
  const busy = () => (busyAt = performance.now());

  const shadows = ctx.renderer.shadowMap;
  shadows.autoUpdate = false;
  shadows.needsUpdate = true;
  /** When the sun's shadows were last drawn afresh. */
  let shadowsAt = -Infinity;

  function apply() {
    const preset = GRAPHICS[quality];
    const ratio = pixelRatioFor(quality, window.devicePixelRatio);
    if (ctx.renderer.getPixelRatio() !== ratio) ctx.renderer.setPixelRatio(ratio);
    const { shadow } = deps.sun;
    if (shadow.mapSize.x !== preset.shadowMap) {
      shadow.mapSize.set(preset.shadowMap, preset.shadowMap);
      // A shadow map is made once, at its size: the next frame makes it again at the new one.
      shadow.map?.dispose();
      shadow.map = null;
      shadows.needsUpdate = true;
    }
  }
  apply();
  onGraphics((q) => {
    quality = q;
    apply();
  });
  // Dragged over to a screen of another resolution.
  window.addEventListener('resize', apply);
  window.addEventListener('focus', () => (away = false));
  // Clicking into a frame on the page (the DJ's YouTube player) blurs the window too, but the office is still in front.
  window.addEventListener('blur', () => setTimeout(() => (away = !document.hasFocus())));
  onModalChange((open) => (covered = open));
  for (const type of ['keydown', 'pointerdown', 'wheel', 'touchstart']) window.addEventListener(type, busy, { capture: true, passive: true });
  // Only a mouse that really moved: the browser sends one that didn't whenever what's under it changes.
  window.addEventListener('pointermove', (e) => (e.movementX || e.movementY) && busy(), { capture: true, passive: true });

  // The view moving counts as doing something too, keys or not: a ride in the lift, a car, a fall.
  const seen = { at: new THREE.Vector3(), turn: new THREE.Quaternion(), fov: 0 };
  ctx.ticks.add('hud', ({ now }) => {
    const { camera } = ctx;
    if (seen.at.distanceToSquared(camera.position) > 1e-6 || 1 - Math.abs(seen.turn.dot(camera.quaternion)) > 1e-7 || seen.fov !== camera.fov) {
      seen.at.copy(camera.position);
      seen.turn.copy(camera.quaternion);
      seen.fov = camera.fov;
      busyAt = now;
    }
    if (shadowDue(quality, now, shadowsAt)) {
      shadows.needsUpdate = true;
      shadowsAt = now;
    }
  });
  ctx.pace.cap(() => fpsFor(quality, away || covered, performance.now() - busyAt > IDLE_AFTER));
}
