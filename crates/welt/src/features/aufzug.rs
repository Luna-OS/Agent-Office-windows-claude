//! Der Aufzug: E darin (oder am Knopf) öffnet die Etagenwahl der Oberfläche; die Türen gleiten auf,
//! wenn jemand davorsteht. `welt::zur_etage` fährt los: schließt die Türen und schickt `Gehen`;
//! die Ankunft kommt mit `Betreten` (die Schleife baut dann die neue Etage).

use std::cell::Cell;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::etagen::Client as E;

use crate::api::{WeltAktion, melden};
use crate::grundriss::{AUFZUG_KABINE, AUFZUG_TUER, AUFZUG_VORNE, AUFZUG_WAND, AUFZUG_X};
use crate::register::{Phase, Register};
use crate::welt::Kontext;

thread_local! {
    /// Eine Fahrt läuft (bis `Betreten` kommt): Türen zu.
    static FAEHRT: Cell<bool> = const { Cell::new(false) };
}

/// Mit dem Aufzug zu einer Etage (oder `protokoll::etagen::DACH`).
pub fn fahren(kx: &mut Kontext, etage: &str) {
    if kx.zustand.etage.as_deref() == Some(etage) {
        return;
    }
    FAEHRT.set(true);
    crate::api::klang(crate::api::Klang::Aufzug { pos: [AUFZUG_X, 1.2, crate::grundriss::AUFZUG_ANKUNFT.1] });
    // Wer außerhalb der Kabine wählt, kommt trotzdem drin an: der Server setzt den Ort.
    kx.senden(ClientNachricht::Etagen(E::Gehen { etage: etage.to_string(), an: None }));
}

/// Auf dem Dach angekommen: ein paar Schritte aus dem Aufzug, Blick zur Leinwand über der Bühne.
pub fn dach_ankunft(kx: &mut Kontext) {
    use crate::grundriss::dach::LEINWAND;
    let (x, z) = (2.5, -3.0);
    let blick = (LEINWAND.x - x).atan2(LEINWAND.z - z);
    kx.spieler.versetzen(x, 0.0, z, blick);
    kx.spieler.neigung = 0.12;
}

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.interaktion(
        "aufzug",
        |_, _| Some("E: Aufzug – Etage wählen".into()),
        |_, _| melden(WeltAktion::AufzugOeffnen),
    );

    let offen = Rc::new(Cell::new(0.0f32));
    reg.tick(Phase::Umgebung, move |kx: &mut Kontext, dt| {
        if kx.neu_gebaut {
            FAEHRT.set(false);
            // Angekommen: die Türen stehen schon offen.
            offen.set(1.0);
        }
        let p = kx.spieler.pos();
        let nah = (p[0] - AUFZUG_X).abs() < 1.6 && p[2] > AUFZUG_KABINE.min_z && p[2] < AUFZUG_VORNE + 2.2 && p[1].abs() < 1.0;
        let andere = kx.zustand.personen_hier().any(|q| (q.x - AUFZUG_X).abs() < 1.6 && q.z < AUFZUG_VORNE + 2.2 && q.z > AUFZUG_KABINE.min_z);
        let soll = if (nah || andere) && !FAEHRT.get() { 1.0 } else { 0.0 };
        let o = offen.get() + (soll - offen.get()).clamp(-dt * 2.5, dt * 2.5);
        offen.set(o);
        let halb = AUFZUG_TUER / 2.0;
        let z = AUFZUG_VORNE - AUFZUG_WAND / 2.0 + 0.03;
        let schub = o * (halb - 0.05);
        if let Some(l) = kx.teil("aufzug-tuer-l") {
            l.set_position(Vec3::new(AUFZUG_X - halb / 2.0 - schub, crate::grundriss::AUFZUG_TUER_HOEHE / 2.0, z));
        }
        if let Some(r) = kx.teil("aufzug-tuer-r") {
            r.set_position(Vec3::new(AUFZUG_X + halb / 2.0 + schub, crate::grundriss::AUFZUG_TUER_HOEHE / 2.0, z));
        }
    });
}
