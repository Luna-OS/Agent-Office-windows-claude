/**
 * Arbeiter reden miteinander (worker.says, siehe server/gespraech.ts): Über dem, der spricht, steht eine
 * Sprechblase mit dem, was er sagt; ein Papierflieger fliegt von seinem Platz in hohem Bogen zu dem, den
 * er anspricht, und über dem steht, sobald er ankommt, von wem er Post hat. Im Chat steht es auch.
 */
import * as THREE from 'three';
import type { Ctx } from '../../core/context';
import { store } from '../../state';
import type { Worker } from '../../world/character';
import { cardSprite, disposeSprite, textSprite } from '../../world/toon';

export interface GespraechDeps {
  /** Die Arbeiter auf dieser Etage, wie sie an ihren Plätzen sitzen. */
  workerViews: ReadonlyMap<string, { model: Worker }>;
}

/** So lange steht die Sprechblase über dem, der spricht, und die über dem, der Post bekommt (Sekunden). */
const SAGT = 6;
const POST = 3.5;
/** Die Blasen schweben über dem Namen und der Karte, die sagt, woran er arbeitet. */
const HOCH = 2.95;
/** Wie groß der Papierflieger ist. */
const FLIEGER = 2.2;

interface Blase {
  sprite: THREE.Sprite;
  /** Wie hoch sie schwebt: eine Karte steht auf ihrer Spitze, eine Zeile auf ihrer Mitte. */
  y: number;
  bis: number;
  lang: number;
}

interface Flieger {
  mesh: THREE.Mesh;
  von: THREE.Vector3;
  nach: THREE.Vector3;
  hoch: number;
  t: number;
  dauer: number;
  an: string;
  vonName: string;
}

/** Ein Papierflieger aus vier Dreiecken, die Spitze nach +z. */
function papierflieger(material: THREE.Material): THREE.Mesh {
  const nase = [0, 0, 0.2];
  const links = [-0.14, 0.03, -0.1];
  const rechts = [0.14, 0.03, -0.1];
  const heck = [0, 0.02, -0.12];
  const kiel = [0, -0.05, -0.1];
  const v = [nase, heck, links, nase, rechts, heck, nase, kiel, heck, nase, heck, kiel].flat();
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(v, 3));
  geo.computeVertexNormals();
  return new THREE.Mesh(geo, material);
}

export function installGespraech(ctx: Ctx, deps: GespraechDeps) {
  const papier = new THREE.MeshToonMaterial({ color: '#e7f0ff', side: THREE.DoubleSide });
  const blasen = new Map<string, Blase>();
  const flieger: Flieger[] = [];
  const name = (id: string) => store.workers.get(id)?.name ?? 'einem Kollegen';

  /** Über dem Arbeiter `id` eine Blase, `lang` Sekunden lang (an Stelle einer, die schon da ist). */
  function blase(id: string, make: () => THREE.Sprite, lang: number, now: number, y = HOCH) {
    const model = deps.workerViews.get(id)?.model;
    if (!model) return;
    weg(id);
    const sprite = make();
    sprite.position.y = y;
    model.root.add(sprite);
    blasen.set(id, { sprite, y, bis: now + lang * 1000, lang });
  }
  function weg(id: string) {
    const b = blasen.get(id);
    if (!b) return;
    b.sprite.removeFromParent();
    disposeSprite(b.sprite);
    blasen.delete(id);
  }
  /** Wo über dem Kopf des Arbeiters `id` der Flieger startet oder ankommt, in der Welt. */
  function kopf(id: string): THREE.Vector3 | undefined {
    const model = deps.workerViews.get(id)?.model;
    return model?.root.localToWorld(new THREE.Vector3(0, 1.3, 0));
  }

  ctx.messages.on('worker.says', (m) => {
    const now = performance.now();
    // Was er sagt, als Sprechblase über ihm: an wen, und die ersten Zeilen.
    blase(m.from, () => cardSprite({ chip: { text: `💬 an ${name(m.to)}`, bg: '#4cc9f0', color: '#2b2d42' }, title: m.text, bg: '#fffaf3', maxWidth: 360 }), SAGT, now);
    const von = kopf(m.from);
    const nach = kopf(m.to);
    // Sitzt der Angesprochene nicht hier (auf einer anderen Etage), gibt es nur die Sprechblase.
    if (!von || !nach) return;
    const weit = von.distanceTo(nach);
    const mesh = papierflieger(papier);
    mesh.scale.setScalar(FLIEGER);
    mesh.position.copy(von);
    ctx.scene.add(mesh);
    flieger.push({ mesh, von, nach, hoch: 1.2 + weit * 0.15, t: 0, dauer: Math.min(2.4, Math.max(0.9, 0.6 + weit * 0.12)), an: m.to, vonName: name(m.from) });
  });

  const da = new THREE.Vector3();
  ctx.ticks.add('world', ({ dt, now, t }) => {
    for (let i = flieger.length - 1; i >= 0; i--) {
      const f = flieger[i];
      f.t += dt;
      const u = Math.min(1, f.t / f.dauer);
      // Im Bogen hinüber, die Spitze voraus, ein wenig schaukelnd.
      const bahn = (x: number, out: THREE.Vector3) => out.copy(f.von).lerp(f.nach, x).setY(out.y + 4 * f.hoch * x * (1 - x));
      bahn(u, f.mesh.position);
      f.mesh.lookAt(bahn(Math.min(1, u + 0.03), da));
      f.mesh.rotateZ(Math.sin(t * 9 + i) * 0.25);
      if (u < 1) continue;
      f.mesh.removeFromParent();
      f.mesh.geometry.dispose();
      flieger.splice(i, 1);
      blase(f.an, () => textSprite(`📨 von ${f.vonName}`, { bg: '#fffaf3', size: 32, border: '#2b2d42' }), POST, now, HOCH + 0.2);
    }
    for (const [id, b] of blasen) {
      const rest = (b.bis - now) / 1000;
      if (rest <= 0 || !b.sprite.parent) {
        weg(id);
        continue;
      }
      // Die letzte halbe Sekunde verblasst sie, und sie wippt sacht.
      b.sprite.material.opacity = Math.min(1, rest / 0.5);
      b.sprite.position.y = b.y + Math.sin(t * 2.5) * 0.025;
    }
  });
}
