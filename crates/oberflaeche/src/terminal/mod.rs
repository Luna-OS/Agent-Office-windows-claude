//! Das Terminal eines Arbeiters: ein vt100-Parser im WASM ([`sitzung`]), als HTML gezeichnet
//! ([`bild`]), mit Tastatur ([`tasten`]), Maus und Scrollen ([`maus`]) und Reitern für Webseiten ([`reiter`]).
//!
//! Den PTY teilen sich alle Zuschauer; wer tippt, bekommt ihn in seiner Fenstergröße. Cursor-Abfragen
//! (`ESC[6n`) beantwortet der Server, die Ansicht hier antwortet nie selbst.

mod bild;
mod kopf;
mod maus;
mod reiter;
mod sitzung;
mod tastenfeld;
pub mod tasten;

use leptos::prelude::*;
use protokoll::{ClientNachricht, arbeiter::Client as A};
use wasm_bindgen::JsCast;

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::zustand::zustand;

pub use sitzung::{daten, groesse_setzen as groesse, snapshot};

/// Tippt etwas in das Terminal eines Arbeiters.
pub fn eingeben(arbeiter: &str, daten: String) {
    sitzung::ans_ende(arbeiter);
    senden(ClientNachricht::Arbeiter(A::Eingabe { arbeiter: arbeiter.to_string(), daten }));
}

/// Wie groß eine Zeichenzelle ist (px), gemessen an einem versteckten Muster im Terminal.
fn zellgroesse(el: &web_sys::HtmlElement) -> (f64, f64) {
    let doc = document();
    let Ok(probe) = doc.create_element("span") else { return (8.0, 16.0) };
    probe.set_text_content(Some("MMMMMMMMMM"));
    let _ = probe.set_attribute("style", "position:absolute;visibility:hidden;white-space:pre");
    let _ = el.append_child(&probe);
    let r = probe.get_bounding_client_rect();
    probe.remove();
    if r.width() > 0.0 { (r.width() / 10.0, r.height()) } else { (8.0, 16.0) }
}

/// Spalten und Zeilen, die in den Bereich passen.
fn passend(el: &web_sys::HtmlElement) -> (u16, u16) {
    let (b, h) = zellgroesse(el);
    let r = el.get_bounding_client_rect();
    let spalten = ((r.width() - 16.0) / b).floor().clamp(20.0, 400.0) as u16;
    let zeilen = ((r.height() - 12.0) / h).floor().clamp(5.0, 200.0) as u16;
    (spalten, zeilen)
}

