//! 📋 Das Aufgabenboard der Etage: anlegen, übernehmen, Status ändern, sortieren. Aufgaben mit 🤖
//! vergibt das Office selbst an neue Arbeiter, höchstens `max_arbeiter` gleichzeitig.

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::aufgaben::{Aufgabe, AufgabenStatus, Client as A};

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse, Textfeld};
use crate::zustand::zustand;

fn schicken(a: A) {
    senden(ClientNachricht::Aufgaben(a));
}

const SPALTEN: [(AufgabenStatus, &str); 4] = [
    (AufgabenStatus::Offen, "Offen"),
    (AufgabenStatus::InArbeit, "In Arbeit"),
    (AufgabenStatus::Blockiert, "Blockiert"),
    (AufgabenStatus::Erledigt, "Erledigt"),
];

fn status_wert(s: AufgabenStatus) -> &'static str {
    match s {
        AufgabenStatus::Offen => "offen",
        AufgabenStatus::InArbeit => "in_arbeit",
        AufgabenStatus::Blockiert => "blockiert",
        AufgabenStatus::Erledigt => "erledigt",
    }
}

fn status_aus(w: &str) -> AufgabenStatus {
    match w {
        "in_arbeit" => AufgabenStatus::InArbeit,
        "blockiert" => AufgabenStatus::Blockiert,
        "erledigt" => AufgabenStatus::Erledigt,
        _ => AufgabenStatus::Offen,
    }
}

#[component]
fn Zeile(a: Aufgabe) -> impl IntoView {
    let id = a.id;
    let ich = zustand().mein_name();
    let meins = a.besitzer.as_deref() == Some(ich.as_str());
    view! {
        <li class="flex flex-col gap-1 rounded-lg border bg-card p-2">
            <div class="flex items-start gap-2">
                <span class="flex-1 text-sm font-bold">
                    {a.automatisch.then_some("🤖 ")}
                    {a.titel.clone()}
                    {a.issue.map(|n| view! { <span class="text-muted-foreground font-semibold">{format!(" #{n}")}</span> })}
                </span>
                <select
                    class="rounded-md border bg-background px-1 text-xs"
                    aria-label="Status"
                    on:change=move |e| schicken(A::Aendern { id, titel: None, status: Some(status_aus(&event_target_value(&e))), besitzer: None, notiz: None })
                >
                    {SPALTEN.iter().map(|(s, name)| view! { <option value=status_wert(*s) selected=*s == a.status>{*name}</option> }).collect_view()}
                </select>
            </div>
            {a.notiz.clone().map(|n| view! { <p class="text-muted-foreground text-xs whitespace-pre-wrap">{n}</p> })}
            <div class="text-muted-foreground flex flex-wrap items-center gap-2 text-xs">
                <span>{a.besitzer.clone().map(|b| format!("👤 {b}")).unwrap_or_else(|| "niemand".into())}</span>
                <span>{format!("von {} · {}", a.angelegt_von, texte::her(a.geaendert_um))}</span>
                <span class="ml-auto flex gap-1">
                    {(!meins && a.status != AufgabenStatus::Erledigt).then(|| {
                        let ich = ich.clone();
                        view! {
                            <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| schicken(A::Aendern { id, titel: None, status: Some(AufgabenStatus::InArbeit), besitzer: Some(ich.clone()), notiz: None })>
                                "✋ Übernehmen"
                            </Knopf>
                        }
                    })}
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::SymbolKlein attr:title="Nach oben" on:click=move |_| schicken(A::Verschieben { id, delta: -1 })>"↑"</Knopf>
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::SymbolKlein attr:title="Nach unten" on:click=move |_| schicken(A::Verschieben { id, delta: 1 })>"↓"</Knopf>
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::SymbolKlein attr:title="Entfernen" on:click=move |_| schicken(A::Entfernen { id })>"🗑"</Knopf>
                </span>
            </div>
        </li>
    }
}

