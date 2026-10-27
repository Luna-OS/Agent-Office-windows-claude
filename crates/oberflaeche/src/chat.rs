//! Der Chat unten links: die letzten Zeilen und, nach T, das Eingabefeld. Enter schickt, Esc bricht ab;
//! beides gibt die Maus sofort an die Welt zurück.

use leptos::prelude::*;
use protokoll::{ClientNachricht, praesenz};

use crate::texte;
use crate::zustand::zustand;

fn zu() {
    let z = zustand();
    if z.chat_offen.get_untracked() {
        z.chat_offen.set(false);
        if !crate::modal::offen() {
            crate::bruecke::fenster(false);
        }
    }
}

#[component]
pub fn Chat() -> impl IntoView {
    let z = zustand();
    let text = RwSignal::new(String::new());
    let feld = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if z.chat_offen.get()
            && let Some(f) = feld.get()
        {
            let _ = f.focus();
        }
    });

    // Nach Enter oder Esc gibt das Feld den Fokus ab, sonst hielten die Tastenkürzel es für Tippen.
    let weg = move || {
        if let Some(f) = feld.get_untracked() {
            let _ = f.blur();
        }
        zu();
    };
    let taste = move |e: web_sys::KeyboardEvent| match e.key().as_str() {
        "Enter" => {
            e.prevent_default();
            let t = text.get_untracked();
            if !t.trim().is_empty() {
                crate::bruecke::senden(ClientNachricht::Praesenz(praesenz::Client::Chat {
                    text: t.trim().to_string(),
                }));
            }
            text.set(String::new());
            weg();
        }
        "Escape" => {
            e.prevent_default();
            e.stop_propagation();
            text.set(String::new());
            weg();
        }
        _ => {}
    };

    view! {
        <section class="pointer-events-auto absolute bottom-3 left-3 flex w-[min(28rem,60vw)] flex-col gap-1" aria-label="Chat">
            <ul class="flex max-h-48 flex-col gap-0.5 overflow-hidden text-sm" class:opacity-80=move || !z.chat_offen.get()>
                {move || {
                    let zeilen = z.chat.get();
                    let ab = zeilen.len().saturating_sub(8);
                    zeilen[ab..].iter().map(|c| view! {
                        <li class="w-fit max-w-full rounded-md bg-background/85 px-2 py-0.5 shadow-sm">
                            <span class="text-muted-foreground text-xs">{texte::uhrzeit(c.um)}" "</span>
                            <b style=format!("color:{}", c.farbe)>{c.name.clone()}</b>
                            ": "
                            <span class="break-words">{c.text.clone()}</span>
                        </li>
                    }).collect_view()
                }}
            </ul>
            <input
                node_ref=feld
                class="hud-panel h-9 px-3 text-sm outline-none"
                class:opacity-60=move || !z.chat_offen.get()
                maxlength="500"
                placeholder="T drücken zum Chatten"
                autocomplete="off"
                prop:value=move || text.get()
                on:input=move |e| text.set(event_target_value(&e))
                on:focus=move |_| {
                    if !z.chat_offen.get_untracked() {
                        z.chat_offen.set(true);
                        crate::bruecke::fenster(true);
                    }
                }
                on:blur=move |_| zu()
                on:keydown=taste
            />
        </section>
    }
}
