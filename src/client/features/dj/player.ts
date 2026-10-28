// Der Song, den jemand beim DJ aufgelegt hat: ein kleiner YouTube-Player in der Ecke, solange du auf
// dem Dach bist, an derselben Stelle wie bei allen anderen. Er kommt über YouTubes IFrame-API
// (www.youtube.com/iframe_api), die erst geladen wird, wenn zum ersten Mal ein Song läuft.
import type { DjState } from '../../../shared/protocol';
import { h } from '../../ui/dom';

/** Das Nötigste von YouTubes Player (https://developers.google.com/youtube/iframe_api_reference). */
interface YtPlayer {
  loadVideoById(o: { videoId: string; startSeconds: number }): void;
  playVideo(): void;
  pauseVideo(): void;
  seekTo(s: number, allowSeekAhead: boolean): void;
  setVolume(v: number): void;
  getCurrentTime(): number;
  getPlayerState(): number;
  getVideoData?(): { title?: string };
  destroy(): void;
}
interface Yt {
  Player: new (el: HTMLElement, o: Record<string, unknown>) => YtPlayer;
}
const PLAYING = 1;
const ENDED = 0;

let api: Promise<Yt> | null = null;
function youtube(): Promise<Yt> {
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

/** Weicht der Player mehr ab (s), springt er zurück an die Stelle, an der alle sind. */
const DRIFT = 2;

export class RoofPlayer {
  private readonly box = h('div.dj-player', { hidden: true, role: 'region', 'aria-label': 'Song vom DJ' });
  private readonly frame = h('div.dj-frame');
  private readonly title = h('div.dj-title');
  private readonly go = h('button.btn.primary.dj-go', { type: 'button', hidden: true }, '▶ Ton an');
  private player: YtPlayer | null = null;
  private loading: Promise<void> | null = null;
  private shown: DjState | null = null;
  private timer = 0;
  /** Der Song ist bei dir zu Ende: Der DJ spielt wieder sein eigenes Set, bis jemand den nächsten auflegt. */
  ended = false;

  constructor(
    /** Wo im Song alle gerade sind, in Sekunden. */
    private readonly position: (s: DjState) => number,
    /** Ändert sich, ob er zu Ende ist. */
    private readonly onEnded: () => void,
  ) {
    this.box.append(h('div.dj-head', {}, '🎧 Vom DJ auf dem Dach'), this.frame, this.title, this.go);
    this.go.addEventListener('click', () => {
      this.player?.playVideo();
      this.go.hidden = true;
    });
    document.body.append(this.box);
  }

  /** Zeigt und spielt `s`, an der Stelle, an der alle sind. */
  show(s: DjState, volume: number) {
    const same = this.shown && this.shown.video === s.video && this.shown.startedAt === s.startedAt;
    this.box.hidden = false;
    this.title.textContent = `aufgelegt von ${s.by}`;
    if (same) return this.setVolume(volume);
    this.shown = s;
    this.ended = false;
    void this.load(s, volume);
  }

  /** Weg damit: Du bist nicht auf dem Dach, oder der Song wurde runtergenommen. */
  hide() {
    this.box.hidden = true;
    this.shown = null;
    this.ended = false;
    clearInterval(this.timer);
    this.player?.destroy();
    this.player = null;
    this.loading = null;
    this.frame.replaceChildren();
    this.go.hidden = true;
  }

  setVolume(v: number) {
    this.player?.setVolume(Math.round(Math.max(0, Math.min(100, v))));
  }

  /** Der Titel des Songs, sobald YouTube ihn verraten hat. */
  songTitle(): string | undefined {
    return this.player?.getVideoData?.()?.title || undefined;
  }

  private async load(s: DjState, volume: number) {
    const yt = await youtube().catch(() => null);
    if (!yt || this.shown !== s) {
      if (!yt) this.title.textContent = 'YouTube lädt gerade nicht. Ist dieser Computer online?';
      return;
    }
    this.player?.destroy();
    const host = h('div');
    this.frame.replaceChildren(host);
    const at = Math.max(0, this.position(s));
    this.loading = new Promise<void>((ready) => {
      this.player = new yt.Player(host, {
        width: '240',
        height: '135',
        videoId: s.video,
        playerVars: { autoplay: 1, start: Math.floor(at), controls: 0, disablekb: 1, playsinline: 1, rel: 0, iv_load_policy: 3 },
        events: {
          onReady: () => {
            this.setVolume(volume);
            this.player?.seekTo(this.position(s), true);
            this.player?.playVideo();
            ready();
          },
          onStateChange: (e: { data: number }) => {
            if (e.data === PLAYING) {
              this.go.hidden = true;
              const t = this.songTitle();
              if (t) this.title.textContent = `${t} · aufgelegt von ${s.by}`;
            }
            if (e.data === ENDED) this.over('Zu Ende. Der DJ legt wieder selbst auf.');
          },
          // 2: kaputte ID, 5: der Browser kann es nicht, 100: gibt es nicht (mehr), 101/150: nur auf YouTube selbst.
          onError: (e: { data: number }) => {
            const why = e.data === 101 || e.data === 150 ? 'Dieses Video darf nur auf YouTube selbst laufen' : e.data === 100 ? 'Dieses Video gibt es nicht (mehr)' : 'YouTube kann dieses Video hier nicht abspielen';
            this.over(`${why}. Der DJ legt wieder selbst auf.`);
          },
        },
      });
    });
    await this.loading;
    clearInterval(this.timer);
    this.timer = window.setInterval(() => this.keepInTime(s), 3000);
    // Lässt der Browser es nicht von selbst spielen, hilft ein Klick.
    setTimeout(() => {
      if (this.shown === s && !this.ended && this.player && this.player.getPlayerState() !== PLAYING) this.go.hidden = false;
    }, 2500);
  }

  /** Der Song ist bei dir vorbei (zu Ende, oder er läuft hier nicht): Der DJ übernimmt wieder. */
  private over(text: string) {
    if (this.ended) return;
    this.ended = true;
    this.go.hidden = true;
    this.title.textContent = text;
    this.onEnded();
  }

  /** Alle paar Sekunden: Ist der Player noch an der Stelle, an der alle sind? */
  private keepInTime(s: DjState) {
    const p = this.player;
    if (!p || this.shown !== s || this.ended || p.getPlayerState() !== PLAYING) return;
    const want = this.position(s);
    if (Math.abs(p.getCurrentTime() - want) > DRIFT) p.seekTo(want, true);
  }
}
