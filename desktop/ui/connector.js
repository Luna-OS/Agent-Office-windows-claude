// The connector's window: which lobby, signing in there, your own Claude and GitHub, and the office
// in your browser. It only draws the state the app sends (controller.cjs) and hands each button's
// action back through window.connector (preload.cjs).
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

function header(s) {
  const where = s.lobby
    ? h('div.where', {}, h('b', {}, s.lobby.name), h('span', {}, s.lobby.here ? 'Das Office auf diesem Computer' : s.lobby.origin))
    : h('div.where', {}, h('span', {}, 'Connector'));
  const back = s.lobby && s.view !== 'lobbies' && s.view !== 'busy' ? h('button.link', { type: 'button', onclick: click('lobbies') }, '‹ Lobbys') : null;
  return h('header', {}, h('div.brand', {}, '🏢 Agent Office'), where, back);
}

function lobbiesView(s) {
  const address = field('address', { placeholder: 'Adresse, z. B. 100.78.107.162:4600, oder Einladungslink', spellcheck: 'false', autocomplete: 'off', required: true });
  if (s.address && !address.value) address.value = s.address;
  const name = field('lobbyName', { placeholder: 'Name der Lobby (optional)', maxlength: 40, autocomplete: 'off' });
  const saved = s.lobbies.length
    ? h(
        'ul.lobbies',
        {},
        s.lobbies.map((l) =>
          h(
            'li',
            {},
            h('button.lobby', { type: 'button', onclick: click('connect', { address: l.url }) }, h('b', {}, l.name), h('span', {}, l.url)),
            h('button.forget', { type: 'button', title: 'Vergessen', 'aria-label': `${l.name} vergessen`, onclick: click('forget', { url: l.url }) }, '✕'),
          ),
        ),
      )
    : null;
  return [
    h('p.lead', {}, 'Das Office läuft in einer Lobby, auf deinem Umbrel oder einem Server. Hier meldest du dich an, jeder mit seinem eigenen Claude und GitHub, und öffnest das Office in deinem Browser.'),
    problem(s),
    h(
      'section',
      {},
      h('h2', {}, '🌐 Lobby beitreten'),
      saved,
      form([address, name], 'Verbinden', (b) => act(b, 'connect', { address: address.value, name: name.value }).then((ok) => ok && clear('lobbyName'))),
      h('p.note', {}, 'Mit einem Einladungslink (…/join#…) legst du gleich dein Konto in der Lobby an.'),
    ),
    h(
      'section.quiet',
      {},
      h('h2', {}, '🖥️ Ohne Server'),
      h('p', {}, 'Das Office und seine Worker laufen auf diesem Computer. Das braucht Rechenleistung, je mehr Worker, desto mehr.'),
      h('button', { type: 'button', onclick: click('here') }, 'Auf diesem Computer starten'),
    ),
    relaySection(s),
    h('p.foot', {}, 'Eine eigene Lobby: über den Vermittler (Umbrel-App „Agent Office Vermittler“) · ', h('a', { href: `${DOCS}vermittler.md` }, 'Anleitung')),
  ];
}

/** Wie es beim Vermittler steht, in einem Satz (siehe src/server/vermittler.ts). */
function relayStandText(st) {
  if (!st || st.stand === 'verbinde') return '⏳ Verbinde mit dem Vermittler …';
  if (st.stand === 'gastgeber') return '✅ Du bist Gastgeber: Wer zum Vermittler kommt, landet in deinem Office.';
  if (st.stand === 'ersatz') return `🕒 ${st.gastgeber} ist gerade Gastgeber. Geht er, übernimmt dein Office.`;
  if (st.stand === 'abgelehnt') return `❌ Der Vermittler lehnt ab: ${st.grund}`;
  return `⚠️ ${st.grund}. Neuer Versuch läuft …`;
}

/** Gastgeber über einen Vermittler (die Umbrel-App): die Lobby läuft hier, der Vermittler reicht nur weiter. */
function relaySection(s) {
  if (s.relay) {
    return h(
      'section',
      {},
      h('h2', {}, '🛰️ Über den Vermittler gastgeben'),
      h('p', {}, relayStandText(s.relay.stand)),
      h('p.note', {}, `Mitspieler treten in ihrem Connector unter „Lobby beitreten“ mit ${s.relay.url} bei. Lade sie im Office mit einem Einladungslink ein: so bekommt jeder ein eigenes Konto mit eigenem Claude und GitHub.`),
      h('button', { type: 'button', onclick: click('relayStop') }, 'Nicht mehr gastgeben'),
    );
  }
  const url = field('relayUrl', { placeholder: 'Vermittler, z. B. umbrel.local:4600', spellcheck: 'false', autocomplete: 'off', required: true });
  const pw = field('relayPw', { type: 'password', placeholder: 'Lobby-Passwort des Vermittlers', autocomplete: 'off', required: true });
  return h(
    'section.quiet',
    {},
    h('h2', {}, '🛰️ Über den Vermittler gastgeben'),
    h('p', {}, 'Die Lobby läuft in deinem Office auf diesem Computer, mit allen Etagen und Workern. Der Vermittler (die Umbrel-App) reicht nur weiter: dafür reicht ein schwacher Server.'),
    form([url, pw], 'Gastgeber werden', (b) => act(b, 'relayHost', { url: url.value, password: pw.value }).then((ok) => ok && clear('relayPw'))),
  );
}

