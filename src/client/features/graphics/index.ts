import type * as THREE from 'three';
import type { Ctx } from '../../core/context';
import { modalOpen, onModalChange } from '../../ui/dom';
import { GRAPHICS, fpsFor, loadGraphics, onGraphics, pixelRatioFor } from './quality';

export interface GraphicsDeps {
  /** The sun, whose shadow map is the one that costs (see core/scene.ts). */
  sun: THREE.DirectionalLight;
}

/**
 * The renderer works as hard as ⚙️ Settings → You → Graphics says (see quality.ts): how sharp it
 * draws on a high-resolution screen, how many frames a second, and the sun's shadows. Fewer frames
 * still while a window of the office's own is open over it or another app is in front, since
 * nobody is walking around then; none at all in a hidden tab, as ever.
 */
export function installGraphics(ctx: Ctx, deps: GraphicsDeps) {
  let quality = loadGraphics();
  /** Another app (or window) is in front of the office's. */
  let away = !document.hasFocus();
  /** A window of the office's own is open over the 3D. */
  let covered = modalOpen();

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
  window.addEventListener('blur', () => (away = true));
  onModalChange((open) => (covered = open));
  ctx.pace.cap(() => fpsFor(quality, away || covered));
}
