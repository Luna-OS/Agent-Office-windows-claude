//! Die Burg-Karte (wie `shared/maps/castle.ts`): eine lange Steinhalle mit Holzdach, Tafeln an
//! beiden Seiten, am Nordende auf einem Podest ein Thron aus Eisenklingen, davor die Schlange der
//! Arbeiter, die fertig sind oder warten. Unter dem Südende der Kerker mit neun Zellen.
//!
//! Die Halle läuft von Norden (-z, Thron) nach Süden (+z, Tor). Die Arbeitsplätze haben dieselben
//! IDs wie im Büro (desk-1 …, beanbag-1 …, station-…, meeting-…), damit Arbeiter beim Kartenwechsel
//! ihren Platz behalten.

use std::f32::consts::{FRAC_PI_2, PI};

use super::sitze::besprechungs_tische;
use super::{Board, Flaeche, Tafel, Tisch, TischArt};

/// Die ID der Karte in den Einstellungen (`einstellungen::Karte::wahl`).
pub const BURG_ID: &str = "burg";

pub const BREITE: f32 = 26.0;
pub const LAENGE: f32 = 60.0;
pub const HOEHE: f32 = 13.0;
pub const HALLE: Flaeche = Flaeche::neu(-BREITE / 2.0, BREITE / 2.0, -LAENGE / 2.0, LAENGE / 2.0);
/// Säulen an beiden Seiten, alle 6 m.
pub const SAEULE_X: f32 = 10.4;
pub const SAEULEN_Z: [f32; 9] = [-24.0, -18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0, 24.0];
/// Wo man ankommt: vor dem Podest, mit Blick zum Thron.
pub const ANKUNFT: (f32, f32, f32) = (-2.6, -21.0, PI);
/// Der Thron auf dem Podest (Breite, Tiefe, Höhe).
pub const THRON: (f32, f32) = (0.0, -27.4);
pub const PODEST: (f32, f32, f32) = (12.0, 5.0, 0.9);
/// Der Schlüssel des Throns als Sitzplatz (`Person::sitz`).
pub const THRON_SITZ: &str = "throne:0";

/// Der Platz auf dem Thron: oben auf dem Podest, Blick zur Halle (+z).
pub fn thron_platz() -> super::sitze::Platz {
    super::sitze::Platz {
        schluessel: THRON_SITZ.into(),
        x: THRON.0,
        y: PODEST.2,
        z: THRON.1 + 0.1,
        rot_y: 0.0,
        huefte: 0.55,
        raus: 1.2,
    }
}

/// Die Hand des Königs, links vom Thron.
pub const HAND: (f32, f32, f32) = (3.3, -26.3, -0.35);
/// Die Schlange vor dem Thron: Anfang, Abstand, höchstens so viele.
pub const SCHLANGE: (f32, f32) = (0.0, -25.6);
pub const SCHLANGE_SCHRITT: f32 = 1.3;
pub const SCHLANGE_PLAETZE: usize = 8;
/// Der Ratstisch (Besprechung): Mitte und Radius.
pub const RAT: (f32, f32, f32) = (7.2, -18.8, 1.15);
/// Gong westlich vom Podest, Fass Bier an der Ostwand, Kamin an der Westwand.
pub const GONG: (f32, f32) = (-7.6, -19.6);
pub const FASS: (f32, f32) = (BREITE / 2.0 - 0.65, 15.0);
pub const KAMIN: (f32, f32) = (-BREITE / 2.0 + 0.55, 15.0);

/// Eine lange Tafel: Mitte, Länge, Plätze je Seite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tafelreihe {
    pub x: f32,
    pub z: f32,
    pub laenge: f32,
    pub plaetze: usize,
}
pub const TAFEL_BREITE: f32 = 1.4;
pub const TAFELN: [Tafelreihe; 4] = [
    Tafelreihe {
        x: -6.8,
        z: -8.0,
        laenge: 10.0,
        plaetze: 4,
    },
    Tafelreihe {
        x: 6.8,
        z: -8.0,
        laenge: 10.0,
        plaetze: 4,
    },
    Tafelreihe {
        x: -6.8,
        z: 6.0,
        laenge: 10.0,
        plaetze: 4,
    },
    Tafelreihe {
        x: 6.8,
        z: 6.0,
        laenge: 10.0,
        plaetze: 4,
    },
];

/// Die IDs der Plätze, in der Reihenfolge, in der die Tafeln sie füllen (Schreibtische, dann Sitzsäcke).
fn platz_ids() -> Vec<String> {
    let mut v: Vec<String> = (1..=20).map(|n| format!("desk-{n}")).collect();
    v.extend((1..=12).map(|n| format!("beanbag-{n}")));
    v
}

