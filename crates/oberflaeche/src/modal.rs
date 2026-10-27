//! Der Stapel offener Fenster (Modale) und ihr Rahmen.
//!
//! Regel: Jedes Fenster hat oben rechts ein ✕. ✕ oder Esc schließt das oberste, und ist danach keins
//! mehr offen, fängt die Welt sofort wieder die Maus (über [`crate::bruecke::fenster`]).

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use crate::zustand::zustand;

/// Alle Fenster, die die Oberfläche kennt. Ein neues Fenster ist eine neue Variante hier und ein Arm in
/// [`crate::fenster::ansicht`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Fenster {
    Menue,
    Hilfe,
    Einstellungen,
    Palette,
    Aufzug,
    Terminal(String),
    Aufgaben,
    Agenten,
    Github,
    Aenderungen(String),
    Team,
    Konten,
    Nutzung,
    Meeting,
    DachUrl,
    Teilen,
    Bildschirm(String),
    Jukebox,
    Schild(String),
    Heimschicken(String),
    HundName,
    BildAufhaengen,
    Prompts,
    Dienste,
    Bar,
    Buecher,
    Whiteboard,
    Arcade,
    Station(String),
    Kerker,
    Minen,
    Anlegen(String),
    Repos,
}

/// Öffnet ein Fenster (oder holt es nach oben, wenn es schon offen ist).
pub fn oeffnen(f: Fenster) {
    let z = zustand();
    let war_leer = z.fenster.with_untracked(|s| s.is_empty());
    z.fenster.update(|s| {
        s.retain(|x| x != &f);
        s.push(f);
    });
    if war_leer {
        crate::bruecke::fenster(true);
    }
}

/// Öffnet es, oder schließt es, wenn es schon oben liegt (für Tasten wie Tab).
pub fn umschalten(f: Fenster) {
    if zustand().fenster.with_untracked(|s| s.last() == Some(&f)) {
        schliessen(&f);
    } else {
        oeffnen(f);
    }
}

/// Schließt ein bestimmtes Fenster.
pub fn schliessen(f: &Fenster) {
    let z = zustand();
    if !z.fenster.with_untracked(|s| s.contains(f)) {
        return;
    }
    z.fenster.update(|s| s.retain(|x| x != f));
    if z.fenster.with_untracked(|s| s.is_empty()) {
        crate::bruecke::fenster(false);
    }
}

/// Schließt das oberste Fenster; `false`, wenn keins offen war.
pub fn schliessen_oben() -> bool {
    let oben = zustand().fenster.with_untracked(|s| s.last().cloned());
    match oben {
        Some(f) => {
            schliessen(&f);
            true
        }
        None => false,
    }
}

/// Schließt alle Fenster.
pub fn alle_schliessen() {
    let z = zustand();
    if z.fenster.with_untracked(|s| s.is_empty()) {
        return;
    }
    z.fenster.set(Vec::new());
    crate::bruecke::fenster(false);
}

pub fn offen() -> bool {
    !zustand().fenster.with_untracked(|s| s.is_empty())
}

/// Hört im Capture-Schritt auf Esc, damit das Esc, das ein Terminal schließt, nicht auch hineingetippt wird.
pub(crate) fn esc_abfangen() {
    let cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
        if e.key() == "Escape" && offen() {
            e.prevent_default();
            e.stop_propagation();
            schliessen_oben();
        }
    });
    let _ = window().add_event_listener_with_callback_and_bool(
        "keydown",
        cb.as_ref().unchecked_ref(),
        true,
    );
    cb.forget();
}

/// Der Rahmen jedes Fensters: Kopf mit Titel und ✕, darunter der Inhalt. Ein Klick neben das Fenster schließt es.
#[component]
pub fn Rahmen(
    fenster: Fenster,
    #[prop(into)] titel: Signal<String>,
    /// Zusätzliche Tailwind-Klassen für die Breite (`max-w-2xl` …).
    #[prop(into, optional)]
    breite: String,
    /// Etwas im Kopf links neben dem ✕ (Knöpfe, Reiter).
    #[prop(optional)]
    kopf: Option<AnyView>,
    children: Children,
) -> impl IntoView {
    let f_zu = fenster.clone();
    let f_hinter = fenster.clone();
    let breite = if breite.is_empty() {
        "max-w-xl".to_string()
    } else {
        breite
    };
    view! {
        <div
            class="fenster-hinter fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
            on:mousedown=move |e| {
                if e.target() == e.current_target() {
                    schliessen(&f_hinter);
                }
            }
        >
            <div
                role="dialog"
                aria-label=move || titel.get()
                class=format!("fenster relative flex max-h-[88vh] w-full flex-col overflow-hidden rounded-2xl border-[3px] border-foreground bg-background text-foreground shadow-[0_5px_0_var(--foreground)] {breite}")
            >
                <header class="flex items-center gap-2 border-b-2 border-foreground/15 px-4 py-3">
                    <h2 class="mr-auto truncate text-lg font-extrabold">{move || titel.get()}</h2>
                    {kopf}
                    <button
                        type="button"
                        class="schliessen inline-flex size-8 items-center justify-center rounded-md text-lg font-bold hover:cursor-pointer hover:bg-accent"
                        aria-label="Schließen"
                        title="Schließen (Esc)"
                        on:click=move |_| schliessen(&f_zu)
                    >
                        "✕"
                    </button>
                </header>
                <div class="min-h-0 flex-1 overflow-auto p-4">{children()}</div>
            </div>
        </div>
    }
}
