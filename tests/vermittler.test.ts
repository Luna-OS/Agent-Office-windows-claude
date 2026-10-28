// Gastgeber über einen Vermittler (src/server/vermittler.ts), gegen einen Vermittler zum Schein:
// anmelden, eine durchgereichte Verbindung zum Office und zurück, und Ablehnung bei falschem Passwort.
// Der echte Vermittler (crates/vermittler) hat dieselben Fälle in seinen eigenen Tests.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { WebSocketServer, type WebSocket } from 'ws';
import { gastgebenUeberVermittler, knotenAdresse, rahmen } from '../src/server/vermittler.js';

const port = (s: { address(): unknown }) => (s.address() as AddressInfo).port;

function lauschen<T extends http.Server>(s: T): Promise<T> {
  return new Promise((resolve) => s.listen(0, '127.0.0.1', () => resolve(s)));
}

/** Ein Vermittler zum Schein: gibt jede neue Anmeldung an `bei` weiter. */
async function scheinVermittler(bei: (ws: WebSocket, hallo: Record<string, unknown>) => void) {
  const server = await lauschen(http.createServer());
  const wss = new WebSocketServer({ server, path: '/vermittler/knoten' });
  wss.on('connection', (ws) => ws.once('message', (d) => bei(ws, JSON.parse(String(d)))));
  return { url: `http://127.0.0.1:${port(server)}`, close: () => (wss.close(), server.close()) };
}

test('knotenAdresse: aus der Adresse des Vermittlers wird sein WebSocket', () => {
  assert.equal(knotenAdresse('http://umbrel.local:4600'), 'ws://umbrel.local:4600/vermittler/knoten');
  assert.equal(knotenAdresse('umbrel.local'), 'ws://umbrel.local:4600/vermittler/knoten');
  assert.equal(knotenAdresse('100.78.107.162:4600/'), 'ws://100.78.107.162:4600/vermittler/knoten');
  assert.equal(knotenAdresse('https://lobby.example'), 'wss://lobby.example/vermittler/knoten');
});

test('rahmen: vier Bytes Verbindungsnummer, dann die Daten', () => {
  assert.deepEqual([...rahmen(258, Buffer.from('ok'))], [0, 0, 1, 2, 111, 107]);
});

test('eine Verbindung vom Vermittler landet im Office und die Antwort geht zurück', async () => {
  const office = await lauschen(http.createServer((req, res) => res.end(`Hallo vom Office, ${req.headers['x-forwarded-for']}`)));
  let antwort = '';
  let halloGesehen: Record<string, unknown> = {};
  const fertig = new Promise<void>((resolve) => {
    void scheinVermittler((ws, hallo) => {
      halloGesehen = hallo;
      ws.send(JSON.stringify({ t: 'willkommen', version: 1, aktiv: true, gastgeber: 'Luna' }));
      ws.send(JSON.stringify({ t: 'auf', strom: 7, von: '100.64.0.9' }));
      ws.send(rahmen(7, Buffer.from('GET / HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: 100.64.0.9\r\nConnection: close\r\n\r\n')));
      ws.on('message', (d: Buffer, binaer: boolean) => {
        if (binaer) {
          assert.equal(d.readUInt32BE(0), 7);
          antwort += d.subarray(4).toString();
        } else if (JSON.parse(String(d)).t === 'zu') resolve();
      });
    }).then((v) => {
      const gast = gastgebenUeberVermittler({ url: v.url, passwort: 'geheim', name: 'Luna', officeHost: '127.0.0.1', officePort: port(office), log: () => {} });
      void fertig.then(() => {
        gast.stop();
        v.close();
        office.close();
      });
    });
  });
  await fertig;
  assert.deepEqual(halloGesehen, { t: 'hallo', version: 1, passwort: 'geheim', name: 'Luna' });
  assert.match(antwort, /^HTTP\/1\.1 200 OK/);
  assert.match(antwort, /Hallo vom Office, 100\.64\.0\.9$/);
});

test('bei falschem Passwort hört das Office auf, es zu versuchen', async () => {
  let versuche = 0;
  const v = await scheinVermittler((ws) => {
    versuche++;
    ws.send(JSON.stringify({ t: 'abgelehnt', grund: 'Falsches Lobby-Passwort.' }));
  });
  const gast = gastgebenUeberVermittler({ url: v.url, passwort: 'falsch', name: 'Luna', officeHost: '127.0.0.1', officePort: 1, log: () => {} });
  for (let i = 0; i < 50 && gast.stand().stand !== 'abgelehnt'; i++) await new Promise((r) => setTimeout(r, 20));
  assert.deepEqual(gast.stand(), { stand: 'abgelehnt', grund: 'Falsches Lobby-Passwort.' });
  await new Promise((r) => setTimeout(r, 2500));
  assert.equal(versuche, 1);
  gast.stop();
  v.close();
});
