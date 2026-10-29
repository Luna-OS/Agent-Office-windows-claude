import * as THREE from 'three';
import { mesh, textSprite, toon } from '../toon';
import { ease, popIn } from './curves';
import { undress } from './props';
import type { WorkerRig } from './rig';

// A worker on a break at its desk, while the PC of the player it runs on is off (WorkerInfo.pause):
// leaning back with a mug of coffee and taking a sip now and then, scrolling on its phone and having a
// laugh, or eating a doughnut bite by bite. Every so often it moves on to the next of them.

export type PauseKind = 'kaffee' | 'handy' | 'essen';
const KINDS: PauseKind[] = ['kaffee', 'handy', 'essen'];
/** How long it keeps at one before the next (seconds). */
const KIND_SECONDS = 40;
/** Its status light on a break: a warm amber, breathing slowly. */
export const PAUSE_BULB = '#f4a261';

/** What its body is doing on its break, eased from one to the next (see unwind). */
interface Pose {
  armLx: number;
  armLz: number;
  armRx: number;
  armRz: number;
  /** 0..1: each shoulder brought forward and in, to reach the mouth or hold the phone in front. */
  reachL: number;
  reachR: number;
  lean: number;
  roll: number;
  lift: number;
  kick: number;
  look: number;
  lid: number;
  /** The mug or the doughnut tipped toward its mouth. */
  tip: number;
}

export interface Pausing {
  kind: PauseKind;
  seed: number;
  pose: Pose;
  /** How much of each kind's prop is in its hands right now (0..1). */
  shown: Record<PauseKind, number>;
  mug: THREE.Group;
  steam: THREE.Mesh[];
  phone: THREE.Group;
  feed: THREE.Mesh[];
  doughnut: THREE.Group;
  crumbs: THREE.Mesh[];
  /** Everything it brought, to take away again (see endBreak). */
  parts: THREE.Object3D[];
}

/** A mug of coffee with a curl of steam over it; held round its middle. */
function mug(): { group: THREE.Group; steam: THREE.Mesh[] } {
  const group = new THREE.Group();
  group.add(mesh(new THREE.CylinderGeometry(0.065, 0.058, 0.13, 14), toon('#e63946'), 0, 0, 0, false));
  group.add(mesh(new THREE.CylinderGeometry(0.056, 0.056, 0.01, 14), toon('#6f4518'), 0, 0.062, 0, false));
  const handle = mesh(new THREE.TorusGeometry(0.035, 0.012, 6, 12), toon('#e63946'), 0.07, 0, 0, false);
  handle.rotation.y = Math.PI / 2;
  group.add(handle);
  const puff = toon('#ffffff', { opacity: 0.65 });
  const steam = [0, 1, 2].map(() => mesh(new THREE.SphereGeometry(0.026, 8, 6), puff, 0, 0.09, 0, false));
  group.add(...steam);
  return { group, steam };
}

/** A phone, its screen on its -z face with a feed scrolling up it. */
function phone(): { group: THREE.Group; feed: THREE.Mesh[] } {
  const group = new THREE.Group();
  group.add(mesh(new THREE.BoxGeometry(0.1, 0.18, 0.016), toon('#2b2d42'), 0, 0, 0, false));
  group.add(mesh(new THREE.BoxGeometry(0.086, 0.158, 0.004), toon('#4cc9f0', { emissive: '#1b6f8a' }), 0, 0, -0.009, false));
  const feed = ['#fffaf3', '#ffd166', '#fffaf3', '#ff8fab'].map((c, i) => mesh(new THREE.BoxGeometry(0.066, i % 2 ? 0.034 : 0.012, 0.003), toon(c, { emissive: '#444444' }), 0, 0, -0.012, false));
  group.add(...feed);
  return { group, feed };
}

/** A pink-iced doughnut with sprinkles, held up on its edge. */
function doughnut(): THREE.Group {
  const group = new THREE.Group();
  group.add(mesh(new THREE.TorusGeometry(0.058, 0.03, 8, 18), toon('#d4a373'), 0, 0, 0, false));
  const icing = mesh(new THREE.TorusGeometry(0.058, 0.026, 8, 18), toon('#ff8fab'), 0, 0, 0.01, false);
  icing.scale.z = 0.7;
  group.add(icing);
  ['#ffd166', '#4cc9f0', '#06d6a0', '#fffaf3', '#9d4edd', '#ffd166'].forEach((c, i) => {
    const a = (i / 6) * Math.PI * 2;
    const s = mesh(new THREE.BoxGeometry(0.018, 0.006, 0.006), toon(c), Math.cos(a) * 0.058, Math.sin(a) * 0.058, 0.034, false);
    s.rotation.z = a * 1.7;
    group.add(s);
  });
  return group;
}