/// Alle Arbeitsplätze der Burg: erst die Tafelseiten zur Hallenmitte, dann die äußeren.
pub fn tische() -> Vec<Tisch> {
    let mut innen = Vec::new();
    let mut aussen = Vec::new();
    for t in TAFELN {
        for seite in [1.0f32, -1.0] {
            // Die Tafeln laufen nord-süd; +x ist die Ostseite.
            let nach_innen = (seite > 0.0) == (t.x < 0.0);
            let rand = TAFEL_BREITE / 2.0 - 0.35;
            for k in 0..t.plaetze {
                let entlang =
                    (k as f32 - (t.plaetze as f32 - 1.0) / 2.0) * (t.laenge / t.plaetze as f32);
                let platz = (
                    t.x + seite * rand,
                    t.z + entlang,
                    if seite > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 },
                );
                if nach_innen {
                    innen.push(platz)
                } else {
                    aussen.push(platz)
                }
            }
        }
    }
    let mut v: Vec<Tisch> = innen
        .into_iter()
        .chain(aussen)
        .zip(platz_ids())
        .map(|((x, z, rot_y), id)| Tisch {
            id,
            x,
            z,
            rot_y,
            art: TischArt::Schreibtisch,
            fluegel: 0,
        })
        .collect();
    // Die Pulte der Board-Agenten an den Seitenwänden.
    for (id, b, x, z, rot) in [
        ("station-issues", Board::Issues, -11.3, -15.0, -FRAC_PI_2),
        (
            "station-queue",
            Board::Warteschlange,
            -11.3,
            -3.0,
            -FRAC_PI_2,
        ),
        ("station-pulls", Board::PullRequests, 11.3, -15.0, FRAC_PI_2),
    ] {
        v.push(Tisch {
            id: id.into(),
            x,
            z,
            rot_y: rot,
            art: TischArt::Kiosk(b),
            fluegel: 0,
        });
    }
    // Die fünf Stühle am runden Ratstisch.
    for (i, m) in besprechungs_tische().into_iter().enumerate() {
        let w = i as f32 * std::f32::consts::TAU / 5.0;
        v.push(Tisch {
            x: RAT.0 + w.sin() * 0.5,
            z: RAT.1 + w.cos() * 0.5,
            rot_y: w,
            ..m
        });
    }
    v
}

pub fn tisch_nach_id(id: &str) -> Option<Tisch> {
    tische().into_iter().find(|t| t.id == id)
}

/// Die Wandtafeln an den Seitenwänden.
pub fn tafel(b: Board) -> Tafel {
    let w = BREITE / 2.0 - 0.08;
    let (x, z, rot) = match b {
        Board::Issues => (-w, -15.0, FRAC_PI_2),
        Board::Warteschlange => (-w, -3.0, FRAC_PI_2),
        Board::PullRequests => (w, -15.0, -FRAC_PI_2),
        Board::Dienste => (w, -3.0, -FRAC_PI_2),
    };
    Tafel {
        x,
        y: 3.5,
        z,
        rot_y: rot,
        breite: 4.4,
        hoehe: 2.6,
    }
}

/// Der Kerker unter dem Südende der Halle.
pub const KERKER: Flaeche = Flaeche::neu(-11.4, 9.4, 13.6, 29.4);
pub const KERKER_TIEFE: f32 = 4.6;
pub const ZELLEN_TIEFE: f32 = 4.0;
/// Die Treppe hinunter an der Ostwand, südlich der Osttafeln: oben bei z = KERKER.min_z.
pub const TREPPE_X: f32 = 8.3;
pub const TREPPE_BREITE: f32 = 2.2;
/// Mitte des Gangs zwischen den Zellreihen.
pub const GANG_Z: f32 = (KERKER.min_z + ZELLEN_TIEFE + KERKER.max_z - ZELLEN_TIEFE) / 2.0;
/// Der Knochenhaufen am Westende.
pub const KNOCHEN: (f32, f32) = (KERKER.min_x + 1.3, GANG_Z);
/// Der Wächter am Fuß der Treppe.
pub const WAECHTER: (f32, f32) = (6.3, GANG_Z + 0.3);
/// Tod nach so vielen Stunden im Kerker, Knochen nach so vielen weiteren.
pub const VERHUNGERN_H: f32 = 24.0;
pub const VERWESEN_H: f32 = 12.0;

