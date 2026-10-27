//! Medien-Fenster: 📺 die YouTube-Leinwand auf dem Dach, 🖥️ Bildschirm teilen (und einen geteilten
//! Bildschirm groß ansehen) und 🎵 die Jukebox.

use leptos::prelude::*;
use protokoll::{ClientNachricht, dach, medien as m};

use crate::bruecke::{Befehl, befehl, senden};
use crate::modal::{self, Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};
use crate::zustand::zustand;

fn dach(c: dach::Client) {
    senden(ClientNachricht::Dach(c));
}

#[component]
pub fn DachUrl() -> impl IntoView {
    let z = zustand();
    let url = RwSignal::new(String::new());
    let fehler = RwSignal::new(None::<String>);
    let feld = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(f) = feld.get() {
            let _ = f.focus();
        }
    });
    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let u = url.get_untracked();
        match medien::youtube::video_aus_url(&u) {
            Ok(_) => {
                dach(dach::Client::VideoSetzen { url: u.trim().to_string() });
                modal::schliessen(&Fenster::DachUrl);
            }
            Err(f) => fehler.set(Some(f)),
        }
    };
    let l = move || z.leinwand.get();
    view! {
        <Rahmen fenster=Fenster::DachUrl titel="📺 Dach-Leinwand" breite="max-w-lg">
            <div class="flex flex-col gap-3">
                <p class="text-muted-foreground text-sm">"Ein YouTube-Video für alle auf dem Dach, synchron für jeden."</p>
                <form class="flex gap-2" on:submit=absenden>
                    <Eingabe node_ref=feld wert=url platzhalter="https://youtube.com/watch?v=… oder youtu.be/…" />
                    <Knopf absenden=true>"Abspielen"</Knopf>
                </form>
                {move || fehler.get().map(|f| view! { <p class="text-sm font-semibold text-destructive">{f}</p> })}
                {move || l().video_id.map(|id| {
                    let laeuft = l().laeuft;
                    view! {
                        <div class="flex flex-wrap items-center gap-2 rounded-lg border p-2 text-sm">
                            <span class="font-mono">{format!("▶ {id}")}</span>
                            {l().von.map(|v| view! { <span class="text-muted-foreground text-xs">{format!("von {v}")}</span> })}
                            <span class="ml-auto flex gap-1">
                                {if laeuft {
                                    view! { <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=|_| dach(dach::Client::Pausieren { position_s: None })>"⏸ Pause"</Knopf> }.into_any()
                                } else {
                                    view! { <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=|_| dach(dach::Client::Abspielen { position_s: None })>"▶ Weiter"</Knopf> }.into_any()
                                }}
                                <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=|_| dach(dach::Client::Leeren)>"⏹ Leeren"</Knopf>
                            </span>
                        </div>
                    }
                })}
            </div>
        </Rahmen>
    }
}

#[component]
pub fn Teilen() -> impl IntoView {
    let z = zustand();
    let du = z.du.get_untracked().unwrap_or_default();
    view! {
        <Rahmen fenster=Fenster::Teilen titel="🖥️ Bildschirm teilen" breite="max-w-2xl">
            <div class="flex flex-col gap-3">
                <p class="text-muted-foreground text-sm">"Teile deinen Bildschirm mit allen auf der Etage: er läuft auf dem Fernseher und ist hier groß zu sehen."</p>
                {move || if z.teilt.get() {
                    view! { <Knopf art=KnopfArt::Gefahr class="self-start" on:click=|_| befehl(Befehl::TeilenStoppen)>"⏹ Teilen beenden"</Knopf> }.into_any()
                } else {
                    view! { <Knopf class="self-start" on:click=|_| befehl(Befehl::TeilenStarten)>"🖥️ Meinen Bildschirm teilen"</Knopf> }.into_any()
                }}
                <h3 class="text-sm font-extrabold">"Wer gerade teilt"</h3>
                <ul class="flex flex-col gap-1">
                    {move || {
                        let l = z.bildschirme.get();
                        if l.is_empty() {
                            return view! { <li class="text-muted-foreground text-sm">"Niemand teilt gerade."</li> }.into_any();
                        }
                        l.into_iter().map(|b| {
                            let spieler = b.spieler.clone();
                            let ich = b.spieler == du;
                            view! {
                                <li class="flex items-center gap-2 rounded-md border px-2 py-1 text-sm">
                                    <span class="font-bold">{b.name.clone()}{ich.then_some(" (du)")}</span>
                                    <span class="text-muted-foreground text-xs">{format!("seit {}", texte::her(b.seit_ms))}</span>
                                    <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein class="ml-auto" on:click=move |_| modal::oeffnen(Fenster::Bildschirm(spieler.clone()))>"Ansehen"</Knopf>
                                </li>
                            }
                        }).collect_view().into_any()
                    }}
                </ul>
            </div>
        </Rahmen>
    }
}