/** Sits it back for a break, with its mug, phone and doughnut to hand. Its `id` spreads breaks out across workers. */
export function takeBreak(rig: WorkerRig, id: string): Pausing {
  let h = 0;
  for (const ch of id) h = (h * 31 + ch.charCodeAt(0)) | 0;
  const m = mug();
  const p = phone();
  const d = doughnut();
  const crumb = toon('#d4a373');
  const crumbs = [0, 1, 2, 3].map(() => mesh(new THREE.SphereGeometry(0.012, 5, 4), crumb, 0, 0, 0, false));
  // Its hands are at the end of its arms, about this far down from the shoulder.
  for (const [prop, arm] of [[m.group, rig.armR], [d, rig.armR], [p.group, rig.armL]] as const) {
    prop.position.set(0, -0.27, 0.03);
    prop.rotation.order = 'ZYX';
    prop.visible = false;
    arm.add(prop);
  }
  for (const c of crumbs) ((c.visible = false), rig.body.add(c));
  const seed = (Math.abs(h % 997) / 997) * KINDS.length;
  const pose: Pose = { armLx: -0.3, armLz: 0, armRx: -0.3, armRz: 0, reachL: 0, reachR: 0, lean: 0, roll: 0, lift: 0, kick: 0, look: 0, lid: 1, tip: 0 };
  return {
    kind: kindAt(seed),
    seed,
    pose,
    shown: { kaffee: 0, handy: 0, essen: 0 },
    mug: m.group,
    steam: m.steam,
    phone: p.group,
    feed: p.feed,
    doughnut: d,
    crumbs,
    parts: [m.group, p.group, d, ...crumbs],
  };
}

/** Which of them it's at now: the same for everyone looking, give or take their clocks. */
function kindAt(seed: number): PauseKind {
  return KINDS[Math.floor(Date.now() / 1000 / KIND_SECONDS + seed) % KINDS.length];
}

/** Back to work: its things go away (its arms and the rest are posed again by Worker.update). */
export function endBreak(p: Pausing) {
  undress(p.parts);
}

/** The bubble over its head on a break, and a key that changes when it would look different. */
export function pauseBubble(p: Pausing): { key: string; draw(): THREE.Sprite } {
  const text = p.kind === 'kaffee' ? '☕ Kaffeepause' : p.kind === 'handy' ? '📱 am Handy' : '🍩 Snackpause';
  return { key: `pause|${p.kind}`, draw: () => textSprite(text, { bg: '#ffe8cc', size: 34 }) };
}

/** 0 → 1, held, → 0 over a cycle `c` seconds in: up at `at`, there for `hold`, back over `back`. */
function raise(c: number, at: number, up: number, hold: number, back: number): number {
  if (c < at) return 0;
  if (c < at + up) return ease((c - at) / up);
  if (c < at + up + hold) return 1;
  if (c < at + up + hold + back) return 1 - ease((c - at - up - hold) / back);
  return 0;
}

/** What each kind of break looks like `c` seconds into its cycle, into `s`. */
function target(kind: PauseKind, c: number, t: number, s: Pose): Pose {
  // Leaning back, one hand resting on its belly, feet out: the same for all of them.
  Object.assign(s, { armLx: -0.55, armLz: 0.45, reachL: 0.3, lean: -0.1, roll: Math.sin(t * 0.9) * 0.025, lift: 0, kick: 0.06, look: 0, lid: 1, tip: 0 });
  if (kind === 'handy') {
    // Hunched over its phone, thumbing through it, and every so often something makes it laugh.
    const laugh = raise(c % 9, 6, 0.15, 1, 0.3);
    Object.assign(s, { armLx: -1.15, armLz: 0.55, reachL: 0.6, armRx: -1.2 + Math.sin(t * 5) * 0.07, armRz: -0.6, reachR: 0.6, lean: 0.08, look: -0.045 + laugh * 0.03, kick: 0.02 });
    s.lift = laugh * Math.abs(Math.sin(t * 28)) * 0.03;
    s.roll += laugh * Math.sin(t * 20) * 0.05;
    s.lid = 1 - laugh * 0.65;
    return s;
  }
  // The mug or the doughnut: held in front, now and then brought up to its mouth.
  const cycle = kind === 'kaffee' ? 7 : 6;
  const up = kind === 'kaffee' ? raise(c % cycle, 3.6, 0.6, 1, 0.8) : raise(c % cycle, 2.4, 0.4, 0.4, 0.5);
  s.armRx = -0.95 + (-1.74 + 0.95) * up;
  s.armRz = -0.35 + (-0.71 + 0.35) * up;
  s.reachR = 0.2 + 0.8 * up;
  s.look = up * 0.012;
  if (kind === 'kaffee') {
    // A sip: tipped toward its mouth, eyes shut to enjoy it, then a contented sigh.
    s.tip = -0.9 * up * up;
    s.lid = 1 - 0.75 * up;
    s.lift = raise(c % cycle, 6, 0.3, 0.2, 0.5) * 0.02;
  } else {
    // A bite, then chewing for a bit, its eyes half shut.
    const chew = raise(c % cycle, 3.6, 0.15, 1.3, 0.3);
    s.lift = chew * Math.abs(Math.sin(t * 14)) * 0.012;
    s.lid = 1 - 0.35 * chew;
  }
  return s;
}

