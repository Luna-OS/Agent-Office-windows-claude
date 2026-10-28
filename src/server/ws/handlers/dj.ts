// Der DJ auf dem Dach: einen YouTube-Song auflegen und wieder runternehmen (siehe server/dj.ts).
import { djBooth } from '../../dj.js';
import { ROOF } from '../../../shared/rooftop.js';
import type { DjClientMsg } from '../../../shared/protocol.js';
import { throttle } from '../../office/client.js';
import type { HandlerMap, ViewPieces } from './types.js';

export const djView: ViewPieces['dj'] = (ctx) => djBooth(ctx).state();

export const djHandlers = {
  'dj.play'(ctx, c, msg) {
    if (c.peer.floor !== ROOF || !throttle(c, 'dj', 2000)) return;
    const r = djBooth(ctx).play(msg.url, c.peer.name);
    if ('error' in r) return ctx.warn(c, r.error);
    ctx.broadcast({ t: 'dj', state: r.state, by: c.peer.name });
  },
  'dj.stop'(ctx, c) {
    if (c.peer.floor !== ROOF || !throttle(c, 'dj', 1000) || !djBooth(ctx).stop()) return;
    ctx.broadcast({ t: 'dj', state: null, by: c.peer.name });
  },
} satisfies HandlerMap<DjClientMsg>;
