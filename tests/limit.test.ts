// Ein Arbeiter, der sein Limit erreicht hat (src/server/workers/limit.ts): was im Terminal steht, wann
// es weitergeht, und dass er schläft, bis es so weit ist oder er wieder arbeitet.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import headless from '@xterm/headless';
import { checkLimit, limitIn, planVoll, resetTime } from '../src/server/workers/limit.js';

// Donnerstag, 29. Oktober 2026, 08:00 UTC (in Berlin 09:00, Winterzeit; in New York 04:00, noch Sommerzeit).
const NOW = Date.UTC(2026, 9, 29, 8, 0);

test('wann es weitergeht, so wie die Anbieter es sagen', () => {
  assert.equal(resetTime("You've hit your limit · resets 3pm (Europe/Berlin)", NOW), Date.UTC(2026, 9, 29, 14, 0));
  assert.equal(resetTime('Your limit will reset at 5pm (America/New_York).', NOW), Date.UTC(2026, 9, 29, 21, 0));
  assert.equal(resetTime('Claude AI usage limit reached|1793289600', NOW), 1793289600 * 1000);
  assert.equal(resetTime("You've hit your usage limit. Upgrade to Pro or try again in 2 hours 13 minutes.", NOW), NOW + 133 * 60_000);
  assert.equal(resetTime('Weekly limit reached ∙ resets Mon 9am (Europe/Berlin)', NOW), Date.UTC(2026, 10, 2, 8, 0));
  assert.equal(resetTime('Opus weekly limit reached ∙ resets Nov 3, 9:30am (UTC)', NOW), Date.UTC(2026, 10, 3, 9, 30));
  // Schon vorbei heute: morgen.
  assert.equal(resetTime('resets 8am (Europe/Berlin)', NOW), Date.UTC(2026, 9, 30, 7, 0));
  // Ohne Zeitzone: die dieses Computers, heute oder morgen um 15 Uhr.
  const lokal = resetTime('5-hour limit reached ∙ resets 3pm', NOW)!;
  assert.equal(new Date(lokal).getHours(), 15);
  assert.ok(lokal > NOW && lokal - NOW <= 86_400_000);
  // Eine Zeitzone, die es nicht gibt: auch die dieses Computers.
  assert.equal(new Date(resetTime('resets 3pm (Mars/Olympus)', NOW)!).getHours(), 15);
  assert.equal(resetTime('limit reached, sorry', NOW), undefined);
});

test('was als erreichtes Limit zählt, und was nicht', () => {
  assert.deepEqual(limitIn("> fix the login\n\n  ⎿  You've hit your limit · resets 3pm (Europe/Berlin)\n\n> ", NOW), { line: "⎿  You've hit your limit · resets 3pm (Europe/Berlin)", until: Date.UTC(2026, 9, 29, 14, 0) });
  // Die Zeit in der Zeile darunter.
  assert.equal(limitIn('Claude usage limit reached.\nYour limit will reset at 5pm (UTC).', NOW)?.until, Date.UTC(2026, 9, 29, 17, 0));
  assert.ok(limitIn('5-hour limit reached ∙ resets 3pm', NOW));
  assert.ok(limitIn('Opus weekly limit reached ∙ resets Mon 9am', NOW));
  assert.equal(limitIn('Context limit reached · /compact or /clear to continue', NOW), undefined);
  assert.equal(limitIn('Approaching usage limit · resets at 3pm', NOW), undefined);
  // Weiter oben, mit viel Neuem darunter: gilt nicht mehr.
  const alt = ["You've hit your limit · resets 3pm", ...Array.from({ length: 20 }, (_, i) => `Zeile ${i}`)].join('\n');
  assert.equal(limitIn(alt, NOW), undefined);
});

/** Ein Arbeiter mit einem Terminal, auf dem `text` steht. */
async function arbeiter(text: string) {
  const term = new headless.Terminal({ cols: 100, rows: 24, allowProposedApi: true });
  await new Promise<void>((r) => term.write(text.replace(/\n/g, '\r\n'), r));
  const info: any = { id: 'w1', kind: 'agent', status: 'done' };
  return { info, term } as any;
}

