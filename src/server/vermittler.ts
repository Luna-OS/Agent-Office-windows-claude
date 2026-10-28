// Gastgeber über einen Vermittler (crates/vermittler, etwa als Umbrel-App): Das Office meldet sich dort
// an, und jede Verbindung, die beim Vermittler ankommt, reicht er hierher durch, als käme sie direkt.
// Der Vermittler rechnet nichts; die Lobby, ihre Etagen und Arbeiter laufen hier, auf diesem Computer.
//
// Über einen WebSocket zu <vermittler>/vermittler/knoten: Steuernachrichten als JSON-Text (hallo,
// willkommen, abgelehnt, aktiv, auf, zu), Daten binär mit vier Bytes Verbindungsnummer davor
// (siehe docs/vermittler.md).
import net from 'node:net';
import { WebSocket } from 'ws';

export const VERMITTLER_PROTOKOLL = 1;
/** Mehr wartet nicht im WebSocket, sonst halten die Verbindungen an, bis er wieder leer ist. */
const STAU = 4 * 1024 * 1024;
/** Kommt so lange nichts vom Vermittler (er pingt alle 20 s), ist die Verbindung tot. */
const STILLE_MS = 60_000;

export interface VermittlerOpts {
  /** Wo der Vermittler ist, z. B. http://umbrel.local:4600. */
  url: string;
  passwort: string;
  /** Wie die anderen den Gastgeber sehen. */
  name: string;
  /** Wo dieses Office lauscht. */
  officeHost: string;
  officePort: number;
  log?: (text: string) => void;
}

export type VermittlerStand =
  | { stand: 'verbinde' }
  | { stand: 'gastgeber' }
  | { stand: 'ersatz'; gastgeber: string }
  | { stand: 'abgelehnt'; grund: string }
  | { stand: 'getrennt'; grund: string };

