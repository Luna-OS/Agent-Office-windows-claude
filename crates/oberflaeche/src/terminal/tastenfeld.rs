//! Das Tastenfeld unter dem Terminal, für Handys und Tablets: die Tasten, die eine Bildschirmtastatur
//! nicht hat (1 2 3 für ein Menü, Pfeile, Enter, Tab, Esc, Strg+C), und ein Feld für einen ganzen Auftrag.

use leptos::prelude::*;

use crate::ui::{Eingabe, Knopf, KnopfGroesse};

const TASTEN: &[(&str, &str, &str)] = &[
    ("1", "1", "1"),
    ("2", "2", "2"),
    ("3", "3", "3"),
    ("↑", "\x1b[A", "Pfeil hoch"),
    ("↓", "\x1b[B", "Pfeil runter"),
    ("←", "\x1b[D", "Pfeil links"),
    ("→", "\x1b[C", "Pfeil rechts"),
    ("⏎", "\r", "Enter"),
    ("Tab", "\t", "Tab"),
    ("Esc", "\x1b", "Escape"),
    ("^C", "\x03", "Strg+C"),
];

/// Ob dieses Gerät eher getippt als geklickt wird.
pub fn beruehrung() -> bool {
    window()
        .match_media("(pointer: coarse)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}

#[component]
pub fn Tastenfeld(arbeiter: String) -> impl IntoView {
    let id = StoredValue::new(arbeiter);
    let text = RwSignal::new(String::new());
    view! {
        <div class="mt-2 flex flex-col gap-2">
            <div class="flex flex-wrap gap-1">
                {TASTEN.iter().map(|(name, bytes, titel)| view! {
                    <button type="button" class="min-w-10 rounded-md border-2 border-foreground/30 bg-secondary px-2 py-1.5 font-mono text-sm font-bold active:bg-accent"
                        title=*titel aria-label=*titel on:click=move |_| super::eingeben(&id.get_value(), (*bytes).to_string())>
                        {*name}
                    </button>
                }).collect_view()}
            </div>
            <form class="flex gap-2" on:submit=move |e| {
                e.prevent_default();
                let t = text.get_untracked();
                if !t.is_empty() {
                    super::eingeben(&id.get_value(), format!("{t}\r"));
                    text.set(String::new());
                }
            }>
                <Eingabe wert=text platzhalter="Text ins Terminal (Enter schickt)" />
                <Knopf absenden=true groesse=KnopfGroesse::Klein>"Senden"</Knopf>
            </form>
        </div>
    }
}
