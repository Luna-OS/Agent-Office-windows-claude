//! Eine Einladung annehmen: `/beitreten#<token>`. Erst ansehen (wer lädt ein, in welches Projekt),
//! dann Name und eigenes Passwort wählen; danach ist man mit eigenem Konto angemeldet.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::{Value, json};

use crate::anmeldung::ins_office;
use crate::http;
use crate::ui::{Eingabe, Karte, KarteInhalt, KarteKopf, KarteTitel, Knopf};

#[component]
pub fn Beitrittsseite(token: String) -> impl IntoView {
    let einladung = RwSignal::new(None::<Value>);
    let fehler = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let passwort = RwSignal::new(String::new());
    let laeuft = RwSignal::new(false);

    let t = token.clone();
    spawn_local(async move {
        match http::schicken("/api/beitreten", &json!({ "token": t, "passwort": "", "nur_ansehen": true })).await {
            Ok(a) if a.ok() => {
                if let Some(n) = a.json.get("name").and_then(Value::as_str) {
                    name.set(n.to_string());
                }
                einladung.set(Some(a.json));
            }
            Ok(a) if a.status == 410 => fehler.set(Some("Dieser Einladungslink ist abgelaufen oder wurde schon benutzt.".into())),
            Ok(a) => fehler.set(Some(a.fehler())),
            Err(f) => fehler.set(Some(f)),
        }
    });

    let feld = |k: &'static str| move || einladung.with(|e| e.as_ref().and_then(|e| e.get(k)).and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
    let von = feld("von");
    let projekt = feld("projekt");
    let fester_name = move || einladung.with(|e| e.as_ref().and_then(|e| e.get("name")).is_some_and(|n| n.is_string()));

    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        if laeuft.get_untracked() {
            return;
        }
        if passwort.get_untracked().len() < 6 {
            fehler.set(Some("Das Passwort braucht mindestens 6 Zeichen.".into()));
            return;
        }
        laeuft.set(true);
        fehler.set(None);
        let koerper = json!({ "token": token, "name": name.get_untracked().trim(), "passwort": passwort.get_untracked() });
        spawn_local(async move {
            match http::schicken("/api/beitreten", &koerper).await {
                Ok(a) if a.ok() => {
                    let _ = window().history().and_then(|h| h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/")));
                    ins_office();
                }
                Ok(a) if a.status == 410 => fehler.set(Some("Dieser Einladungslink ist abgelaufen.".into())),
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
                    <KarteTitel class="text-2xl">"✉️ Einladung"</KarteTitel>
                    <p class="text-muted-foreground text-sm">
                        {move || {
                            let (v, p) = (von(), projekt());
                            if v.is_empty() {
                                "Einladung wird geprüft …".to_string()
                            } else if p.is_empty() {
                                format!("{v} lädt dich ins Office ein.")
                            } else {
                                format!("{v} lädt dich ins Office zu {p} ein.")
                            }
                        }}
                    </p>
                </KarteKopf>
                <KarteInhalt>
                    <form class="flex flex-col gap-3" on:submit=absenden>
                        <label class="flex flex-col gap-1 text-sm font-semibold">
                            "Dein Name"
                            <Show when=move || !fester_name() fallback=move || view! { <span class="text-base">{move || name.get()}</span> }>
                                <Eingabe wert=name platzhalter="So sehen dich die anderen" />
                            </Show>
                        </label>
                        <label class="flex flex-col gap-1 text-sm font-semibold">
                            "Eigenes Passwort"
                            <Eingabe wert=passwort typ="password" platzhalter="Mindestens 6 Zeichen" />
                        </label>
                        <Show when=move || fehler.get().is_some()>
                            <p class="text-destructive text-sm font-semibold" role="alert">{move || fehler.get()}</p>
                        </Show>
                        <Knopf absenden=true deaktiviert=Signal::derive(move || laeuft.get() || einladung.with(Option::is_none)) class="w-full">
                            "Konto anlegen und eintreten"
                        </Knopf>
                    </form>
                </KarteInhalt>
            </Karte>
        </div>
    }
}
