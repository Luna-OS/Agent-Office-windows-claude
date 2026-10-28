//! Die Burg und andere Hallen-Karten (wie `shared/maps/castle.ts`): eine lange Steinhalle mit Tafeln,
//! am Nordende auf einem Podest ein Thron, davor die Schlange der Arbeiter, die fertig sind oder
//! warten. Unter dem Südende der Kerker mit neun Zellen. Wo was steht, sagt der `Hallenplan`
//! (`halle.rs`); die Burg ist der eingebaute Plan, eigene Karten überschreiben ihn.
//!
//! Die Arbeitsplätze haben dieselben IDs wie im Büro (desk-1 …, beanbag-1 …, station-…, meeting-…),
//! damit Arbeiter beim Kartenwechsel ihren Platz behalten.

use std::f32::consts::PI;

use super::halle::Hallenplan;
use super::sitze::besprechungs_tische;
use super::{Board, Flaeche, Tafel, Tisch, TischArt};

/// Die ID der Burg in den Einstellungen (`einstellungen::Karte::wahl`).
pub const BURG_ID: &str = "burg";

/// Das Podest des Throns (Breite, Tiefe, Höhe).
pub const PODEST: (f32, f32, f32) = (12.0, 5.0, 0.9);
/// Der Schlüssel des Throns als Sitzplatz (`Person::sitz`).
pub const THRON_SITZ: &str = "throne:0";
pub const SCHLANGE_SCHRITT: f32 = 1.3;
pub const SCHLANGE_PLAETZE: usize = 8;
/// Radius des runden Ratstischs.
pub const RAT_RADIUS: f32 = 1.15;
pub const TAFEL_BREITE: f32 = 1.4;

/// Die Fläche der Halle.
pub fn halle(p: &Hallenplan) -> Flaeche {
    Flaeche::neu(
        -p.breite / 2.0,
        p.breite / 2.0,
        -p.laenge / 2.0,
        p.laenge / 2.0,
    )
}

/// Der Platz auf dem Thron: oben auf dem Podest, Blick zur Halle (+z).
pub fn thron_platz(p: &Hallenplan) -> Option<super::sitze::Platz> {
    let t = p.thron?;
    Some(super::sitze::Platz {
        schluessel: THRON_SITZ.into(),
        x: t.x,
        y: PODEST.2,
        z: t.z + 0.1,
        rot_y: 0.0,
        huefte: 0.55,
        raus: 1.2,
    })
}

/// Der Anfang der Schlange vor dem Thron.
pub fn schlange(p: &Hallenplan) -> Option<(f32, f32)> {
    p.thron.map(|t| (t.x, t.z + 1.8))
}

/// Die Hand des Königs, links vom Thron (x, z, Blick).
pub fn hand(p: &Hallenplan) -> Option<(f32, f32, f32)> {
    p.thron.map(|t| (t.x + 3.3, t.z + 1.1, -0.35))
}

/// Gong westlich vom Podest.
pub fn gong(p: &Hallenplan) -> (f32, f32) {
    let (tx, tz) = p
        .thron
        .map(|t| (t.x, t.z))
        .unwrap_or((0.0, -p.laenge / 2.0 + 2.6));
    (tx - 7.6, tz + 7.8)
}

/// Fass Bier an der Ostwand.
pub fn fass(p: &Hallenplan) -> (f32, f32) {
    (p.breite / 2.0 - 0.65, p.laenge / 2.0 - 15.0)
}

/// Kamin an der Westwand.
pub fn kamin(p: &Hallenplan) -> (f32, f32) {
    (-p.breite / 2.0 + 0.55, p.laenge / 2.0 - 15.0)
}

/// Die IDs der Plätze, in der Reihenfolge, in der die Tafeln sie füllen (Schreibtische, dann Sitzsäcke).
fn platz_ids() -> Vec<String> {
    let mut v: Vec<String> = (1..=20).map(|n| format!("desk-{n}")).collect();
    v.extend((1..=12).map(|n| format!("beanbag-{n}")));
    v
}