const scratch = {} as Pose;

/** A frame of its break. Returns how open its eyes are (for its blink). */
export function unwind(rig: WorkerRig, p: Pausing, dt: number, t: number): number {
  p.kind = kindAt(p.seed);
  const c = t + p.seed * 10;
  const goal = target(p.kind, c, t, scratch);
  const k = Math.min(1, dt * 5);
  const s = p.pose;
  for (const key of Object.keys(goal) as (keyof Pose)[]) s[key] += (goal[key] - s[key]) * k;

  rig.armL.rotation.set(s.armLx, 0, s.armLz);
  rig.armR.rotation.set(s.armRx, 0, s.armRz);
  rig.armL.position.set(-0.3 + s.reachL * 0.07, 0.55, 0.05 + s.reachL * 0.14);
  rig.armR.position.set(0.3 - s.reachR * 0.07, 0.55 + s.reachR * 0.04, 0.05 + s.reachR * 0.14);
  rig.body.rotation.set(s.lean, 0, s.roll);
  rig.body.position.y = s.lift;
  rig.body.scale.setScalar(1);
  rig.feet.forEach((f, i) => f.position.set(i ? 0.12 : -0.12, 0.2, 0.05 + s.kick));
  for (const pu of rig.pupils) pu.position.y = 0.7 + s.look;
  const glow = 0.35 + 0.25 * Math.sin(t * 1.5);
  rig.bulb.color.set(PAUSE_BULB);
  rig.bulb.emissive.set(PAUSE_BULB).multiplyScalar(glow);
  rig.bulbMesh.scale.setScalar(1);

  // Each prop comes and goes with its kind, and stays upright in its hand whichever way the arm turns.
  for (const kind of KINDS) p.shown[kind] += ((kind === p.kind ? 1 : 0) - p.shown[kind]) * Math.min(1, dt * 4);
  const hold = (prop: THREE.Object3D, kind: PauseKind, armX: number, armZ: number, tip: number, size = 1) => {
    const w = p.shown[kind];
    prop.visible = w > 0.02;
    if (!prop.visible) return false;
    prop.scale.setScalar(Math.max(0.001, popIn(w)) * size);
    prop.rotation.set(-armX - s.lean + tip, 0, -armZ);
    return true;
  };
  if (hold(p.mug, 'kaffee', s.armRx, s.armRz, s.tip)) {
    p.steam.forEach((puff, i) => {
      const v = (t * 0.55 + i / 3) % 1;
      puff.position.set(Math.sin(v * 6 + i * 2) * 0.022, 0.08 + v * 0.2, 0);
      puff.scale.setScalar(0.4 + v * 0.9);
      puff.visible = v < 0.92;
    });
  }
  // Facing up at its eyes: the screen tipped back toward them, the feed scrolling up it.
  if (hold(p.phone, 'handy', s.armLx, s.armLz, 1.1)) {
    p.feed.forEach((bar, i) => {
      const y = ((i * 0.042 + t * 0.045) % 0.168) - 0.084;
      bar.position.y = y;
      bar.visible = Math.abs(y) < 0.068;
    });
  }
  // The doughnut gets smaller bite by bite, and a fresh one comes out after the last.
  const bites = (((Math.floor((c - 3.1) / 6)) % 5) + 5) % 5;
  hold(p.doughnut, 'essen', s.armRx, s.armRz, s.tip, 1 - bites * 0.15);
  p.crumbs.forEach((crumb, i) => {
    const fall = (c % 6) - 3.65 - i * 0.12;
    crumb.visible = p.kind === 'essen' && fall > 0 && fall < 0.5;
    if (crumb.visible) crumb.position.set((i - 1.5) * 0.03, 0.56 - 1.1 * fall * fall, 0.36);
  });
  return s.lid;
}
