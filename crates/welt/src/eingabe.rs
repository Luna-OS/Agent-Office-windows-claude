//! Tastatur und Maus im Browser (wie `input/keyboard.ts`, `player/pointer.ts`).
//!
//! Wir hören selbst auf `window`/`document` statt auf kiss3d: für Pointer-Lock brauchen wir
//! `movementX/Y`, und Tasten sollen auch ohne Fokus auf dem Canvas ankommen. Tasten in Eingabefeldern
//! (Chat, Terminal) und alles, solange die Oberfläche die Eingabe sperrt, ignorieren wir.

use std::cell::RefCell;
use std::collections::HashSet;

/// Was seit dem letzten Frame passiert ist.
#[derive(Debug, Default)]
pub struct Eingabe {
    /// Gerade gehaltene Tasten (`KeyboardEvent.code`).
    pub gehalten: HashSet<String>,
    /// Gedrückt/losgelassen, in Reihenfolge, seit dem letzten Abholen.
    pub tasten: Vec<(String, bool)>,
    /// Mausbewegung seit dem letzten Abholen (nur bei gefangenem Zeiger).
    pub maus: (f32, f32),
    /// Mausrad seit dem letzten Abholen.
    pub rad: f32,
    /// Linksklicks ins Bild bei gefangenem Zeiger.
    pub klicks: u32,
    pub gefangen: bool,
    pub gesperrt: bool,
    /// Fangen ist gescheitert (Esc-Sperre des Browsers): beim Loslassen von Esc nochmal versuchen.
    fangen_bei_esc_hoch: bool,
}

thread_local! {
    static EINGABE: RefCell<Eingabe> = RefCell::new(Eingabe::default());
}

/// Holt die gesammelten Ereignisse ab (leert Tasten, Maus, Rad, Klicks).
pub fn abholen() -> Eingabe {
    EINGABE.with_borrow_mut(|e| Eingabe {
        gehalten: e.gehalten.clone(),
        tasten: std::mem::take(&mut e.tasten),
        maus: std::mem::take(&mut e.maus),
        rad: std::mem::take(&mut e.rad),
        klicks: std::mem::take(&mut e.klicks),
        gefangen: e.gefangen,
        gesperrt: e.gesperrt,
        fangen_bei_esc_hoch: false,
    })
}

pub(crate) fn gesperrt_gesetzt(an: bool) {
    EINGABE.with_borrow_mut(|e| {
        e.gesperrt = an;
        if an {
            // Gehaltene Tasten loslassen, sonst läuft man nach dem Schließen weiter.
            e.gehalten.clear();
        }
    });
}

/// Fängt den Mauszeiger für das Umsehen (Pointer-Lock auf dem Canvas). Klappt nur in einer
/// Nutzergeste; scheitert es (Chrome sperrt kurz nach Esc), versuchen wir es beim Loslassen von Esc
/// und sonst beim nächsten Klick ins Bild.
pub fn zeiger_fangen() {
    EINGABE.with_borrow_mut(|e| e.fangen_bei_esc_hoch = true);
    #[cfg(target_arch = "wasm32")]
    web::fangen();
}

/// Gibt den Mauszeiger frei (z. B. wenn ein Fenster aufgeht).
pub fn zeiger_freigeben() {
    EINGABE.with_borrow_mut(|e| e.fangen_bei_esc_hoch = false);
    #[cfg(target_arch = "wasm32")]
    web::freigeben();
}

