import type { DjState } from '../../../shared/protocol';
import type { Slice } from '../store';

declare module '../store' {
  interface Store {
    /** Der YouTube-Song, den der DJ auf dem Dach gerade spielt (null: sein eigenes Set, siehe features/dj). */
    dj: DjState | null;
  }
  interface Topics {
    dj: true;
  }
}

export const dj: Slice = {
  init(s) {
    s.dj = null;
  },
  on: {
    dj(s, m) {
      s.dj = m.state;
      return ['dj'];
    },
  },
  enter(s, v) {
    s.dj = v.dj;
    return ['dj'];
  },
};
