//! ⏳ Nutzung und Limits: was die Arbeiter heute und insgesamt verbraucht haben, die Fenster des
//! Claude-Plans (5 Stunden, Woche), dazu ✏️ die Prompts, die das Office selbst schickt, und 🔌 die
//! Dienste (Dev-Server), die Arbeiter gestartet haben.

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::agenten::{Client as G, PromptInfo};
use protokoll::arbeiter::Verbrauch;

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::texte;
use crate::ui::{Knopf, KnopfArt, KnopfGroesse, Textfeld};
use crate::zustand::zustand;

fn verbrauch(v: &Verbrauch) -> String {
    let tokens = format!(
        "{} ein · {} aus",
        texte::zahl_kurz(v.tokens_ein),
        texte::zahl_kurz(v.tokens_aus)
    );
    match v.kosten {
        Some(k) => format!("{} · {tokens}", texte::dollar(k)),
        None => tokens,
    }
}

/// Wie lange bis `um` (ms), in Worten.
fn bis(um: u64) -> String {
    let s = um.saturating_sub(texte::jetzt()) / 1000;
    match s {
        0..3600 => format!("in {} Min.", s / 60),
        3600..86400 => format!("in {} Std. {} Min.", s / 3600, (s % 3600) / 60),
        _ => format!("in {} Tagen", s / 86400),
    }
}

