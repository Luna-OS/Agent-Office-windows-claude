import os from 'node:os';
import type { AccountsState } from '../../shared/protocol.js';
import type { Config } from '../config.js';
import type { Ctx } from './context.js';

/** Whether an IPv4 address is Tailscale's (100.64.0.0/10). */
const tailnet = (ip: string) => {
  const [a, b] = ip.split('.').map(Number);
  return a === 100 && b >= 64 && b <= 127;
};

/**
 * Where people on other computers reach the office: nowhere when it listens on this computer alone
 * (127.0.0.1, the desktop app's own office), the one address it was given, or, listening on every
 * network (0.0.0.0, a lobby the desktop app hosts), its address on each of them: the home network's
 * first, then Tailscale's. Behind a reverse proxy (the Umbrel app's) it can't tell: its own
 * addresses are its container's, which nobody comes in by.
 */
export function networkOrigins(cfg: Pick<Config, 'host' | 'port' | 'tls'> & { trustProxy?: boolean }, interfaces = os.networkInterfaces()): string[] {
  const scheme = cfg.tls ? 'https' : 'http';
  if (cfg.trustProxy || /^(127\.|localhost$|::1$)/.test(cfg.host)) return [];
  if (cfg.host !== '0.0.0.0' && cfg.host !== '::') return [`${scheme}://${cfg.host.includes(':') ? `[${cfg.host}]` : cfg.host}:${cfg.port}`];
  const found: { ip: string; rank: number }[] = [];
  for (const list of Object.values(interfaces)) {
    for (const a of list ?? []) {
      if (a.family !== 'IPv4' || a.internal || a.address.startsWith('169.254.')) continue;
      found.push({ ip: a.address, rank: /^(192\.168|10)\./.test(a.address) ? 0 : tailnet(a.address) ? 1 : 2 });
    }
  }
  return found.sort((x, y) => x.rank - y.rank).map(({ ip }) => `${scheme}://${ip}:${cfg.port}`);
}

/** The accounts as admins see them, with where the office is reached from other computers (for invite links made on this one). */
export const accountsView = (ctx: Ctx): AccountsState => ({ ...ctx.accounts.state(ctx.onlineAccounts()), origins: networkOrigins(ctx.cfg) });
