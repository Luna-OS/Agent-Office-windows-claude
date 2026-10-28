// Browser, die es nur zum Schein gibt: Auf dem Arbeitsplatz steht jeder Browser beim Gastgeber, der
// mit einem seiner Arbeiter zu tun hat, als Verbindung im Office, damit dessen eigene Handler und
// Nachrichten (Terminals, Antworten, Warnungen) wie immer laufen. Was so eine Verbindung bekommt,
// geht an `weiter`.
import { WebSocket } from 'ws';
import type { ServerMsg } from '../../shared/protocol.js';
import { lookFromSeed } from '../../shared/avatar.js';
import { newClient, type Client } from '../office/client.js';

/** Das Stück WebSocket, das das Office von einer Verbindung braucht. */
function scheinSocket(weiter: (msg: ServerMsg) => void): WebSocket {
  const ws = {
    readyState: WebSocket.OPEN as number,
    bufferedAmount: 0,
    send(json: string) {
      try {
        weiter(JSON.parse(json) as ServerMsg);
      } catch {
        // kein JSON: nichts weiterzugeben
      }
    },
    close() {
      ws.readyState = WebSocket.CLOSED;
    },
    terminate() {
      ws.readyState = WebSocket.CLOSED;
    },
    ping() {},
    on() {
      return ws;
    },
  };
  return ws as unknown as WebSocket;
}

/**
 * Eine Verbindung zum Schein, für `name`, auf `floor` (oder auf keiner Etage). Sie antwortet selbst auf
 * den Herzschlag des Office (office/timers.ts), der stumme Verbindungen sonst nach 40 s schließt.
 */
export function scheinBrowser(id: string, name: string, floor: string | undefined, weiter: (msg: ServerMsg) => void): Client {
  const ws = scheinSocket(weiter);
  const c = newClient(id, ws, { accountId: undefined, admin: true }, {
    id,
    name,
    color: '#8d99ae',
    look: lookFromSeed(id),
    x: 0,
    y: 0,
    z: 0,
    rotY: 0,
    moving: false,
    voice: false,
    muted: true,
    sharing: false,
    ...(floor ? { floor } : {}),
  });
  (ws as unknown as { ping(): void }).ping = () => {
    c.isAlive = true;
  };
  return c;
}
