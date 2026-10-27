//! Die Verdrahtung mit der 3D-Welt (crates/welt): starten, Maus fangen und freigeben, und was man in
//! der Welt anklickt, als Fenster der Oberfläche öffnen. Ohne das Feature `welt3d` bleibt die Welt aus.

#[cfg(feature = "welt3d")]
mod an {
    use std::cell::Cell;

    use oberflaeche::Fenster;

    thread_local! {
        static GESTARTET: Cell<bool> = const { Cell::new(false) };
    }

    pub fn starten() {
        if GESTARTET.with(|g| g.replace(true)) {
            return;
        }
        welt::bei_senden(Box::new(crate::verbindung::senden));
        welt::bei_aktion(Box::new(aktion));
        wasm_bindgen_futures::spawn_local(welt::starten(welt::WeltStart::default()));
    }

    pub fn fenster(offen: bool) {
        welt::eingabe_sperren(offen);
        if offen {
            welt::zeiger_freigeben();
        } else {
            welt::zeiger_fangen();
        }
    }

    pub fn nachricht(n: &protokoll::ServerNachricht) {
        welt::nachricht(n);
    }

    /// Was man in der Welt benutzt hat, als Fenster.
    fn aktion(a: welt::WeltAktion) {
        use welt::WeltAktion as A;
        let f = match a {
            A::AufzugOeffnen => Fenster::Aufzug,
            A::TerminalOeffnen { arbeiter } => Fenster::Terminal(arbeiter),
            #[allow(unreachable_patterns)]
            _ => return,
        };
        oberflaeche::fenster_oeffnen(f);
    }
}

#[cfg(not(feature = "welt3d"))]
mod an {
    pub fn starten() {}
    pub fn fenster(_offen: bool) {}
    pub fn nachricht(_n: &protokoll::ServerNachricht) {}
}

/// Startet die Welt beim ersten Verbinden (danach nie wieder).
pub fn starten() {
    an::starten();
}

/// Ein Fenster ging auf (`true`) oder das letzte zu (`false`).
pub fn fenster(offen: bool) {
    an::fenster(offen);
}

/// Server-Frames für die Welt.
pub fn nachricht(n: &protokoll::ServerNachricht) {
    an::nachricht(n);
}
