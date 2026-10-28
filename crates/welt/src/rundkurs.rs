//! Rundenzeiten auf dem Rundkurs (Straße + Ring hinter der Nachbarschaft). Reine Logik.
//!
//! Die Ziellinie quert die Straße bei x = 0. Eine Runde zählt, wenn man seit dem letzten
//! Überqueren den West-, Nord- und Ostteil des Rings befahren hat (Richtung egal).

use crate::grundriss::draussen::{RUNDKURS, STRASSE};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rundenzaehler {
    /// Wann (Sekunden) die laufende Runde begann; keine, bevor man das erste Mal über die Linie fuhr.
    start: Option<f64>,
    /// Welche Abschnitte (West, Nord, Ost) in dieser Runde schon befahren wurden.
    besucht: [bool; 3],
    letzte_x: Option<f32>,
    pub beste: Option<f64>,
}

impl Rundenzaehler {
    /// Läuft gerade eine Runde (für die Anzeige)?
    pub fn laeuft_seit(&self) -> Option<f64> {
        self.start
    }

    /// Die Lage des Autos zur Zeit `t`; gibt die Rundenzeit zurück, wenn eine Runde fertig ist.
    pub fn melden(&mut self, x: f32, z: f32, t: f64) -> Option<f64> {
        for (i, f) in RUNDKURS.iter().enumerate() {
            if f.enthaelt(x, z) {
                self.besucht[i] = true;
            }
        }
        let auf_strasse = z > STRASSE.0 && z < STRASSE.1;
        let ueber_linie = auf_strasse && self.letzte_x.is_some_and(|alt| (alt < 0.0) != (x < 0.0));
        self.letzte_x = auf_strasse.then_some(x);
        if !ueber_linie {
            return None;
        }
        let mut zeit = None;
        if let Some(s) = self.start
            && self.besucht.iter().all(|b| *b)
        {
            let r = t - s;
            self.beste = Some(self.beste.map_or(r, |b| b.min(r)));
            zeit = Some(r);
        }
        if zeit.is_some() || self.start.is_none() {
            self.start = Some(t);
            self.besucht = [false; 3];
        }
        zeit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grundriss::draussen::STRASSE_ENDE;

    #[test]
    fn eine_runde_rundherum() {
        let mut r = Rundenzaehler::default();
        let z = (STRASSE.0 + STRASSE.1) / 2.0;
        assert_eq!(r.melden(-1.0, z, 0.0), None);
        assert_eq!(r.melden(1.0, z, 1.0), None, "Start");
        // Abkürzung zurück über die Linie zählt nicht.
        assert_eq!(r.melden(-1.0, z, 2.0), None);
        assert_eq!(r.melden(1.0, z, 3.0), None);
        for (x, zz) in [
            (STRASSE_ENDE + 4.0, 0.0),
            (0.0, -64.0),
            (-STRASSE_ENDE - 4.0, 0.0),
            (-1.0, z),
        ] {
            assert_eq!(r.melden(x, zz, 50.0), None);
        }
        assert_eq!(r.melden(1.0, z, 61.0), Some(60.0));
        assert_eq!(r.beste, Some(60.0));
    }
}
