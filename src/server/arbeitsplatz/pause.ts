// Beim Gastgeber: die Arbeiter der Arbeitsplätze, die gerade weg sind (der PC ihres Spielers ist aus).
// Sie bleiben an ihren Plätzen sitzen und machen Pause (WorkerInfo.pause), bis sich ihr Arbeitsplatz
// wieder meldet, und ihr Platz bleibt so lange belegt. Der Gastgeber merkt sie sich in seinen Daten
// (arbeitsplatz-pausen.json), damit sie auch nach einem Neustart seines Office dort sitzen.
import { existsSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import type { WorkerInfo } from '../../shared/protocol.js';

export interface InPause {
  /** Das Konto des Spielers, auf dessen Arbeitsplatz er läuft, und sein Name. */
  account: string;
  name: string;
  /** Die Etage des Gastgebers, auf der er sitzt. */
  etage: string;
  /** Wie er zu sehen ist: wie zuletzt, aber in Pause. */
  info: WorkerInfo;
}

export class Pausen {
  private byId = new Map<string, InPause>();

  constructor(private file: string | undefined) {
    if (!file || !existsSync(file)) return;
    try {
      const saved = JSON.parse(readFileSync(file, 'utf8')) as unknown;
      if (!Array.isArray(saved)) return;
      for (const p of saved as InPause[]) {
        if (typeof p?.account === 'string' && typeof p.name === 'string' && typeof p.etage === 'string' && typeof p.info?.id === 'string') this.byId.set(p.info.id, p);
      }
    } catch {
      // Kaputt: ohne die Pausen weiter, die Arbeitsplätze melden ihre Arbeiter ja wieder.
    }
  }

  get size(): number {
    return this.byId.size;
  }

  get(id: string): InPause | undefined {
    return this.byId.get(id);
  }

  all(): IterableIterator<InPause> {
    return this.byId.values();
  }

  ofAccount(account: string): InPause[] {
    return [...this.byId.values()].filter((p) => p.account === account);
  }

  /** Schickt den Arbeiter `info` von `name` in die Pause; zurück kommt, wie er jetzt zu sehen ist. */
  add(account: string, name: string, etage: string, info: WorkerInfo): WorkerInfo {
    const ruht: WorkerInfo = { ...info, status: 'offline', pause: name, action: undefined, viewers: [], viewerIds: [] };
    this.byId.set(info.id, { account, name, etage, info: ruht });
    this.save();
    return ruht;
  }

  delete(id: string): InPause | undefined {
    const p = this.byId.get(id);
    if (p && this.byId.delete(id)) this.save();
    return p;
  }

  private save() {
    if (!this.file) return;
    try {
      writeFileSync(`${this.file}.tmp`, JSON.stringify([...this.byId.values()]), { mode: 0o600 });
      renameSync(`${this.file}.tmp`, this.file);
    } catch (e) {
      console.warn(`  arbeitsplatz: Die Pausen ließen sich nicht speichern: ${(e as Error).message}`);
    }
  }
}
