// The connector's window: the server (its address, its password and your name), your workers on
// this computer with its Claude and GitHub, your sign-ins on the server, and the office in your
// browser. It only draws the state the app sends (controller.cjs) and hands each button's action
// back through window.connector (preload.cjs).
const root = document.getElementById('app');
const toast = document.getElementById('toast');
const DOCS = 'https://github.com/Luna-OS/Agent-Office-windows-claude/blob/main/docs/';
const NAMES = { claude: 'Claude', github: 'GitHub' };

let drawn = '';
let lastView = '';

function h(tag, attrs = {}, ...kids) {
  const [name, ...classes] = tag.split('.');
  const el = document.createElement(name);
  if (classes.length) el.className = classes.join(' ');
  for (const [k, v] of Object.entries(attrs ?? {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'class') el.className = `${el.className} ${v}`.trim();
    else el.setAttribute(k, v === true ? '' : String(v));
  }
  for (const kid of kids.flat(2)) if (kid !== null && kid !== undefined && kid !== false && kid !== '') el.append(kid instanceof Node ? kid : String(kid));
  return el;
}

/** Inputs kept across redraws, by key: what's typed, and where the cursor is, survives an update. */
const kept = new Map();
function field(key, attrs) {
  if (!kept.has(key)) kept.set(key, h(attrs.tag ?? 'input', { ...attrs, tag: undefined, options: undefined }, (attrs.options ?? []).map(([v, label]) => h('option', { value: v }, label))));
  return kept.get(key);
}
const clear = (...keys) => keys.forEach((k) => kept.has(k) && (kept.get(k).value = ''));
/** Which "Oder einen Token einfügen" are open, across redraws. */
const openDetails = new Set();

let toastTimer = 0;
function say(text, bad = false) {
  toast.textContent = text;
  toast.className = bad ? 'bad' : '';
  toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.hidden = true), bad ? 9000 : 2500);
}

/** Runs one of the app's actions, its button busy meanwhile. Resolves to whether it went through. */
async function act(button, action, args) {
  if (button) button.disabled = true;
  try {
    const r = await window.connector.do(action, args);
    if (r?.error) say(r.error, true);
    return !r?.error;
  } finally {
    if (button) button.disabled = false;
  }
}
const click = (action, args, then) => (e) => act(e.currentTarget, action, typeof args === 'function' ? args() : args).then((ok) => ok && then?.());

function form(fields, label, run) {
  const submit = h('button.primary', { type: 'submit' }, label);
  const f = h('form', {}, fields, submit);
  f.addEventListener('submit', (e) => {
    e.preventDefault();
    run(submit);
  });
  return f;
}

const problem = (s) => s.error && h('p.error', {}, s.error);

const host = (url) => {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
};

function header(s) {
  const where = s.connected
    ? h('div.where', {}, h('b', {}, host(s.server.url)), h('span', {}, `Verbunden als ${s.me.account?.name ?? s.server.name}`))
    : h('div.where', {}, h('b', {}, 'Nicht verbunden'), h('span', {}, s.server ? host(s.server.url) : 'Connector'));
  const gear = s.connected && s.view === 'home' ? h('button.link', { type: 'button', onclick: click('settings') }, '⚙️ Einstellungen') : null;
  return h('header', {}, h('div.brand', {}, '🏢 Agent Office'), where, gear);
}