#[component]
pub fn Nutzung() -> impl IntoView {
    let z = zustand();
    let pro_arbeiter = move || {
        let mut l: Vec<_> = z
            .arbeiter
            .get()
            .into_iter()
            .filter_map(|a| a.verbrauch.clone().map(|v| (a.name, v)))
            .collect();
        l.sort_by(|a, b| {
            b.1.kosten
                .unwrap_or(0.0)
                .total_cmp(&a.1.kosten.unwrap_or(0.0))
        });
        l
    };
    view! {
        <Rahmen fenster=Fenster::Nutzung titel="⏳ Nutzung und Limits" breite="max-w-2xl">
            <div class="flex flex-col gap-4">
                <section class="flex flex-col gap-1">
                    <h3 class="text-sm font-extrabold">"💸 Verbrauch"</h3>
                    {move || match z.nutzung.get() {
                        Some(n) => view! {
                            <p class="text-sm"><b>"Heute: "</b>{verbrauch(&n.heute)}</p>
                            <p class="text-sm"><b>"Insgesamt: "</b>{verbrauch(&n.gesamt)}</p>
                        }.into_any(),
                        None => view! { <p class="text-muted-foreground text-sm">"Noch keine Zahlen."</p> }.into_any(),
                    }}
                    <ul class="text-muted-foreground mt-1 text-xs">
                        {move || pro_arbeiter().into_iter().map(|(n, v)| view! { <li>{format!("{n}: {}", verbrauch(&v))}</li> }).collect_view()}
                    </ul>
                </section>
                <section class="flex flex-col gap-2">
                    <h3 class="flex items-center gap-2 text-sm font-extrabold">
                        "⏳ Claude-Limits"
                        {move || z.limits.get().and_then(|l| l.plan).map(|p| view! { <span class="text-muted-foreground font-semibold">{format!("({p})")}</span> })}
                        <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein class="ml-auto" attr:title="Jetzt neu lesen (höchstens alle 20 s)"
                            on:click=|_| senden(ClientNachricht::Agenten(G::LimitsAuffrischen))>"⟳"</Knopf>
                    </h3>
                    {move || {
                        let l = z.limits.get().unwrap_or_default();
                        if l.fenster.is_empty() {
                            return view! { <p class="text-muted-foreground text-sm">"Keine Limits bekannt (kein Plan oder noch nicht gelesen)."</p> }.into_any();
                        }
                        view! {
                            {l.fenster.into_iter().map(|f| {
                                let p = f.prozent.clamp(0.0, 100.0);
                                let farbe = if p >= 90.0 { "bg-destructive" } else if p >= 70.0 { "bg-warning" } else { "bg-success" };
                                view! {
                                    <div class="flex flex-col gap-0.5">
                                        <div class="flex text-xs font-bold">
                                            <span>{f.name.clone()}</span>
                                            <span class="ml-auto">{format!("{p:.0} %")}{f.neu_um.map(|u| format!(" · neu {}", bis(u)))}</span>
                                        </div>
                                        <div class="h-3 overflow-hidden rounded-full border-2 border-foreground/30 bg-muted">
                                            <div class=format!("h-full {farbe}") style=format!("width:{p}%")></div>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                            <p class="text-muted-foreground text-xs">{format!("Gelesen {}", texte::her(l.um))}</p>
                        }.into_any()
                    }}
                </section>
            </div>
        </Rahmen>
    }
}

#[component]
fn PromptKarte(p: PromptInfo) -> impl IntoView {
    let text = RwSignal::new(p.eigener.clone().unwrap_or_else(|| p.standard.clone()));
    let id = StoredValue::new(p.id.clone());
    let eigen = p.eigener.is_some();
    view! {
        <details class="rounded-lg border p-2">
            <summary class="font-bold hover:cursor-pointer">{p.titel.clone()}{eigen.then_some(" ✏️")}</summary>
            <p class="text-muted-foreground my-1 text-xs">{p.verwendet.clone()}</p>
            {(!p.platzhalter.is_empty()).then(|| view! {
                <p class="text-muted-foreground mb-1 text-xs">"Platzhalter: "<code>{p.platzhalter.join(", ")}</code></p>
            })}
            <Textfeld wert=text class="min-h-32 font-mono text-xs" />
            <div class="mt-2 flex gap-2">
                <Knopf groesse=KnopfGroesse::Klein on:click=move |_| senden(ClientNachricht::Agenten(G::PromptSetzen { id: id.get_value(), text: Some(text.get_untracked()) }))>"Speichern"</Knopf>
                <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=move |_| senden(ClientNachricht::Agenten(G::PromptSetzen { id: id.get_value(), text: None }))>"Standard wiederherstellen"</Knopf>
            </div>
        </details>
    }
}

#[component]
pub fn Prompts() -> impl IntoView {
    let z = zustand();
    view! {
        <Rahmen fenster=Fenster::Prompts titel="✏️ Prompts des Office" breite="max-w-3xl">
            <p class="text-muted-foreground mb-3 text-sm">"Diese Texte schickt das Office selbst an die Agenten. Hier kannst du sie umschreiben."</p>
            <div class="flex flex-col gap-2">
                {move || z.prompts.get().into_iter().map(|p| view! { <PromptKarte p=p /> }).collect_view()}
            </div>
        </Rahmen>
    }
}

#[component]
pub fn Dienste() -> impl IntoView {
    let z = zustand();
    let host = window()
        .location()
        .hostname()
        .unwrap_or_else(|_| "localhost".into());
    view! {
        <Rahmen fenster=Fenster::Dienste titel="🔌 Dienste" breite="max-w-xl">
            <p class="text-muted-foreground mb-3 text-sm">"Was die Arbeiter dieser Etage gestartet haben und auf einem Port lauscht (Dev-Server, APIs)."</p>
            <ul class="flex flex-col gap-1">
                {move || {
                    let l = z.dienste.get();
                    if l.is_empty() {
                        return view! { <li class="text-muted-foreground text-sm">"Gerade lauscht nichts."</li> }.into_any();
                    }
                    let host = host.clone();
                    l.into_iter().map(|d| view! {
                        <li class="flex items-center gap-2 rounded-md border px-2 py-1 text-sm">
                            <a class="text-primary font-mono font-bold hover:underline" href=format!("http://{host}:{}", d.port) target="_blank" rel="noopener noreferrer">{format!(":{}", d.port)}</a>
                            <span>{d.programm.clone()}</span>
                            <span class="text-muted-foreground ml-auto text-xs">{format!("{} · {}", d.name, texte::her(d.seit))}</span>
                        </li>
                    }).collect_view().into_any()
                }}
            </ul>
        </Rahmen>
    }
}
