//! 📚 Das Bücherregal: die Markdown-Dokumente des Projekts der Etage (README, docs/ …) zum Lesen.
//! Liste und Inhalt kommen über HTTP (`/api/buecher`, `/api/buch`); Bilder über `/api/buch/bild`.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::modal::{Fenster, Rahmen};
use crate::zustand::zustand;

#[derive(Clone, Deserialize)]
struct Buch {
    pfad: String,
    titel: String,
}

#[derive(Clone, Deserialize, Default)]
struct Liste {
    #[serde(default)]
    buecher: Vec<Buch>,
    #[serde(default)]
    gekuerzt: bool,
}

fn kodieren(s: &str) -> String {
    js_sys::encode_uri_component(s).into()
}

/// Den Text eines Buchs holen (Markdown, kein JSON).
async fn text_holen(url: &str) -> Result<String, String> {
    let fenster = web_sys::window().ok_or("Kein Browserfenster.")?;
    let antwort: web_sys::Response = JsFuture::from(fenster.fetch_with_str(url))
        .await
        .map_err(|_| "Der Server ist nicht erreichbar.")?
        .unchecked_into();
    if !antwort.ok() {
        return Err(format!(
            "Das Buch ließ sich nicht laden (Status {}).",
            antwort.status()
        ));
    }
    let p = antwort.text().map_err(|_| "Kein Text.")?;
    Ok(JsFuture::from(p)
        .await
        .ok()
        .and_then(|t| t.as_string())
        .unwrap_or_default())
}

#[component]
pub fn Buecher() -> impl IntoView {
    let z = zustand();
    let etage = z.etage.get_untracked().unwrap_or_default();
    let liste = RwSignal::new(None::<Result<Liste, String>>);
    let offen = RwSignal::new(None::<(String, String, Result<String, String>)>);
    let e = etage.clone();
    spawn_local(async move {
        let r = match crate::http::holen(&format!("/api/buecher?etage={}", kodieren(&e))).await {
            Ok(a) if a.ok() => serde_json::from_value::<Liste>(a.json)
                .map_err(|_| "Unerwartete Antwort.".to_string()),
            Ok(a) => Err(a.fehler()),
            Err(f) => Err(f),
        };
        liste.set(Some(r));
    });
    let etage = StoredValue::new(etage);
    let lesen = move |b: Buch| {
        offen.set(Some((b.titel.clone(), b.pfad.clone(), Ok("Lädt …".into()))));
        spawn_local(async move {
            let url = format!(
                "/api/buch?etage={}&pfad={}",
                kodieren(&etage.get_value()),
                kodieren(&b.pfad)
            );
            let t = text_holen(&url).await;
            offen.set(Some((b.titel, b.pfad, t)));
        });
    };
    view! {
        <Rahmen fenster=Fenster::Buecher titel="📚 Bücherregal" breite="max-w-5xl">
            <div class="grid h-[70vh] gap-4 md:grid-cols-[16rem_1fr]">
                <ul class="flex min-h-0 flex-col gap-0.5 overflow-auto">
                    {move || match liste.get() {
                        None => view! { <li class="text-muted-foreground text-sm">"Lädt …"</li> }.into_any(),
                        Some(Err(f)) => view! { <li class="text-sm text-destructive">{f}</li> }.into_any(),
                        Some(Ok(l)) if l.buecher.is_empty() => view! { <li class="text-muted-foreground text-sm">"Keine Dokumente in diesem Projekt."</li> }.into_any(),
                        Some(Ok(l)) => view! {
                            {l.buecher.into_iter().map(|b| {
                                let titel = b.titel.clone();
                                let pfad = b.pfad.clone();
                                view! {
                                    <li><button type="button" class="flex w-full flex-col rounded-md px-2 py-1 text-left hover:cursor-pointer hover:bg-accent" on:click=move |_| lesen(b.clone())>
                                        <span class="truncate text-sm font-bold">{titel}</span>
                                        <span class="text-muted-foreground truncate font-mono text-[11px]">{pfad}</span>
                                    </button></li>
                                }
                            }).collect_view()}
                            {l.gekuerzt.then(|| view! { <li class="text-muted-foreground text-xs">"… und weitere"</li> })}
                        }.into_any(),
                    }}
                </ul>
                <article class="min-h-0 overflow-auto rounded-lg border bg-card p-4">
                    {move || match offen.get() {
                        None => view! { <p class="text-muted-foreground">"Links ein Buch wählen."</p> }.into_any(),
                        Some((titel, pfad, inhalt)) => view! {
                            <h3 class="mb-2 text-lg font-extrabold">{titel}</h3>
                            {match inhalt {
                                Ok(t) => {
                                    // Bilder liegen relativ zum Buch im Projekt.
                                    let ordner = pfad.rsplit_once('/').map(|(o, _)| format!("{o}/")).unwrap_or_default();
                                    let e = etage.get_value();
                                    let basis: crate::markdown::BildBasis = Box::new(move |rel: &str| {
                                        format!("/api/buch/bild?etage={}&pfad={}", kodieren(&e), kodieren(&format!("{ordner}{}", rel.trim_start_matches("./"))))
                                    });
                                    view! { <crate::markdown::Markdown text=t bild_basis=basis /> }.into_any()
                                }
                                Err(f) => view! { <p class="text-destructive">{f}</p> }.into_any(),
                            }}
                        }.into_any(),
                    }}
                </article>
            </div>
        </Rahmen>
    }
}
