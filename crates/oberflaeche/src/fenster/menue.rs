//! ☰ Das Menü (Tab): jedes Fenster des Office, nach Abschnitten. Ein neuer Eintrag ist eine Zeile in [`EINTRAEGE`].

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::modal::{self, Fenster, Rahmen};

/// Wohin ein Eintrag führt.
#[derive(Clone)]
enum Ziel {
    Fenster(Fenster),
    Lite,
    Abmelden,
}

struct Eintrag {
    abschnitt: &'static str,
    symbol: &'static str,
    name: &'static str,
    was: &'static str,
    taste: Option<&'static str>,
    ziel: fn() -> Ziel,
}

const EINTRAEGE: &[Eintrag] = &[
    Eintrag { abschnitt: "Öffnen", symbol: "📋", name: "Aufgabenboard", was: "Was ansteht, wer es macht", taste: None, ziel: || Ziel::Fenster(Fenster::Aufgaben) },
    Eintrag { abschnitt: "Öffnen", symbol: "📡", name: "Agenten", was: "Nachrichten und Status-Feed", taste: None, ziel: || Ziel::Fenster(Fenster::Agenten) },
    Eintrag { abschnitt: "Öffnen", symbol: "🐙", name: "GitHub", was: "Issues und Pull Requests", taste: None, ziel: || Ziel::Fenster(Fenster::Github) },
    Eintrag { abschnitt: "Öffnen", symbol: "🛗", name: "Etagen", was: "Ein anderes Projekt, ein neues hinzufügen", taste: None, ziel: || Ziel::Fenster(Fenster::Aufzug) },
    Eintrag { abschnitt: "Öffnen", symbol: "⏳", name: "Nutzung und Limits", was: "Verbrauch, Kosten, Claude-Limits", taste: None, ziel: || Ziel::Fenster(Fenster::Nutzung) },
    Eintrag { abschnitt: "Zusammen", symbol: "🤝", name: "Meeting", was: "Alle Arbeiter an einen Tisch", taste: None, ziel: || Ziel::Fenster(Fenster::Meeting) },
    Eintrag { abschnitt: "Zusammen", symbol: "🖥️", name: "Bildschirm teilen", was: "Deinen Bildschirm für alle", taste: None, ziel: || Ziel::Fenster(Fenster::Teilen) },
    Eintrag { abschnitt: "Zusammen", symbol: "📺", name: "Dach-Leinwand", was: "Ein YouTube-Video auf dem Dach", taste: None, ziel: || Ziel::Fenster(Fenster::DachUrl) },
    Eintrag { abschnitt: "Zusammen", symbol: "🎵", name: "Jukebox", was: "Musik für die Etage", taste: None, ziel: || Ziel::Fenster(Fenster::Jukebox) },
    Eintrag { abschnitt: "Zusammen", symbol: "👥", name: "Team", was: "Konten und Einladungen", taste: None, ziel: || Ziel::Fenster(Fenster::Team) },
    Eintrag { abschnitt: "Zusammen", symbol: "🔑", name: "Anmeldungen", was: "Dein Claude und dein GitHub", taste: None, ziel: || Ziel::Fenster(Fenster::Konten) },
    Eintrag { abschnitt: "Office", symbol: "🔎", name: "Befehlspalette", was: "Alles finden", taste: Some("Strg+K"), ziel: || Ziel::Fenster(Fenster::Palette) },
    Eintrag { abschnitt: "Öffnen", symbol: "🔌", name: "Dienste", was: "Dev-Server der Arbeiter", taste: None, ziel: || Ziel::Fenster(Fenster::Dienste) },
    Eintrag { abschnitt: "Office", symbol: "✏️", name: "Prompts", was: "Was das Office den Agenten schreibt", taste: None, ziel: || Ziel::Fenster(Fenster::Prompts) },
    Eintrag { abschnitt: "Office", symbol: "⚙️", name: "Einstellungen", was: "Ansicht, Rechner, Updates", taste: None, ziel: || Ziel::Fenster(Fenster::Einstellungen) },
    Eintrag { abschnitt: "Office", symbol: "🎮", name: "Steuerung", was: "Alle Tasten", taste: Some("F1"), ziel: || Ziel::Fenster(Fenster::Hilfe) },
    Eintrag { abschnitt: "Office", symbol: "📱", name: "2D-Ansicht", was: "Das Office ohne 3D, fürs Handy", taste: None, ziel: || Ziel::Lite },
    Eintrag { abschnitt: "Office", symbol: "🚪", name: "Abmelden", was: "Diesen Browser abmelden", taste: None, ziel: || Ziel::Abmelden },
];

fn ausfuehren(ziel: Ziel) {
    modal::schliessen(&Fenster::Menue);
    match ziel {
        Ziel::Fenster(f) => modal::oeffnen(f),
        Ziel::Lite => {
            let _ = window().location().set_href("/lite");
        }
        Ziel::Abmelden => spawn_local(async {
            let _ = crate::http::schicken("/api/abmelden", &serde_json::json!({})).await;
            let _ = window().location().reload();
        }),
    }
}

#[component]
pub fn Menue() -> impl IntoView {
    let abschnitte = ["Öffnen", "Zusammen", "Office"];
    view! {
        <Rahmen fenster=Fenster::Menue titel="☰ Menü" breite="max-w-3xl">
            <div class="grid gap-4 sm:grid-cols-3">
                {abschnitte.into_iter().map(|a| view! {
                    <section class="flex flex-col gap-1">
                        <h3 class="text-muted-foreground px-2 text-xs font-extrabold tracking-wide uppercase">{a}</h3>
                        {EINTRAEGE.iter().filter(|e| e.abschnitt == a).map(|e| {
                            let ziel = e.ziel;
                            view! {
                                <button
                                    type="button"
                                    class="flex items-center gap-3 rounded-lg px-2 py-2 text-left hover:cursor-pointer hover:bg-accent"
                                    on:click=move |_| ausfuehren(ziel())
                                >
                                    <span class="text-2xl">{e.symbol}</span>
                                    <span class="flex min-w-0 flex-1 flex-col">
                                        <span class="font-bold">{e.name}</span>
                                        <span class="text-muted-foreground truncate text-xs">{e.was}</span>
                                    </span>
                                    {e.taste.map(|t| view! { <kbd class="rounded border bg-secondary px-1.5 font-mono text-[11px]">{t}</kbd> })}
                                </button>
                            }
                        }).collect_view()}
                    </section>
                }).collect_view()}
            </div>
        </Rahmen>
    }
}
