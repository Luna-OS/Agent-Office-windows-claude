//! 🔑 Anmeldungen: das eigene Claude und GitHub für die eigenen Arbeiter (nur mit eigenem Konto),
//! und ein Einmal-Link, um sich in einem anderen Browser anzumelden.

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::konten::{AnmeldeArt, AnmeldeStatus, AnmeldeWeg, Anmeldung, Client as K};

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};
use crate::zustand::zustand;

fn schicken(k: K) {
    senden(ClientNachricht::Konten(k));
}

thread_local! {
    static LINK: OnceCell<RwSignal<Option<String>>> = const { OnceCell::new() };
}

fn link() -> RwSignal<Option<String>> {
    LINK.with(|l| *l.get_or_init(|| RwSignal::new(None)))
}

/// Antwort auf `LinkErzeugen`.
pub fn link_erhalten(pfad: &str) {
    let voll = format!("{}{pfad}", window().location().origin().unwrap_or_default());
    crate::fenster::team::kopieren(&voll);
    link().set(Some(voll));
}

fn weg_name(w: AnmeldeWeg) -> &'static str {
    match w {
        AnmeldeWeg::Login => "eigener Login",
        AnmeldeWeg::Token => "Token",
        AnmeldeWeg::Office => "Anmeldung des Office-Rechners",
    }
}

#[component]
fn Karte(art: AnmeldeArt, a: Anmeldung, office: bool) -> impl IntoView {
    let (symbol, name) = match art {
        AnmeldeArt::Claude => ("✳️", "Claude"),
        AnmeldeArt::Github => ("🐙", "GitHub"),
    };
    let token = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let offen = a.offen.clone();
    view! {
        <section class="flex flex-col gap-2 rounded-xl border p-3">
            <h3 class="flex items-center gap-2 font-extrabold">
                {symbol}" "{name}
                <span class="text-muted-foreground ml-auto text-xs font-semibold">{match a.status {
                    AnmeldeStatus::Ok => format!("✅ {} ({})", a.wer.clone().unwrap_or_default(), weg_name(a.weg)),
                    AnmeldeStatus::Laeuft => "⏳ läuft …".into(),
                    AnmeldeStatus::Keine => "nicht angemeldet".into(),
                }}</span>
            </h3>
            {a.fehler.clone().map(|f| view! { <p class="text-sm text-destructive">{f}</p> })}
            {offen.map(|o| view! {
                <div class="flex flex-col gap-2 rounded-md bg-secondary p-2 text-sm">
                    {o.url.clone().map(|u| view! { <a class="text-primary font-bold hover:underline" href=u target="_blank" rel="noopener noreferrer">"Anmeldeseite öffnen ↗"</a> })}
                    {o.code.clone().map(|c| view! { <p>"Code eingeben: "<b class="font-mono text-base">{c}</b></p> })}
                    {(art == AnmeldeArt::Claude && !o.geschickt).then(|| view! {
                        <form class="flex gap-2" on:submit=move |e| { e.prevent_default(); schicken(K::AnmeldungCode { code: code.get_untracked().trim().to_string() }); }>
                            <Eingabe wert=code platzhalter="Code von der Anmeldeseite einfügen" />
                            <Knopf absenden=true groesse=KnopfGroesse::Klein>"Senden"</Knopf>
                        </form>
                    })}
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein class="self-start" on:click=move |_| schicken(K::AnmeldungAbbrechen { art })>"Abbrechen"</Knopf>
                </div>
            })}
            <div class="flex flex-wrap gap-2">
                {(a.status != AnmeldeStatus::Laeuft).then(|| view! {
                    <Knopf groesse=KnopfGroesse::Klein on:click=move |_| schicken(K::AnmeldungStarten { art })>"Anmelden"</Knopf>
                })}
                {office.then(|| view! {
                    <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| schicken(K::AnmeldungOffice { art })>"Die des Office nutzen"</Knopf>
                })}
                {(a.status == AnmeldeStatus::Ok).then(|| view! {
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=move |_| schicken(K::Abmelden { art })>"Abmelden"</Knopf>
                })}
            </div>
            <form class="flex gap-2" on:submit=move |e| {
                e.prevent_default();
                let t = token.get_untracked().trim().to_string();
                if !t.is_empty() {
                    schicken(K::AnmeldungToken { art, token: t });
                    token.set(String::new());
                }
            }>
                <Eingabe wert=token typ="password" platzhalter="… oder einen Token einfügen" />
                <Knopf absenden=true art=KnopfArt::Umriss groesse=KnopfGroesse::Klein>"Nutzen"</Knopf>
            </form>
        </section>
    }
}

#[component]
pub fn Konten() -> impl IntoView {
    let z = zustand();
    let mit_konto = move || z.ich.with(|i| i.konto.is_some());
    schicken(K::AnmeldungenHolen);
    view! {
        <Rahmen fenster=Fenster::Konten titel="🔑 Anmeldungen" breite="max-w-2xl">
            <div class="flex flex-col gap-3">
                <Show when=mit_konto fallback=|| view! {
                    <p class="text-muted-foreground text-sm">"Eigene Anmeldungen gibt es nur mit eigenem Konto. Mit dem gemeinsamen Passwort arbeiten die Arbeiter mit der Anmeldung des Office-Rechners."</p>
                }>
                    <p class="text-muted-foreground text-sm">"Deine Arbeiter nutzen dein eigenes Claude und dein GitHub."</p>
                    {move || match z.anmeldungen.get() {
                        None => view! { <p class="text-muted-foreground text-sm">"Lädt …"</p> }.into_any(),
                        Some(s) => view! {
                            <Karte art=AnmeldeArt::Claude a=s.claude.clone() office=s.office_claude />
                            <Karte art=AnmeldeArt::Github a=s.github.clone() office=s.office_github />
                        }.into_any(),
                    }}
                </Show>
                <section class="flex flex-col gap-2 rounded-xl border p-3">
                    <h3 class="font-extrabold">"📱 In einem anderen Browser anmelden"</h3>
                    <p class="text-muted-foreground text-sm">"Ein Link, der fünf Minuten lang einmal ohne Passwort hereinlässt."</p>
                    <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein class="self-start" on:click=|_| schicken(K::LinkErzeugen)>"Link erzeugen"</Knopf>
                    {move || link().get().map(|l| view! { <p class="rounded-md bg-success/15 p-2 font-mono text-xs break-all">{l}</p> })}
                </section>
            </div>
        </Rahmen>
    }
}
