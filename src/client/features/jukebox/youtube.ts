// Ein YouTube-Link auf der Jukebox: nur der Ton, aus einem Player, den keiner sieht (das Bild braucht
// hier keiner). Für alle auf der Etage an derselben Stelle, wie ein Stream, und ist das Video zu Ende,
// fängt es wieder von vorn an. Es kommt über YouTubes IFrame-API (sound/youtube.ts).
import type { YoutubeVideo } from '../../../shared/youtube';
import { ENDED, PLAYING, destroyPlayer, youtube, youtubeError, type YtPlayer } from '../../sound/youtube';

/** Weicht der Player mehr ab (s), springt er zurück an die Stelle, an der alle sind. */
const DRIFT = 2;

export class JukeboxYoutube {
  /** Wo der Player wohnt: außerhalb des Bildes, aber groß genug, dass YouTube ihn spielen lässt. */
  private readonly host = document.createElement('div');
  /** Der Player, sobald YouTube ihn bereit meldet; vorher hat er seine Methoden noch nicht. */
  private player: YtPlayer | null = null;
  private pending: YtPlayer | null = null;
  private timer = 0;
  private gone = false;
  /** Der Titel des Videos, sobald YouTube ihn verraten hat. */
  title: string | undefined;

  constructor(
    private readonly video: YoutubeVideo,
    /** Seit wann es für alle läuft, in Sekunden. */
    private readonly at: () => number,
    /** Wie laut es dort ist, wo du stehst: 0–100. */
    private readonly volume: () => number,
    private readonly onError: (text: string) => void,
  ) {
    this.host.setAttribute('aria-hidden', 'true');
    this.host.style.cssText = 'position:fixed;left:-10000px;top:0;width:200px;height:200px;opacity:0;pointer-events:none';
    document.body.append(this.host);
    void this.load();
  }

  /** Wo im Video alle gerade sind: ab dem Start im Link, und nach dem Ende wieder von dort. */
  private want(): number {
    const { start } = this.video;
    const span = (this.player?.getDuration?.() ?? 0) - start;
    return start + (span > 1 ? this.at() % span : this.at());
  }

  private async load() {
    const yt = await youtube().catch(() => null);
    if (this.gone) return;
    if (!yt) return this.onError('🎵 YouTube lädt gerade nicht: Ist dieser Computer online?');
    const el = document.createElement('div');
    this.host.append(el);
    const p: YtPlayer = new yt.Player(el, {
      width: '200',
      height: '200',
      videoId: this.video.id,
      playerVars: { autoplay: 1, start: Math.floor(this.video.start + this.at()), controls: 0, disablekb: 1, playsinline: 1, rel: 0, iv_load_policy: 3 },
      events: {
        onReady: () => {
          if (this.pending !== p) return;
          this.pending = null;
          this.player = p;
          this.hear();
          p.seekTo(this.want(), true);
          p.playVideo();
          this.timer = window.setInterval(() => this.keepInTime(), 3000);
        },
        onStateChange: (e: { data: number }) => {
          if (this.player !== p) return;
          if (e.data === PLAYING) this.title = p.getVideoData?.()?.title || this.title;
          // Zu Ende: wieder von vorn, wie ein Stream in der Schleife.
          if (e.data === ENDED) {
            p.seekTo(this.video.start, true);
            p.playVideo();
          }
        },
        onError: (e: { data: number }) => this.onError(`🎵 ${youtubeError(e.data)}.`),
      },
    });
    this.pending = p;
  }

  /** So laut, wie es dort ist, wo du stehst. */
  hear() {
    this.player?.setVolume(Math.round(Math.max(0, Math.min(100, this.volume()))));
  }

  /** Die Seite wurde angefasst: Hat der Browser den Ton zurückgehalten, geht es jetzt los. */
  resume() {
    const p = this.player;
    if (p && p.getPlayerState() !== PLAYING) p.playVideo();
  }

  playing(): boolean {
    return !!this.player && this.player.getPlayerState() === PLAYING;
  }

  /** Alle paar Sekunden: Ist der Player noch an der Stelle, an der alle sind? */
  private keepInTime() {
    const p = this.player;
    if (!p || p.getPlayerState() !== PLAYING) return;
    const want = this.want();
    if (Math.abs(p.getCurrentTime() - want) > DRIFT) p.seekTo(want, true);
  }

  stop() {
    this.gone = true;
    clearInterval(this.timer);
    destroyPlayer(this.pending);
    destroyPlayer(this.player);
    this.pending = this.player = null;
    this.host.remove();
  }
}
