//! 🐙 GitHub: Issues und Pull Requests des Projekts der Etage. Links die Liste, rechts das gewählte
//! Stück mit Kommentar, Mergen, Schließen und "an einen Arbeiter vergeben".

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::github::{Client as G, Issue, Label, MergeArt, Pruefungen, PullRequest, Server};

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse, Reiter, ReiterLeiste, Textfeld};

/// Der letzte Stand vom Server.
#[derive(Clone, Default)]
struct Stand {
    issues: Vec<Issue>,
    prs: Vec<PullRequest>,
    fehler: Option<String>,
    laedt: bool,
}

thread_local! {
    static STAND: OnceCell<RwSignal<Stand>> = const { OnceCell::new() };
}

fn stand() -> RwSignal<Stand> {
    STAND.with(|s| *s.get_or_init(|| RwSignal::new(Stand::default())))
}

pub fn empfangen(m: &Server) {
    #[allow(irrefutable_let_patterns)]
    if let Server::Stand { issues, prs, fehler, laedt } = m {
        stand().set(Stand { issues: issues.clone(), prs: prs.clone(), fehler: fehler.clone(), laedt: *laedt });
    }
}

/// Wie viele offene Issues und PRs es gibt (für das Dock).
pub fn offen() -> usize {
    stand().with(|s| s.issues.len() + s.prs.len())
}

fn schicken(g: G) {
    senden(ClientNachricht::Github(g));
}

#[derive(Clone, Copy, PartialEq)]
enum Wahl {
    Issue(u64),
    Pr(u64),
    Neu,
}

fn labels(l: &[Label]) -> impl IntoView + use<> {
    l.iter()
        .map(|l| view! { <span class="rounded-full border px-1.5 text-[11px] font-bold" style=format!("border-color:{0};color:{0}", l.farbe)>{l.name.clone()}</span> })
        .collect_view()
}

fn pruefung(p: Pruefungen) -> &'static str {
    match p {
        Pruefungen::Bestanden => "✅",
        Pruefungen::Fehlgeschlagen => "❌",
        Pruefungen::Laufen => "⏳",
        _ => "",
    }
}

#[component]
fn Kommentar(nummer: u64) -> impl IntoView {
    let text = RwSignal::new(String::new());
    view! {
        <form class="flex flex-col gap-2" on:submit=move |e| {
            e.prevent_default();
            let t = text.get_untracked();
            if !t.trim().is_empty() {
                schicken(G::Kommentieren { nummer, text: t });
                text.set(String::new());
            }
        }>
            <Textfeld wert=text platzhalter="Kommentar schreiben …" class="min-h-16" />
            <Knopf absenden=true groesse=KnopfGroesse::Klein class="self-end">"💬 Kommentieren"</Knopf>
        </form>
    }
}

#[component]
fn IssueAnsicht(i: Issue) -> impl IntoView {
    let n = i.nummer;
    view! {
        <h3 class="text-lg font-extrabold">{format!("#{} {}", i.nummer, i.titel)}</h3>
        <div class="text-muted-foreground flex flex-wrap items-center gap-2 text-xs">
            <span>{format!("von {}", i.autor)}</span>
            {(!i.zugewiesen.is_empty()).then(|| view! { <span>{format!("👤 {}", i.zugewiesen.join(", "))}</span> })}
            <span>{format!("💬 {}", i.kommentare)}</span>
            {labels(&i.labels)}
            <a class="text-primary ml-auto font-bold hover:underline" href=i.url.clone() target="_blank" rel="noopener noreferrer">"Auf GitHub ↗"</a>
        </div>
        <div class="flex flex-wrap gap-2">
            <Knopf groesse=KnopfGroesse::Klein attr:title="Einen Arbeiter am nächsten freien Schreibtisch darauf ansetzen"
                on:click=move |_| schicken(G::IssueVergeben { nummer: n, tisch: None, provider: None })>
                "🤖 An einen Arbeiter vergeben"
            </Knopf>
            <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click={
                let titel = i.titel.clone();
                move |_| senden(ClientNachricht::Aufgaben(protokoll::aufgaben::Client::Hinzufuegen {
                    titel: titel.clone(), notiz: None, prompt: None, issue: Some(n), besitzer: None, automatisch: false, provider: None,
                }))
            }>
                "📋 Aufs Aufgabenboard"
            </Knopf>
            <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| schicken(G::Schliessen { nummer: n })>"Schließen"</Knopf>
        </div>
        <div class="rounded-lg bg-secondary/60 p-3 text-sm">{if i.text.is_empty() { "(keine Beschreibung)".to_string().into_any() } else { view! { <crate::markdown::Markdown text=i.text.clone() /> }.into_any() }}</div>
        <Kommentar nummer=n />
    }
}

