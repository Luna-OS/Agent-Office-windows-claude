import * as THREE from 'three';
import { mesh, toon, toonUnique } from '../toon';
import { STATUS_BULB } from './worker-badges';

/** The parts of a worker's body that move or change: its eyes, headset, status light, arms and feet. */
export interface WorkerBody {
  eyes: THREE.Mesh[];
  pupils: THREE.Mesh[];
  headset: THREE.Object3D[];
  bulb: THREE.MeshToonMaterial;
  bulbMesh: THREE.Mesh;
  armL: THREE.Object3D;
  armR: THREE.Object3D;
  feet: THREE.Mesh[];
}

/** Builds the little Claude worker's bean of a body onto `body`, in `skin`. Forward is +z. */
export function buildBody(body: THREE.Group, skin: THREE.MeshToonMaterial): WorkerBody {
  const white = toon('#ffffff');
  const ink = toon('#1d1d1d');
  const eyes: THREE.Mesh[] = [];
  const pupils: THREE.Mesh[] = [];
  const headset: THREE.Object3D[] = [];
  const feet: THREE.Mesh[] = [];
  // Bean-shaped body
  body.add(mesh(new THREE.CapsuleGeometry(0.28, 0.3, 8, 16), skin, 0, 0.55, 0));
  // Big cartoon eyes
  for (const sx of [-1, 1]) {
    const eye = mesh(new THREE.SphereGeometry(0.09, 12, 10), white, sx * 0.11, 0.7, 0.23, false);
    eye.scale.z = 0.6;
    body.add(eye);
    const pupil = mesh(new THREE.SphereGeometry(0.045, 10, 8), ink, sx * 0.11, 0.7, 0.29, false);
    body.add(pupil);
    eyes.push(eye, pupil);
    pupils.push(pupil);
  }
  // Headset: band + mic
  const band = mesh(new THREE.TorusGeometry(0.29, 0.025, 6, 20, Math.PI), toon('#2b2d42'), 0, 0.72, 0, false);
  band.rotation.y = Math.PI / 2;
  body.add(band);
  headset.push(band);
  for (const sx of [-1, 1]) {
    const cup = mesh(new THREE.SphereGeometry(0.07, 10, 8), toon('#2b2d42'), sx * 0.29, 0.72, 0, false);
    body.add(cup);
    headset.push(cup);
  }
  // Antenna with status bulb
  body.add(mesh(new THREE.CylinderGeometry(0.015, 0.015, 0.22, 6), toon('#2b2d42'), 0, 1.07, 0, false));
  const bulb = toonUnique(STATUS_BULB.starting);
  bulb.emissive = new THREE.Color(STATUS_BULB.starting).multiplyScalar(0.6);
  const bulbMesh = mesh(new THREE.SphereGeometry(0.075, 12, 10), bulb, 0, 1.2, 0, false);
  body.add(bulbMesh);

  const arm = (x: number) => {
    const pivot = new THREE.Group();
    pivot.position.set(x, 0.55, 0.05);
    pivot.add(mesh(new THREE.CapsuleGeometry(0.055, 0.16, 4, 8), skin, 0, -0.12, 0));
    body.add(pivot);
    return pivot;
  };
  const armL = arm(-0.3);
  const armR = arm(0.3);
  for (const sx of [-1, 1]) {
    const foot = mesh(new THREE.CapsuleGeometry(0.06, 0.1, 4, 8), skin, sx * 0.12, 0.2, 0.05);
    body.add(foot);
    feet.push(foot);
  }
  return { eyes, pupils, headset, bulb, bulbMesh, armL, armR, feet };
}
