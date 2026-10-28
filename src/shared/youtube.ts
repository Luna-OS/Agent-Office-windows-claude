// YouTube-Adressen lesen: Video-ID und Startzeit aus allen üblichen Formen, für den DJ auf dem Dach
// (features/dj). `watch?v=`, `youtu.be/`, `shorts/`, `embed/`, `live/`, `v/`, `m.`/`music.`/`www.`,
// `youtube-nocookie.com`, Startzeit als `t=90`, `t=1m30s`, `t=1h2m3s`, `start=90`, auch hinter `#`.
// Eine nackte Video-ID geht auch.

export interface YoutubeVideo {
  /** Die Video-ID: 11 Zeichen aus A–Z, a–z, 0–9, - und _. */
  id: string;
  /** Wo es losgehen soll, in Sekunden (0 ohne Angabe). */
  start: number;
}

const YOUTUBE = new Set(['youtube.com', 'www.youtube.com', 'm.youtube.com', 'music.youtube.com', 'youtube-nocookie.com', 'www.youtube-nocookie.com']);

/** Eine Video-ID: genau 11 Zeichen aus `A-Z a-z 0-9 - _`. */
export const isVideoId = (s: string): boolean => /^[A-Za-z0-9_-]{11}$/.test(s);

/** `90`, `90s`, `1m30s`, `1h2m3s`, `01:30`, `1:02:03` in Sekunden; undefined, wenn es keine Zeit ist. */
export function readTime(s: string): number | undefined {
  const t = s.trim();
  if (!t) return undefined;
  if (/^\d+(\.\d+)?$/.test(t)) return Number(t);
  if (/^\d+(:\d+)+$/.test(t)) return t.split(':').reduce((sum, part) => sum * 60 + Number(part), 0);
  const m = /^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s?)?$/.exec(t);
  if (!m || (!m[1] && !m[2] && !m[3])) return undefined;
  return Number(m[1] ?? 0) * 3600 + Number(m[2] ?? 0) * 60 + Number(m[3] ?? 0);
}

function params(s: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const pair of s.split('&')) {
    if (!pair) continue;
    const i = pair.indexOf('=');
    const k = i < 0 ? pair : pair.slice(0, i);
    let v = i < 0 ? '' : pair.slice(i + 1).replace(/\+/g, ' ');
    try {
      v = decodeURIComponent(v);
    } catch {
      // kaputte Prozent-Kodierung: so lassen, wie sie ist
    }
    if (!out.has(k)) out.set(k, v);
  }
  return out;
}

/** Liest eine YouTube-Adresse (oder eine nackte Video-ID). Wirft mit einer deutschen Meldung, wenn es keine ist. */
export function parseYoutube(input: string): YoutubeVideo {
  const s = String(input ?? '').trim();
  if (!s) throw new Error('Bitte eine YouTube-Adresse eingeben');
  if (s.length > 2048) throw new Error('Die Adresse ist zu lang');
  if (isVideoId(s)) return { id: s, start: 0 };
  const bare = s.replace(/^(https?:)?\/\//i, '');
  const cut = bare.search(/[/?#]/);
  const host = (cut < 0 ? bare : bare.slice(0, cut)).toLowerCase().split(':')[0];
  const rest = cut < 0 ? '' : bare.slice(cut);
  const hashAt = rest.indexOf('#');
  const beforeHash = hashAt < 0 ? rest : rest.slice(0, hashAt);
  const hash = hashAt < 0 ? '' : rest.slice(hashAt + 1);
  const qAt = beforeHash.indexOf('?');
  const path = qAt < 0 ? beforeHash : beforeHash.slice(0, qAt);
  const query = params(qAt < 0 ? '' : beforeHash.slice(qAt + 1));
  for (const [k, v] of params(hash)) if (!query.has(k)) query.set(k, v);

  let id: string | undefined;
  if (host === 'youtu.be' || host === 'www.youtu.be') {
    id = path.replace(/^\/+/, '').split('/')[0];
  } else if (YOUTUBE.has(host)) {
    const parts = path.split('/').filter(Boolean);
    if (['shorts', 'embed', 'live', 'v', 'e'].includes(parts[0]) && parts[1]) id = parts[1];
    else if (parts[0] === 'attribution_link' && query.get('u')) {
      try {
        id = parseYoutube(`https://www.youtube.com${query.get('u')}`).id;
      } catch {
        id = undefined;
      }
    } else id = query.get('v');
  } else {
    throw new Error('Das ist keine YouTube-Adresse');
  }
  if (!id || !isVideoId(id)) throw new Error('In der Adresse steckt keine gültige Video-ID');
  const start = readTime(query.get('t') ?? query.get('start') ?? '') ?? 0;
  return { id, start };
}
