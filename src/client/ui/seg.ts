import { h } from './dom';

/**
 * A row of buttons to pick one of `choices` with (a radio group, as in ⚙️ Settings): `get` says which
 * is picked now, and `pick` is called with another one. `paint` draws it again after a change made
 * elsewhere (the store's, for a setting that's everyone's).
 */
export function radioSeg<T>(label: string, choices: readonly (readonly [T, string])[], get: () => T, pick: (value: T) => void): { el: HTMLElement; paint(): void } {
  const el = h('div.seg', { role: 'radiogroup', 'aria-label': label });
  const paint = () => {
    const now = get();
    el.replaceChildren(
      ...choices.map(([value, text]) =>
        h(
          'button.btn',
          {
            type: 'button',
            role: 'radio',
            'aria-checked': String(now === value),
            class: now === value ? 'on' : '',
            onclick: () => {
              if (get() === value) return;
              pick(value);
              paint();
            },
          },
          text,
        ),
      ),
    );
  };
  paint();
  return { el, paint };
}
