import * as THREE from 'three';
import { disposeSprite, mesh, textSprite, toon } from '../toon';
import { undress } from './props';
import type { Rest, WorkerRig } from './rig';

// A worker that has hit its limit (WorkerInfo.limit): asleep in its chair until it can go on, sunk back
// with its head to one side under a tartan blanket tucked in under its chin, breathing slowly, its
// eyes shut, now and then a little snore that jolts it, and Zs drifting up off it.

/** Its status light while it sleeps: a deep blue, glowing slowly. */
export const SLEEP_BULB = '#4361ee';
/** One breath, in seconds. */
const BREATH = 4.6;
/** A Z drifts up off it over this many seconds, three at a time. */
const Z_SECONDS = 3.3;

/** A tartan blanket over its front and its lap, tucked in under its chin with a rolled hem. Forward is +z. */
function blanket(): THREE.Group {
  const g = new THREE.Group();
  const wool = toon('#4361ee');
  const light = toon('#a0c4ff');
  const red = toon('#ef476f');
  // Over its front, from under its chin down to its lap, wrapped round the bean.
  const arc = (r: number, h: number, y: number, m: THREE.Material) => mesh(new THREE.CylinderGeometry(r, r + 0.02, h, 22, 1, true, -Math.PI * 0.56, Math.PI * 1.12), m, 0, y, 0, false);
  g.add(arc(0.31, 0.4, 0.42, wool));
  for (const [y, m] of [[0.31, light], [0.42, red], [0.53, light]] as const) g.add(arc(0.315, 0.022, y, m));
  // The rolled hem under its chin.
  const hem = new THREE.Group();
  const roll = mesh(new THREE.TorusGeometry(0.315, 0.035, 8, 26, Math.PI * 1.1), wool, 0, 0, 0, false);
  roll.rotation.x = Math.PI / 2;
  hem.add(roll);
  hem.rotation.y = Math.PI * 0.05;
  hem.position.y = 0.62;
  g.add(hem);
  // Over its lap and its legs stretched out, hanging down over its feet at the front.
  const lap = new THREE.Group();
  lap.position.set(0, 0.23, 0.2);
  lap.rotation.x = 0.22;
  lap.add(mesh(new THREE.BoxGeometry(0.66, 0.06, 0.4), wool, 0, 0, 0.17, false));
  for (const x of [-0.17, 0.17]) lap.add(mesh(new THREE.BoxGeometry(0.03, 0.065, 0.4), light, x, 0, 0.17, false));
  lap.add(mesh(new THREE.BoxGeometry(0.665, 0.065, 0.03), red, 0, 0, 0.12, false));
  lap.add(mesh(new THREE.BoxGeometry(0.66, 0.16, 0.05), wool, 0, -0.08, 0.37, false));
  g.add(lap);
  return g;
}

/** "bis 15:00", or "bis Mo 09:00" when it's further off than today, in this browser's time. */
function bis(until: number | undefined): string {
  if (until === undefined) return '';
  const at = new Date(until);
  const time = at.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
  const day = until - Date.now() > 20 * 3600_000 ? `${at.toLocaleDateString('de-DE', { weekday: 'short' })} ` : '';
  return ` · bis ${day}${time}`;
}

/** Asleep under its blanket, for Worker.setRest, until `until` (ms) if it's known. */
export const asleep =
  (until: number | undefined) =>
  (rig: WorkerRig): Rest => {
    const cover = blanket();
    rig.body.add(cover);
    const zs = [0, 1, 2].map(() => {
      const z = textSprite('Z', { color: '#3a0ca3', size: 44 });
      rig.root.add(z);
      return { z, size: z.scale.clone() };
    });
    // Eased into from whatever it was doing.
    const s = { lean: 0, roll: 0, armX: -0.3, armZ: 0, kick: 0 };
    const text = `💤 Limit erreicht${bis(until)}`;
    return {
      bulb: SLEEP_BULB,
      frame(dt, t) {
        const k = Math.min(1, dt * 3);
        s.lean += (-0.3 - s.lean) * k;
        s.roll += (0.15 + Math.sin(t * 0.45) * 0.03 - s.roll) * k;
        s.armX += (-0.35 - s.armX) * k;
        s.armZ += (0.55 - s.armZ) * k;
        s.kick += (0.12 - s.kick) * k;
        // Breathing slowly in and out, and now and then a snore that jolts it.
        const breath = Math.sin((t / BREATH) * Math.PI * 2);
        const snore = t % 9 < 0.5 ? Math.sin(((t % 9) / 0.5) * Math.PI) : 0;
        rig.armL.rotation.set(s.armX, 0, s.armZ);
        rig.armR.rotation.set(s.armX, 0, -s.armZ);
        rig.armL.position.set(-0.3, 0.55, 0.05);
        rig.armR.position.set(0.3, 0.55, 0.05);
        rig.body.rotation.set(s.lean - snore * 0.06, 0, s.roll);
        rig.body.position.y = breath * 0.008 + snore * 0.025;
        rig.body.scale.set(1 + breath * 0.012, 1 + breath * 0.025, 1 + breath * 0.012);
        rig.feet.forEach((f, i) => f.position.set(i ? 0.12 : -0.12, 0.2, 0.05 + s.kick));
        for (const p of rig.pupils) p.position.y = 0.68;
        rig.bulb.color.set(SLEEP_BULB);
        rig.bulb.emissive.set(SLEEP_BULB).multiplyScalar(0.2 + 0.2 * (breath + 1) * 0.5);
        rig.bulbMesh.scale.setScalar(1);
        // Zs drift up and off to the side, growing and fading as they go.
        zs.forEach(({ z, size }, i) => {
          const v = (t / Z_SECONDS + i / 3) % 1;
          z.position.set(0.2 + v * 0.35 + Math.sin(v * 7 + i) * 0.04, 1.0 + v * 0.55, 0.05);
          z.scale.copy(size).multiplyScalar(0.45 + v * 0.8);
          z.material.opacity = v < 0.15 ? v / 0.15 : 1 - (v - 0.15) / 0.85;
        });
        return 0.06;
      },
      bubble: () => ({ key: `schlaf|${text}`, draw: () => textSprite(text, { bg: '#dbe4ff', size: 34 }) }),
      end() {
        undress([cover]);
        for (const { z } of zs) (z.removeFromParent(), disposeSprite(z));
      },
    };
  };
