// Der DJ auf dem Dach (features/dj): welcher YouTube-Song gerade läuft. Einer fürs ganze Haus, nur im
// Speicher: Startet das Office neu, spielt der DJ wieder sein eigenes Set.
import type { DjState } from '../shared/protocol.js';
import { parseYoutube } from '../shared/youtube.js';

export class DjBooth {
  private now: DjState | null = null;

  state(): DjState | null {
    return this.now;
  }

  /** Legt den Song unter `url` auf; mit einer deutschen Meldung, wenn es keine YouTube-Adresse ist. */
  play(url: unknown, by: string, at = Date.now()): { error: string } | { state: DjState } {
    let video;
    try {
      video = parseYoutube(String(url ?? '').slice(0, 2048));
    } catch (err) {
      return { error: (err as Error).message };
    }
    this.now = { video: video.id, start: video.start, startedAt: at, by };
    return { state: this.now };
  }

  /** Nimmt den Song runter; false, wenn keiner lief. */
  stop(): boolean {
    const was = !!this.now;
    this.now = null;
    return was;
  }
}

const booths = new WeakMap<object, DjBooth>();

/** Das DJ-Pult dieses Office (eins fürs ganze Haus). */
export function djBooth(office: object): DjBooth {
  let b = booths.get(office);
  if (!b) booths.set(office, (b = new DjBooth()));
  return b;
}