/// Hängt die Zuhörer an Fenster, Dokument und Canvas (einmal beim Start).
pub fn installieren(canvas_id: &str) {
    #[cfg(target_arch = "wasm32")]
    web::installieren(canvas_id);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = canvas_id;
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{Element, HtmlElement, KeyboardEvent, MouseEvent, WheelEvent};

    use super::EINGABE;

    fn canvas() -> Option<Element> {
        let id = crate::api::CANVAS_ID;
        web_sys::window()?.document()?.get_element_by_id(id)
    }

    pub fn fangen() {
        if let Some(c) = canvas() {
            c.request_pointer_lock();
        }
    }

    pub fn freigeben() {
        if let Some(d) = web_sys::window().and_then(|w| w.document()) {
            d.exit_pointer_lock();
        }
    }

    /// Ob die Taste in einem Eingabefeld getippt wurde (Chat, Terminal, Dialoge).
    fn in_eingabefeld(ev: &KeyboardEvent) -> bool {
        let Some(ziel) = ev.target().and_then(|t| t.dyn_into::<HtmlElement>().ok()) else {
            return false;
        };
        let tag = ziel.tag_name().to_ascii_uppercase();
        tag == "INPUT"
            || tag == "TEXTAREA"
            || tag == "SELECT"
            || ziel.is_content_editable()
            || ziel.closest(".xterm").ok().flatten().is_some()
    }

    fn haengen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
        ziel: &web_sys::EventTarget,
        art: &str,
        f: impl FnMut(E) + 'static,
    ) {
        let c = Closure::<dyn FnMut(E)>::new(f);
        let _ = ziel.add_event_listener_with_callback(art, c.as_ref().unchecked_ref());
        c.forget();
    }

    pub fn installieren(canvas_id: &str) {
        let Some(fenster) = web_sys::window() else {
            return;
        };
        let Some(dokument) = fenster.document() else {
            return;
        };

        haengen(&fenster, "keydown", |ev: KeyboardEvent| {
            if in_eingabefeld(&ev) || ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
                return;
            }
            let code = ev.code();
            let genommen = EINGABE.with_borrow_mut(|e| {
                if e.gesperrt {
                    return false;
                }
                if !ev.repeat() {
                    e.tasten.push((code.clone(), true));
                }
                e.gehalten.insert(code.clone());
                true
            });
            // Leertaste und Pfeile sollen die Seite nicht scrollen.
            if genommen && (code == "Space" || code.starts_with("Arrow")) {
                ev.prevent_default();
            }
        });
        haengen(&fenster, "keyup", |ev: KeyboardEvent| {
            let code = ev.code();
            let nochmal = EINGABE.with_borrow_mut(|e| {
                e.gehalten.remove(&code);
                if !e.gesperrt {
                    e.tasten.push((code.clone(), false));
                }
                let n = code == "Escape" && e.fangen_bei_esc_hoch && !e.gefangen;
                e.fangen_bei_esc_hoch = false;
                n
            });
            if nochmal {
                fangen();
            }
        });
        // Fenster verliert den Fokus: nichts bleibt gedrückt.
        haengen(&fenster, "blur", |_: web_sys::Event| {
            EINGABE.with_borrow_mut(|e| e.gehalten.clear())
        });

        haengen(&dokument, "mousemove", |ev: MouseEvent| {
            EINGABE.with_borrow_mut(|e| {
                if e.gefangen && !e.gesperrt {
                    e.maus.0 += ev.movement_x() as f32;
                    e.maus.1 += ev.movement_y() as f32;
                }
            });
        });
        let id = canvas_id.to_string();
        haengen(&dokument, "pointerlockchange", move |_: web_sys::Event| {
            let gefangen = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.pointer_lock_element())
                .is_some_and(|el| el.id() == id);
            EINGABE.with_borrow_mut(|e| e.gefangen = gefangen);
        });
        haengen(&dokument, "pointerlockerror", |_: web_sys::Event| {
            log::info!(
                "Mauszeiger konnte nicht gefangen werden; Klick ins Bild versucht es erneut."
            );
        });

        if let Some(c) = canvas() {
            haengen(&c, "mousedown", |ev: MouseEvent| {
                let (gefangen, gesperrt) = EINGABE.with_borrow(|e| (e.gefangen, e.gesperrt));
                if gesperrt {
                    return;
                }
                if !gefangen {
                    fangen();
                } else if ev.button() == 0 {
                    EINGABE.with_borrow_mut(|e| e.klicks += 1);
                }
            });
            haengen(&c, "wheel", |ev: WheelEvent| {
                EINGABE.with_borrow_mut(|e| {
                    if !e.gesperrt {
                        e.rad += ev.delta_y() as f32;
                    }
                });
            });
            haengen(&c, "contextmenu", |ev: web_sys::Event| ev.prevent_default());
        }
    }
}