#[component]
fn Neu() -> impl IntoView {
    let z = zustand();
    let titel = RwSignal::new(String::new());
    let notiz = RwSignal::new(String::new());
    let prompt = RwSignal::new(String::new());
    let automatisch = RwSignal::new(false);
    let provider = RwSignal::new(String::new());
    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let t = titel.get_untracked().trim().to_string();
        if t.is_empty() {
            return;
        }
        let leer_weg = |s: String| (!s.trim().is_empty()).then(|| s.trim().to_string());
        schicken(A::Hinzufuegen {
            titel: t,
            notiz: leer_weg(notiz.get_untracked()),
            prompt: leer_weg(prompt.get_untracked()).filter(|_| automatisch.get_untracked()),
            issue: None,
            besitzer: None,
            automatisch: automatisch.get_untracked(),
            provider: leer_weg(provider.get_untracked()).filter(|_| automatisch.get_untracked()),
        });
        titel.set(String::new());
        notiz.set(String::new());
        prompt.set(String::new());
    };
    view! {
        <form class="flex flex-col gap-2 rounded-lg border bg-secondary/50 p-3" on:submit=absenden>
            <div class="flex gap-2">
                <Eingabe wert=titel platzhalter="Neue Aufgabe …" />
                <Knopf absenden=true>"Hinzufügen"</Knopf>
            </div>
            <Textfeld wert=notiz platzhalter="Notiz (optional)" class="min-h-12" />
            <label class="flex items-center gap-2 text-sm font-semibold">
                <input type="checkbox" prop:checked=move || automatisch.get() on:change=move |e| automatisch.set(event_target_checked(&e)) />
                "🤖 Das Office setzt selbst einen Arbeiter darauf an"
            </label>
            <Show when=move || automatisch.get()>
                <Textfeld wert=prompt platzhalter="Auftrag für den Agenten (leer: Titel und Notiz)" class="min-h-12" />
                <select class="h-9 rounded-md border bg-background px-2 text-sm" on:change=move |e| provider.set(event_target_value(&e))>
                    <option value="">"Standard-Agent"</option>
                    {move || z.provider.get().0.into_iter().filter(|p| p.verfuegbar).map(|p| view! { <option value=p.id.clone()>{p.name.clone()}</option> }).collect_view()}
                </select>
            </Show>
        </form>
    }
}

#[component]
pub fn Aufgaben() -> impl IntoView {
    let z = zustand();
    let limit = RwSignal::new(String::new());
    Effect::new(move |_| limit.set(z.board.with(|b| b.max_arbeiter.to_string())));
    if z.provider.with_untracked(|p| p.0.is_empty()) {
        senden(ClientNachricht::Arbeiter(protokoll::arbeiter::Client::ProviderAbfragen));
    }
    let kopf = view! {
        <label class="text-muted-foreground flex items-center gap-1 text-xs font-semibold" title="Wie viele Arbeiter die 🤖-Aufgaben gleichzeitig beschäftigen dürfen; 0 hält die Warteschlange an">
            "🤖 max."
            <input
                type="number"
                min="0"
                max="20"
                class="h-8 w-14 rounded-md border bg-background px-1 text-sm"
                prop:value=move || limit.get()
                on:change=move |e| {
                    if let Ok(n) = event_target_value(&e).parse::<u32>() {
                        schicken(A::Limit { max_arbeiter: n });
                    }
                }
            />
        </label>
        <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=move |_| schicken(A::ErledigteLeeren)>"Erledigte leeren"</Knopf>
    }
    .into_any();
    view! {
        <Rahmen fenster=Fenster::Aufgaben titel="📋 Aufgabenboard" breite="max-w-6xl" kopf=kopf>
            <div class="flex flex-col gap-4">
                <Neu />
                <div class="grid gap-3 md:grid-cols-4">
                    {SPALTEN.iter().map(|(s, name)| {
                        let s = *s;
                        let liste = move || z.board.with(|b| b.aufgaben.iter().filter(|a| a.status == s).cloned().collect::<Vec<_>>());
                        view! {
                            <section class="flex min-w-0 flex-col gap-2">
                                <h3 class="text-sm font-extrabold">{*name}" "<span class="text-muted-foreground">{move || liste().len()}</span></h3>
                                <ul class="flex flex-col gap-2">
                                    {move || liste().into_iter().map(|a| view! { <Zeile a=a /> }).collect_view()}
                                </ul>
                            </section>
                        }
                    }).collect_view()}
                </div>
            </div>
        </Rahmen>
    }
}