test('er schläft, bis es weitergeht oder er wieder arbeitet', async () => {
  const updates: any[] = [];
  const events: any = { update: (i: any) => updates.push(i) };
  const w = await arbeiter("> mach weiter\n  ⎿  You've hit your limit · resets 3pm (Europe/Berlin)\n");
  checkLimit(w, events, NOW);
  assert.deepEqual(w.info.limit, { until: Date.UTC(2026, 9, 29, 14, 0) });
  assert.equal(updates.length, 1);
  // Noch einmal gelesen: nichts Neues.
  checkLimit(w, events, NOW + 60_000);
  assert.equal(updates.length, 1);
  // Die Zeit ist um: Er wacht auf, und dieselbe Meldung schickt ihn nicht wieder schlafen.
  checkLimit(w, events, Date.UTC(2026, 9, 29, 14, 1));
  assert.equal(w.info.limit, undefined);
  checkLimit(w, events, Date.UTC(2026, 9, 29, 14, 2));
  assert.equal(w.info.limit, undefined);
  assert.equal(updates.length, 2);

  // Ohne Uhrzeit schläft er, bis er wieder arbeitet.
  const v = await arbeiter('  ⎿  Claude usage limit reached.\n');
  checkLimit(v, events, NOW);
  assert.deepEqual(v.info.limit, {});
  v.info.status = 'working';
  checkLimit(v, events, NOW + 1000);
  assert.equal(v.info.limit, undefined);
  // Eine Shell schläft nie.
  const s = await arbeiter("You've hit your limit · resets 3pm\n");
  s.info.kind = 'shell';
  checkLimit(s, events, NOW);
  assert.equal(s.info.limit, undefined);
});

test('die Claude-Limits des Kontos voll: wann das zählt', () => {
  const in2h = NOW + 2 * 3600_000;
  assert.deepEqual(planVoll({ windows: [{ label: '5h session', pct: 100, resetsAt: in2h }, { label: 'Week', pct: 40 }], at: NOW }, NOW), { until: in2h });
  // Beide voll: bis das spätere zurückgesetzt wird.
  assert.deepEqual(planVoll({ windows: [{ label: '5h session', pct: 100, resetsAt: in2h }, { label: 'Week', pct: 100, resetsAt: in2h + 86_400_000 }], at: NOW }, NOW), { until: in2h + 86_400_000 });
  // Schon zurückgesetzt (der Stand ist von vorher), nur ein Modell voll, oder noch nicht ganz voll: zählt nicht.
  assert.equal(planVoll({ windows: [{ label: '5h session', pct: 100, resetsAt: NOW - 1 }], at: NOW - 3600_000 }, NOW), undefined);
  assert.equal(planVoll({ windows: [{ label: 'Opus week', pct: 100, resetsAt: in2h }], at: NOW }, NOW), undefined);
  assert.equal(planVoll({ windows: [{ label: '5h session', pct: 99.5, resetsAt: in2h }], at: NOW }, NOW), undefined);
  // Ohne Zeitpunkt: solange der Stand frisch ist.
  assert.deepEqual(planVoll({ windows: [{ label: 'Week', pct: 100 }], at: NOW - 60_000 }, NOW), {});
  assert.equal(planVoll({ windows: [{ label: 'Week', pct: 100 }], at: NOW - 3600_000 }, NOW), undefined);
  assert.equal(planVoll(undefined, NOW), undefined);
});

test('sind die Limits des Kontos voll, schlafen auch die Claude-Arbeiter, die gerade nichts tun', async () => {
  const in2h = NOW + 2 * 3600_000;
  let voll = true;
  const gefragt: (string | undefined)[] = [];
  const events: any = {
    update: () => {},
    plan: (owner: string | undefined) => (gefragt.push(owner), { windows: [{ label: '5h session', pct: voll ? 100 : 12, resetsAt: in2h }], at: NOW }),
  };
  const w = await arbeiter('> \n');
  w.info.provider = 'claude';
  w.info.status = 'idle';
  w.owner = 'konto-kim';
  checkLimit(w, events, NOW);
  assert.deepEqual(w.info.limit, { until: in2h });
  assert.equal(gefragt[0], 'konto-kim', 'die Limits ihres Kontos');
  // Arbeitet er doch (etwa mit Zusatz-Nutzung), wacht er auf; ist er wieder fertig, schläft er weiter.
  w.info.status = 'working';
  checkLimit(w, events, NOW + 1000);
  assert.equal(w.info.limit, undefined);
  w.info.status = 'done';
  checkLimit(w, events, NOW + 2000);
  assert.deepEqual(w.info.limit, { until: in2h });
  // Nicht mehr voll: Er wacht auf.
  voll = false;
  checkLimit(w, events, NOW + 3000);
  assert.equal(w.info.limit, undefined);
  // Ein Arbeiter mit Codex schläft nicht wegen Claudes Limits.
  voll = true;
  const c = await arbeiter('> \n');
  c.info.provider = 'codex';
  c.info.status = 'idle';
  checkLimit(c, events, NOW);
  assert.equal(c.info.limit, undefined);
});