#[component]
fn PrAnsicht(p: PullRequest) -> impl IntoView {
    let n = p.nummer;
    let art = RwSignal::new(MergeArt::Squash);
    let loeschen = RwSignal::new(true);
    let offen = p.zustand == "OPEN";
    view! {
        <h3 class="text-lg font-extrabold">{format!("#{} {}", p.nummer, p.titel)}{p.entwurf.then_some(" (Entwurf)")}</h3>
        <div class="text-muted-foreground flex flex-wrap items-center gap-2 text-xs">
            <span>{format!("von {} · {} → {}", p.autor, p.branch, p.ziel)}</span>
            <span class="text-success font-bold">{format!("+{}", p.plus)}</span>
            <span class="text-destructive font-bold">{format!("−{}", p.minus)}</span>
            <span>{pruefung(p.pruefungen)}</span>
            {labels(&p.labels)}
            <a class="text-primary ml-auto font-bold hover:underline" href=p.url.clone() target="_blank" rel="noopener noreferrer">"Auf GitHub ↗"</a>
        </div>
        {offen.then(|| view! {
            <div class="flex flex-wrap items-center gap-2">
                <select class="h-8 rounded-md border bg-background px-2 text-sm" aria-label="Wie mergen" on:change=move |e| art.set(match event_target_value(&e).as_str() {
                    "merge" => MergeArt::Merge,
                    "rebase" => MergeArt::Rebase,
                    _ => MergeArt::Squash,
                })>
                    <option value="squash">"Squash"</option>
                    <option value="merge">"Merge-Commit"</option>
                    <option value="rebase">"Rebase"</option>
                </select>
                <label class="flex items-center gap-1 text-sm">
                    <input type="checkbox" prop:checked=move || loeschen.get() on:change=move |e| loeschen.set(event_target_checked(&e)) />
                    "Branch löschen"
                </label>
                <Knopf art=KnopfArt::Erfolg groesse=KnopfGroesse::Klein on:click=move |_| schicken(G::Mergen { nummer: n, art: art.get_untracked(), branch_loeschen: loeschen.get_untracked() })>
                    "🔀 Mergen"
                </Knopf>
                <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| schicken(G::Schliessen { nummer: n })>"Schließen"</Knopf>
            </div>
        })}
        <div class="rounded-lg bg-secondary/60 p-3 text-sm">{if p.text.is_empty() { "(keine Beschreibung)".to_string().into_any() } else { view! { <crate::markdown::Markdown text=p.text.clone() /> }.into_any() }}</div>
        <Kommentar nummer=n />
    }
}

#[component]
fn NeuesIssue(wahl: RwSignal<Option<Wahl>>) -> impl IntoView {
    let titel = RwSignal::new(String::new());
    let text = RwSignal::new(String::new());
    view! {
        <h3 class="text-lg font-extrabold">"Neues Issue"</h3>
        <form class="flex flex-col gap-2" on:submit=move |e| {
            e.prevent_default();
            let t = titel.get_untracked();
            if t.trim().is_empty() {
                return;
            }
            schicken(G::IssueAnlegen { titel: t.trim().to_string(), text: text.get_untracked() });
            wahl.set(None);
        }>
            <Eingabe wert=titel platzhalter="Titel" />
            <Textfeld wert=text platzhalter="Beschreibung" class="min-h-40" />
            <Knopf absenden=true class="self-end">"Anlegen"</Knopf>
        </form>
    }
}

