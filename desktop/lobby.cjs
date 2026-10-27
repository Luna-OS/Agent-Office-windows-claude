// Online lobbies: an office on another machine (your Umbrel, a server in the cloud) that the app's
// window joins instead of starting one on this computer. The workers run there, so this computer
// only draws the office; everyone else joins the same address from their own app or a browser.
//
// The start page (startPage) is a data: page of the app's own. Its links and its form go to
// APP_LINK, an address that never resolves (.invalid), which main.cjs catches before it's loaded.

/** Where the start page's links point: main.cjs handles them (handleAppLink), nothing is loaded. */
const APP_LINK = 'https://app.agent-office.invalid/';

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

/**
 * Whether Chromium has to be told an origin counts as secure, for voice and screen sharing: plain
 * http on any machine but this one (an Umbrel on the home network or on Tailscale, say).
 */
function needsSecureFlag(origin) {
  return origin.startsWith('http://') && !/^http:\/\/(localhost|127\.\d+\.\d+\.\d+|\[::1\])(:\d+)?$/.test(origin);
}

/**
 * Whether an Agent Office answers at `origin`: its sign-in page, within `ms`. Resolves to why not, or
 * undefined. `get` is the app's fetch (Electron's net.fetch, which goes through the system's proxy, as the window does).
 */
async function unreachable(origin, get = fetch, ms = 8000) {
  try {
    const res = await get(`${origin}/login.html`, { signal: AbortSignal.timeout(ms), redirect: 'follow' });
    if (!res.ok) return `${origin} antwortet mit HTTP ${res.status}.`;
    if (!/Agent Office/i.test(await res.text())) return `Unter ${origin} läuft etwas anderes als Agent Office.`;
    return undefined;
  } catch (err) {
    const why = err?.name === 'TimeoutError' ? 'keine Antwort' : (err?.cause?.code ?? err?.message ?? String(err));
    return `${origin} ist nicht erreichbar (${why}). Läuft die Umbrel-App, und ist dieser Computer im selben Netz oder Tailnet?`;
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

const escapeHtml = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const link = (action, params = {}) => `${APP_LINK}${action}${Object.keys(params).length ? `?${new URLSearchParams(params)}` : ''}`;

/**
 * The start page: run the office on this computer, or join an online lobby (a saved one, or a new
 * address). `here` is whether the office on this computer is the one last used.
 */
function startPage({ lobbies = [], current, here, error, address = '' }) {
  const saved = lobbies
    .map(
      (l) =>
        `<li><a class="lobby${l.url === current ? ' on' : ''}" href="${escapeHtml(link('join', { url: l.url }))}"><b>${escapeHtml(l.name)}</b><span>${escapeHtml(l.url)}</span></a>` +
        `<a class="forget" title="Vergessen" href="${escapeHtml(link('forget', { url: l.url }))}">✕</a></li>`,
    )
    .join('');
  return `
<style>
main{max-width:860px;text-align:left}h1{text-align:center;margin-bottom:4px}.lead{text-align:center;opacity:.8;margin-top:0}
.choices{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px;margin-top:20px}
section{background:#252839;border:1px solid #34384f;border-radius:14px;padding:18px 20px}section h2{font-size:18px;margin:0 0 6px}
section p{font-size:14px;line-height:1.45;opacity:.85;margin:0 0 14px}
.btn,button{display:inline-block;background:#d97757;color:#fff;border:0;border-radius:9px;padding:10px 16px;font:600 15px system-ui,sans-serif;text-decoration:none;cursor:pointer}
.btn.quiet{background:#34384f}
form{display:grid;gap:8px}input{background:#151722;color:#e8e8f0;border:1px solid #34384f;border-radius:8px;padding:10px 12px;font:15px system-ui,sans-serif}
ul{list-style:none;padding:0;margin:0 0 12px;display:grid;gap:6px}li{display:flex;gap:6px}
.lobby{flex:1;display:flex;flex-direction:column;background:#1b1d2a;border:1px solid #34384f;border-radius:9px;padding:8px 12px;color:#e8e8f0;text-decoration:none}
.lobby.on{border-color:#d97757}.lobby span{font-size:12px;opacity:.7}.lobby:hover{border-color:#d97757}
.forget{display:grid;place-items:center;width:34px;border-radius:9px;background:#1b1d2a;border:1px solid #34384f;color:#e8e8f0;text-decoration:none}
.error{background:#4a2330;border:1px solid #8a3a4f;border-radius:10px;padding:10px 14px;font-size:14px}
.foot{text-align:center;font-size:13px;opacity:.75;margin-top:18px}.foot a{color:#f0a487}
</style>
<h1>🏢 Agent Office</h1>
<p class="lead">Wo soll das Office laufen?</p>
${error ? `<p class="error">${escapeHtml(error)}</p>` : ''}
<div class="choices">
  <section>
    <h2>🖥️ Auf diesem Computer</h2>
    <p>Das Office und seine Worker laufen hier. Für dich allein, ohne Server: braucht aber Rechenleistung und Speicher dieses Computers, je mehr Worker, desto mehr.</p>
    <a class="btn${here ? '' : ' quiet'}" href="${escapeHtml(link('local'))}">Hier starten</a>
  </section>
  <section>
    <h2>🌐 Online-Lobby</h2>
    <p>Ein Office auf deinem Umbrel oder einem Server: Die Worker laufen dort, dieser Computer zeigt nur das Office. Mitspieler treten mit derselben Adresse bei, mit der App oder im Browser.</p>
    ${saved ? `<ul>${saved}</ul>` : ''}
    <form action="${escapeHtml(link('join'))}" method="get">
      <input name="url" placeholder="Adresse, z. B. umbrel.local:4600" value="${escapeHtml(address)}" required autofocus spellcheck="false" autocomplete="off">
      <input name="name" placeholder="Name der Lobby (optional)" maxlength="40" autocomplete="off">
      <button type="submit">Beitreten</button>
    </form>
  </section>
</div>
<p class="foot">Eine eigene Lobby hosten: die Umbrel-App „Agent Office“ — <a href="${DOCS}online-lobby.md">Anleitung</a></p>`;
}

module.exports = { APP_LINK, DOCS, lobbyOrigin, needsSecureFlag, unreachable, lobbyName, startPage, escapeHtml };
