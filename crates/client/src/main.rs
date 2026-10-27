//! Der Browser-Client (WASM): setzt Welt, Oberfläche und Medien zusammen.
//!
//! Ablauf: Die Oberfläche prüft die Anmeldung; danach baut [`verbindung`] den WebSocket auf `/ws` auf,
//! startet beim ersten Mal die Welt, und das [`register`] verteilt jeden Server-Frame an seine Empfänger.

mod register;
mod verbindung;
mod welt_anbindung;

use std::rc::Rc;

fn main() {
    console_error_panic_hook::set_once();
    register::anmelden();
    oberflaeche::starten(oberflaeche::Bruecke {
        senden: Rc::new(verbindung::senden),
        verbinden: Rc::new(|| {
            verbindung::verbinden();
            welt_anbindung::starten();
        }),
        fenster: Rc::new(welt_anbindung::fenster),
    });
}
