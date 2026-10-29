// What the desktop app's connector asks the office for (see desktop/connector.cjs): your own Claude
// and GitHub sign-ins, over plain HTTP so the connector isn't one more person walking around the
// office, and a one-time link that signs the normal browser in as you.
import type http from 'node:http';
import type { SignInKind } from '../../../shared/protocol.js';
import type { Ctx } from '../../office/context.js';
import { str } from '../../office/input.js';
import { readBody, sameOrigin, send } from '../util.js';
import type { Route } from '../router.js';

const SHARED = "On the shared office password, workers run on the office's own sign-ins. Make an account of your own to sign in with your own Claude and GitHub.";

/**
 * A small JSON body, from a page that is the office itself (or the connector, which sends the
 * office's origin): another site can't make sign-ins with a visitor's cookie. Undefined
 * once it has already answered.
 */
async function readPost(ctx: Ctx, req: http.IncomingMessage, res: http.ServerResponse): Promise<Record<string, unknown> | undefined> {
  if (!sameOrigin(req, ctx.cfg)) return void send(res, 403, { error: 'Forbidden' });
  try {
    const body = JSON.parse((await readBody(req, 8192)) || '{}');
    if (body && typeof body === 'object') return body;
  } catch {
    // answered below
  }
  send(res, 400, { error: 'Bad request' });
}

const whichOf = (v: unknown): SignInKind => (v === 'github' ? 'github' : 'claude');

export const connectorRoutes = {
  // A sign-in link for another browser of yours: the same session, once, for a few minutes.
  handoff: {
    method: 'POST',
    path: '/api/link/new',
    auth: 'session',
    async handle(ctx, { req, res, session }) {
      if (!(await readPost(ctx, req, res))) return;
      send(res, 200, { key: ctx.auth.linkKey(session) });
    },
  },
  // Your own sign-ins (accounts only), as 🔐 Your sign-ins shows them.
  signins: {
    method: 'GET',
    path: '/api/signins',
    auth: 'session',
    handle(ctx, { res, session }) {
      const id = session.account?.id;
      if (!id) return send(res, 409, { error: SHARED });
      void ctx.signins.look(id);
      send(res, 200, { state: ctx.signins.state(id) });
    },
  },
  // What 🔐 Your sign-ins' buttons do (see ws/handlers/signins.ts): { do, which, code?, token? }.
  signin: {
    method: 'POST',
    path: '/api/signins',
    auth: 'session',
    async handle(ctx, { req, res, session }) {
      const body = await readPost(ctx, req, res);
      if (!body) return;
      const id = session.account?.id;
      if (!id) return send(res, 409, { error: SHARED });
      const which = whichOf(body.which);
      let error: string | undefined;
      if (body.do === 'start') error = ctx.signins.start(id, which);
      else if (body.do === 'code') error = ctx.signins.code(id, str(body.code, 4096));
      else if (body.do === 'token') error = await ctx.signins.token(id, which, str(body.token, 4096));
      else if (body.do === 'office') error = ctx.signins.useOffice(id, which);
      else if (body.do === 'cancel') ctx.signins.cancel(id, which);
      else if (body.do === 'signout') await ctx.signins.signOut(id, which);
      else return send(res, 400, { error: 'Bad request' });
      send(res, error ? 400 : 200, { state: ctx.signins.state(id), ...(error ? { error } : {}) });
    },
  },
} satisfies Record<string, Route>;
