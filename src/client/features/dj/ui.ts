import './ui.css';
import { parseYoutube } from '../../../shared/youtube';
import type { Net } from '../../net';
import { store } from '../../state';
import { h, openModal, toast } from '../../ui/dom';

export interface DjWindowDeps {
  net: Net;
  /** Der Titel des Songs, sobald YouTube ihn verraten hat (siehe player.ts). */
  songTitle(): string | undefined;
  /** Das Lufthorn, für alle auf dem Dach. */
  horn(): void;
}

/** Das DJ-Pult: was läuft, einen YouTube-Song auflegen oder runternehmen, und das Lufthorn. */
export function openDjBooth(deps: DjWindowDeps) {
  const close = h('button.btn.close', { 'aria-label': 'Schließen' }, '✕');
  const now = h('div.jb-now');
  const url = h('input', { type: 'text', placeholder: 'YouTube-Link, z. B. https://youtu.be/…', 'aria-label': 'YouTube-Link', spellcheck: 'false', autocomplete: 'off' }) as HTMLInputElement;
  const put = h('button.btn.primary', { type: 'button' }, '▶️ Auflegen');
  const horn = h('button.btn', { type: 'button', title: 'Alle auf dem Dach hören es' }, '📯 Lufthorn');
  const el = h(
    'div.modal.jukebox.dj-booth',
    { role: 'dialog', 'aria-label': 'DJ-Pult' },
    h('header', {}, h('h2', {}, '🎧 DJ Merge Conflict'), close),
    h(
      'div.body',
      {},
      now,
      h('label', { style: 'margin-top:16px' }, 'Einen Song auflegen'),
      h('div.webhook', {}, url, put),
      h('p.setting-note', {}, 'Ein Link von YouTube (auch youtu.be, Shorts oder mit Startzeit wie ?t=1m30s). Alle auf dem Dach hören ihn gleichzeitig, solange der DJ ihn spielt.'),
    ),
    h('footer', {}, h('span.grow', {}, 'Ohne Song legt der DJ selbst auf: Drum & Bass.'), horn),
  );

  const render = () => {
    const d = store.dj;
    const title = d ? (deps.songTitle() ?? `YouTube-Video ${d.video}`) : 'Drum & Bass vom DJ';
    now.replaceChildren(
      h('span.jb-disc.spin', {}, d ? '📺' : '💿'),
      h('div.svc-main', {}, h('div.svc-title', {}, title), h('div.svc-meta', {}, d ? `aufgelegt von ${d.by}` : 'sein eigenes Set, bis jemand einen Song auflegt')),
      d ? h('button.btn', { type: 'button', title: 'Den Song runternehmen', onclick: () => deps.net.send({ t: 'dj.stop' }) }, '⏹️ Zurück zum DJ-Set') : '',
    );
    horn.toggleAttribute('disabled', !!d);
    horn.title = d ? 'Das Lufthorn gehört zum Set des DJs' : 'Alle auf dem Dach hören es';
  };

  const play = () => {
    try {
      parseYoutube(url.value);
    } catch (err) {
      toast((err as Error).message, 'warn');
      return url.focus();
    }
    deps.net.send({ t: 'dj.play', url: url.value.trim() });
    url.value = '';
  };
  put.addEventListener('click', play);
  url.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') play();
  });
  horn.addEventListener('click', () => deps.horn());

  const modal = openModal(el, { doing: '🎧 am DJ-Pult', onClose: store.on('dj', render) });
  close.addEventListener('click', () => modal.close());
  render();
  url.focus();
}