#[component]
pub fn TerminalFenster(arbeiter: String) -> impl IntoView {
    let z = zustand();
    let id = StoredValue::new(arbeiter.clone());
    let info = Memo::new(move |_| z.arbeiter_mit(&id.get_value()));
    let stand = sitzung::stand(&arbeiter);
    let fokus = RwSignal::new(false);
    let flaeche = NodeRef::<leptos::html::Div>::new();
    let mac = tasten::ist_mac();
    // Die Größe, die wir zuletzt verlangt haben (damit ein fremdes Resize uns nicht in eine Schleife zieht).
    let verlangt = StoredValue::new((0u16, 0u16));

    senden(ClientNachricht::Arbeiter(A::Anschauen { arbeiter: arbeiter.clone() }));
    on_cleanup(move || {
        let a = id.get_value();
        senden(ClientNachricht::Arbeiter(A::Wegschauen { arbeiter: a.clone() }));
        sitzung::vergessen(&a);
    });

    // Den PTY auf diese Fenstergröße bringen (beim Öffnen, beim Tippen nach fremdem Resize, bei Fenster-Resize).
    let anpassen = move || {
        let Some(el) = flaeche.get_untracked() else { return };
        let (spalten, zeilen) = passend(&el);
        let pty = info.with_untracked(|i| i.as_ref().map(|i| (i.spalten, i.zeilen)));
        if pty != Some((spalten, zeilen)) && verlangt.get_value() != (spalten, zeilen) {
            verlangt.set_value((spalten, zeilen));
            senden(ClientNachricht::Arbeiter(A::Groesse { arbeiter: id.get_value(), spalten, zeilen }));
        }
    };
    Effect::new(move |_| {
        if let Some(el) = flaeche.get() {
            let _ = el.focus();
            set_timeout(anpassen, std::time::Duration::from_millis(30));
        }
    });
    let groesse_weg = window_event_listener(leptos::ev::resize, move |_| {
        verlangt.set_value((0, 0));
        anpassen();
    });
    on_cleanup(move || groesse_weg.remove());

    let taste = move |e: web_sys::KeyboardEvent| {
        let key = e.key();
        // Shift+Bild auf/ab: im Verlauf blättern, nicht an das Programm.
        if e.shift_key() && (key == "PageUp" || key == "PageDown") {
            e.prevent_default();
            let seite = info.with_untracked(|i| i.as_ref().map_or(20, |i| i.zeilen as i32 - 2));
            sitzung::scrollen(&id.get_value(), if key == "PageUp" { seite } else { -seite });
            return;
        }
        // Kopieren mit Strg+Shift+C, oder Strg+C, solange etwas markiert ist; Einfügen geht über das paste-Ereignis.
        let markiert = window().get_selection().ok().flatten().is_some_and(|s| !s.is_collapsed());
        let strg_oder_cmd = e.ctrl_key() || e.meta_key();
        if strg_oder_cmd && key.eq_ignore_ascii_case("c") && (markiert || e.shift_key()) {
            return;
        }
        if strg_oder_cmd && key.eq_ignore_ascii_case("v") {
            return;
        }
        let anwendung = sitzung::mit(&id.get_value(), |s| s.parser.screen().application_cursor()).unwrap_or(false);
        let t = tasten::Taste { key: &key, strg: e.ctrl_key(), alt: e.alt_key(), shift: e.shift_key(), meta: e.meta_key() };
        if let Some(b) = tasten::bytes(&t, anwendung, mac) {
            e.prevent_default();
            e.stop_propagation();
            anpassen();
            eingeben(&id.get_value(), b);
        }
    };

    let einfuegen = move |e: web_sys::ClipboardEvent| {
        let Some(text) = e.clipboard_data().and_then(|d| d.get_data("text").ok()) else { return };
        e.prevent_default();
        let klammern = sitzung::mit(&id.get_value(), |s| s.parser.screen().bracketed_paste()).unwrap_or(false);
        eingeben(&id.get_value(), tasten::einfuegen(&text, klammern));
    };

    let titel = Signal::derive(move || info.with(|i| i.as_ref().map(|i| i.name.clone())).unwrap_or_else(|| "Terminal".into()));
    let gewaehlt = reiter::gewaehlt(&arbeiter);
    let kopf_id = arbeiter.clone();

    view! {
        <Rahmen
            fenster=Fenster::Terminal(arbeiter.clone())
            titel=titel
            breite="max-w-[min(96vw,1400px)]"
            kopf=view! { <kopf::Kopf arbeiter=kopf_id info=info /> }.into_any()
        >
            <reiter::Leiste arbeiter=arbeiter.clone() />
            <div class=if crate::lite::ist_lite() { "relative h-[55vh]" } else { "relative h-[70vh]" }>
                <div
                    node_ref=flaeche
                    class="terminal absolute inset-0 overflow-hidden rounded-lg p-2 outline-none"
                    class:hidden=move || gewaehlt.get().is_some()
                    style=format!("background:{};color:{}", bild::HINTERGRUND, bild::SCHRIFT)
                    tabindex="0"
                    on:keydown=taste
                    on:paste=einfuegen
                    on:focus=move |_| fokus.set(true)
                    on:blur=move |_| fokus.set(false)
                    on:wheel=move |e| maus::rad(&id.get_value(), &e, flaeche)
                    on:mousedown=move |e| maus::knopf(&id.get_value(), &e, flaeche, true)
                    on:mouseup=move |e| maus::knopf(&id.get_value(), &e, flaeche, false)
                    on:dragover=|e| e.prevent_default()
                    on:drop=move |e: web_sys::DragEvent| maus::ablegen(&id.get_value(), e)
                    inner_html=move || {
                        stand.get();
                        let f = fokus.get();
                        sitzung::mit(&id.get_value(), |s| bild::html(s.parser.screen(), f)).unwrap_or_default()
                    }
                ></div>
                <reiter::Seiten arbeiter=arbeiter.clone() />
            </div>
            {(crate::lite::ist_lite() || tastenfeld::beruehrung()).then(|| view! { <tastenfeld::Tastenfeld arbeiter=arbeiter.clone() /> })}
            <Show when=move || info.with(Option::is_none)>
                <p class="text-muted-foreground mt-2 text-sm">"Dieser Arbeiter ist nicht mehr da."</p>
            </Show>
        </Rahmen>
    }
}

/// Fokus zurück ins Terminal (nach einem Klick auf einen Knopf im Kopf).
pub(crate) fn fokus_zurueck() {
    if let Some(el) = document().query_selector(".terminal").ok().flatten()
        && let Ok(el) = el.dyn_into::<web_sys::HtmlElement>()
    {
        let _ = el.focus();
    }
}
