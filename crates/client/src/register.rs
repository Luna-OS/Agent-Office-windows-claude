//! Das Register der Empfänger: wer welche Server-Frames bekommt. Ein neuer Teil des Clients meldet
//! sich hier mit einer Zeile in [`anmelden`] an, statt in die Verbindung hineingeschrieben zu werden.

use std::cell::RefCell;

use protokoll::ServerNachricht;

type Empfaenger = Box<dyn Fn(&ServerNachricht)>;

thread_local! {
    static EMPFAENGER: RefCell<Vec<(&'static str, Empfaenger)>> = const { RefCell::new(Vec::new()) };
}

/// Meldet einen Empfänger an.
pub fn eintragen(name: &'static str, f: impl Fn(&ServerNachricht) + 'static) {
    EMPFAENGER.with(|e| e.borrow_mut().push((name, Box::new(f))));
}

/// Gibt einen Frame an alle Empfänger.
pub fn verteilen(n: &ServerNachricht) {
    EMPFAENGER.with(|e| {
        for (_, f) in e.borrow().iter() {
            f(n);
        }
    });
}

/// Alle Teile des Clients.
pub fn anmelden() {
    eintragen("oberflaeche", oberflaeche::empfangen);
    eintragen("welt", crate::welt_anbindung::nachricht);
    eintragen("medien", crate::medien_anbindung::nachricht);
}
