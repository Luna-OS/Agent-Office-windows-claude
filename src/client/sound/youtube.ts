// YouTubes IFrame-API (www.youtube.com/iframe_api), für den DJ auf dem Dach (features/dj) und die
// Jukebox (features/jukebox): erst geladen, wenn zum ersten Mal ein Video laufen soll.

/** Das Nötigste von YouTubes Player (https://developers.google.com/youtube/iframe_api_reference). */
export interface YtPlayer {
  loadVideoById(o: { videoId: string; startSeconds: number }): void;
  playVideo(): void;
  pauseVideo(): void;
  seekTo(s: number, allowSeekAhead: boolean): void;
  setVolume(v: number): void;
  getCurrentTime(): number;
  getDuration?(): number;
  getPlayerState(): number;
  getVideoData?(): { title?: string };
  destroy(): void;
}
export interface Yt {
  Player: new (el: HTMLElement, o: Record<string, unknown>) => YtPlayer;
}
/** Die Zustände aus onStateChange, die hier zählen. */
export const PLAYING = 1;
export const ENDED = 0;

let api: Promise<Yt> | null = null;
/** Die API, einmal geladen. Lädt sie nicht (offline), wird es beim nächsten Mal wieder versucht. */
export function youtube(): Promise<Yt> {
  const w = window as unknown as { YT?: Yt; onYouTubeIframeAPIReady?: () => void };
  if (w.YT?.Player) return Promise.resolve(w.YT);
  api ??= new Promise<Yt>((resolve, reject) => {
    const before = w.onYouTubeIframeAPIReady;
    w.onYouTubeIframeAPIReady = () => {
      before?.();
      resolve(w.YT!);
    };
    const s = document.createElement('script');
    s.src = 'https://www.youtube.com/iframe_api';
    s.async = true;
    s.onerror = () => {
      api = null;
      reject(new Error('YouTube lädt nicht'));
    };
    document.head.append(s);
  });
  return api;
}

/** Weg mit einem Player, auch einem, der noch lädt: Dem fehlen seine Methoden womöglich noch. */
export function destroyPlayer(p: YtPlayer | null) {
  if (typeof p?.destroy === 'function') p.destroy();
}

/** Warum YouTube ein Video nicht abspielt, in Worten (die Codes aus onError). */
export function youtubeError(code: number): string {
  // 2: kaputte ID, 5: der Browser kann es nicht, 100: gibt es nicht (mehr), 101/150: nur auf YouTube selbst.
  return code === 101 || code === 150 ? 'Dieses Video darf nur auf YouTube selbst laufen' : code === 100 ? 'Dieses Video gibt es nicht (mehr)' : 'YouTube kann dieses Video hier nicht abspielen';
}