/** Die Adresse des WebSockets, an dem sich ein Office beim Vermittler `url` anmeldet. */
export function knotenAdresse(url: string): string {
  const u = new URL(/^[a-z]+:\/\//i.test(url) ? url : `http://${url}`);
  if (!u.port && u.protocol === 'http:') u.port = '4600';
  u.protocol = u.protocol === 'https:' ? 'wss:' : 'ws:';
  u.pathname = '/vermittler/knoten';
  u.search = '';
  u.hash = '';
  return u.toString();
}

/** Ein Rahmen mit Daten für die Verbindung `strom`. */
export function rahmen(strom: number, daten: Buffer): Buffer {
  const kopf = Buffer.alloc(4);
  kopf.writeUInt32BE(strom >>> 0);
  return Buffer.concat([kopf, daten]);
}

/**
 * Hält dieses Office beim Vermittler angemeldet, bis `stop()`: Bricht die Verbindung ab, versucht
 * es nach einer Pause wieder (2 s, länger bis 30 s). Nur ein falsches Passwort beendet es.
 */
export function gastgebenUeberVermittler(opts: VermittlerOpts) {
  const log = opts.log ?? ((t: string) => console.log(`vermittler: ${t}`));
  const ziel = /^(0\.0\.0\.0|::|)$/.test(opts.officeHost) ? '127.0.0.1' : opts.officeHost;
  let stand: VermittlerStand = { stand: 'verbinde' };
  let gestoppt = false;
  let pause = 2000;
  let ws: WebSocket | undefined;
  let wieder: NodeJS.Timeout | undefined;
  const stroeme = new Map<number, net.Socket>();

  const alleZu = () => {
    for (const s of stroeme.values()) s.destroy();
    stroeme.clear();
  };

  const verbinden = () => {
    if (gestoppt) return;
    const sock = new WebSocket(knotenAdresse(opts.url), { maxPayload: 8 * 1024 * 1024 });
    ws = sock;
    let stille: NodeJS.Timeout | undefined;
    const lebt = () => {
      clearTimeout(stille);
      stille = setTimeout(() => sock.terminate(), STILLE_MS);
    };
    const senden = (nachricht: object) => {
      if (sock.readyState === WebSocket.OPEN) sock.send(JSON.stringify(nachricht));
    };
    const zu = (strom: number, melden: boolean) => {
      const s = stroeme.get(strom);
      if (!s) return;
      stroeme.delete(strom);
      if (melden) {
        s.destroy();
        senden({ t: 'zu', strom });
      } else {
        // Der Browser ist fertig: Was noch zum Office unterwegs ist, kommt noch an, dann ist Schluss.
        s.end();
        setTimeout(() => s.destroy(), 5000).unref();
      }
    };
    const auf = (strom: number) => {
      const s = net.connect(opts.officePort, ziel);
      s.setNoDelay(true);
      stroeme.set(strom, s);
      s.on('data', (d: Buffer) => {
        if (sock.readyState !== WebSocket.OPEN) return void s.destroy();
        sock.send(rahmen(strom, d));
        // Staut es sich, wartet diese Verbindung, bis der WebSocket wieder Luft hat.
        if (sock.bufferedAmount > STAU) {
          s.pause();
          const weiter = setInterval(() => {
            if (sock.readyState !== WebSocket.OPEN || sock.bufferedAmount < STAU / 2) {
              clearInterval(weiter);
              s.resume();
            }
          }, 50);
        }
      });
      s.on('error', () => zu(strom, true));
      s.on('close', () => zu(strom, true));
    };

    sock.on('open', () => {
      lebt();
      senden({ t: 'hallo', version: VERMITTLER_PROTOKOLL, passwort: opts.passwort, name: opts.name });
    });
    sock.on('ping', lebt);
    sock.on('message', (d: Buffer, binaer: boolean) => {
      lebt();
      if (binaer) {
        if (d.length < 4) return;
        stroeme.get(d.readUInt32BE(0))?.write(d.subarray(4));
        return;
      }
      let m: { t?: string; strom?: number; aktiv?: boolean; gastgeber?: string; grund?: string };
      try {
        m = JSON.parse(d.toString('utf8'));
      } catch {
        return;
      }
      switch (m.t) {
        case 'willkommen':
          pause = 2000;
          stand = m.aktiv ? { stand: 'gastgeber' } : { stand: 'ersatz', gastgeber: m.gastgeber ?? '?' };
          log(m.aktiv ? `angemeldet: Die Lobby läuft jetzt hier, erreichbar unter ${opts.url}` : `angemeldet als Ersatz: Gastgeber ist gerade ${m.gastgeber}`);
          break;
        case 'aktiv':
          stand = { stand: 'gastgeber' };
          log('Der bisherige Gastgeber ist gegangen: Die Lobby läuft jetzt hier.');
          break;
        case 'abgelehnt':
          stand = { stand: 'abgelehnt', grund: m.grund ?? 'abgelehnt' };
          log(`abgelehnt: ${stand.grund}`);
          gestoppt = true;
          sock.close();
          break;
        case 'auf':
          if (typeof m.strom === 'number') auf(m.strom);
          break;
        case 'zu':
          if (typeof m.strom === 'number') zu(m.strom, false);
          break;
      }
    });
    sock.on('error', (e) => {
      if (stand.stand !== 'abgelehnt') stand = { stand: 'getrennt', grund: e.message };
    });
    sock.on('close', () => {
      clearTimeout(stille);
      alleZu();
      if (gestoppt) return;
      if (stand.stand !== 'getrennt') stand = { stand: 'getrennt', grund: 'Verbindung zum Vermittler unterbrochen' };
      log(`${stand.stand === 'getrennt' ? stand.grund : 'getrennt'}; neuer Versuch in ${Math.round(pause / 1000)} s`);
      wieder = setTimeout(verbinden, pause);
      pause = Math.min(pause * 2, 30_000);
    });
  };

  verbinden();
  return {
    stand: (): VermittlerStand => stand,
    stop() {
      gestoppt = true;
      clearTimeout(wieder);
      alleZu();
      ws?.close();
    },
  };
}
