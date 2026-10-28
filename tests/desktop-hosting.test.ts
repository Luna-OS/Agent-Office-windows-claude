// A lobby hosted on this computer (desktop/hosting.cjs): which addresses friends are told, and the invite.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { inviteText, isTailnet, newPassword, shareAddresses } = require('../desktop/hosting.cjs');

const nic = (address: string, internal = false) => ({ address, family: 'IPv4', internal, netmask: '255.255.255.0', mac: '00:00:00:00:00:00', cidr: `${address}/24` });

test('friends are told the home network first, then Tailscale, never this computer itself', () => {
  const addresses = shareAddresses(4600, {
    lo: [nic('127.0.0.1', true)],
    Tailscale: [nic('100.78.135.198'), { ...nic('fd7a:115c::1'), family: 'IPv6' }],
    'vEthernet (WSL)': [nic('172.27.16.1')],
    WLAN: [nic('192.168.178.20')],
    nothing: [nic('169.254.12.3')],
  });
  assert.deepEqual(addresses, [
    { url: 'http://192.168.178.20:4600', label: 'Netzwerk (WLAN)' },
    { url: 'http://100.78.135.198:4600', label: 'Tailscale' },
    { url: 'http://172.27.16.1:4600', label: 'Netzwerk (vEthernet (WSL))' },
  ]);
});

test('Tailscale is 100.64.0.0/10 and nothing else', () => {
  assert.equal(isTailnet('100.64.0.1'), true);
  assert.equal(isTailnet('100.127.255.254'), true);
  assert.equal(isTailnet('100.78.135.198'), true);
  assert.equal(isTailnet('100.63.0.1'), false);
  assert.equal(isTailnet('100.128.0.1'), false);
  assert.equal(isTailnet('10.0.0.1'), false);
});

test('the invite has every address without http:// and the password; each lobby gets a password of its own', () => {
  const text = inviteText([{ url: 'http://192.168.178.20:4601', label: 'Netzwerk (WLAN)' }], 'pw-1234567890');
  assert.match(text, /192\.168\.178\.20:4601 {3}\(Netzwerk \(WLAN\)\)/);
  assert.doesNotMatch(text, /http:\/\//);
  assert.match(text, /Passwort: pw-1234567890/);
  assert.match(inviteText([], 'x'), /in keinem Netzwerk/);
  const a = newPassword();
  assert.match(a, /^[A-Za-z0-9_-]{12}$/);
  assert.notEqual(a, newPassword());
});

test("the office's own addresses for invite links: none when it's open to this computer only", async () => {
  const { networkOrigins } = await import('../src/server/office/origins.js');
  const nics = { lo: [nic('127.0.0.1', true)], tailscale0: [nic('100.78.135.198')], wlan0: [nic('192.168.178.20')] };
  assert.deepEqual(networkOrigins({ host: '127.0.0.1', port: 4600, tls: undefined }, nics), []);
  assert.deepEqual(networkOrigins({ host: '0.0.0.0', port: 4601, tls: undefined }, nics), ['http://192.168.178.20:4601', 'http://100.78.135.198:4601']);
  assert.deepEqual(networkOrigins({ host: '192.168.178.20', port: 4600, tls: { cert: '', key: '' } }, nics), ['https://192.168.178.20:4600']);
  // Behind a reverse proxy (the Umbrel app's), its own addresses are its container's: nobody comes in by those.
  assert.deepEqual(networkOrigins({ host: '0.0.0.0', port: 4600, tls: undefined, trustProxy: true }, nics), []);
});