/// Ein geteilter Bildschirm, groß: das Bild ist ein MJPEG-Strom, den der Browser selbst abspielt.
#[component]
pub fn Bildschirm(spieler: String) -> impl IntoView {
    let z = zustand();
    let s = spieler.clone();
    let b = Memo::new(move |_| z.bildschirme.with(|l| l.iter().find(|b| b.spieler == s).cloned()));
    let du = z.du.get_untracked().unwrap_or_default();
    let titel = Signal::derive(move || b.with(|b| b.as_ref().map(|b| format!("🖥️ {}", b.name))).unwrap_or_else(|| "🖥️ Bildschirm".into()));
    view! {
        <Rahmen fenster=Fenster::Bildschirm(spieler.clone()) titel=titel breite="max-w-[min(96vw,1600px)]">
            {move || match b.get() {
                Some(b) => {
                    let trenner = if b.stream_url.contains('?') { '&' } else { '?' };
                    view! { <img class="mx-auto max-h-[78vh] max-w-full rounded-lg bg-black" alt=format!("Bildschirm von {}", b.name) src=format!("{}{trenner}verbindung={du}", b.stream_url) /> }.into_any()
                }
                None => view! { <p class="text-muted-foreground">"Hier wird nicht mehr geteilt."</p> }.into_any(),
            }}
        </Rahmen>
    }
}

#[component]
pub fn Jukebox() -> impl IntoView {
    let z = zustand();
    let url = RwSignal::new(String::new());
    let spielen = |stueck: Option<String>, url: Option<String>| senden(ClientNachricht::Medien(m::Client::JukeboxSpielen { stueck, url }));
    let laeuft = move || z.jukebox.with(|j| j.as_ref().filter(|j| j.an).map(|j| (j.stueck.clone(), j.von.clone())));
    view! {
        <Rahmen fenster=Fenster::Jukebox titel="🎵 Jukebox" breite="max-w-lg">
            <div class="flex flex-col gap-3">
                <p class="text-sm">{move || match laeuft() {
                    Some((s, von)) => format!("Läuft: {}{}", medien::jukebox::STUECKE.iter().find(|x| x.id == s).map_or("Stream", |x| x.titel), von.map(|v| format!(" (von {v})")).unwrap_or_default()),
                    None => "Die Jukebox ist aus.".into(),
                }}</p>
                <ul class="flex flex-col gap-1">
                    {medien::jukebox::STUECKE.iter().map(|s| view! {
                        <li><button type="button" class="flex w-full flex-col rounded-md px-2 py-1.5 text-left hover:cursor-pointer hover:bg-accent"
                            class:bg-accent=move || laeuft().is_some_and(|(id, _)| id == s.id)
                            on:click=move |_| spielen(Some(s.id.to_string()), None)>
                            <span class="font-bold">{s.titel}</span>
                            <span class="text-muted-foreground text-xs">{s.stimmung}</span>
                        </button></li>
                    }).collect_view()}
                </ul>
                <form class="flex gap-2" on:submit=move |e| { e.prevent_default(); let u = url.get_untracked().trim().to_string(); if !u.is_empty() { spielen(None, Some(u)); } }>
                    <Eingabe wert=url platzhalter="… oder eine Radio-/Stream-Adresse" />
                    <Knopf absenden=true art=KnopfArt::Umriss>"Auflegen"</Knopf>
                </form>
                <div class="flex gap-2">
                    <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=|_| senden(ClientNachricht::Medien(m::Client::JukeboxWeiter))>"⏭ Nächstes"</Knopf>
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=|_| senden(ClientNachricht::Medien(m::Client::JukeboxStopp))>"⏹ Aus"</Knopf>
                </div>
            </div>
        </Rahmen>
    }
}
