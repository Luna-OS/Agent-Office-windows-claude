// Ein Arbeiter, der sein Limit erreicht hat: Sein Anbieter sagt es im Terminal ("You've hit your limit ·
// resets 3pm (Europe/Berlin)", "5-hour limit reached ∙ resets 3pm", "Claude usage limit reached. Your
// limit will reset at 5pm", Codex: "You've hit your usage limit … try again in 2 hours 13 minutes").
// Oder die Claude-Limits seines Kontos (der Zähler unter den Arbeitern) stehen bei 100 %, auch wenn er
// gerade nichts tut. Dann schläft er an seinem Platz (WorkerInfo.limit), bis es weitergeht oder er wieder arbeitet.
import type { PlanLimits } from '../../shared/protocol.js';
import type { Worker, WorkerEvents } from './types.js';
import { screenText } from './terminal.js';

/**
 * Eine Zeile, die sagt, dass das Limit erreicht ist: nicht nur bald ("Approaching usage limit"), und nicht
 * das Ende des Kontexts ("Context limit reached · /compact").
 */
const LIMIT = /usage limit reached|(?:hour|session|weekly|week|daily|monthly|plan|opus|sonnet|fable|haiku)\s+limit reached|hit your (?:\w+ )?limit|out of (?:extra )?usage/i;
const KEIN_LIMIT = /context|approaching/i;
/** So weit unten auf dem Bildschirm muss die Meldung stehen, damit sie noch gilt. */
const ZEILEN = 14;

const TAGE = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];
const MONATE = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

