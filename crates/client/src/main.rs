//! Der Browser-Client (WASM): setzt Welt, Oberfläche und Medien zusammen.
//!
//! Ablauf: Die Oberfläche prüft die Anmeldung; danach baut [`verbindung`] den WebSocket auf `/ws` auf,
//! startet beim ersten Mal Welt und Medien, und das [`register`] verteilt jeden Server-Frame an seine
//! Empfänger.

mod medien_anbindung;
mod register;
mod verbindung;
mod welt_anbindung;

use std::cell::Cell;
use std::rc::Rc;

use oberflaeche::Befehl;

thread_local! {
    static GESTARTET: Cell<bool> = const { Cell::new(false) };
}

fn main() {
    console_error_panic_hook::set_once();
    register::anmelden();
    oberflaeche::starten(oberflaeche::Bruecke {
        senden: Rc::new(verbindung::senden),
        verbinden: Rc::new(|| {
            verbindung::verbinden();
            if !GESTARTET.with(|g| g.replace(true)) {
                if !oberflaeche::ist_lite() {
                    welt_anbindung::starten();
                }
                medien_anbindung::starten(Rc::new(verbindung::senden));
            }
        }),
        fenster: Rc::new(welt_anbindung::fenster),
        befehl: Rc::new(|b| match b {
            Befehl::ZurEtage(etage) => welt_anbindung::zur_etage(&etage),
            Befehl::Perspektive(ich) => welt_anbindung::perspektive(ich),
            andere => medien_anbindung::befehl(&andere),
        }),
    });
}
