//! Reiter am Terminal eines Arbeiters: das Terminal selbst und daneben angeheftete Webseiten (ein
//! verlinkter Chat, Doku zur Aufgabe). Sie gehören nur diesem Browser und bleiben je Arbeiter, bis die
//! Seite neu lädt.

use std::cell::OnceCell;
use std::collections::HashMap;

use leptos::prelude::*;

use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};

/// Was eine angeheftete Seite in ihrem Rahmen darf: laufen, anmelden, Formulare, Links in neuem Tab;
/// nie das Office selbst wegnavigieren, nie Kamera, Mikro oder Zwischenablage.
const SANDBOX: &str =
    "allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox";

#[derive(Clone, PartialEq)]
struct Webseite {
    name: String,
    url: String,
}

/// Je Arbeiter: die Seiten und welche gerade gezeigt wird (`None` = das Terminal).
#[derive(Clone, Copy)]
struct Reiter {
    seiten: RwSignal<HashMap<String, Vec<Webseite>>>,
    gewaehlt: RwSignal<HashMap<String, Option<usize>>>,
}

thread_local! {
    static REITER: OnceCell<Reiter> = const { OnceCell::new() };
}

fn reiter() -> Reiter {
    REITER.with(|r| {
        *r.get_or_init(|| {
            crate::zustand::global(|| Reiter {
                seiten: RwSignal::new(HashMap::new()),
                gewaehlt: RwSignal::new(HashMap::new()),
            })
        })
    })
}

/// Welche Seite bei diesem Arbeiter gezeigt wird (`None` = das Terminal).
pub fn gewaehlt(arbeiter: &str) -> Signal<Option<usize>> {
    let a = arbeiter.to_string();
    let r = reiter();
    Signal::derive(move || r.gewaehlt.with(|g| g.get(&a).copied().flatten()))
}

fn waehlen(arbeiter: &str, n: Option<usize>) {
    reiter().gewaehlt.update(|g| {
        g.insert(arbeiter.to_string(), n);
    });
    if n.is_none() {
        set_timeout(super::fokus_zurueck, std::time::Duration::from_millis(10));
    }
}

/// Eine Adresse, die man in einen Rahmen laden darf (nur http und https).
fn adresse(url: &str) -> Option<String> {
    let url = url.trim();
    let mit = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    (mit.starts_with("https://") || mit.starts_with("http://")).then_some(mit)
}

#[component]
pub fn Leiste(arbeiter: String) -> impl IntoView {
    let r = reiter();
    let a = StoredValue::new(arbeiter.clone());
    let wahl = gewaehlt(&arbeiter);
    let formular = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let url = RwSignal::new(String::new());
    let seiten = move || {
        r.seiten
            .with(|s| s.get(&a.get_value()).cloned().unwrap_or_default())
    };

    let hinzufuegen = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let Some(u) = adresse(&url.get_untracked()) else {
            crate::zustand::fehler("Das ist keine Webadresse (https://…).");
            return;
        };
        let n = name.get_untracked().trim().to_string();
        let n = if n.is_empty() {
            crate::texte::kuerzen(u.trim_start_matches("https://"), 24)
        } else {
            n
        };
        let mut neu = 0;
        r.seiten.update(|s| {
            let l = s.entry(a.get_value()).or_default();
            l.push(Webseite { name: n, url: u });
            neu = l.len() - 1;
        });
        waehlen(&a.get_value(), Some(neu));
        formular.set(false);
        name.set(String::new());
        url.set(String::new());
    };

    view! {
        <div class="mb-2 flex flex-wrap items-center gap-1" role="tablist" aria-label="Reiter">
            <button type="button" role="tab" class="rounded-md px-3 py-1 text-sm font-bold hover:cursor-pointer"
                class:bg-secondary=move || wahl.get().is_none()
                on:click=move |_| waehlen(&a.get_value(), None)>
                "⌨ Terminal"
            </button>
            {move || seiten().into_iter().enumerate().map(|(i, s)| view! {
                <span class="inline-flex items-center rounded-md" class:bg-secondary=move || wahl.get() == Some(i)>
                    <button type="button" role="tab" class="px-3 py-1 text-sm font-bold hover:cursor-pointer" title=s.url.clone()
                        on:click=move |_| waehlen(&a.get_value(), Some(i))>
                        {s.name.clone()}
                    </button>
                    <button type="button" class="px-1.5 text-xs hover:cursor-pointer" aria-label="Reiter schließen" title="Reiter schließen"
                        on:click=move |_| {
                            r.seiten.update(|m| { if let Some(l) = m.get_mut(&a.get_value()) && i < l.len() { l.remove(i); } });
                            waehlen(&a.get_value(), None);
                        }>
                        "✕"
                    </button>
                </span>
            }).collect_view()}
            <Show when=move || !formular.get()>
                <button type="button" class="text-muted-foreground rounded-md px-2 py-1 text-sm hover:cursor-pointer hover:bg-accent"
                    title="Eine Webseite neben diesem Terminal anheften (ein Chat, Doku, alles mit Adresse)"
                    on:click=move |_| formular.set(true)>
                    "+ Webseite"
                </button>
            </Show>
            {move || wahl.get().and_then(|i| seiten().get(i).cloned()).map(|s| view! {
                <a class="text-primary ml-auto text-sm font-bold hover:underline" href=s.url target="_blank" rel="noopener noreferrer"
                    title="Diese Seite in einem eigenen Browser-Tab öffnen (für Seiten, die sich nicht einbetten lassen)">
                    "↗ Neuer Tab"
                </a>
            })}
            <Show when=move || formular.get()>
                <form class="flex w-full items-center gap-2" on:submit=hinzufuegen>
                    <Eingabe wert=name platzhalter="Name (z. B. Doku)" class="max-w-48" />
                    <Eingabe wert=url platzhalter="https://…" />
                    <Knopf absenden=true groesse=KnopfGroesse::Klein>"Anheften"</Knopf>
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=move |_| formular.set(false)>"Abbrechen"</Knopf>
                </form>
            </Show>
        </div>
    }
}

/// Die angehefteten Seiten; nur die gewählte ist sichtbar, die anderen bleiben geladen.
#[component]
pub fn Seiten(arbeiter: String) -> impl IntoView {
    let r = reiter();
    let a = StoredValue::new(arbeiter.clone());
    let wahl = gewaehlt(&arbeiter);
    view! {
        {move || r.seiten.with(|s| s.get(&a.get_value()).cloned().unwrap_or_default()).into_iter().enumerate().map(|(i, s)| view! {
            <iframe class="absolute inset-0 size-full rounded-lg border bg-white" class:hidden=move || wahl.get() != Some(i)
                src=s.url title=s.name sandbox=SANDBOX referrerpolicy="no-referrer"></iframe>
        }).collect_view()}
    }
}