function signinView(s) {
  const p = s.pending;
  if (p) {
    const name = field('joinName', { placeholder: 'Dein Name', maxlength: 24, autocomplete: 'username', required: true });
    name.readOnly = !!p.name;
    if (p.name) name.value = p.name;
    const pw = field('joinPw', { type: 'password', placeholder: 'Dein Passwort (mindestens 8 Zeichen)', autocomplete: 'new-password', required: true });
    const again = field('joinAgain', { type: 'password', placeholder: 'Passwort wiederholen', autocomplete: 'new-password', required: true });
    return [
      problem(s),
      h(
        'section',
        {},
        h('h2', {}, '👋 Du bist eingeladen'),
        h('p', {}, `${p.by} lädt dich in ${p.project ? `„${p.project}“` : 'diese Lobby'} ein, als ${p.role === 'admin' ? 'Admin' : 'Mitspieler'}. Wähle deinen Namen und ein Passwort: damit meldest du dich hier wieder an. Danach meldest du dich mit deinem eigenen Claude und GitHub an.`),
        form([name, pw, again], 'Konto anlegen', (b) => act(b, 'join', { name: name.value, password: pw.value, again: again.value }).then((ok) => ok && clear('joinPw', 'joinAgain'))),
      ),
    ];
  }
  const o = s.options ?? { accounts: true, shared: false };
  const name = field('loginName', { maxlength: 64, autocomplete: 'username' });
  name.placeholder = o.shared ? 'Dein Name (leer lassen: mit dem Office-Passwort)' : 'Dein Name';
  name.required = !o.shared;
  const pw = field('loginPw', { type: 'password', placeholder: 'Passwort', autocomplete: 'current-password', required: true });
  return [
    problem(s),
    h(
      'section',
      {},
      h('h2', {}, '🔑 Anmelden'),
      form([name, pw], 'Anmelden', (b) => act(b, 'login', { name: name.value, password: pw.value }).then((ok) => ok && clear('loginPw'))),
      o.shared ? h('p.note', {}, 'Zum ersten Mal hier? Lass den Namen leer und nimm das Passwort des Office (bei der Umbrel-App steht es auf ihrer Seite im App Store). Danach legst du dein eigenes Konto an.') : null,
      h('p.note', {}, 'Neu in dieser Lobby? Füge den Einladungslink, den du bekommen hast, unter „‹ Lobbys“ ein.'),
    ),
  ];
}

/** "Use the host's Claude" for a member it's shared with, the office's own for admins. */
const officeLabel = (s, which) => (which === 'claude' && !s.me.admin ? 'Claude des Hosts nutzen' : `${NAMES[which]} des Office nutzen`);
const officeName = (s, which) => (which === 'claude' && !s.me.admin ? 'Claude des Hosts' : `${NAMES[which]} des Office`);

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
 * The office on this computer, on its password: its workers run on this computer's own Claude Code
 * and GitHub CLI. Signing in opens a console window; the card turns ✅ by itself once it's done.
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
  if (!w.on) return 'Aus: Deine Arbeiter laufen beim Gastgeber, mit deinen Anmeldungen dort.';
  const st = w.stand;
  if (!st || st.stand === 'verbinde') return '⏳ Dein Arbeitsplatz verbindet sich mit der Lobby …';
  if (st.stand === 'verbunden') return '✅ Die Arbeiter, die du einstellst, laufen auf diesem Computer, mit seinem Claude und GitHub. Alle in der Lobby sehen und benutzen sie trotzdem.';
  if (st.stand === 'abgelehnt') return `❌ ${st.grund}`;
  return `⚠️ ${st.grund}. Neuer Versuch läuft …`;
}

/** Deine Arbeiter auf diesem Computer: dein Arbeitsplatz in der Lobby, und die Anmeldungen, mit denen sie hier laufen. */
function workstationSection(s) {
  const w = s.workstation;
  return h(
    'section',
    {},
    h('h2', {}, '🖥️ Deine Arbeiter auf diesem Computer'),
    h('p', {}, workstationText(w)),
    w.on ? hereCard(s, 'claude') : null,
    w.on ? hereCard(s, 'github') : null,
    h('div.row', {}, h('button', { type: 'button', onclick: click('ownWorkers', { on: !w.on }) }, w.on ? 'Lieber beim Gastgeber laufen lassen' : 'Auf diesem Computer laufen lassen')),
  );
}

