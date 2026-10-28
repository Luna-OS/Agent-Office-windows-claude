//! 🖼️ Ein Bild an der Wand (E daran): Adresse, Titel, Größe und Rahmen ändern oder es abnehmen.
//! Die Bilder der Etage kommen mit dem Spielzeug-Stand; neue hängt [`super::tisch::BildAufhaengen`] auf.

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::spielzeug::{Client as S, Dekor, DekorAenderung, Server};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf, KnopfArt};

/// Die Rahmenfarben der Welt, in ihrer Reihenfolge (Index = `rahmen`).
const RAHMEN: [(&str, &str); 6] = [
    ("Holz", "#8b5a2b"),
    ("Schwarz", "#1e1e1e"),
    ("Weiß", "#f5f5f5"),
    ("Gold", "#d4a73a"),
    ("Koralle", "#ff7f6a"),
    ("Petrol", "#1f6f78"),
];

thread_local! {
    static BILDER: OnceCell<RwSignal<Vec<Dekor>>> = const { OnceCell::new() };
}

fn bilder() -> RwSignal<Vec<Dekor>> {
    BILDER.with(|b| *b.get_or_init(|| crate::zustand::global(|| RwSignal::new(Vec::new()))))
}

/// Spielzeug-Frames mit den Bildern der Etage.
pub fn empfangen(m: &Server) {
    match m {
        Server::Stand { stand } => bilder().set(stand.dekor.clone()),
        Server::Dekor { liste } => bilder().set(liste.clone()),
        _ => {}
    }
}

fn zahl(s: &str) -> Option<f32> {
    s.trim()
        .replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|v| *v > 0.1 && *v < 10.0)
}

#[component]
pub fn Bild(id: String) -> impl IntoView {
    let i = StoredValue::new(id.clone());
    let jetzt = bilder().with_untracked(|l| l.iter().find(|d| d.id == id).cloned());
    let p = jetzt.as_ref().map(|d| d.platz.clone());
    let url = RwSignal::new(p.as_ref().map(|p| p.url.clone()).unwrap_or_default());
    let titel = RwSignal::new(p.as_ref().and_then(|p| p.titel.clone()).unwrap_or_default());
    let b = RwSignal::new(
        p.as_ref()
            .map(|p| format!("{:.2}", p.b))
            .unwrap_or_default(),
    );
    let h = RwSignal::new(
        p.as_ref()
            .map(|p| format!("{:.2}", p.h))
            .unwrap_or_default(),
    );
    let rahmen = RwSignal::new(p.as_ref().map(|p| p.rahmen).unwrap_or(0));
    let zu = move || modal::schliessen(&Fenster::Bild(i.get_value()));

    let speichern = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let u = url.get_untracked().trim().to_string();
        if !u.starts_with("https://") && !u.starts_with("http://") {
            crate::zustand::fehler("Bitte die Adresse eines Bildes (https://…).");
            return;
        }
        let t = titel.get_untracked().trim().to_string();
        let aenderung = DekorAenderung {
            url: Some(u),
            titel: Some(t),
            wand: None,
            u: None,
            y: None,
            b: zahl(&b.get_untracked()),
            h: zahl(&h.get_untracked()),
            rahmen: Some(rahmen.get_untracked()),
        };
        senden(ClientNachricht::Spielzeug(S::DekorAendern {
            id: i.get_value(),
            aenderung,
        }));
        zu();
    };
    let abnehmen = move |_| {
        senden(ClientNachricht::Spielzeug(S::DekorEntfernen {
            id: i.get_value(),
        }));
        zu();
    };

    view! {
        <Rahmen fenster=Fenster::Bild(id.clone()) titel="🖼️ Bild an der Wand" breite="max-w-md">
            {match jetzt {
                None => view! { <p class="text-muted-foreground text-sm">"Dieses Bild hängt nicht mehr."</p> }.into_any(),
                Some(d) => view! {
                    <form class="flex flex-col gap-3" on:submit=speichern>
                        <p class="text-muted-foreground text-xs">{format!("Aufgehängt von {} · {}", d.von, texte::her(d.um))}</p>
                        <label class="flex flex-col gap-1 text-sm font-semibold">"Adresse"<Eingabe wert=url platzhalter="https://…" /></label>
                        <label class="flex flex-col gap-1 text-sm font-semibold">"Titel"<Eingabe wert=titel platzhalter="(ohne)" /></label>
                        <div class="grid grid-cols-2 gap-2">
                            <label class="flex flex-col gap-1 text-sm font-semibold">"Breite (m)"<Eingabe wert=b /></label>
                            <label class="flex flex-col gap-1 text-sm font-semibold">"Höhe (m)"<Eingabe wert=h /></label>
                        </div>
                        <div class="flex flex-wrap items-center gap-2 text-sm" role="radiogroup" aria-label="Rahmen">
                            <span class="font-semibold">"Rahmen:"</span>
                            {RAHMEN.iter().enumerate().map(|(n, (name, farbe))| view! {
                                <button type="button" class="size-8 rounded-md border-2 hover:cursor-pointer" title=*name aria-label=*name
                                    style=format!("background:{farbe}")
                                    class:border-foreground=move || rahmen.get() == n as u32 class:border-transparent=move || rahmen.get() != n as u32
                                    on:click=move |_| rahmen.set(n as u32)></button>
                            }).collect_view()}
                        </div>
                        <div class="flex gap-2">
                            <Knopf absenden=true>"Speichern"</Knopf>
                            <Knopf art=KnopfArt::Gefahr on:click=abnehmen>"Abnehmen"</Knopf>
                        </div>
                    </form>
                }.into_any(),
            }}
        </Rahmen>
    }
}