/// Eine Zelle: Mitte ihrer Gitterfront (x, z), Blick der Front, Breite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zelle {
    pub x: f32,
    pub z: f32,
    pub rot_y: f32,
    pub breite: f32,
}

/// Die neun Zellen: vier im Norden (bis vor die Treppe), fünf im Süden.
pub fn zellen() -> Vec<Zelle> {
    let nord_ende = 7.0;
    let nb = (nord_ende - KERKER.min_x) / 4.0;
    let sb = KERKER.breite() / 5.0;
    let mut v: Vec<Zelle> = (0..4)
        .map(|i| Zelle {
            x: KERKER.min_x + nb * (i as f32 + 0.5),
            z: KERKER.min_z + ZELLEN_TIEFE,
            rot_y: 0.0,
            breite: nb,
        })
        .collect();
    v.extend((0..5).map(|i| Zelle {
        x: KERKER.min_x + sb * (i as f32 + 0.5),
        z: KERKER.max_z - ZELLEN_TIEFE,
        rot_y: PI,
        breite: sb,
    }));
    v
}

/// Wo der `i`-te Gefangene liegt oder hockt: reihum eine Zelle nach der anderen, dann tiefer hinein.
pub fn gefangenen_platz(i: usize) -> (f32, f32) {
    let z = zellen();
    let zelle = z[i % z.len()];
    let reihe = (i / z.len()) as f32;
    let quer = ((reihe * 0.37).fract() - 0.5) * zelle.breite * 0.7;
    let tief = 0.8 + (reihe * 0.45) % (ZELLEN_TIEFE - 1.2);
    let (s, c) = zelle.rot_y.sin_cos();
    (zelle.x + quer * c - tief * s, zelle.z - tief * c - quer * s)
}

/// Wie es einem Gefangenen nach `stunden` im Kerker geht.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verfall {
    /// Lebt, 0..1 wie ausgezehrt.
    Lebt(f32),
    /// Tot, 0..1 wie verwest.
    Tot(f32),
    Knochen,
}

pub fn verfall(stunden: f32) -> Verfall {
    if stunden < VERHUNGERN_H {
        Verfall::Lebt((stunden / VERHUNGERN_H).max(0.0))
    } else if stunden < VERHUNGERN_H + VERWESEN_H {
        Verfall::Tot((stunden - VERHUNGERN_H) / VERWESEN_H)
    } else {
        Verfall::Knochen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alle_plaetze_des_bueros_gibt_es_auch_in_der_burg() {
        let burg = tische();
        for id in [
            "desk-1",
            "desk-20",
            "beanbag-12",
            "station-issues",
            "station-queue",
            "station-pulls",
            "meeting-1",
            "meeting-5",
        ] {
            let t = burg
                .iter()
                .find(|t| t.id == id)
                .unwrap_or_else(|| panic!("{id} fehlt"));
            assert!(HALLE.enthaelt(t.x, t.z), "{id} liegt außerhalb der Halle");
        }
        let ids: std::collections::HashSet<_> = burg.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids.len(), burg.len());
    }

    #[test]
    fn innen_zuerst() {
        // desk-1 sitzt an der Seite zur Hallenmitte.
        let t = tisch_nach_id("desk-1").unwrap();
        assert!(t.x.abs() < 6.8);
    }

    #[test]
    fn gefangene_liegen_in_zellen() {
        for i in 0..67 {
            let (x, z) = gefangenen_platz(i);
            assert!(KERKER.enthaelt(x, z), "Gefangener {i} bei {x},{z}");
            assert!(
                !(KERKER.min_z + ZELLEN_TIEFE..KERKER.max_z - ZELLEN_TIEFE).contains(&z),
                "Gefangener {i} liegt im Gang"
            );
        }
    }

    #[test]
    fn sitze_je_karte() {
        use crate::grundriss::Karte;
        assert!(Karte::Burg.sitz(THRON_SITZ, false).is_some());
        assert!(Karte::Burg.sitz("couch:0", false).is_none());
        assert!(Karte::Buero.sitz("couch:0", false).is_some());
        assert!(Karte::Buero.sitz("roof-stool-1:0", false).is_none());
        assert!(Karte::Burg.sitz("roof-stool-1:0", true).is_some());
        assert!(Karte::Buero.sitz(THRON_SITZ, false).is_none());
    }

    #[test]
    fn verfall_der_reihe_nach() {
        assert_eq!(verfall(0.0), Verfall::Lebt(0.0));
        assert!(matches!(verfall(30.0), Verfall::Tot(_)));
        assert_eq!(verfall(40.0), Verfall::Knochen);
    }
}
