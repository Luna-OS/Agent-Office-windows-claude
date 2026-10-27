//! 🤝 Meeting im Besprechungsraum: 2–5 Arbeiter an einer Frage oder Aufgabe, in Runden nach einem Muster
//! (Debatte, Leitung, Map-Reduce, Rot gegen Blau, Review). Einberufen, verfolgen, stoppen, abräumen.

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::agenten::{Besprechung, BesprechungStatus, BesprechungWunsch, Client as G, Muster, ZugZustand};

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse, Textfeld};
use crate::zustand::zustand;

fn schicken(g: G) {
    senden(ClientNachricht::Agenten(g));
}

const MUSTER: [(Muster, &str, &str); 5] = [
    (Muster::Debatte, "🗣 Debatte", "Jeder schlägt vor, dann kritisieren sie einander; der Vorsitz schreibt die Entscheidung."),
    (Muster::Leitung, "🧭 Leitung", "Die Leitung teilt die Aufgabe auf, das Team erledigt je einen Teil, die Leitung führt zusammen."),
    (Muster::MapReduce, "🗂 Map-Reduce", "Dieselbe Aufgabe über viele Teile parallel; der Kopf des Tisches fasst zusammen."),
    (Muster::RotBlau, "🟥🟦 Rot gegen Blau", "Rot greift an, Blau behebt, Runde um Runde."),
    (Muster::Review, "🔍 Review", "Prüfer lesen einen Pull Request je aus ihrem Blickwinkel; das Ergebnis kommt als Kommentar an den PR."),
];

fn muster_name(m: Muster) -> &'static str {
    MUSTER.iter().find(|x| x.0 == m).map_or("", |x| x.1)
}

fn zug_symbol(z: ZugZustand) -> &'static str {
    match z {
        ZugZustand::Wartet => "⏳",
        ZugZustand::Gesendet => "📨",
        ZugZustand::Arbeitet => "⚙️",
        ZugZustand::Fertig => "✅",
    }
}

#[component]
fn Laufend(b: Besprechung) -> impl IntoView {
    let laeuft = b.status == BesprechungStatus::Laeuft;
    let anteil = if b.budget > 0 { (b.tokens as f64 * 100.0 / b.budget as f64).min(100.0) } else { 0.0 };
    view! {
        <div class="flex flex-col gap-3">
            <div class="flex flex-wrap items-baseline gap-2">
                <h3 class="text-lg font-extrabold">{b.titel.clone()}</h3>
                <span class="text-muted-foreground text-sm">{format!("{} · Runde {} von {} · von {}", muster_name(b.muster), b.runde, b.runden, b.einberufen_von)}</span>
            </div>
            <p class="text-sm whitespace-pre-wrap">{b.auftrag.clone()}</p>
            <ul class="grid gap-2 sm:grid-cols-2">
                {b.plaetze.iter().enumerate().map(|(i, p)| {
                    let zug = b.zuege.iter().find(|z| z.platz == i).cloned();
                    view! {
                        <li class="flex flex-col rounded-lg border p-2 text-sm">
                            <span class="font-extrabold">{p.name.clone().unwrap_or_else(|| "(noch leer)".into())}{if i == 0 { " 👑" } else { "" }}</span>
                            <span class="text-muted-foreground text-xs">{format!("{} · {} Tokens", p.rolle, texte::zahl_kurz(p.tokens))}</span>
                            {zug.map(|z| view! { <span class="text-xs">{format!("{} {} → {}", zug_symbol(z.zustand), z.tut, z.datei)}</span> })}
                        </li>
                    }
                }).collect_view()}
            </ul>
            {(b.budget > 0).then(|| view! {
                <div class="flex flex-col gap-0.5 text-xs">
                    <span>{format!("Budget: {} von {} Tokens", texte::zahl_kurz(b.tokens), texte::zahl_kurz(b.budget))}</span>
                    <div class="h-2 overflow-hidden rounded-full bg-muted"><div class="h-full bg-primary" style=format!("width:{anteil}%")></div></div>
                </div>
            })}
            {b.grund.clone().map(|g| view! { <p class="text-sm font-semibold text-destructive">{g}</p> })}
            {b.review_url.clone().map(|u| view! { <a class="text-primary font-bold hover:underline" href=u target="_blank" rel="noopener noreferrer">"Review-Kommentar am PR ↗"</a> })}
            {b.review_fehler.clone().map(|f| view! { <p class="text-sm text-destructive">{f}</p> })}
            {b.commit.clone().map(|c| view! { <p class="text-muted-foreground text-xs">{format!("Ergebnis in {} (Commit {})", b.ausgabe, &c[..c.len().min(8)])}</p> })}
            {b.vorschau.clone().map(|v| view! { <div class="max-h-64 overflow-auto rounded-lg bg-secondary/60 p-3 text-sm"><crate::markdown::Markdown text=v /></div> })}
            <div class="flex gap-2">
                {if laeuft {
                    view! { <Knopf art=KnopfArt::Gefahr groesse=KnopfGroesse::Klein on:click=|_| schicken(G::BesprechungStoppen)>"⏹ Stoppen"</Knopf> }.into_any()
                } else {
                    view! { <Knopf groesse=KnopfGroesse::Klein attr:title="Die Arbeiter gehen heim, der Tisch wird frei" on:click=|_| schicken(G::BesprechungAbraeumen)>"🧹 Abräumen"</Knopf> }.into_any()
                }}
            </div>
        </div>
    }
}

