// Der DJ auf dem Dach: ein YouTube-Song, den jemand am DJ-Pult aufgelegt hat (features/dj).

/** Was der DJ gerade für alle auf dem Dach spielt, wenn jemand einen Song aufgelegt hat. */
export interface DjState {
  /** Die YouTube-Video-ID (11 Zeichen, siehe shared/youtube.ts). */
  video: string;
  /** Wo im Video es losging, in Sekunden (aus t= oder start= im Link). */
  start: number;
  /** Wann es losging, auf der Uhr des Office (ms, siehe die 'pong'-Nachricht), damit alle dieselbe Stelle hören. */
  startedAt: number;
  /** Wer ihn aufgelegt hat. */
  by: string;
}

export type DjClientMsg =
  /** Einen YouTube-Song auflegen (eine Adresse oder eine Video-ID); nur auf dem Dach. */
  | { t: 'dj.play'; url: string }
  /** Den Song wieder runternehmen: Der DJ spielt sein eigenes Set. */
  | { t: 'dj.stop' };

export type DjServerMsg =
  /** Was der DJ jetzt spielt (null: sein eigenes Set), an alle; `by` hat es geändert. */
  { t: 'dj'; state: DjState | null; by: string };
