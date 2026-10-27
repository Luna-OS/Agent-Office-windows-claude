//! 📝 Das Whiteboard der Etage: zusammen zeichnen, live. Die Elemente sind im Excalidraw-Format
//! ([`elemente`]), gezeichnet auf ein Canvas ([`zeichnen`]); jede Änderung geht an alle auf der Etage.

mod elemente;
mod zeichnen;

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::spielzeug::{Client as S, Server, WbZeiger};
use serde_json::Value;
use wasm_bindgen::JsCast;

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::zustand::zustand;
use elemente::Werkzeug;
use zeichnen::Ansicht;

#[derive(Clone, Copy)]
struct Tafel {
    /// Zählt hoch, wenn neu gezeichnet werden muss.
    stand: RwSignal<u64>,
    leute: RwSignal<Vec<String>>,
}

thread_local! {
    static TAFEL: OnceCell<Tafel> = const { OnceCell::new() };
    static ELEMENTE: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    static ZEIGER: RefCell<HashMap<String, (f64, f64)>> = RefCell::new(HashMap::new());
}

fn tafel() -> Tafel {
    TAFEL.with(|t| *t.get_or_init(|| Tafel { stand: RwSignal::new(0), leute: RwSignal::new(Vec::new()) }))
}

fn neu_zeichnen() {
    tafel().stand.update(|n| *n += 1);
}

/// Spielzeug-Frames, die das Whiteboard betreffen.
pub fn empfangen(m: &Server) {
    match m {
        Server::Stand { stand } => {
            ELEMENTE.with_borrow_mut(|e| *e = stand.whiteboard.clone());
            tafel().leute.set(stand.whiteboard_leute.clone());
            neu_zeichnen();
        }
        Server::WbAenderung { elemente } => {
            ELEMENTE.with_borrow_mut(|e| elemente::zusammenfuehren(e, elemente));
            neu_zeichnen();
        }
        Server::WbLeute { leute } => tafel().leute.set(leute.clone()),
        Server::WbZeiger { id, zeiger } => {
            ZEIGER.with_borrow_mut(|z| z.insert(id.clone(), (zeiger.x as f64, zeiger.y as f64)));
            neu_zeichnen();
        }
        _ => {}
    }
}

fn schicken(e: Vec<Value>) {
    senden(ClientNachricht::Spielzeug(S::WbAendern { elemente: e }));
}

const FARBEN: [&str; 6] = ["#1e1e1e", "#e03131", "#2f9e44", "#1971c2", "#f08c00", "#9c36b5"];
const WERKZEUGE: [(Werkzeug, &str, &str); 7] = [
    (Werkzeug::Stift, "✏️", "Stift"),
    (Werkzeug::Rechteck, "▭", "Rechteck"),
    (Werkzeug::Ellipse, "◯", "Ellipse"),
    (Werkzeug::Pfeil, "➜", "Pfeil"),
    (Werkzeug::Text, "T", "Text"),
    (Werkzeug::Radierer, "🧽", "Radierer"),
    (Werkzeug::Hand, "✋", "Verschieben (oder Mausrad / Leertaste)"),
];

