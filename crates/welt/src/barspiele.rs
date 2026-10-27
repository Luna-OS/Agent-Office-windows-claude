//! Darts und Axtwurf an der Dachbar (wie `shared/bargames.ts`): Wertung, Ausholen mit Kraftanzeige
//! und wo ein Wurf landet. Reine Logik, ohne 3D. `u` läuft auf der Scheibe nach rechts, `v` nach
//! oben, in Metern von ihrer Mitte.

use std::f32::consts::PI;

use protokoll::praesenz::Barspiel;

use crate::grundriss::dach::{AXT_LINIE, AXT_ZIEL, AXTBAHN, DART_ABSTAND, DARTSCHEIBE};

/// Würfe pro Runde.
pub fn runde(s: Barspiel) -> u32 {
    match s {
        Barspiel::Darts => 3,
        Barspiel::Axt => 5,
    }
}

/// Wo man zum Werfen steht und wohin man schaut (0 = +z).
pub fn wurfplatz(s: Barspiel) -> (f32, f32, f32) {
    match s {
        Barspiel::Darts => (
            DARTSCHEIBE.0 + DART_ABSTAND + 0.28,
            DARTSCHEIBE.2,
            -PI / 2.0,
        ),
        Barspiel::Axt => (AXT_ZIEL.0, AXT_ZIEL.2 + AXT_LINIE + 0.3, PI),
    }
}

/// Die Mitte des Ziels und die Richtungen „rechts“ und „aus der Scheibe heraus“ (x, z).
pub fn ziel(s: Barspiel) -> ([f32; 3], (f32, f32), (f32, f32)) {
    match s {
        Barspiel::Darts => (
            [DARTSCHEIBE.0, DARTSCHEIBE.1, DARTSCHEIBE.2],
            (0.0, -1.0),
            (1.0, 0.0),
        ),
        Barspiel::Axt => ([AXT_ZIEL.0, AXT_ZIEL.1, AXT_ZIEL.2], (1.0, 0.0), (0.0, 1.0)),
    }
}

/// Eine Wertung: Punkte und was an der Tafel steht ("T20", "Bull" …).
#[derive(Debug, Clone, PartialEq)]
pub struct Wertung {
    pub punkte: u32,
    pub text: String,
}

fn w(punkte: u32, text: impl Into<String>) -> Wertung {
    Wertung {
        punkte,
        text: text.into(),
    }
}

const BULL: f32 = 0.00635;
const AUSSEN_BULL: f32 = 0.0159;
const TRIPLE: (f32, f32) = (0.099, 0.107);
const DOPPEL: (f32, f32) = (0.162, 0.17);
const SCHEIBE: f32 = 0.2255;
const ZAHLEN: [u32; 20] = [
    20, 1, 18, 4, 13, 6, 10, 15, 2, 17, 3, 19, 7, 16, 8, 11, 14, 9, 12, 5,
];

/// Was ein Dart bei (u, v) zählt.
pub fn dart(u: f32, v: f32) -> Wertung {
    let r = u.hypot(v);
    if r <= BULL {
        return w(50, "Bull");
    }
    if r <= AUSSEN_BULL {
        return w(25, "25");
    }
    if r > DOPPEL.1 {
        return w(0, if r <= SCHEIBE { "Rand" } else { "Daneben" });
    }
    let a = u.atan2(v);
    let feld = (((a + PI / 20.0 + 2.0 * PI) % (2.0 * PI)) / (PI / 10.0)).floor() as usize % 20;
    let n = ZAHLEN[feld];
    if r > TRIPLE.0 && r <= TRIPLE.1 {
        return w(n * 3, format!("T{n}"));
    }
    if r > DOPPEL.0 {
        return w(n * 2, format!("D{n}"));
    }
    w(n, n.to_string())
}

const AXT_RINGE: [(f32, u32); 5] = [(0.075, 6), (0.17, 4), (0.27, 3), (0.37, 2), (0.47, 1)];
const KILL: (f32, f32, f32, u32) = (0.52, 0.55, 0.05, 8);
const AXT_FLAECHE: (f32, f32) = (1.5, 1.9);
const KLINGE: f32 = 0.1;

/// Ob eine Axt bei (u, v) die Rückwand trifft (wo sie stecken bleiben kann).
pub fn rueckwand(u: f32, v: f32) -> bool {
    let halb = AXTBAHN.breite() / 2.0 - 0.12;
    u.abs() < halb && v > 0.15 - AXT_ZIEL.1 && v < 2.6 - 0.12 - AXT_ZIEL.1
}