/** Der Server: seine Adresse, sein Passwort und dein Name. Beim ersten Mal legt er dein Konto an. */
function serverView(s) {
  const saved = s.server;
  const address = field('address', { placeholder: 'Adresse, z. B. 100.78.107.162:4600', spellcheck: 'false', autocomplete: 'off', required: true });
  const pw = field('serverPw', { type: 'password', autocomplete: 'off' });
  const name = field('name', { placeholder: 'Dein Name', maxlength: 24, autocomplete: 'username', required: true });
  if (saved && !address.value) address.value = saved.url;
  if (saved && !name.value) name.value = saved.name;
  // Ein gespeichertes Passwort bleibt, solange das Feld leer bleibt und es derselbe Server ist.
  pw.placeholder = saved?.password ? 'Passwort des Servers (gespeichert, leer lassen)' : 'Passwort des Servers';
  pw.required = !saved?.password;
  const submit = (b) => act(b, 'connect', { address: address.value, password: pw.value, name: name.value }).then((ok) => ok && clear('serverPw'));
  return [
    h('p.lead', {}, 'Das Office läuft auf eurem Server, der Umbrel-App „Agent Office Vermittler“. Hier verbindest du dich mit ihm und öffnest das Office in deinem Browser; die Arbeiter, die du einstellst, laufen auf diesem Computer.'),
    problem(s),
    h(
      'section',
      {},
      h('h2', {}, '🛰️ Server'),
      form([address, pw, name], s.editing ? 'Speichern und verbinden' : 'Verbinden', submit),
      h('p.note', {}, 'Das Passwort steht bei der Umbrel-App auf ihrer Seite im App Store. Beim ersten Mal legt der Server dein Konto unter deinem Namen an; mit diesem Namen bist du danach immer du.'),
      s.editing ? h('div.row', {}, h('button', { type: 'button', onclick: click('back') }, 'Abbrechen')) : null,
    ),
    h('p.foot', {}, 'Den Server einrichten: ', h('a', { href: `${DOCS}vermittler.md` }, 'Anleitung')),
  ];
}

/** "Use the host's Claude" for a member it's shared with, the office's own for admins. */
const officeLabel = (s, which) => (which === 'claude' && !s.me.admin ? 'Claude des Servers nutzen' : `${NAMES[which]} des Servers nutzen`);
const officeName = (s, which) => `${NAMES[which]} des Servers`;

/** One of your sign-ins on the server (🔐 Your sign-ins in the office). */
function card(s, which) {
  const x = s.signins[which];
  const office = s.signins.office[which];
  const go = (what, more) => click('signin', () => ({ what, which, ...(typeof more === 'function' ? more() : more) }));
  const status =
    x.status === 'ok'
      ? h('span.ok', {}, '✅ ', x.how === 'office' ? `${officeName(s, which)}${x.who ? ` (${x.who})` : ''}` : (x.who ?? 'Angemeldet'))
      : x.status === 'busy'
        ? h('span', {}, '⏳ Einen Moment …')
        : h('span', {}, 'Nicht angemeldet');
  const body = [];
  if (x.error) body.push(h('p.error', {}, x.error));
  if (x.pending) {
    if (!x.pending.url) {
      body.push(h('p.note', {}, `Die Anmeldung bei ${NAMES[which]} startet …`));
    } else if (which === 'claude') {
      const code = field('claudeCode', { placeholder: 'Code hier einfügen', autocomplete: 'off', spellcheck: 'false', required: true });
      body.push(
        h(
          'ol',
          {},
          h('li', {}, 'Claudes Anmeldeseite ist in deinem Browser aufgegangen: melde dich dort mit deinem eigenen Claude-Konto an. ', h('button.link', { type: 'button', onclick: click('claudePage') }, 'Seite noch einmal öffnen')),
          h(
            'li',
            {},
            'Sie zeigt dir einen Code. Füge ihn hier ein:',
            form([code], x.pending.sent ? 'Wird geprüft …' : 'Senden', (b) => act(b, 'signin', { what: 'code', which, code: code.value.trim() }).then((ok) => ok && clear('claudeCode'))),
          ),
        ),
      );
    } else {
      body.push(
        h(
          'ol',
          {},
          h('li', {}, 'Dein Einmal-Code für GitHub:', h('div.code', {}, x.pending.code ?? '')),
          h('li', {}, h('button.primary', { type: 'button', onclick: click('githubPage', {}, () => say('Code kopiert: füge ihn auf der GitHub-Seite ein')) }, 'Code kopieren und GitHub öffnen')),
          h('li', {}, 'Sobald GitHub zustimmt, geht es hier von selbst weiter.'),
        ),
      );
    }
    body.push(h('div.row', {}, h('button', { type: 'button', onclick: go('cancel') }, 'Abbrechen')));
  } else if (x.status === 'ok') {
    const row = h(
      'div.row',
      {},
      x.how === 'office'
        ? h('button', { type: 'button', onclick: go('signout') }, 'Mein eigenes nehmen')
        : h('button', { type: 'button', onclick: (e) => window.confirm(`Von ${NAMES[which]} abmelden? ${which === 'claude' ? 'Neue Worker brauchen dann eine neue Anmeldung; laufende arbeiten weiter.' : 'Das Office handelt dann nicht mehr auf GitHub als du.'}`) && go('signout')(e) }, 'Abmelden'),
    );
    if (office && x.how !== 'office') row.append(h('button', { type: 'button', onclick: go('office') }, officeLabel(s, which)));
    body.push(row);
  } else {
    // Already signed in to GitHub on this computer: the same account, in one click.
    const fromHere = which === 'github' && s.mine?.github?.status === 'ok' ? s.mine.github.who : null;
    const row = h('div.row', {}, h(fromHere ? 'button' : 'button.primary', { type: 'button', onclick: go('start') }, `Mit ${NAMES[which]} anmelden`));
    if (fromHere) row.prepend(h('button.primary', { type: 'button', onclick: click('githubFromHere') }, `${fromHere} von diesem Computer übernehmen`));
    if (office) row.append(h('button', { type: 'button', onclick: go('office') }, officeLabel(s, which)));
    const token = field(`${which}Token`, { type: 'password', placeholder: which === 'claude' ? 'sk-ant-oat01-…' : 'ghp_… oder github_pat_…', autocomplete: 'off', spellcheck: 'false', required: true });
    const details = h(
      'details',
      { open: openDetails.has(which) },
      h('summary', {}, 'Oder einen Token einfügen'),
      which === 'claude'
        ? h('p.note', {}, 'Einen Token aus „claude setup-token“ (auf deinem eigenen Computer), oder einen Anthropic-API-Key.')
        : h('p.note', {}, 'Einen GitHub-Token mit repo, read:org und workflow (', h('a', { href: 'https://github.com/settings/tokens/new?scopes=repo,read:org,workflow&description=Agent%20Office' }, 'einen machen'), ').'),
      form([token], 'Speichern', (b) => act(b, 'signin', { what: 'token', which, token: token.value.trim() }).then((ok) => ok && clear(`${which}Token`))),
    );
    details.addEventListener('toggle', () => (details.open ? openDetails.add(which) : openDetails.delete(which)));
    body.push(row, details);
  }
  return h('div.card', { class: x.status }, h('div.head', {}, h('h3', {}, which === 'claude' ? '✳️ Claude' : '🐙 GitHub'), status), body);
}

