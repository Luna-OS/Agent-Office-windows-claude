// Online lobbies: an office on another machine (your Umbrel, a server) that the connector signs you
// in to (see controller.cjs). The workers run there; this computer only shows the office, in the
// browser, and everyone else joins the same address from their own connector or a browser.

const DOCS = 'https://github.com/Luna-OS/Agent-Office-windows-claude/blob/main/docs/';

/**
 * The lobby's address as typed ("umbrel.local:4600", "100.78.107.162:4600",
 * "https://office.example.com/"), as an origin: http unless it says https. Null when it isn't one.
 */
function lobbyOrigin(input) {
  let s = String(input ?? '').trim();
  if (!s) return null;
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(s)) s = `http://${s}`;
  let u;
  try {
    u = new URL(s);
  } catch {
    return null;
  }
  if ((u.protocol !== 'http:' && u.protocol !== 'https:') || u.username || u.password || !u.hostname) return null;
  return u.origin;
}

/** The /join#<token> part of an invite link (from 🔑 Accounts), when `input` is one. */
function inviteOf(input) {
  return /\/join#[A-Za-z0-9_-]{16,}$/.exec(String(input ?? '').trim())?.[0];
}

/**
 * Whether the browser gets the lobby through the passthrough on this computer (passthrough.cjs), for
 * voice and screen sharing: plain http on any machine but this one (an Umbrel at home, on Tailscale
 * or NetBird, say). Browsers allow those on https:// and on this computer's own addresses only.
 */
function needsPassthrough(origin) {
  return origin.startsWith('http://') && !/^http:\/\/(localhost|127\.\d+\.\d+\.\d+|\[::1\])(:\d+)?$/.test(origin);
}

/**
 * Whether an Agent Office answers at `origin`: its sign-in page, within `ms`. Resolves to why not, or
 * undefined. `get` is a fetch (the tests hand in their own).
 */
async function unreachable(origin, get = fetch, ms = 8000) {
  try {
    const res = await get(`${origin}/login.html`, { signal: AbortSignal.timeout(ms), redirect: 'follow' });
    if (!res.ok) return `${origin} antwortet mit HTTP ${res.status}.`;
    if (!/Agent Office/i.test(await res.text())) return `Unter ${origin} läuft etwas anderes als Agent Office.`;
    return undefined;
  } catch (err) {
    const why = err?.name === 'TimeoutError' ? 'keine Antwort' : (err?.cause?.code ?? err?.message ?? String(err));
    return `${origin} ist nicht erreichbar (${why}). Läuft die Umbrel-App, und ist dieser Computer im selben Netz (Heimnetz, Tailscale oder NetBird)?`;
  }
}

/** A short name for a lobby: the one given, or its address. */
function lobbyName(name, origin) {
  const clean = String(name ?? '')
    .replace(/[\p{C}<>]/gu, '')
    .trim()
    .slice(0, 40);
  return clean || new URL(origin).host;
}

module.exports = { DOCS, inviteOf, lobbyOrigin, needsPassthrough, unreachable, lobbyName };
