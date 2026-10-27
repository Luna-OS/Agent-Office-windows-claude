//! Je Arbeiter ein vt100-Parser, gefüttert mit Snapshot und laufenden Daten vom Server.
//! Wer zeichnet, hört auf [`Sitzung::stand`]: es zählt hoch, sobald sich der Bildschirm geändert hat
//! (höchstens einmal je Frame, damit ein Schwall Ausgabe nicht jedes Mal neu zeichnet).

use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroU16;

use leptos::prelude::*;

/// So viele Zeilen hält der Verlauf.
const VERLAUF: usize = 5000;

pub struct Sitzung {
    pub parser: vt100::Parser,
    /// Zählt hoch bei jeder Änderung.
    pub stand: ArcRwSignal<u64>,
    /// Ein Neuzeichnen ist schon für den nächsten Frame bestellt.
    bestellt: bool,
}

thread_local! {
    static SITZUNGEN: RefCell<HashMap<String, Sitzung>> = RefCell::new(HashMap::new());
}

fn groesse(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n.max(1)).unwrap_or(NonZeroU16::MIN)
}

/// Mit dem Parser eines Arbeiters arbeiten (falls es ihn schon gibt).
pub fn mit<R>(arbeiter: &str, f: impl FnOnce(&mut Sitzung) -> R) -> Option<R> {
    SITZUNGEN.with(|s| s.borrow_mut().get_mut(arbeiter).map(f))
}

/// Das Signal, das bei jeder Änderung hochzählt; legt die Sitzung an, wenn es sie noch nicht gibt.
pub fn stand(arbeiter: &str) -> ArcRwSignal<u64> {
    SITZUNGEN.with(|s| {
        s.borrow_mut()
            .entry(arbeiter.to_string())
            .or_insert_with(|| Sitzung {
                parser: vt100::Parser::new(groesse(24), groesse(80), VERLAUF),
                stand: ArcRwSignal::new(0),
                bestellt: false,
            })
            .stand
            .clone()
    })
}

/// Vergisst den Parser, wenn niemand mehr hinschaut (der nächste Snapshot baut ihn neu).
pub fn vergessen(arbeiter: &str) {
    SITZUNGEN.with(|s| s.borrow_mut().remove(arbeiter));
}

/// Ein frisch geöffnetes Terminal: der ganze Bildschirm.
pub fn snapshot(arbeiter: &str, daten: &str, spalten: u16, zeilen: u16) {
    let stand = stand(arbeiter);
    mit(arbeiter, |s| {
        s.parser = vt100::Parser::new(groesse(zeilen), groesse(spalten), VERLAUF);
        s.parser.process(daten.as_bytes());
    });
    stand.update(|n| *n += 1);
}

/// Neue Ausgabe.
pub fn daten(arbeiter: &str, daten: &str) {
    let bestellen = mit(arbeiter, |s| {
        // Wer im Verlauf zurückgescrollt hat, bleibt an seiner Stelle, solange er liest.
        let zurueck = s.parser.screen().scrollback();
        s.parser.process(daten.as_bytes());
        if zurueck > 0 {
            s.parser.screen_mut().set_scrollback(zurueck);
        }
        !std::mem::replace(&mut s.bestellt, true)
    });
    if bestellen == Some(true) {
        let id = arbeiter.to_string();
        request_animation_frame(move || {
            if let Some(stand) = mit(&id, |s| {
                s.bestellt = false;
                s.stand.clone()
            }) {
                stand.update(|n| *n += 1);
            }
        });
    }
}

/// Der PTY hat eine andere Größe bekommen (von jemand anderem oder von uns).
pub fn groesse_setzen(arbeiter: &str, spalten: u16, zeilen: u16) {
    let geaendert = mit(arbeiter, |s| {
        let (r, c) = s.parser.screen().size();
        if r.get() == zeilen && c.get() == spalten {
            return None;
        }
        s.parser.set_size(groesse(zeilen), groesse(spalten));
        Some(s.stand.clone())
    })
    .flatten();
    if let Some(stand) = geaendert {
        stand.update(|n| *n += 1);
    }
}

/// Im Verlauf scrollen: positiv nach oben (älter), negativ nach unten.
pub fn scrollen(arbeiter: &str, zeilen: i32) {
    let stand = mit(arbeiter, |s| {
        let jetzt = s.parser.screen().scrollback() as i32;
        s.parser
            .screen_mut()
            .set_scrollback((jetzt + zeilen).max(0) as usize);
        s.stand.clone()
    });
    if let Some(stand) = stand {
        stand.update(|n| *n += 1);
    }
}

/// Zurück ans Ende des Verlaufs (beim Tippen).
pub fn ans_ende(arbeiter: &str) {
    let stand = mit(arbeiter, |s| {
        if s.parser.screen().scrollback() == 0 {
            return None;
        }
        s.parser.screen_mut().set_scrollback(0);
        Some(s.stand.clone())
    })
    .flatten();
    if let Some(stand) = stand {
        stand.update(|n| *n += 1);
    }
}
