import './accounts.css';
import type { Net } from '../net';
import { store } from '../state';
import { h, openModal, timeAgo } from './dom';
import { confirmDialog } from './prompt';

/** 🔑 Accounts, for admins: who has one (made the first time someone signs in with their name), their role, or revoking them. */
export function openAccounts(net: Net) {
  const body = h('div.body.team.accounts');
  const close = h('button.btn.close', { 'aria-label': 'Close' }, '✕');
  const signedInAs = h('span.grow');
  const el = h(
    'div.modal',
    { role: 'dialog', 'aria-label': 'Accounts', style: 'width:min(680px,100%)' },
    h('header', {}, h('h2', {}, '🔑 Accounts'), close),
    body,
    h('footer', {}, signedInAs),
  );

  const render = () => {
    const s = store.accounts;
    const me = store.me;
    signedInAs.textContent = me.account ? `You're signed in as ${me.account.name} (${me.account.role}).` : "You're signed in with the shared office password.";
    body.replaceChildren();
    if (!s) return body.append(h('p.empty', {}, 'Loading…'));

    body.append(
      h('p.note', {}, 'Wer sich in der App „Agent Office“ mit seinem Namen und dem Passwort des Servers anmeldet, bekommt beim ersten Mal hier sein Konto, als Admin. Einladungslinks braucht es nicht.'),
    );

    const list = h('ul.team-list');
    for (const a of s.accounts) {
      const you = me.account?.name === a.name;
      const seen = a.online ? 'in the office' : a.lastSeenAt ? `seen ${timeAgo(a.lastSeenAt)}` : 'never came in';
      const role = h('button.btn', { type: 'button', title: a.role === 'admin' ? 'Take away admin rights' : 'Let them manage accounts too' }, a.role === 'admin' ? 'Make member' : 'Make admin');
      role.addEventListener('click', () => net.send({ t: 'accounts.role', accountId: a.id, role: a.role === 'admin' ? 'member' : 'admin' }));
      const revoke = h('button.btn.danger', { type: 'button', title: `Delete ${a.name}'s account` }, 'Revoke');
      revoke.addEventListener('click', () =>
        confirmDialog(
          `Revoke ${a.name}?`,
          `Their account is deleted and they're signed out everywhere right away. Terminals they typed in keep running. Wer das Passwort des Servers kennt, bekommt mit seinem Namen beim nächsten Anmelden wieder ein Konto.`,
          'Revoke',
          () => net.send({ t: 'accounts.revoke', accountId: a.id }),
        ),
      );
      list.append(
        h(
          'li',
          {},
          h('span.dot', { class: a.online ? 'on' : '', title: seen }),
          h('span.name', {}, a.name, you ? h('span.you', {}, ' (you)') : null),
          h('span.role', { class: a.role }, a.role),
          h('span.keys', { title: `Dabei seit ${timeAgo(a.createdAt)}` }, seen),
          you ? null : role,
          you ? null : revoke,
        ),
      );
    }
    if (!s.accounts.length) list.append(h('li.empty', {}, 'Nobody has an account yet'));
    body.append(h('h4', {}, 'People ', h('span.count', {}, String(s.accounts.length))), list);

    // The office password without a name: a guest without an account of their own.
    const toggle = h('button.btn', { type: 'button', class: s.sharedPassword ? 'danger' : '' }, s.sharedPassword ? 'Switch it off' : 'Switch it back on');
    const canSwitchOff = me.account?.role === 'admin';
    if (s.sharedPassword && !canSwitchOff) toggle.setAttribute('disabled', '');
    toggle.addEventListener('click', () => {
      if (!s.sharedPassword) return net.send({ t: 'accounts.shared', on: true });
      confirmDialog(
        'Switch off the shared password?',
        'Ab jetzt kommt nur noch herein, wer seinen Namen dazu sagt (die App macht das immer): jeder als sein Konto. Wer ohne Namen hereinkam, wird gleich abgemeldet.',
        'Switch it off',
        () => net.send({ t: 'accounts.shared', on: false }),
      );
    });
    // The host's AI: members may run their workers on the office machine's own Claude, not only on their own.
    const share = h('button.btn', { type: 'button', class: s.shareClaude ? 'on' : '' }, s.shareClaude ? 'Stop sharing it' : 'Share it with everyone');
    share.addEventListener('click', () => net.send({ t: 'accounts.shareClaude', on: !s.shareClaude }));
    body.append(
      h('div.team-head', {}, h('h4', {}, '✳️ The office’s own Claude'), share),
      h(
        'p.note',
        {},
        s.shareClaude
          ? 'Shared. Every member picks in 🔐 Your sign-ins: their workers run on the office machine’s own Claude (the host’s plan, and its limits), or on their own. Admins always may.'
          : 'Admins only. Everyone else runs their workers on their own Claude plan. Share it, and members can pick the host’s Claude instead, from 🔐 Your sign-ins.',
      ),
    );
    body.append(
      h('div.team-head', {}, h('h4', {}, 'Passwort ohne Namen'), toggle),
      h(
        'p.note',
        {},
        s.sharedPassword
          ? 'An: Wer das Passwort des Servers kennt, kommt im Browser auch ohne Namen herein, als Gast ohne eigenes Konto. Mit Namen ist er immer sein Konto.'
          : 'Aus: Jeder kommt nur mit seinem Namen herein, als sein Konto. Wieder an: agent-office accounts password on auf dem Server.',
        s.sharedPassword && !canSwitchOff ? h('b', {}, ' Melde dich zuerst mit deinem Namen an, dann kannst du es ausschalten.') : null,
      ),
    );
  };
  // No longer an admin (someone changed your role): the list isn't yours to see any more.
  const unsubs = [store.on('accounts', render), store.on('me', () => (store.me.admin ? render() : modal.close()))];
  const modal = openModal(el, {
    onClose: () => unsubs.forEach((u) => u()),
  });
  close.addEventListener('click', () => modal.close());
  render();
  net.send({ t: 'accounts.get' });
}
