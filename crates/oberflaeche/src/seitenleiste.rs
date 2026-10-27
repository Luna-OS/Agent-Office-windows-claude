//! Rechts im HUD: die Arbeiter der Etage (Klick öffnet das Terminal) und wer im Office ist.

use leptos::prelude::*;
use protokoll::arbeiter::{ArbeiterArt, ArbeiterStatus};

use crate::modal::{self, Fenster};
use crate::texte;
use crate::zustand::zustand;

#[component]
fn Arbeiterliste() -> impl IntoView {
    let z = zustand();
    let wartend = move || {
        z.arbeiter.with(|l| {
            l.iter()
                .filter(|a| a.status == ArbeiterStatus::Wartet)
                .count()
        })
    };
    let kosten = move || {
        let k: f64 = z
            .arbeiter
            .with(|l| l.iter().filter_map(|a| a.verbrauch.as_ref()?.kosten).sum());
        (k > 0.0).then(|| texte::dollar(k))
    };
    view! {
        <section class="hud-panel pointer-events-auto flex max-h-[45vh] flex-col overflow-hidden" aria-label="Arbeiter">
            <h3 class="flex items-center gap-2 px-3 pt-2 pb-1 text-sm font-extrabold">
                "🤖 Arbeiter"
                <span class="text-muted-foreground">{move || z.arbeiter.with(Vec::len)}</span>
                <span class="text-muted-foreground ml-auto text-xs" title="Ausgegeben von den Arbeitern dieser Etage">{kosten}</span>
            </h3>
            <Show when=move || { wartend() > 0 }>
                <p class="mx-3 mb-1 rounded-md bg-destructive px-2 py-1 text-xs font-bold text-white">
                    {move || match wartend() { 1 => "1 Arbeiter braucht dich".to_string(), n => format!("{n} Arbeiter brauchen dich") }}
                </p>
            </Show>
            <ul class="flex flex-col gap-0.5 overflow-auto px-1.5 pb-2">
                {move || {
                    let liste = z.arbeiter.get();
                    if liste.is_empty() {
                        return view! { <li class="text-muted-foreground px-2 py-1 text-xs font-semibold">"Noch niemand. Stell einen Arbeiter an einen leeren Schreibtisch (E)."</li> }.into_any();
                    }
                    liste
                        .into_iter()
                        .map(|a| {
                            let id = a.id.clone();
                            let symbol = if a.art == ArbeiterArt::Shell { "🐚" } else { "🤖" };
                            let unter = a.aufgabe.as_ref().map(|t| t.name.clone()).or(a.aktivitaet.clone()).unwrap_or_default();
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left hover:cursor-pointer hover:bg-accent"
                                        title="Terminal öffnen"
                                        on:click=move |_| modal::oeffnen(Fenster::Terminal(id.clone()))
                                    >
                                        <span class="size-3 shrink-0 rounded-full border-2 border-foreground" style=format!("background:{}", a.farbe)></span>
                                        <span class="flex min-w-0 flex-1 flex-col">
                                            <span class="truncate text-sm font-bold">{symbol}" "{a.name.clone()}</span>
                                            <span class="text-muted-foreground truncate text-xs">{texte::kuerzen(&unter, 48)}</span>
                                        </span>
                                        <span class=format!("rounded-full px-2 text-[11px] font-extrabold {}", texte::status_klassen(a.status))>
                                            {texte::status(a.status)}
                                        </span>
                                    </button>
                                </li>
                            }
                        })
                        .collect_view()
                        .into_any()
                }}
            </ul>
        </section>
    }
}

#[component]
fn Personenliste() -> impl IntoView {
    let z = zustand();
    view! {
        <section class="hud-panel pointer-events-auto flex max-h-[30vh] flex-col overflow-hidden" aria-label="Im Office">
            <h3 class="flex items-center gap-2 px-3 pt-2 pb-1 text-sm font-extrabold">
                "👥 Im Office"
                <span class="text-muted-foreground">{move || z.personen.with(Vec::len) + 1}</span>
            </h3>
            <ul class="flex flex-col gap-0.5 overflow-auto px-3 pb-2 text-sm">
                {move || z.ich_selbst.get().map(|p| view! {
                    <li class="flex items-center gap-2">
                        <span class="size-3 rounded-full border-2 border-foreground" style=format!("background:{}", p.farbe)></span>
                        <span class="font-bold">{p.name}</span>
                        <span class="text-muted-foreground text-xs">"(du)"</span>
                    </li>
                })}
                {move || z.personen.get().into_iter().map(|p| {
                    let unter = p.tut.clone().unwrap_or_default();
                    view! {
                        <li class="flex items-center gap-2" title=unter.clone()>
                            <span class="size-3 shrink-0 rounded-full border-2 border-foreground" style=format!("background:{}", p.farbe)></span>
                            <span class="flex min-w-0 flex-col">
                                <span class="truncate font-bold">{p.name.clone()}</span>
                                {(!unter.is_empty()).then(|| view! {
                                    <span class="text-muted-foreground truncate text-xs">{unter.clone()}</span>
                                })}
                            </span>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </section>
    }
}

#[component]
pub fn Seitenleiste() -> impl IntoView {
    let l = crate::lokal::lokal();
    view! {
        <aside class="absolute top-20 right-3 flex w-72 flex-col gap-3">
            <Show when=move || l.with(|l| l.tafel_arbeiter)><Arbeiterliste /></Show>
            <Show when=move || l.with(|l| l.tafel_personen)><Personenliste /></Show>
        </aside>
    }
}
