// How hard the 3D office works your computer: picked in ⚙️ Settings → You, kept in this browser.
// Pure apart from localStorage, so the tests run it under node.

export type GraphicsQuality = 'eco' | 'balanced' | 'high';

export interface GraphicsPreset {
  label: string;
  /** What it does, for the note under the buttons. */
  blurb: string;
  /** The sharpest it draws on a high-resolution screen (devicePixelRatio, at most this). */
  pixelRatio: number;
  /** The most frames a second while you're in the office (0: as many as the screen shows). */
  fps: number;
  /** The most while you stand and watch: no key, the mouse still, the view not moving (0: as `fps`). */
  idleFps: number;
  /** The most while a window is open over it, or another app is in front (0: no limit). */
  quietFps: number;
  /** How many times a second the sun's shadows are drawn afresh (0: every frame). Most of the office stands still. */
  shadowHz: number;
  /** The sun's shadow map, a side in pixels. */
  shadowMap: number;
  /** Asks for the computer's power-saving graphics where it has two (a laptop's built-in one). Takes a reload. */
  lowPower: boolean;
}

export const GRAPHICS: Record<GraphicsQuality, GraphicsPreset> = {
  eco: {
    label: '🍃 Eco',
    blurb: '30 frames a second (15 behind a window), drawn at the screen’s own size and with softer shadows, on the power-saving graphics of a laptop. The same office, the same toon look: for a laptop on battery, or several apps at once.',
    pixelRatio: 1,
    fps: 30,
    idleFps: 30,
    quietFps: 15,
    shadowHz: 15,
    shadowMap: 1024,
    lowPower: true,
  },
  balanced: {
    label: '⚖️ Balanced',
    blurb: '60 frames a second while you walk or look around, 30 while you stand and watch (20 while a window is open or another app is in front), the shadows drawn afresh 30 times a second, and a little less than full sharpness on a Retina or 4K screen, where nobody can tell. Looks as good as High, for a good deal less work.',
    pixelRatio: 1.5,
    fps: 60,
    idleFps: 30,
    quietFps: 20,
    shadowHz: 30,
    shadowMap: 2048,
    lowPower: false,
  },
  high: {
    label: '✨ High',
    blurb: 'As many frames as the screen shows (120 or 144 on a fast one) and full sharpness, always. For a desktop with a strong graphics card.',
    pixelRatio: 2,
    fps: 0,
    idleFps: 0,
    quietFps: 0,
    shadowHz: 0,
    shadowMap: 2048,
    lowPower: false,
  },
};

export const GRAPHICS_ORDER: readonly GraphicsQuality[] = ['eco', 'balanced', 'high'];
export const DEFAULT_GRAPHICS: GraphicsQuality = 'balanced';

const KEY = 'agent-office.graphics';
const isQuality = (v: unknown): v is GraphicsQuality => typeof v === 'string' && v in GRAPHICS;

export function loadGraphics(): GraphicsQuality {
  try {
    const saved = localStorage.getItem(KEY);
    return isQuality(saved) ? saved : DEFAULT_GRAPHICS;
  } catch {
    return DEFAULT_GRAPHICS;
  }
}

const listeners = new Set<(q: GraphicsQuality) => void>();

/** Picks `q`, keeps it in this browser and tells the office's renderer (see installGraphics). */
export function setGraphics(q: GraphicsQuality) {
  try {
    localStorage.setItem(KEY, q);
  } catch {
    // storage blocked: it lasts until the page is reloaded
  }
  for (const fn of listeners) fn(q);
}

export function onGraphics(fn: (q: GraphicsQuality) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** The pixel ratio to draw at, for a screen of `dpr`. Never above the screen's own. */
export function pixelRatioFor(q: GraphicsQuality, dpr: number): number {
  return Math.max(0.5, Math.min(dpr || 1, GRAPHICS[q].pixelRatio));
}

/**
 * The most frames a second (0: no limit): while you're in the office, while you stand and watch
 * (`idle`), or while it's `quiet` (a window over it, another app in front).
 */
export function fpsFor(q: GraphicsQuality, quiet: boolean, idle = false): number {
  const p = GRAPHICS[q];
  if (quiet) return p.quietFps;
  return idle && p.idleFps ? p.idleFps : p.fps;
}

/** How long after the last key, mouse move or view change you count as standing and watching (ms). */
export const IDLE_AFTER = 1500;

/** Whether the sun's shadows are drawn afresh in the frame at `now`, last drawn at `last` (ms). A frame a little early still counts. */
export function shadowDue(q: GraphicsQuality, now: number, last: number): boolean {
  const hz = GRAPHICS[q].shadowHz;
  return !hz || now - last >= (1000 / hz) * 0.8;
}