/** Your sign-ins, first thing: the lobby's own for an account, this computer's for the office here on its password. */
function signinsSection(s) {
  const here = !s.signins;
  return h(
    'section',
    {},
    h('h2', {}, '🔐 Deine Anmeldungen'),
    here
      ? h('p', {}, 'Das Office auf diesem Computer arbeitet mit deinem Claude-Abo und deinem GitHub, so wie du sie hier anmeldest. Einmal anmelden reicht, für alle Worker hier.')
      : h('p', {}, 'Deine Worker laufen mit deinem eigenen Claude-Abo, und das Office handelt auf GitHub als du: Kommentare, Commits und Pull Requests erscheinen unter deinem Namen. Nur deine Worker nutzen sie.'),
    !here && s.signins.office.claude && !s.me.admin ? h('p.note', {}, '✳️ Der Host teilt sein Claude: nimm es oder dein eigenes. Laufende Worker behalten das, mit dem sie gestartet sind.') : null,
    here ? [hereCard(s, 'claude'), hereCard(s, 'github')] : [card(s, 'claude'), card(s, 'github')],
  );
}

/** On the office password: an account of your own, for your own sign-ins. */
function ownAccount() {
  const name = field('ownName', { placeholder: 'Dein Name', maxlength: 24, autocomplete: 'username', required: true });
  const pw = field('ownPw', { type: 'password', placeholder: 'Dein Passwort (mindestens 8 Zeichen)', autocomplete: 'new-password', required: true });
  const again = field('ownAgain', { type: 'password', placeholder: 'Passwort wiederholen', autocomplete: 'new-password', required: true });
  return h(
    'section',
    {},
    h('h2', {}, '👤 Dein eigenes Konto'),
    h('p', {}, 'Du bist mit dem Passwort des Office angemeldet. Leg dir ein eigenes Konto an: Gleich danach meldest du dich hier mit deinem eigenen Claude und GitHub an. Ohne laufen deine Worker mit den Anmeldungen des Office.'),
    form([name, pw, again], 'Konto anlegen', (b) => act(b, 'account', { name: name.value, password: pw.value, again: again.value }).then((ok) => ok && clear('ownPw', 'ownAgain'))),
  );
}

function inviteSection(s) {
  const name = field('inviteName', { placeholder: 'Name (optional)', maxlength: 24, autocomplete: 'off' });
  const role = field('inviteRole', { tag: 'select', 'aria-label': 'Rolle', options: [['member', 'Mitspieler'], ['admin', 'Admin']] });
  return h(
    'section',
    {},
    h('h2', {}, '👥 Mitspieler einladen'),
    h('p', {}, 'Der Link legt einmal ein Konto an und gilt 7 Tage. Wer ihn bekommt, fügt ihn im eigenen Connector unter „Lobby beitreten“ ein, oder öffnet ihn im Browser.'),
    form([name, role], 'Einladungslink erstellen', (b) => act(b, 'invite', { name: name.value, role: role.value }).then((ok) => ok && clear('inviteName'))),
    s.invited ? h('div.invited', {}, h('code', {}, s.invited.link), h('button', { type: 'button', onclick: click('copy', { text: s.invited.link }, () => say('Einladungslink kopiert')) }, 'Kopieren')) : null,
  );
}

function homeView(s) {
  const me = s.me;
  const who = me.account ? `Angemeldet als ${me.account.name} (${me.account.role === 'admin' ? 'Admin' : 'Mitspieler'})` : 'Mit dem Office-Passwort angemeldet';
  // First thing: what you sign in with. In a lobby on its password that takes an account of your own first.
  const first = me.account || s.lobby.here ? signinsSection(s) : ownAccount();
  return [
    problem(s),
    first,
    h(
      'section',
      {},
      h('button.primary.big', { type: 'button', onclick: click('office') }, '🌐 Office im Browser öffnen'),
      h(
        'p.note',
        {},
        s.viaPassthrough
          ? `Dein Browser bekommt die Lobby über ${s.passthrough ?? 'http://127.0.0.1:48600'} von diesem Computer, damit Sprache und Bildschirm teilen gehen. Lass den Connector dafür laufen.`
          : 'Er öffnet das Office in deinem Browser, angemeldet als du.',
      ),
    ),
    s.lobby.here && s.relay ? relaySection(s) : null,
    s.workstation ? workstationSection(s) : null,
    me.admin ? inviteSection(s) : null,
    h('p.foot', {}, who, ' · ', h('button.link', { type: 'button', onclick: click('logout') }, 'Abmelden')),
  ];
}

const busyView = (s) => h('div.loading', {}, h('span.dot'), h('p', {}, s.busy || 'Einen Moment …'));

function render(s) {
  const json = JSON.stringify(s);
  if (json === drawn) return;
  drawn = json;
  const focused = document.activeElement;
  const sel = focused && typeof focused.selectionStart === 'number' ? [focused.selectionStart, focused.selectionEnd] : null;
  const view = s.view === 'busy' ? busyView(s) : s.view === 'signin' ? signinView(s) : s.view === 'home' && s.me ? homeView(s) : lobbiesView(s);
  root.replaceChildren(header(s), h('main', {}, view));
  if (s.view !== lastView) {
    lastView = s.view;
    const first = [...root.querySelectorAll('input:not([readonly])')].find((i) => !i.value);
    first?.focus();
  } else if (focused && root.contains(focused)) {
    focused.focus();
    if (sel) focused.setSelectionRange?.(sel[0], sel[1]);
  }
}

window.connector.onState(render);
window.connector.state().then(render);
