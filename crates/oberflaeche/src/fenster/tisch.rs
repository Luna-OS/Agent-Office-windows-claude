//! Kleine Fenster aus der Welt: 🪧 ein Schild über einen Schreibtisch hängen (L), 🏠 einen Arbeiter
//! heimschicken (X, mit der Frage, was mit seinem Worktree passiert), 🐶 den Hund umbenennen und
//! 🖼️ ein Bild aus dem Netz an die Wand hängen (F).

use std::cell::RefCell;

use leptos::prelude::*;
use protokoll::arbeiter::{Aufraeumen, Client as A};
use protokoll::spielzeug::{Client as S, DekorPlatz};
use protokoll::{ClientNachricht, etagen};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};
use crate::ui::{Eingabe, Knopf, KnopfArt};
use crate::zustand::zustand;

/// Die Schildfarben.
const FARBEN: [&str; 6] = ["#ff8a5b", "#06d6a0", "#5bc0eb", "#ffd166", "#ef476f", "#9b5de5"];

#[component]
pub fn Schild(tisch: String) -> impl IntoView {
    let z = zustand();
    let t = StoredValue::new(tisch.clone());
    let jetzt = z.grundriss.with_untracked(|g| g.schilder.get(&tisch).cloned());
    let text = RwSignal::new(jetzt.as_ref().map(|s| s.text.clone()).unwrap_or_default());
    let farbe = RwSignal::new(jetzt.as_ref().map(|s| s.farbe.clone()).unwrap_or_else(|| FARBEN[0].into()));
    let haengen = move |text: String| {
        senden(ClientNachricht::Etagen(etagen::Client::Schild { tisch: t.get_value(), text, farbe: Some(farbe.get_untracked()) }));
        modal::schliessen(&Fenster::Schild(t.get_value()));
    };
    view! {
        <Rahmen fenster=Fenster::Schild(tisch.clone()) titel="🪧 Schild über dem Tisch" breite="max-w-md">
            <form class="flex flex-col gap-3" on:submit=move |e| { e.prevent_default(); haengen(text.get_untracked().trim().to_string()); }>
                <Eingabe wert=text platzhalter="z. B. Betrieb, Aufräumen, Frontend" />
                <div class="flex gap-2" role="radiogroup" aria-label="Farbe">
                    {FARBEN.iter().map(|f| view! {
                        <button type="button" class="size-8 rounded-full border-2 hover:cursor-pointer" style=format!("background:{f}")
                            class:border-foreground=move || farbe.get() == *f class:border-transparent=move || farbe.get() != *f
                            aria-label=*f on:click=move |_| farbe.set((*f).into())></button>
                    }).collect_view()}
                </div>
                <div class="flex gap-2">
                    <Knopf absenden=true>"Aufhängen"</Knopf>
                    {jetzt.is_some().then(|| view! { <Knopf art=KnopfArt::Umriss on:click=move |_| haengen(String::new())>"Abnehmen"</Knopf> })}
                </div>
            </form>
        </Rahmen>
    }
}

