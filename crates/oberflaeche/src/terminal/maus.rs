//! Maus im Terminal: hat das Programm um Maus-Ereignisse gebeten (vim, htop, Claudes Menüs), gehen
//! Klicks und Rad als xterm-Sequenzen hin. Sonst blättert das Rad im Verlauf, und im Vollbild-Modus
//! (alternativer Bildschirm) wird es zu Pfeiltasten, wie in xterm.

use leptos::html::Div;
use leptos::prelude::*;
use vt100::{MouseProtocolEncoding, MouseProtocolMode};

use super::sitzung;

/// Die Zelle unter der Maus (1-basiert, wie die Sequenzen sie wollen).
fn zelle(x: f64, y: f64, flaeche: NodeRef<Div>, spalten: u16, zeilen: u16) -> Option<(u16, u16)> {
    let el = flaeche.get_untracked()?;
    let erste = el.first_element_child()?;
    let r = erste.get_bounding_client_rect();
    let zeile_h = r.height().max(1.0);
    let breite = r.width().max(1.0) / spalten as f64;
    let s = ((x - r.left()) / breite).floor().clamp(0.0, spalten as f64 - 1.0) as u16;
    let z = ((y - r.top()) / zeile_h).floor().clamp(0.0, zeilen as f64 - 1.0) as u16;
    Some((s + 1, z + 1))
}

fn sequenz(knopf: u8, s: u16, z: u16, gedrueckt: bool, kodierung: MouseProtocolEncoding) -> String {
    match kodierung {
        MouseProtocolEncoding::Sgr => format!("\x1b[<{knopf};{s};{z}{}", if gedrueckt { 'M' } else { 'm' }),
        _ => {
            let b = if gedrueckt { knopf } else { 3 };
            let zeichen = |v: u16| char::from_u32(32 + v.min(223) as u32).unwrap_or(' ');
            format!("\x1b[M{}{}{}", zeichen(b as u16), zeichen(s), zeichen(z))
        }
    }
}

/// Was das Programm will: (Modus, Kodierung, Größe, alternativer Bildschirm).
fn modus(arbeiter: &str) -> Option<(MouseProtocolMode, MouseProtocolEncoding, u16, u16, bool)> {
    sitzung::mit(arbeiter, |s| {
        let sc = s.parser.screen();
        let (z, sp) = sc.size();
        (sc.mouse_protocol_mode(), sc.mouse_protocol_encoding(), sp.get(), z.get(), sc.alternate_screen())
    })
}

/// Mausrad.
pub fn rad(arbeiter: &str, e: &web_sys::WheelEvent, flaeche: NodeRef<Div>) {
    let Some((m, kod, spalten, zeilen, alternativ)) = modus(arbeiter) else { return };
    e.prevent_default();
    let hoch = e.delta_y() < 0.0;
    let schritte = ((e.delta_y().abs() / 40.0).ceil() as usize).clamp(1, 10);
    if m != MouseProtocolMode::None {
        let Some((s, z)) = zelle(e.client_x() as f64, e.client_y() as f64, flaeche, spalten, zeilen) else { return };
        let seq = sequenz(if hoch { 64 } else { 65 }, s, z, true, kod);
        super::eingeben(arbeiter, seq.repeat(schritte));
    } else if alternativ {
        super::eingeben(arbeiter, (if hoch { "\x1b[A" } else { "\x1b[B" }).repeat(schritte));
    } else {
        sitzung::scrollen(arbeiter, if hoch { 3 * schritte as i32 } else { -3 * schritte as i32 });
    }
}

/// Maustaste gedrückt oder losgelassen.
pub fn knopf(arbeiter: &str, e: &web_sys::MouseEvent, flaeche: NodeRef<Div>, gedrueckt: bool) {
    let Some((m, kod, spalten, zeilen, _)) = modus(arbeiter) else { return };
    // Ohne Maus-Modus markiert der Browser Text; Shift markiert immer.
    if m == MouseProtocolMode::None || e.shift_key() {
        return;
    }
    if !gedrueckt && m == MouseProtocolMode::Press {
        return;
    }
    let Some((s, z)) = zelle(e.client_x() as f64, e.client_y() as f64, flaeche, spalten, zeilen) else { return };
    let knopf = match e.button() {
        1 => 1,
        2 => 2,
        _ => 0,
    };
    e.prevent_default();
    super::eingeben(arbeiter, sequenz(knopf, s, z, gedrueckt, kod));
}

/// Eine Datei auf das Terminal gezogen: ihr Name wird eingetippt (der Inhalt bleibt auf diesem Rechner).
pub fn ablegen(arbeiter: &str, e: web_sys::DragEvent) {
    e.prevent_default();
    let Some(daten) = e.data_transfer() else { return };
    if let Ok(text) = daten.get_data("text/plain")
        && !text.is_empty()
    {
        super::eingeben(arbeiter, text);
    }
}