/** Die Uhr in der Zeitzone `tz` (oder der dieses Computers) zum Zeitpunkt `at`. */
function uhr(tz: string | undefined, at: number) {
  const parts = new Intl.DateTimeFormat('en-US', { timeZone: tz, hourCycle: 'h23', year: 'numeric', month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric', weekday: 'short' }).formatToParts(at);
  const n = (t: string) => Number(parts.find((p) => p.type === t)?.value);
  return { y: n('year'), mo: n('month'), d: n('day'), h: n('hour') % 24, mi: n('minute'), s: n('second'), wd: TAGE.indexOf(String(parts.find((p) => p.type === 'weekday')?.value).toLowerCase()) };
}

/** Der Zeitpunkt, an dem die Uhr in `tz` y-mo-d h:mi zeigt. */
function zeitpunkt(tz: string | undefined, y: number, mo: number, d: number, h: number, mi: number): number {
  const wand = Date.UTC(y, mo - 1, d, h, mi);
  const versatz = (at: number) => {
    const u = uhr(tz, at);
    return Date.UTC(u.y, u.mo - 1, u.d, u.h, u.mi, u.s) - Math.floor(at / 1000) * 1000;
  };
  // Zweimal, damit es auch um eine Zeitumstellung herum stimmt.
  const erst = wand - versatz(wand);
  return wand - versatz(erst);
}

/** Wann es laut `line` weitergeht (ms), oder undefined, wenn sie es nicht sagt. */
export function resetTime(line: string, now: number): number | undefined {
  const epoch = /\|(\d{10})\b/.exec(line);
  if (epoch) return Number(epoch[1]) * 1000;
  const dauer = /\b(?:try again|resets?|reset)\s+in\s+(?:(\d+)\s*(?:hours?|hrs?|h)\b)?[\s,]*(?:and\s+)?(?:(\d+)\s*(?:minutes?|mins?|m)\b)?/i.exec(line);
  if (dauer && (dauer[1] || dauer[2])) return now + (Number(dauer[1] ?? 0) * 60 + Number(dauer[2] ?? 0)) * 60_000;
  const m = /\b(?:resets?|reset|try again)(?:\s+at)?\s+(?:(mon|tue|wed|thu|fri|sat|sun)[a-z]*\.?\s+|(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s+(\d{1,2})(?:st|nd|rd|th)?,?\s+(?:at\s+)?)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?(?:\s*\(([^)]+)\))?/i.exec(line);
  if (!m || (!m[6] && !m[5])) return undefined;
  let h = Number(m[4]);
  const mi = Number(m[5] ?? 0);
  if (h > 23 || mi > 59) return undefined;
  if (m[6]) h = (h % 12) + (m[6].toLowerCase() === 'pm' ? 12 : 0);
  let tz: string | undefined = m[7]?.trim();
  try {
    if (tz) uhr(tz, now);
  } catch {
    tz = undefined;
  }
  const heute = uhr(tz, now);
  if (m[2]) {
    // An einem Tag des Jahres: dieses Jahr, oder nächstes, wenn er schon vorbei ist.
    const mo = MONATE.indexOf(m[2].toLowerCase()) + 1;
    const t = zeitpunkt(tz, heute.y, mo, Number(m[3]), h, mi);
    return t > now - 86_400_000 ? t : zeitpunkt(tz, heute.y + 1, mo, Number(m[3]), h, mi);
  }
  // An einem Wochentag, oder heute (morgen, wenn die Uhrzeit heute schon vorbei ist).
  const tage = m[1] ? (TAGE.indexOf(m[1].toLowerCase()) - heute.wd + 7) % 7 : 0;
  const tag = new Date(Date.UTC(heute.y, heute.mo - 1, heute.d + tage));
  let t = zeitpunkt(tz, tag.getUTCFullYear(), tag.getUTCMonth() + 1, tag.getUTCDate(), h, mi);
  if (t <= now) t += (m[1] ? 7 : 1) * 86_400_000;
  return t;
}

/** Ob der Text unten auf dem Bildschirm sagt, dass das Limit erreicht ist: die Zeile, und wann es weitergeht. */
export function limitIn(text: string, now: number): { line: string; until?: number } | undefined {
  const lines = text.split('\n').filter((l) => l.trim()).slice(-ZEILEN);
  for (let i = lines.length - 1; i >= 0; i--) {
    if (!LIMIT.test(lines[i]) || KEIN_LIMIT.test(lines[i])) continue;
    // Die Zeit steht manchmal in der Zeile darunter ("Your limit will reset at 5pm").
    const line = lines[i].trim();
    return { line, until: resetTime(line, now) ?? resetTime(lines[i + 1] ?? '', now) };
  }
  return undefined;
}

/** Die Fenster, die für das ganze Konto gelten (nicht nur für ein Modell, wie "Opus week"). */
const KONTO = new Set(['5h session', 'Week']);
/** Ein Stand ohne Zeitpunkt für das Weitergehen gilt so lange. */
const FRISCH_MS = 15 * 60_000;

/** Ob die Claude-Limits `plan` aufgebraucht sind, und bis wann (das späteste Zurücksetzen der vollen Fenster). */
export function planVoll(plan: PlanLimits | undefined, now: number): { until?: number } | undefined {
  const voll = (plan?.windows ?? []).filter((w) => KONTO.has(w.label) && w.pct >= 100 && (w.resetsAt !== undefined ? w.resetsAt > now : now - plan!.at < FRISCH_MS));
  if (!voll.length) return undefined;
  const bis = voll.map((w) => w.resetsAt).filter((t): t is number => t !== undefined);
  return bis.length ? { until: Math.max(...bis) } : {};
}

/** Wovon jeder Arbeiter zuletzt aufgewacht ist: dieselbe Meldung schickt ihn nicht wieder schlafen. */
const geweckt = new WeakMap<Worker, string>();

/**
 * Beim Lesen seines Bildschirms (terminal.ts flushScreens): Hat ein Arbeiter sein Limit erreicht (die
 * Meldung auf dem Bildschirm, oder bei Claude die vollen Limits seines Kontos), schläft er
 * (WorkerInfo.limit); arbeitet er wieder, ist die Zeit um, oder ist das Limit nicht mehr voll, wacht er auf.
 */
export function checkLimit(w: Worker, events: WorkerEvents, now = Date.now()) {
  if (w.info.kind !== 'agent' || !w.term) return;
  const melden = () => events.update({ ...w.info });
  const s = w.info.status;
  if (s !== 'idle' && s !== 'done' && s !== 'needs_input') {
    if (w.info.limit) (w.info.limit = undefined), melden();
    return;
  }
  // Nur was diesmal auf dem Bildschirm steht: eine Meldung von vor einem Neustart ist alt.
  const found = limitIn(screenText(w.term, w.term.buffer.active.type === 'normal' ? Math.max(0, w.fresh?.line ?? 0) : 0), now);
  const plan = w.info.provider === 'claude' ? planVoll(events.plan?.(w.owner), now) : undefined;
  if (w.info.limit) {
    const aus = w.info.limit.until !== undefined && now >= w.info.limit.until;
    if (!aus && (found || plan)) return;
    if (found) geweckt.set(w, found.line);
    w.info.limit = undefined;
    return melden();
  }
  const neu = found && geweckt.get(w) !== found.line && !(found.until !== undefined && found.until <= now) ? found : undefined;
  if (!neu && !plan) return;
  const until = neu?.until ?? plan?.until;
  w.info.limit = until !== undefined ? { until } : {};
  melden();
}
