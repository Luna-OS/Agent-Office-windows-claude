// Arbeiter reden miteinander (tell_worker, `office-workers tell`): Der Angesprochene bekommt die
// Nachricht mit Absender, damit er antworten kann, und alle auf der Etage sehen das Gespräch: eine
// Sprechblase über dem, der spricht, ein Papierflieger zu dem, den er anspricht (worker.says), und eine
// Zeile im Chat. Auch über die PCs der Spieler hinweg (siehe arbeitsplatz/reden.ts).
import type { ChatLine, WorkerInfo } from '../shared/protocol.js';
import type { Floor } from './floor.js';
import type { Ctx } from './office/context.js';

/** So viel einer Nachricht steht in der Sprechblase und im Chat. */
const BLASE = 200;
const CHAT = 280;

const kurz = (text: string, n: number) => {
  const t = text.replace(/\s+/g, ' ').trim();
  return t.length > n ? `${t.slice(0, n - 1)}…` : t;
};

/** Was der Angesprochene als Prompt bekommt: wer es sagt (und von wessen PC), und wie er antwortet. */
export function mitAbsender(von: { name: string }, text: string, pc?: string): string {
  const wer = `${von.name} (ein anderer Arbeiter im Office${pc ? `, auf dem PC von ${pc}` : ''})`;
  return `💬 Nachricht von ${wer}:\n\n${text}\n\n(Antworten kannst du mit tell_worker an „${von.name}“, oder mit: office-workers tell "${von.name}" --prompt "…")`;
}

/** Zeigt allen auf `floor`, dass `von` zu `an` etwas gesagt hat, und schreibt es in den Chat. */
export function zeigen(ctx: Ctx, floor: Floor, von: Pick<WorkerInfo, 'id' | 'name' | 'color'>, an: Pick<WorkerInfo, 'id' | 'name'>, text: string) {
  ctx.toFloor(floor, { t: 'worker.says', from: von.id, to: an.id, text: kurz(text, BLASE) });
  const line: ChatLine = { from: `arbeiter:${von.id}`, name: `🤖 ${von.name} → ${an.name}`, color: von.color, text: kurz(text, CHAT), at: Date.now() };
  ctx.chat.add(line);
  ctx.broadcast({ t: 'chat', ...line });
}