#[component]
pub fn Heimschicken(arbeiter: String) -> impl IntoView {
    let z = zustand();
    let id = StoredValue::new(arbeiter.clone());
    let info = z.arbeiter_mit(&arbeiter);
    senden(ClientNachricht::Arbeiter(A::WorktreeAbfragen { arbeiter: arbeiter.clone() }));
    let stand = move || z.worktree.get().filter(|(a, _)| *a == id.get_value()).map(|(_, s)| s);
    let schicken = move |aufraeumen: Aufraeumen| {
        senden(ClientNachricht::Arbeiter(A::Stoppen { arbeiter: id.get_value(), aufraeumen }));
        modal::schliessen(&Fenster::Heimschicken(id.get_value()));
    };
    let name = info.as_ref().map(|i| i.name.clone()).unwrap_or_default();
    let mit_worktree = info.as_ref().is_some_and(|i| i.worktree.is_some());
    view! {
        <Rahmen fenster=Fenster::Heimschicken(arbeiter.clone()) titel=format!("🏠 {name} heimschicken") breite="max-w-md">
            <div class="flex flex-col gap-3">
                {if mit_worktree {
                    view! {
                        <p class="text-sm">{move || match stand() {
                            None => "Prüft seinen Worktree …".to_string(),
                            Some(s) if !s.vorhanden => "Sein Worktree ist schon weg.".to_string(),
                            Some(s) => format!("Sein Worktree: {} ungespeicherte Dateien, {} Commits, davon {} nirgends sonst.", s.geaendert, s.voraus, s.ungepusht),
                        }}</p>
                        <div class="flex flex-col gap-2">
                            <Knopf on:click=move |_| schicken(Aufraeumen::Behalten)>"Heimschicken, Worktree und Branch behalten"</Knopf>
                            <Knopf art=KnopfArt::Umriss on:click=move |_| schicken(Aufraeumen::Worktree)>"Worktree-Ordner löschen, Branch behalten"</Knopf>
                            <Knopf art=KnopfArt::Gefahr on:click=move |_| {
                                let verlust = stand().is_some_and(|s| s.geaendert > 0 || s.ungepusht > 0);
                                if !verlust || window().confirm_with_message("Dabei gehen Änderungen verloren. Wirklich alles löschen?").unwrap_or(false) {
                                    schicken(Aufraeumen::Alles);
                                }
                            }>"Worktree und Branch löschen"</Knopf>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <p class="text-sm">"Der Schreibtisch wird frei."</p>
                        <Knopf on:click=move |_| schicken(Aufraeumen::Behalten)>"Heimschicken"</Knopf>
                    }.into_any()
                }}
            </div>
        </Rahmen>
    }
}

#[component]
pub fn HundName() -> impl IntoView {
    let name = RwSignal::new(String::new());
    view! {
        <Rahmen fenster=Fenster::HundName titel="🐶 Wie soll der Hund heißen?" breite="max-w-sm">
            <form class="flex gap-2" on:submit=move |e| {
                e.prevent_default();
                senden(ClientNachricht::Spielzeug(S::HundName { name: name.get_untracked().trim().to_string() }));
                modal::schliessen(&Fenster::HundName);
            }>
                <Eingabe wert=name platzhalter="Name (leer: sein erster Name)" />
                <Knopf absenden=true>"Taufen"</Knopf>
            </form>
        </Rahmen>
    }
}

thread_local! {
    static BILD_PLATZ: RefCell<Option<DekorPlatz>> = const { RefCell::new(None) };
}

/// Wo das nächste Bild hängen soll (von der Welt, bevor sie das Fenster öffnet).
pub fn bild_platz_setzen(platz: Option<DekorPlatz>) {
    BILD_PLATZ.with(|p| *p.borrow_mut() = platz);
}

#[component]
pub fn BildAufhaengen() -> impl IntoView {
    let url = RwSignal::new(String::new());
    let titel = RwSignal::new(String::new());
    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let u = url.get_untracked().trim().to_string();
        if !u.starts_with("https://") && !u.starts_with("http://") {
            crate::zustand::fehler("Bitte die Adresse eines Bildes (https://…).");
            return;
        }
        let Some(mut platz) = BILD_PLATZ.with(|p| p.borrow().clone()) else {
            crate::zustand::fehler("Schau auf eine freie Wand und drück F.");
            return;
        };
        platz.url = u;
        let t = titel.get_untracked().trim().to_string();
        platz.titel = (!t.is_empty()).then_some(t);
        senden(ClientNachricht::Spielzeug(S::DekorHinzufuegen { platz }));
        modal::schliessen(&Fenster::BildAufhaengen);
    };
    view! {
        <Rahmen fenster=Fenster::BildAufhaengen titel="🖼️ Ein Bild aufhängen" breite="max-w-md">
            <form class="flex flex-col gap-2" on:submit=absenden>
                <Eingabe wert=url platzhalter="https://… (Adresse eines Bildes)" />
                <Eingabe wert=titel platzhalter="Titel (optional)" />
                <Knopf absenden=true class="self-start">"Aufhängen"</Knopf>
            </form>
        </Rahmen>
    }
}
