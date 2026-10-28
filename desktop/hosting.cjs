// A lobby hosted on this computer: the office opened to the network (the home network, Tailscale)
// instead of to this computer alone, behind a password, so friends join it from their own app or a
// browser. The workers run here then, on this computer, for everyone.
const { randomBytes } = require('node:crypto');
const os = require('node:os');

/** A password for the shared office: kept in the app's settings, so it stays the same from one start to the next. */
function newPassword() {
  return randomBytes(9).toString('base64url');
}

/** Whether an IPv4 address is Tailscale's (100.64.0.0/10). */
const isTailnet = (ip) => {
  const [a, b] = ip.split('.').map(Number);
  return a === 100 && b >= 64 && b <= 127;
};

/**
 * Where others reach an office on this computer at `port`: an address on each network it's on,
 * the home network's and Tailscale's first; never this computer's own (127.0.0.1) or a
 * self-assigned one (169.254.x.x, no network there).
 */
function shareAddresses(port, interfaces = os.networkInterfaces()) {
  const out = [];
  for (const [name, list] of Object.entries(interfaces)) {
    for (const a of list ?? []) {
      if (a.family !== 'IPv4' && a.family !== 4) continue;
      if (a.internal || a.address.startsWith('169.254.')) continue;
      const tailnet = isTailnet(a.address);
      const home = /^(192\.168|10)\./.test(a.address);
      out.push({ url: `http://${a.address}:${port}`, label: tailnet ? 'Tailscale' : `Netzwerk (${name})`, rank: tailnet ? 1 : home ? 0 : 2 });
    }
  }
  return out.sort((x, y) => x.rank - y.rank).map(({ url, label }) => ({ url, label }));
}

/** What to send a friend: the addresses to put in the app's "Online-Lobby", and the password. */
function inviteText(addresses, password) {
  const where = addresses.length ? addresses.map((a) => `  ${a.url.replace(/^http:\/\//, '')}   (${a.label})`).join('\n') : '  (dieser Computer ist gerade in keinem Netzwerk)';
  return (
    'Komm in meine Agent-Office-Lobby!\n\n' +
    'App „Agent Office“ öffnen → 🌐 Online-Lobby → Adresse eintragen:\n' +
    `${where}\n\n` +
    `Passwort: ${password}\n\n` +
    'Im selben WLAN die Netzwerk-Adresse, sonst die Tailscale-Adresse (dafür muss ich dir meinen Computer in Tailscale freigeben).'
  );
}

module.exports = { newPassword, shareAddresses, inviteText, isTailnet };