#[component]
pub fn Whiteboard() -> impl IntoView {
    let t = tafel();
    let z = zustand();
    let flaeche = NodeRef::<leptos::html::Canvas>::new();
    let werkzeug = RwSignal::new(Werkzeug::Stift);
    let farbe = RwSignal::new(FARBEN[0].to_string());
    let breite = RwSignal::new(2.0f64);
    let ansicht = StoredValue::new(Ansicht::default());
    // Das Element, das gerade gezeichnet wird, und wann es zuletzt verschickt wurde.
    let zieht = StoredValue::new(None::<Value>);
    let zuletzt = StoredValue::new(0u64);
    let schieben = StoredValue::new(None::<(f64, f64)>);

    senden(ClientNachricht::Spielzeug(S::WbOeffnen));
    on_cleanup(|| senden(ClientNachricht::Spielzeug(S::WbSchliessen)));

    let malen = move || {
        let Some(c) = flaeche.get_untracked() else { return };
        let (b, h) = (c.client_width() as f64, c.client_height() as f64);
        if c.width() != b as u32 || c.height() != h as u32 {
            c.set_width(b as u32);
            c.set_height(h as u32);
        }
        let Ok(Some(ctx)) = c.get_context("2d") else { return };
        let Ok(ctx) = ctx.dyn_into::<web_sys::CanvasRenderingContext2d>() else { return };
        let personen = z.personen.get_untracked();
        let zeiger: Vec<_> = ZEIGER.with_borrow(|m| {
            m.iter()
                .filter_map(|(id, (x, y))| personen.iter().find(|p| &p.id == id).map(|p| (p.name.clone(), p.farbe.clone(), *x, *y)))
                .collect()
        });
        ELEMENTE.with_borrow(|e| {
            let mut alle = e.clone();
            if let Some(neu) = zieht.get_value() {
                alle.push(neu);
            }
            zeichnen::alles(&ctx, b, h, ansicht.get_value(), &alle, &zeiger);
        });
    };
    Effect::new(move |_| {
        t.stand.track();
        flaeche.track();
        request_animation_frame(malen);
    });
    let groesse_weg = window_event_listener(leptos::ev::resize, move |_| malen());
    on_cleanup(move || groesse_weg.remove());

    let punkt = move |e: &web_sys::PointerEvent| ansicht.get_value().welt(e.offset_x() as f64, e.offset_y() as f64);

    let runter = move |e: web_sys::PointerEvent| {
        let (x, y) = punkt(&e);
        if let Some(c) = flaeche.get_untracked() {
            let _ = c.set_pointer_capture(e.pointer_id());
        }
        match werkzeug.get_untracked() {
            _ if e.button() == 1 || werkzeug.get_untracked() == Werkzeug::Hand => schieben.set_value(Some((e.offset_x() as f64, e.offset_y() as f64))),
            Werkzeug::Radierer => radieren(x, y),
            Werkzeug::Text => {
                if let Ok(Some(text)) = window().prompt_with_message("Text:")
                    && !text.trim().is_empty()
                {
                    let mut el = elemente::neu("text", x, y, &farbe.get_untracked(), breite.get_untracked());
                    el["text"] = Value::from(text.clone());
                    el["originalText"] = Value::from(text);
                    ELEMENTE.with_borrow_mut(|a| a.push(el.clone()));
                    schicken(vec![el]);
                    neu_zeichnen();
                }
            }
            w => {
                let typ = match w {
                    Werkzeug::Rechteck => "rectangle",
                    Werkzeug::Ellipse => "ellipse",
                    Werkzeug::Pfeil => "arrow",
                    _ => "freedraw",
                };
                zieht.set_value(Some(elemente::neu(typ, x, y, &farbe.get_untracked(), breite.get_untracked())));
            }
        }
    };
    let bewegen = move |e: web_sys::PointerEvent| {
        let (x, y) = punkt(&e);
        let jetzt = crate::texte::jetzt();
        if let Some((sx, sy)) = schieben.get_value() {
            let (nx, ny) = (e.offset_x() as f64, e.offset_y() as f64);
            ansicht.update_value(|a| {
                a.vx += (nx - sx) / a.zoom;
                a.vy += (ny - sy) / a.zoom;
            });
            schieben.set_value(Some((nx, ny)));
            neu_zeichnen();
            return;
        }
        if werkzeug.get_untracked() == Werkzeug::Radierer && e.buttons() == 1 {
            radieren(x, y);
        }
        if zieht.with_value(Option::is_some) {
            zieht.update_value(|el| el.iter_mut().for_each(|el| elemente::ziehen(el, x, y)));
            neu_zeichnen();
            // Striche live an die anderen, höchstens alle 120 ms.
            if jetzt.saturating_sub(zuletzt.get_value()) > 120
                && let Some(el) = zieht.get_value()
            {
                zuletzt.set_value(jetzt);
                schicken(vec![el]);
            }
        }
        if jetzt.saturating_sub(zuletzt.get_value()) > 80 {
            zuletzt.set_value(jetzt);
            let taste = if e.buttons() == 1 { "down" } else { "up" };
            senden(ClientNachricht::Spielzeug(S::WbZeiger { zeiger: WbZeiger { x: x as f32, y: y as f32, werkzeug: "pointer".into(), taste: taste.into(), ausgewaehlt: Vec::new() } }));
        }
    };
    let hoch = move |_e: web_sys::PointerEvent| {
        schieben.set_value(None);
        if let Some(mut el) = zieht.get_value() {
            zieht.set_value(None);
            elemente::normalisieren(&mut el);
            elemente::aendern(&mut el);
            ELEMENTE.with_borrow_mut(|a| elemente::zusammenfuehren(a, std::slice::from_ref(&el)));
            schicken(vec![el]);
            neu_zeichnen();
        }
    };
    let rad = move |e: web_sys::WheelEvent| {
        e.prevent_default();
        let faktor = if e.delta_y() < 0.0 { 1.1 } else { 1.0 / 1.1 };
        let (mx, my) = (e.offset_x() as f64, e.offset_y() as f64);
        ansicht.update_value(|a| {
            let (wx, wy) = a.welt(mx, my);
            a.zoom = (a.zoom * faktor).clamp(0.2, 5.0);
            a.vx = mx / a.zoom - wx;
            a.vy = my / a.zoom - wy;
        });
        neu_zeichnen();
    };

    let kopf = view! {
        <span class="text-muted-foreground text-xs" title="Wer gerade mitzeichnet">{move || format!("✍️ {}", t.leute.with(Vec::len))}</span>
    }
    .into_any();

    view! {
        <Rahmen fenster=Fenster::Whiteboard titel="📝 Whiteboard" breite="max-w-[min(96vw,1600px)]" kopf=kopf>
            <div class="mb-2 flex flex-wrap items-center gap-1">
                {WERKZEUGE.iter().map(|(w, sym, name)| view! {
                    <button type="button" class="size-9 rounded-md border text-lg hover:cursor-pointer" title=*name aria-label=*name
                        class:bg-accent=move || werkzeug.get() == *w class:border-foreground=move || werkzeug.get() == *w
                        on:click=move |_| werkzeug.set(*w)>{*sym}</button>
                }).collect_view()}
                <span class="mx-2 h-6 w-px bg-border"></span>
                {FARBEN.iter().map(|f| view! {
                    <button type="button" class="size-7 rounded-full border-2 hover:cursor-pointer" style=format!("background:{f}") aria-label=*f
                        class:border-foreground=move || farbe.get() == *f class:border-transparent=move || farbe.get() != *f
                        on:click=move |_| farbe.set((*f).into())></button>
                }).collect_view()}
                <span class="mx-2 h-6 w-px bg-border"></span>
                {[(1.0, "dünn"), (2.0, "mittel"), (4.0, "dick")].into_iter().map(|(b, n)| view! {
                    <button type="button" class="h-8 rounded-md border px-2 text-xs hover:cursor-pointer" class:bg-accent=move || breite.get() == b on:click=move |_| breite.set(b)>{n}</button>
                }).collect_view()}
                <button type="button" class="text-muted-foreground ml-auto rounded-md px-2 py-1 text-xs hover:cursor-pointer hover:bg-accent" on:click=move |_| alles_loeschen()>"🗑 Alles wegwischen"</button>
            </div>
            <canvas
                node_ref=flaeche
                class="h-[68vh] w-full touch-none rounded-lg border-2 border-foreground/30 bg-white"
                class:cursor-crosshair=move || werkzeug.get() != Werkzeug::Hand
                class:cursor-grab=move || werkzeug.get() == Werkzeug::Hand
                on:pointerdown=runter
                on:pointermove=bewegen
                on:pointerup=hoch
                on:wheel=rad
            ></canvas>
        </Rahmen>
    }
}

/// Löscht (markiert `isDeleted`) das Element unter dem Punkt.
fn radieren(x: f64, y: f64) {
    let weg = ELEMENTE.with_borrow_mut(|a| {
        let i = elemente::unter(a, x, y)?;
        a[i]["isDeleted"] = Value::Bool(true);
        elemente::aendern(&mut a[i]);
        Some(a[i].clone())
    });
    if let Some(el) = weg {
        schicken(vec![el]);
        neu_zeichnen();
    }
}

fn alles_loeschen() {
    if !window().confirm_with_message("Das ganze Whiteboard für alle wegwischen?").unwrap_or(false) {
        return;
    }
    let weg = ELEMENTE.with_borrow_mut(|a| {
        a.iter_mut()
            .filter(|e| !e["isDeleted"].as_bool().unwrap_or(false))
            .map(|e| {
                e["isDeleted"] = Value::Bool(true);
                elemente::aendern(e);
                e.clone()
            })
            .collect::<Vec<_>>()
    });
    if !weg.is_empty() {
        schicken(weg);
        neu_zeichnen();
    }
}