/**
 * This computer's own Claude Code and GitHub CLI, which your workers here run with. Signing in opens
 * a console window; the card turns ✅ by itself once it's done.
 */
function hereCard(s, which) {
  const x = s.mine?.[which] ?? { status: 'busy' };
  const waiting = s.waiting?.[which] && x.status !== 'ok';
  const open = click(which === 'claude' ? 'hereClaude' : 'hereGithub');
  const status =
    x.status === 'ok'
      ? h('span.ok', {}, '✅ ', x.who)
      : waiting
        ? h('span', {}, '⏳ Wartet auf deine Anmeldung …')
        : x.status === 'busy'
          ? h('span', {}, '⏳ Einen Moment …')
          : h('span', {}, x.missing ? 'Noch nicht installiert' : 'Nicht angemeldet');
  const body = [];
  if (x.status === 'ok') {
    body.push(h('div.row', {}, h('button', { type: 'button', onclick: open }, 'Mit einem anderen Konto anmelden …')));
  } else if (waiting) {
    body.push(
      h(
        'ol',
        {},
        which === 'claude'
          ? h('li', {}, 'Im Fenster, das aufgegangen ist: „Claude account with subscription“ wählen und im Browser bestätigen.')
          : h('li', {}, 'Im Fenster, das aufgegangen ist: den Code kopieren, auf GitHub einfügen und bestätigen.'),
        h('li', {}, 'Sobald es geklappt hat, steht hier ✅ mit deinem Konto.'),
      ),
      h('div.row', {}, h('button', { type: 'button', onclick: open }, 'Fenster noch einmal öffnen')),
    );
  } else if (x.status !== 'busy') {
    body.push(h('div.row', {}, h('button.primary', { type: 'button', onclick: open }, x.missing ? `${NAMES[which]} installieren und anmelden` : `Mit ${NAMES[which]} anmelden`)));
  }
  return h('div.card', { class: waiting ? 'busy' : x.status }, h('div.head', {}, h('h3', {}, which === 'claude' ? '✳️ Claude' : '🐙 GitHub'), status), body);
}

