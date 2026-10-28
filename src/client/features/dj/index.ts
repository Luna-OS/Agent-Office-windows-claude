/**
 * Der DJ auf dem Dach: sein Pult (E: einen YouTube-Song für alle oben auflegen, oder das Lufthorn),
 * der Song in der Ecke, solange du oben bist, an derselben Stelle wie bei allen, und sein eigenes
 * Drum & Bass, solange keiner läuft.
 */
import { DJ_BOOTH } from '../../../shared/layout';
import type { Ctx } from '../../core/context';
import { aside, hintTitle, key, onE } from '../../core/hint';
import { djFrame } from '../../dnb';
import { store } from '../../state';
import { toast } from '../../ui/dom';
import { RoofPlayer } from './player';
import { openDjBooth } from './ui';

// Was man hier benutzen kann (siehe InteractKinds in world/types.ts).
declare module '../../world/types' {
  interface InteractKinds {
    dj: true;
  }
}

export interface DjDeps {
  /** Wie weit das eigene Set des DJs ist (see features/rooftop). */
  djAt(): number;
}

export function installDj(ctx: Ctx, deps: DjDeps) {
  /** Wo im Song alle gerade sind: ab dem Start im Link, seit er aufgelegt wurde, auf der Uhr des Office. */
  const player = new RoofPlayer(
    (s) => s.start + (store.officeNow() - s.startedAt) / 1000,
    () => sync(),
  );

  /** Am lautesten am DJ-Pult, in der hintersten Ecke des Dachs noch ein gutes Drittel. */
  const volume = () => {
    const p = ctx.player.pos;
    const d = Math.hypot(p.x - DJ_BOOTH.x, p.z - DJ_BOOTH.z);
    return 100 * Math.max(0.35, Math.min(1, 1.15 - d / 30));
  };

  /** Auf dem Dach, und ein Song läuft: der Player, und das Set des DJs schweigt. Sonst beides nicht. */
  let up = false;
  function sync() {
    up = ctx.upTop();
    const s = store.dj;
    if (up && s) player.show(s, volume());
    else player.hide();
    ctx.sound.djQuiet(up && !!s && !player.ended);
  }
  store.on('dj', sync);
  let lastLook = 0;
  ctx.ticks.add('env', ({ now }) => {
    // Ein paar Mal pro Sekunde: aufs Dach oder runter, und wie laut es da ist, wo du stehst.
    if (now - lastLook < 400) return;
    lastLook = now;
    if (ctx.upTop() !== up) sync();
    else if (up && store.dj) player.setVolume(volume());
  });

  ctx.messages.on('dj', (m) => {
    if (!ctx.upTop() || m.by === store.profile.name) return;
    toast(m.state ? `🎧 ${m.by} hat einen Song aufgelegt` : `💿 ${m.by} hat den Song runtergenommen: wieder Drum & Bass`);
  });

  ctx.messages.on('horn', (msg) => {
    if (!ctx.upTop()) return;
    ctx.sound.horn();
    if (msg.by !== store.profile.name) toast(`📯 ${msg.by} hat ins Lufthorn geblasen!`);
  });
  let lastHorn = 0;
  /** Das Lufthorn, für alle auf dem Dach. */
  function blowHorn() {
    const now = performance.now();
    if (now - lastHorn < 1500) return;
    lastHorn = now;
    ctx.net.send({ t: 'horn' });
  }

  ctx.interactions.define('dj', {
    reach: 6,
    hint: () => {
      const s = store.dj;
      if (s && !player.ended) {
        const title = player.songTitle();
        return { k: `yt:${s.video}:${title ?? ''}`, parts: [hintTitle('🎧 DJ Merge Conflict'), aside(`▶ ${title ?? 'ein YouTube-Song'} · von ${s.by}`), key('E', 'DJ-Pult')] };
      }
      const f = djFrame(deps.djAt());
      const what = f.part === 'drop' ? '🔥 der Drop' : f.part === 'build' ? 'es baut sich auf …' : f.part === 'breakdown' ? 'der Breakdown' : 'mischt den nächsten Track rein';
      return { k: what, parts: [hintTitle('🎧 DJ Merge Conflict'), aside(`Drum & Bass · ${what}`), key('E', 'DJ-Pult: Song auflegen, Lufthorn')] };
    },
    use: onE(() => openDjBooth({ net: ctx.net, songTitle: () => player.songTitle(), horn: blowHorn })),
  });
}