#[component]
fn Einberufen() -> impl IntoView {
    let z = zustand();
    let muster = RwSignal::new(Muster::Debatte);
    let auftrag = RwSignal::new(String::new());
    let titel = RwSignal::new(String::new());
    let rollen = RwSignal::new(String::new());
    let teile = RwSignal::new(String::new());
    let pr = RwSignal::new(String::new());
    let runden = RwSignal::new(String::new());
    let budget = RwSignal::new(String::new());
    let provider = RwSignal::new(String::new());
    if z.provider.with_untracked(|p| p.0.is_empty()) {
        senden(ClientNachricht::Arbeiter(protokoll::arbeiter::Client::ProviderAbfragen));
    }
    let zeilen = |s: String| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect::<Vec<_>>();
    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let a = auftrag.get_untracked().trim().to_string();
        if a.is_empty() {
            crate::zustand::fehler("Worum geht es? Bitte einen Auftrag schreiben.");
            return;
        }
        let leer_weg = |s: String| (!s.trim().is_empty()).then(|| s.trim().to_string());
        let wunsch = BesprechungWunsch {
            muster: muster.get_untracked(),
            auftrag: a,
            titel: leer_weg(titel.get_untracked()),
            ausgabe: None,
            rollen: zeilen(rollen.get_untracked()),
            teile: zeilen(teile.get_untracked()),
            pr: pr.get_untracked().trim().trim_start_matches('#').parse().ok(),
            issue: None,
            runden: runden.get_untracked().parse().ok(),
            budget: budget.get_untracked().parse::<u64>().ok().map(|k| k * 1000),
            provider: leer_weg(provider.get_untracked()),
            modell: None,
            aufwand: None,
        };
        schicken(G::BesprechungStarten { wunsch: Box::new(wunsch) });
    };
    view! {
        <form class="flex flex-col gap-3" on:submit=absenden>
            <div class="grid gap-2 sm:grid-cols-5">
                {MUSTER.iter().map(|(m, name, was)| view! {
                    <button type="button" class="flex flex-col rounded-lg border-2 p-2 text-left text-xs hover:cursor-pointer" title=*was
                        class:border-primary=move || muster.get() == *m class:bg-accent=move || muster.get() == *m
                        on:click=move |_| muster.set(*m)>
                        <span class="text-sm font-extrabold">{*name}</span>
                    </button>
                }).collect_view()}
            </div>
            <p class="text-muted-foreground text-sm">{move || MUSTER.iter().find(|x| x.0 == muster.get()).map_or("", |x| x.2)}</p>
            <Textfeld wert=auftrag platzhalter="Worum geht es? (die Frage oder Aufgabe für alle)" class="min-h-24" />
            <Eingabe wert=titel platzhalter="Titel (optional)" />
            <Show when=move || muster.get() == Muster::Review>
                <Eingabe wert=pr platzhalter="Pull Request, z. B. #42" />
            </Show>
            <Show when=move || muster.get() == Muster::MapReduce>
                <Textfeld wert=teile platzhalter="Die Teile, einer je Zeile (Ordner, Dateien, Themen)" class="min-h-16" />
            </Show>
            <details class="text-sm">
                <summary class="hover:cursor-pointer font-semibold">"Mehr: Rollen, Runden, Budget, Agent"</summary>
                <div class="mt-2 flex flex-col gap-2">
                    <Textfeld wert=rollen platzhalter="Eine Rolle je Zeile, der Kopf des Tisches zuerst (leer: die des Musters)" class="min-h-16" />
                    <div class="flex gap-2">
                        <Eingabe wert=runden typ="number" platzhalter="Runden" />
                        <Eingabe wert=budget typ="number" platzhalter="Budget in tausend Tokens" />
                        <select class="h-9 rounded-md border bg-background px-2 text-sm" on:change=move |e| provider.set(event_target_value(&e))>
                            <option value="">"Standard-Agent"</option>
                            {move || z.provider.get().0.into_iter().filter(|p| p.verfuegbar).map(|p| view! { <option value=p.id.clone()>{p.name.clone()}</option> }).collect_view()}
                        </select>
                    </div>
                </div>
            </details>
            <Knopf absenden=true class="self-start">"🤝 Einberufen"</Knopf>
        </form>
    }
}

#[component]
pub fn Meeting() -> impl IntoView {
    let z = zustand();
    view! {
        <Rahmen fenster=Fenster::Meeting titel="🤝 Meeting" breite="max-w-3xl">
            <div class="flex flex-col gap-4">
                {move || match z.besprechung.get().aktuell {
                    Some(b) => view! { <Laufend b=*b /> }.into_any(),
                    None => view! { <Einberufen /> }.into_any(),
                }}
                {move || {
                    let frueher = z.besprechung.get().frueher;
                    (!frueher.is_empty()).then(|| view! {
                        <section class="flex flex-col gap-1">
                            <h3 class="text-sm font-extrabold">"Frühere Meetings"</h3>
                            {frueher.into_iter().take(10).map(|e| view! {
                                <details class="rounded-md border px-2 py-1 text-sm">
                                    <summary class="hover:cursor-pointer">
                                        <b>{e.titel.clone()}</b>
                                        <span class="text-muted-foreground text-xs">{format!(" · {} · {}", muster_name(e.muster), texte::her(e.beendet_um))}</span>
                                    </summary>
                                    <p class="mt-1 text-xs whitespace-pre-wrap">{e.zusammenfassung.clone()}</p>
                                    {e.branch.clone().map(|b| view! { <p class="text-muted-foreground text-xs">{format!("⎇ {b} · {}", e.ausgabe)}</p> })}
                                </details>
                            }).collect_view()}
                        </section>
                    })
                }}
            </div>
        </Rahmen>
    }
}
