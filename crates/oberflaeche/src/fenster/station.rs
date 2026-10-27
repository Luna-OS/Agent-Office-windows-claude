//! 🤖 Der Agent am Pult (Issues-, PR- oder Aufgabenboard): ihm in Worten sagen, was man will. Er kennt
//! sein Board; das Office weckt ihn oder stellt ihn neu ein.

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::arbeiter::Client as A;

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};
use crate::texte;
use crate::ui::{Knopf, KnopfArt, Textfeld};
use crate::zustand::zustand;

/// Name und Station zu einer Pult-ID.
fn pult(tisch: &str) -> (&'static str, &'static str, &'static str) {
    match tisch {
        "station-issues" => ("issues", "🐞 Issue-Agent", "z. B. „Leg ein Issue für den Login-Fehler an“ oder „Welche Issues sind am dringendsten?“"),
        "station-pulls" => ("pulls", "🔀 PR-Agent", "z. B. „Fass PR #12 zusammen“ oder „Welche PRs sind bereit zum Mergen?“"),
        _ => ("aufgaben", "📋 Aufgaben-Agent", "z. B. „Teil das Feature in Aufgaben auf“ oder „Was ist noch offen?“"),
    }
}

#[component]
pub fn Station(tisch: String) -> impl IntoView {
    let z = zustand();
    let (station, titel, beispiel) = pult(&tisch);
    let t = StoredValue::new(tisch.clone());
    let text = RwSignal::new(String::new());
    let agent = Memo::new(move |_| z.arbeiter.with(|l| l.iter().find(|a| a.station.as_deref() == Some(station)).cloned()));
    let feld = NodeRef::<leptos::html::Textarea>::new();
    Effect::new(move |_| {
        if let Some(f) = feld.get() {
            let _ = f.focus();
        }
    });
    let absenden = move || {
        let s = text.get_untracked().trim().to_string();
        if s.is_empty() {
            return;
        }
        senden(ClientNachricht::Arbeiter(A::StationAuftrag { tisch: t.get_value(), text: s }));
        text.set(String::new());
        modal::schliessen(&Fenster::Station(t.get_value()));
    };
    view! {
        <Rahmen fenster=Fenster::Station(tisch.clone()) titel=titel breite="max-w-xl">
            <div class="flex flex-col gap-3" on:keydown=move |e: web_sys::KeyboardEvent| {
                if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) {
                    e.prevent_default();
                    absenden();
                }
            }>
                {move || agent.get().map(|a| {
                    let id = a.id.clone();
                    view! {
                        <p class="flex items-center gap-2 text-sm">
                            <span class=format!("rounded-full px-2 text-[11px] font-extrabold {}", texte::status_klassen(a.status))>{texte::status(a.status)}</span>
                            <span class="text-muted-foreground truncate">{a.aktivitaet.clone().unwrap_or_default()}</span>
                            <Knopf art=KnopfArt::Geist class="ml-auto" on:click=move |_| modal::oeffnen(Fenster::Terminal(id.clone()))>"Terminal"</Knopf>
                        </p>
                    }
                })}
                <Textfeld node_ref=feld wert=text platzhalter=beispiel class="min-h-28" />
                <Knopf class="self-start" on:click=move |_| absenden()>"Auftrag geben (Strg+Enter)"</Knopf>
            </div>
        </Rahmen>
    }
}
