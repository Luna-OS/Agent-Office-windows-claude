// Zwischen dem Office des Gastgebers und dem Arbeitsplatz eines Spielers: seinem eigenen Office, das
// im Hintergrund auf seinem Computer läuft (in der App, oder mit --arbeitsplatz). Wer dort angemeldet ist,
// dessen Arbeiter laufen auf seinem Computer, mit seinem Claude und GitHub; der Gastgeber zeigt sie auf
// seinen Etagen und reicht nur weiter, was Browser ihnen schicken und was sie zurückgeben.
//
// Über einen WebSocket zu <Lobby>/arbeitsplatz, angemeldet mit dem Konto des Spielers (seine Sitzung).
import type { ClientMsg, ServerMsg } from '../../shared/protocol.js';

/** Eine Etage des Gastgebers, wie ein Arbeitsplatz sie findet: über ihr GitHub-Repo. */
export interface FernEtage {
  id: string;
  name: string;
  /** owner/name auf GitHub. */
  repo: string;
  /** Wie weit das Hinterzimmer ausgebaut ist (FloorPlan.wing): Dort gibt es dann auch die Schreibtische. */
  wing: number;
}

/** Ein Browser beim Gastgeber, für den der Arbeitsplatz etwas tut. */
export interface FernBrowser {
  id: string;
  name: string;
}

export type ZumArbeitsplatz =
  /** Die Etagen des Gastgebers, die ein GitHub-Repo haben: beim Verbinden, und wenn sich etwas ändert. */
  | { t: 'etagen'; etagen: FernEtage[] }
  /** Eine Nachricht eines Browsers für einen Arbeiter dieses Arbeitsplatzes, oder um einen einzustellen. */
  | { t: 'nachricht'; etage: FernEtage; wer: FernBrowser; msg: ClientMsg }
  /** Dieser Browser ist weg (oder auf einer anderen Etage): seine Terminals und Ansichten los. */
  | { t: 'weg'; id: string };

export type VomArbeitsplatz =
  /** Für alle auf der Etage `etage` des Gastgebers: worker.update, worker.remove, screen oder toast. */
  | { t: 'etage'; etage: string; msg: ServerMsg }
  /** Nur für den Browser `an` beim Gastgeber (sein Terminal, eine Antwort, eine Warnung). */
  | { t: 'an'; an: string; msg: ServerMsg }
  /**
   * Nach dem Verbinden, wenn alle Arbeiter gemeldet sind: wer hier auf den Etagen des Gastgebers sitzt.
   * Wer von seinen Arbeitern beim Gastgeber noch in der Pause sitzt und hier nicht mehr ist, geht.
   */
  | { t: 'bestand'; arbeiter: string[] };

/** Was ein Arbeitsplatz der ganzen Etage sagen darf. */
export const FUER_DIE_ETAGE: ReadonlySet<string> = new Set(['worker.update', 'worker.remove', 'screen', 'toast']);
/** Was ein Arbeitsplatz einem Browser sagen darf. */
export const FUER_EINEN: ReadonlySet<string> = new Set(['term.snapshot', 'term.data', 'term.typing', 'worker.worktree', 'toast', 'changes', 'changes.diff']);