/// Was eine Axt bei (u, v) zählt; nur, wenn sie `steckt`.
pub fn axt(u: f32, v: f32, steckt: bool) -> Wertung {
    if !steckt {
        return w(0, "Abgeprallt");
    }
    let halb = KLINGE / 2.0;
    let nah = |pu: f32, pv: f32| (u - pu).hypot(((v - pv).abs() - halb).max(0.0));
    if nah(-KILL.0, KILL.1) <= KILL.2 || nah(KILL.0, KILL.1) <= KILL.2 {
        return w(KILL.3, "Killshot");
    }
    let r = nah(0.0, 0.0);
    for (rr, p) in AXT_RINGE {
        if r <= rr {
            return w(
                p,
                if p == 6 {
                    "Bull".to_string()
                } else {
                    p.to_string()
                },
            );
        }
    }
    let auf_brett = u.abs() <= AXT_FLAECHE.0 / 2.0 && v.abs() <= AXT_FLAECHE.1 / 2.0;
    w(0, if auf_brett { "Brett" } else { "Wand" })
}

pub fn wertung(s: Barspiel, u: f32, v: f32, steckt: bool) -> Wertung {
    match s {
        Barspiel::Darts => dart(u, v),
        Barspiel::Axt => axt(u, v, steckt),
    }
}

/// Der grüne Bereich der Kraftanzeige (Mitte, Breite).
pub fn gruen(s: Barspiel) -> (f32, f32) {
    match s {
        Barspiel::Darts => (0.7, 0.12),
        Barspiel::Axt => (0.66, 0.12),
    }
}

/// Die Kraftanzeige `sek` nach Beginn des Ausholens: rauf in `periode`, runter, wieder rauf.
pub fn anzeige(sek: f32, periode: f32) -> f32 {
    let p = (sek / periode) % 2.0;
    if p > 1.0 { 2.0 - p } else { p }
}

/// Wo ein Wurf auf (u, v) landet, bei Kraft `kraft` (0..1) und kleinem Zittern (`zittern` je -1..1).
/// Liefert (u, v, steckt).
pub fn landung(s: Barspiel, u: f32, v: f32, kraft: f32, zittern: [f32; 2]) -> (f32, f32, bool) {
    let (mitte, breite) = gruen(s);
    let ab = kraft - mitte;
    let halb = breite / 2.0;
    match s {
        Barspiel::Darts => {
            let fall = if ab.abs() <= halb {
                ab * 0.25
            } else {
                ab.signum() * (halb * 0.25 + (ab.abs() - halb) * 1.1)
            };
            (u + zittern[0] * 0.006, v + fall + zittern[1] * 0.006, true)
        }
        Barspiel::Axt => {
            let (lu, lv) = (u + zittern[0] * 0.02, v + ab * 0.35 + zittern[1] * 0.02);
            (lu, lv, ab.abs() <= halb + 0.015 && rueckwand(lu, lv))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dartwertung() {
        assert_eq!(dart(0.0, 0.0).punkte, 50);
        assert_eq!(dart(0.0, 0.01).punkte, 25);
        assert_eq!(dart(0.0, 0.103), w(60, "T20"));
        assert_eq!(dart(0.0, 0.165), w(40, "D20"));
        assert_eq!(dart(0.0, 0.05).text, "20");
        assert_eq!(dart(0.05, 0.0).text, "6");
        assert_eq!(dart(0.0, 0.3).text, "Daneben");
    }

    #[test]
    fn axtwertung() {
        assert_eq!(axt(0.0, 0.0, true).text, "Bull");
        assert_eq!(axt(0.52, 0.55, true).punkte, 8);
        assert_eq!(axt(0.0, 0.0, false).punkte, 0);
        assert_eq!(
            axt(0.0, 0.3, true).punkte,
            3,
            "die Klinge reicht bis in den 3er-Ring"
        );
    }

    #[test]
    fn im_gruenen_fliegt_es_gerade() {
        let (u, v, steckt) = landung(Barspiel::Axt, 0.0, 0.0, 0.66, [0.0, 0.0]);
        assert!(steckt && u == 0.0 && v.abs() < 1e-6);
        let (_, _, steckt) = landung(Barspiel::Axt, 0.0, 0.0, 0.2, [0.0, 0.0]);
        assert!(!steckt);
        let (_, v, _) = landung(Barspiel::Darts, 0.0, 0.0, 1.0, [0.0, 0.0]);
        assert!(v > 0.2, "zu fest geworfen fliegt hoch: {v}");
    }

    #[test]
    fn anzeige_pendelt() {
        assert_eq!(anzeige(0.5, 1.0), 0.5);
        assert!((anzeige(1.5, 1.0) - 0.5).abs() < 1e-6);
    }
}
