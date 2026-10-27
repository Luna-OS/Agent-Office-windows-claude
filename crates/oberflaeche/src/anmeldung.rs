//! Die Anmeldeseite, der Einmal-Link (`/anmelden#schluessel=…`) und das Prüfen beim Start.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;
use serde_json::json;

use crate::http;
use crate::ui::{Eingabe, Karte, KarteInhalt, KarteKopf, KarteTitel, Knopf};
use crate::zustand::{Phase, zustand};

/// Was `GET /api/anmeldung` sagt.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct AnmeldeInfo {
    #[serde(default)]
    pub angemeldet: bool,
    /// Es gibt Konten: Namensfeld zeigen.
    #[serde(default)]
    pub konten: bool,
    /// Das gemeinsame Passwort gilt.
    #[serde(default)]
    pub gemeinsam: bool,
    /// Kein Passwort nötig.
    #[serde(default)]
    pub offen: bool,
}

/// Beim Start und nach jedem Verbindungsabbruch: angemeldet? Dann verbinden, sonst die Anmeldeseite.
pub fn pruefen() {
    spawn_local(async move {
        let z = zustand();
        let ort = window().location();
        let pfad = ort.pathname().unwrap_or_default();
        let hash = ort.hash().unwrap_or_default();
        if pfad.starts_with("/beitreten") && hash.len() > 1 {
            z.phase.set(Phase::Beitreten(hash[1..].to_string()));
            return;
        }
        if let Some(schluessel) = hash.strip_prefix("#schluessel=") {
            let _ = http::schicken("/api/link", &json!({ "schluessel": schluessel })).await;
            let _ = window().history().and_then(|h| h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/")));
        }
        match http::holen("/api/anmeldung").await {
            Ok(a) => {
                let info: AnmeldeInfo = serde_json::from_value(a.json).unwrap_or_default();
                if info.angemeldet {
                    ins_office();
                } else if info.offen && http::schicken("/api/anmeldung", &json!({ "passwort": "" })).await.is_ok_and(|a| a.ok()) {
                    ins_office();
                } else {
                    z.phase.set(Phase::Anmelden(info));
                }
            }
            // Server weg: gleich noch einmal.
            Err(_) => {
                set_timeout(pruefen, std::time::Duration::from_secs(2));
            }
        }
    });
}

/// Angemeldet: WebSocket aufbauen.
pub(crate) fn ins_office() {
    zustand().phase.set(Phase::Office);
    crate::bruecke::verbinden();
}

/// Die Seite mit Name und Passwort.
#[component]
pub fn Anmeldeseite(info: AnmeldeInfo) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let passwort = RwSignal::new(String::new());
    let fehler = RwSignal::new(None::<String>);
    let laeuft = RwSignal::new(false);
    let mit_name = info.konten;

    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if laeuft.get_untracked() {
            return;
        }
        laeuft.set(true);
        fehler.set(None);
        let mut koerper = json!({ "passwort": passwort.get_untracked() });
        let n = name.get_untracked();
        if mit_name && !n.trim().is_empty() {
            koerper["name"] = json!(n.trim());
        }
        spawn_local(async move {
            match http::schicken("/api/anmeldung", &koerper).await {
                Ok(a) if a.ok() => ins_office(),
                Ok(a) => fehler.set(Some(a.fehler())),
                Err(f) => fehler.set(Some(f)),
            }
            laeuft.set(false);
        });
    };

    view! {
        <div class="anmeldung fixed inset-0 z-[60] flex items-center justify-center p-4">
            <Karte class="w-full max-w-sm border-[3px] border-foreground shadow-[0_5px_0_var(--foreground)]">
                <KarteKopf>
                    <KarteTitel class="text-2xl">"🏢 OpenAgenticOffice"</KarteTitel>
                    <p class="text-muted-foreground text-sm">
                        {if mit_name { "Melde dich mit deinem Namen und Passwort an." } else { "Gib das Passwort des Office ein." }}
                    </p>
                </KarteKopf>
                <KarteInhalt>
                    <form class="flex flex-col gap-3" on:submit=absenden>
                        <Show when=move || mit_name>
                            <label class="flex flex-col gap-1 text-sm font-semibold">
                                "Name"
                                <Eingabe wert=name platzhalter="Dein Name (leer: gemeinsames Passwort)" />
                            </label>
                        </Show>
                        <label class="flex flex-col gap-1 text-sm font-semibold">
                            "Passwort"
                            <Eingabe wert=passwort typ="password" platzhalter="Passwort" />
                        </label>
                        <Show when=move || fehler.get().is_some()>
                            <p class="text-destructive text-sm font-semibold" role="alert">{move || fehler.get()}</p>
                        </Show>
                        <Knopf absenden=true deaktiviert=laeuft class="w-full">
                            {move || if laeuft.get() { "Melde an …" } else { "Anmelden" }}
                        </Knopf>
                    </form>
                </KarteInhalt>
            </Karte>
        </div>
    }
}
