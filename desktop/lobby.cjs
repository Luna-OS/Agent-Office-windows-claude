// Der Server, mit dem der Connector spricht: die Umbrel-App „Agent Office Vermittler“ (oder ein
// anderer Server mit dem Office darauf). Das Office läuft dort; dieser Computer zeigt es im Browser
// und lässt die Arbeiter laufen, die du einstellst (siehe controller.cjs).

const DOCS = 'https://github.com/Luna-OS/Agent-Office-windows-claude/blob/main/docs/';

/**
 * Die Adresse des Servers, wie sie eingetippt wurde ("umbrel.local:4600", "100.78.107.162:4600",
 * "https://office.example.com/"), als Origin: http, wenn nicht https dasteht. Null, wenn es keine ist.
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

/**
 * Whether the browser gets the office through the passthrough on this computer (passthrough.cjs),
 * for voice and screen sharing: plain http on any machine but this one (an Umbrel at home, on
 * Tailscale or NetBird, say). Browsers allow those on https:// and on this computer's own addresses only.
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
    if (!res.ok) return `${origin} antwortet mit HTTP ${res.status}. Läuft dort die Umbrel-App „Agent Office Vermittler“ in der neuesten Version?`;
    if (!/Agent Office/i.test(await res.text())) return `Unter ${origin} läuft etwas anderes als Agent Office.`;
    return undefined;
  } catch (err) {
    const why = err?.name === 'TimeoutError' ? 'keine Antwort' : (err?.cause?.code ?? err?.message ?? String(err));
    return `${origin} ist nicht erreichbar (${why}). Läuft die Umbrel-App, und ist dieser Computer im selben Netz (Heimnetz, Tailscale oder NetBird)?`;
  }
}

/** Dein Name, wie das Office ihn nimmt: ohne Steuerzeichen, Leerraum zusammengezogen, höchstens 24 Zeichen. */
function cleanName(v) {
  return String(v ?? '')
    .replace(/[\p{C}]/gu, '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 24);
}

module.exports = { DOCS, cleanName, lobbyOrigin, needsPassthrough, unreachable };