/// Alle Arbeitsplätze der Halle: erst die Tafelseiten zur Hallenmitte, dann die äußeren.
pub fn tische(p: &Hallenplan) -> Vec<Tisch> {
    let mut innen = Vec::new();
    let mut aussen = Vec::new();
    for t in &p.tafeln {
        for seite in [1.0f32, -1.0] {
            // Die Tafeln laufen nord-süd; +x ist die Ostseite.
            let nach_innen = (seite > 0.0) == (t.x < 0.0);
            let rand = TAFEL_BREITE / 2.0 - 0.35;
            let n = t.plaetze as usize;
            for k in 0..n {
                let entlang = (k as f32 - (n as f32 - 1.0) / 2.0) * (t.laenge / n as f32);
                let platz = (
                    t.x + seite * rand,
                    t.z + entlang,
                    if seite > 0.0 { PI / 2.0 } else { -PI / 2.0 },
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
    // Die Pulte der Board-Agenten.
    for (id, b, o) in [
        ("station-issues", Board::Issues, p.pulte.issues),
        ("station-queue", Board::Warteschlange, p.pulte.queue),
        ("station-pulls", Board::PullRequests, p.pulte.pulls),
    ] {
        v.push(Tisch {
            id: id.into(),
            x: o.x,
            z: o.z,
            rot_y: o.blick,
            art: TischArt::Kiosk(b),
            fluegel: 0,
        });
    }
    // Die fünf Stühle am runden Ratstisch.
    for (i, m) in besprechungs_tische().into_iter().enumerate() {
        let w = p.rat.blick + i as f32 * std::f32::consts::TAU / 5.0;
        v.push(Tisch {
            x: p.rat.x + w.sin() * 0.5,
            z: p.rat.z + w.cos() * 0.5,
            rot_y: w,
            ..m
        });
    }
    v
}

pub fn tisch_nach_id(p: &Hallenplan, id: &str) -> Option<Tisch> {
    tische(p).into_iter().find(|t| t.id == id)
}

/// Die Wandtafeln.
pub fn tafel(p: &Hallenplan, b: Board) -> Tafel {
    let w = match b {
        Board::Issues => p.wandtafeln.issues,
        Board::Warteschlange => p.wandtafeln.queue,
        Board::PullRequests => p.wandtafeln.pulls,
        Board::Dienste => p.wandtafeln.dienste,
    };
    Tafel {
        x: w.x,
        y: w.y,
        z: w.z,
        rot_y: w.blick,
        breite: w.breite,
        hoehe: w.hoehe,
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
        let p = Hallenplan::burg();
        let burg = tische(&p);
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
            assert!(
                halle(&p).enthaelt(t.x, t.z),
                "{id} liegt außerhalb der Halle"
            );
        }
        let ids: std::collections::HashSet<_> = burg.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids.len(), burg.len());
        assert!(
            tisch_nach_id(&p, "desk-1").unwrap().x.abs() < 6.8,
            "innen zuerst"
        );
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
        let burg = Karte::aus_wahl(BURG_ID);
        assert!(burg.sitz(THRON_SITZ, false).is_some());
        assert!(burg.sitz("couch:0", false).is_none());
        assert!(Karte::Buero.sitz("couch:0", false).is_some());
        assert!(Karte::Buero.sitz("roof-stool-1:0", false).is_none());
        assert!(burg.sitz("roof-stool-1:0", true).is_some());
        assert!(Karte::Buero.sitz(THRON_SITZ, false).is_none());
    }

    #[test]
    fn eigene_karte_aus_den_einstellungen() {
        use crate::grundriss::Karte;
        let k: protokoll::einstellungen::Karte = serde_json::from_value(serde_json::json!({
            "wahl": "meine",
            "eigene": [{"id": "meine", "name": "Meine", "daten": {"name": "Meine", "kerker": false, "thron": null}}]
        }))
        .unwrap();
        let karte = Karte::aus_einstellungen(&k);
        let p = karte.halle().expect("eine Halle");
        assert_eq!(p.name, "Meine");
        assert!(!karte.kerker());
        assert!(
            karte.sitz(THRON_SITZ, false).is_none(),
            "ohne Thron kein Thronsitz"
        );
        assert!(karte.tisch("desk-1").is_some());
    }

    #[test]
    fn verfall_der_reihe_nach() {
        assert_eq!(verfall(0.0), Verfall::Lebt(0.0));
        assert!(matches!(verfall(30.0), Verfall::Tot(_)));
        assert_eq!(verfall(40.0), Verfall::Knochen);
    }
}