#[component]
pub fn Github() -> impl IntoView {
    let s = stand();
    let prs_zeigen = RwSignal::new(false);
    let wahl = RwSignal::new(None::<Wahl>);
    schicken(G::Aktualisieren);

    let kopf = view! {
        <Show when=move || s.with(|s| s.laedt)>
            <span class="text-muted-foreground text-xs">"lädt …"</span>
        </Show>
        <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein attr:title="Neu von GitHub holen" on:click=move |_| schicken(G::Aktualisieren)>"⟳"</Knopf>
        <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein attr:title="Den Agenten am Pult fragen"
            on:click=move |_| crate::modal::oeffnen(Fenster::Station(if prs_zeigen.get_untracked() { "station-pulls" } else { "station-issues" }.into()))>"🤖 Agent fragen"</Knopf>
        <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| wahl.set(Some(Wahl::Neu))>"+ Issue"</Knopf>
    }
    .into_any();

    let detail = move || match wahl.get() {
        Some(Wahl::Issue(n)) => s.with(|s| s.issues.iter().find(|i| i.nummer == n).cloned()).map(|i| view! { <IssueAnsicht i=i /> }.into_any()),
        Some(Wahl::Pr(n)) => s.with(|s| s.prs.iter().find(|p| p.nummer == n).cloned()).map(|p| view! { <PrAnsicht p=p /> }.into_any()),
        Some(Wahl::Neu) => Some(view! { <NeuesIssue wahl=wahl /> }.into_any()),
        None => None,
    };

    view! {
        <Rahmen fenster=Fenster::Github titel="🐙 GitHub" breite="max-w-6xl" kopf=kopf>
            <Show when=move || s.with(|s| s.fehler.is_some())>
                <p class="mb-2 rounded-md bg-destructive/15 p-2 text-sm font-semibold">{move || s.with(|s| s.fehler.clone())}</p>
            </Show>
            <div class="grid h-[70vh] gap-4 md:grid-cols-[minmax(0,22rem)_1fr]">
                <section class="flex min-h-0 flex-col gap-2">
                    <ReiterLeiste class="w-full">
                        <Reiter aktiv=Signal::derive(move || !prs_zeigen.get()) on:click=move |_| prs_zeigen.set(false)>
                            {move || format!("Issues ({})", s.with(|s| s.issues.len()))}
                        </Reiter>
                        <Reiter aktiv=prs_zeigen on:click=move |_| prs_zeigen.set(true)>
                            {move || format!("Pull Requests ({})", s.with(|s| s.prs.len()))}
                        </Reiter>
                    </ReiterLeiste>
                    <ul class="flex min-h-0 flex-1 flex-col gap-1 overflow-auto">
                        {move || if prs_zeigen.get() {
                            s.with(|s| s.prs.clone()).into_iter().map(|p| {
                                let n = p.nummer;
                                view! {
                                    <li><button type="button" class="flex w-full flex-col rounded-md px-2 py-1.5 text-left hover:cursor-pointer hover:bg-accent"
                                        class:bg-accent=move || wahl.get() == Some(Wahl::Pr(n)) on:click=move |_| wahl.set(Some(Wahl::Pr(n)))>
                                        <span class="truncate text-sm font-bold">{pruefung(p.pruefungen)}" "{format!("#{} {}", p.nummer, p.titel)}</span>
                                        <span class="text-muted-foreground truncate text-xs">{format!("{} · {}", p.autor, p.branch)}</span>
                                    </button></li>
                                }
                            }).collect_view().into_any()
                        } else {
                            s.with(|s| s.issues.clone()).into_iter().map(|i| {
                                let n = i.nummer;
                                view! {
                                    <li><button type="button" class="flex w-full flex-col rounded-md px-2 py-1.5 text-left hover:cursor-pointer hover:bg-accent"
                                        class:bg-accent=move || wahl.get() == Some(Wahl::Issue(n)) on:click=move |_| wahl.set(Some(Wahl::Issue(n)))>
                                        <span class="truncate text-sm font-bold">{format!("#{} {}", i.nummer, i.titel)}</span>
                                        <span class="text-muted-foreground flex gap-1 truncate text-xs">{i.autor.clone()}" "{labels(&i.labels)}</span>
                                    </button></li>
                                }
                            }).collect_view().into_any()
                        }}
                    </ul>
                </section>
                <section class="flex min-h-0 flex-col gap-3 overflow-auto">
                    {move || detail().unwrap_or_else(|| view! { <p class="text-muted-foreground">"Links ein Issue oder einen Pull Request wählen."</p> }.into_any())}
                </section>
            </div>
        </Rahmen>
    }
}
