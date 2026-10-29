// Reading a worker's screen for whether it can be used at all (see ProviderAdapter.screen), next to
// limit.ts, which reads it for whether it has hit its limit.
import type { WorkerStatus } from '../../shared/protocol.js';
import { providerAdapter } from '../providers/index.js';
import { screenText } from './terminal.js';
import type { Worker } from './types.js';

/**
 * An agent can sit at its prompt without being usable: Claude stuck on a first-run screen, or not
 * signed in on this machine (see ProviderAdapter.screen). Flag that as needing a human, and clear
 * it once the screen moves on.
 */
export function checkBlocked(w: Worker, setStatus: (status: WorkerStatus) => void) {
  const blockedBy = w.info.kind === 'agent' ? providerAdapter(w.info.provider)?.screen?.blocked : undefined;
  if (!w.term || !blockedBy) return;
  const s = w.info.status;
  if (s !== 'starting' && s !== 'idle' && !(w.bootBlocked && s === 'needs_input')) return;
  // Only this run's output counts: a "Not logged in" in the scrollback from before is old news.
  const text = screenText(w.term, w.term.buffer.active.type === 'normal' ? Math.max(0, w.fresh?.line ?? 0) : 0);
  const blocked = blockedBy(text, s === 'starting' || !!w.bootBlocked);
  if (blocked && s !== 'needs_input') {
    w.bootBlocked = true;
    w.info.activity = blocked;
    setStatus('needs_input');
  } else if (!blocked && w.bootBlocked && s === 'needs_input') {
    w.bootBlocked = false;
    w.info.activity = undefined;
    setStatus('idle');
  }
}