/** Wie es um deinen Arbeitsplatz steht (src/server/arbeitsplatz/satellit.ts), in einem Satz. */
function workstationText(w) {
  if (!w.on) return 'Aus: Deine Arbeiter laufen auf dem Server, mit deinen Anmeldungen dort (unten).';
  const st = w.stand;
  if (!st || st.stand === 'verbinde') return '⏳ Dein Arbeitsplatz verbindet sich mit dem Server …';
  if (st.stand === 'verbunden') return '✅ Die Arbeiter, die du einstellst, laufen auf diesem Computer, mit seinem Claude und GitHub. Alle im Office sehen und benutzen sie trotzdem.';
  if (st.stand === 'abgelehnt') return `❌ ${st.grund}`;
  return `⚠️ ${st.grund}. Neuer Versuch läuft …`;
}

/** Deine Arbeiter auf diesem Computer: dein Arbeitsplatz auf dem Server, und die Anmeldungen, mit denen sie hier laufen. */
function workstationSection(s) {
  const w = s.workstation;
  return h(
    'section',
    {},
    h('h2', {}, '🖥️ Deine Arbeiter auf diesem Computer'),
    h('p', {}, workstationText(w)),
    w.on ? hereCard(s, 'claude') : null,
    w.on ? hereCard(s, 'github') : null,
    h('div.row', {}, h('button', { type: 'button', onclick: click('ownWorkers', { on: !w.on }) }, w.on ? 'Lieber auf dem Server laufen lassen' : 'Auf diesem Computer laufen lassen')),
  );
}

/** Your sign-ins on the server: GitHub always (the office acts there as you), Claude while your workers run there. */
function signinsSection(s) {
  const onServer = !s.workstation?.on;
  return h(
    'section',
    {},
    h('h2', {}, '🔐 Deine Anmeldungen auf dem Server'),
    h('p', {}, onServer ? 'Deine Worker laufen auf dem Server mit deinem eigenen Claude-Abo, und das Office handelt auf GitHub als du.' : 'Das Office handelt auf GitHub als du: Kommentare und Pull Requests aus dem Office erscheinen unter deinem Namen.'),
    !onServer ? null : s.signins.office.claude && !s.me.admin ? h('p.note', {}, '✳️ Der Server teilt sein Claude: nimm es oder dein eigenes. Laufende Worker behalten das, mit dem sie gestartet sind.') : null,
    onServer ? card(s, 'claude') : null,
    card(s, 'github'),
  );
}

function homeView(s) {
  return [
    problem(s),
    h(
      'section',
      {},
      h('button.primary.big', { type: 'button', onclick: click('office') }, '🌐 Office im Browser öffnen'),
      h(
        'p.note',
        {},
        s.viaPassthrough
          ? `Dein Browser bekommt das Office über ${s.passthrough ?? 'http://127.0.0.1:48600'} von diesem Computer, damit Sprache und Bildschirm teilen gehen. Lass den Connector dafür laufen (oben in der Menüleiste bzw. unten im Infobereich).`
          : 'Er öffnet das Office in deinem Browser, angemeldet als du.',
      ),
    ),
    s.workstation ? workstationSection(s) : null,
    s.signins ? signinsSection(s) : null,
    h('p.foot', {}, h('button.link', { type: 'button', onclick: click('settings') }, 'Einstellungen'), ' · ', h('button.link', { type: 'button', onclick: click('disconnect') }, 'Verbindung trennen')),
  ];
}

const busyView = (s) => h('div.loading', {}, h('span.dot'), h('p', {}, s.busy || 'Einen Moment …'));

function render(s) {
  const json = JSON.stringify(s);
  if (json === drawn) return;
  drawn = json;
  const focused = document.activeElement;
  const sel = focused && typeof focused.selectionStart === 'number' ? [focused.selectionStart, focused.selectionEnd] : null;
  const view = s.view === 'busy' ? busyView(s) : s.view === 'home' && s.connected ? homeView(s) : serverView(s);
  root.replaceChildren(header(s), h('main', {}, view));
  if (s.view !== lastView) {
    lastView = s.view;
    const first = [...root.querySelectorAll('input:not([readonly])')].find((i) => !i.value && i.required);
    first?.focus();
  } else if (focused && root.contains(focused)) {
    focused.focus();
    if (sel) focused.setSelectionRange?.(sel[0], sel[1]);
  }
}

window.connector.onState(render);
window.connector.state().then(render);
