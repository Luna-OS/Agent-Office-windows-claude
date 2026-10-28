import { h } from '../../ui/dom';
import { GRAPHICS, GRAPHICS_ORDER, loadGraphics, setGraphics, type GraphicsQuality } from './quality';

/**
 * ⚙️ Settings → You → Graphics: Eco, Balanced or High, and what the one picked does. Takes effect
 * at once (a laptop's power-saving graphics only from the next reload).
 */
export function graphicsSetting(): Node[] {
  const row = h('div.seg', { role: 'radiogroup', 'aria-label': 'Graphics' });
  const note = h('p.setting-note');
  const paint = () => {
    const now = loadGraphics();
    row.replaceChildren(
      ...GRAPHICS_ORDER.map((q: GraphicsQuality) =>
        h(
          'button.btn',
          {
            type: 'button',
            role: 'radio',
            'aria-checked': String(now === q),
            class: now === q ? 'on' : '',
            onclick: () => {
              if (loadGraphics() === q) return;
              const reload = GRAPHICS[q].lowPower !== GRAPHICS[loadGraphics()].lowPower;
              setGraphics(q);
              paint();
              if (reload) note.textContent += ' The power-saving graphics switch at the next reload.';
            },
          },
          GRAPHICS[q].label,
        ),
      ),
    );
    note.textContent = GRAPHICS[now].blurb;
  };
  paint();
  return [row, note];
}
