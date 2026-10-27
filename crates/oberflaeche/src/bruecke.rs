//! Die Verbindung der Oberfläche nach außen: was der Client (crates/client) ihr beim Start mitgibt.
//!
//! Die Oberfläche kennt weder den WebSocket noch die Welt; sie ruft nur diese Funktionen auf.

use std::cell::RefCell;
use std::rc::Rc;

use protokoll::ClientNachricht;

/// Was der Client der Oberfläche mitgibt.
#[derive(Clone)]
pub struct Bruecke {
    /// Schickt einen Frame an den Server.
    pub senden: Rc<dyn Fn(ClientNachricht)>,
    /// Baut den WebSocket auf (nach der Anmeldung und nach einem Abbruch); startet beim ersten Mal die Welt.
    pub verbinden: Rc<dyn Fn()>,
    /// `true`: ein Fenster ging auf, die Welt gibt die Maus frei und hört nicht mehr auf Tasten.
    /// `false`: das letzte Fenster ging zu, die Welt fängt die Maus sofort wieder.
    pub fenster: Rc<dyn Fn(bool)>,
}

thread_local! {
    static BRUECKE: RefCell<Option<Bruecke>> = const { RefCell::new(None) };
}

pub(crate) fn setzen(b: Bruecke) {
    BRUECKE.with(|z| *z.borrow_mut() = Some(b));
}

fn mit(f: impl FnOnce(&Bruecke)) {
    let b = BRUECKE.with(|z| z.borrow().clone());
    if let Some(b) = b {
        f(&b);
    }
}

/// Schickt eine Nachricht an den Server.
pub fn senden(n: ClientNachricht) {
    mit(|b| (b.senden)(n));
}

pub(crate) fn verbinden() {
    mit(|b| (b.verbinden)());
}

pub(crate) fn fenster(offen: bool) {
    mit(|b| (b.fenster)(offen));
}
