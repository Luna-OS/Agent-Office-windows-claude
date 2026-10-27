//! Der feste Grundriss einer Etage (wie `shared/layout.ts` im Original).
//!
//! Einheiten sind Meter, +y zeigt nach oben. Der Boden einer Etage liegt bei y = 0 und reicht
//! von `BODEN.min_x..max_x` / `min_z..max_z`. Norden ist -z, Süden (Straße) +z.
//! Dieses Modul braucht kein kiss3d: der Server nutzt es mit `default-features = false`.

pub mod dach;
pub mod draussen;
pub mod sitze;

use std::f32::consts::PI;

/// Ein achsenparalleles Rechteck auf dem Boden.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flaeche {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl Flaeche {
    pub const fn neu(min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Self {
        Self {
            min_x,
            max_x,
            min_z,
            max_z,
        }
    }
    pub fn enthaelt(&self, x: f32, z: f32) -> bool {
        x > self.min_x && x < self.max_x && z > self.min_z && z < self.max_z
    }
    pub fn mitte(&self) -> (f32, f32) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }
    pub fn breite(&self) -> f32 {
        self.max_x - self.min_x
    }
    pub fn tiefe(&self) -> f32 {
        self.max_z - self.min_z
    }
}

/// Der Büroboden.
pub const BODEN: Flaeche = Flaeche::neu(-18.0, 18.0, -13.0, 13.0);
/// Raumhöhe bis zur Decke.
pub const WANDHOEHE: f32 = 6.8;
/// Dicke der Bodenplatte zwischen zwei Etagen.
pub const PLATTE: f32 = 0.3;
/// Von einer Etage zur nächsten.
pub const STOCKWERK: f32 = WANDHOEHE + PLATTE;
/// Dicke der Außenwände (sie stehen direkt außerhalb von `BODEN`).
pub const WANDSTAERKE: f32 = 0.3;
/// Die unterste Etage ist der zweite Stock: so weit liegt die Straße darunter.
pub const STRASSE_Y: f32 = -3.6;

/// Wie weit die Straße unter Etage `index` liegt (0 ist die unterste).
pub fn strasse_unter(index: usize) -> f32 {
    STRASSE_Y - index as f32 * STOCKWERK
}

/// Ein Arbeitsplatz: Schreibtisch, Sitzsack, Kiosk eines Board-Agenten oder Stuhl im Besprechungsraum.
#[derive(Debug, Clone, PartialEq)]
pub struct Tisch {
    pub id: String,
    pub x: f32,
    pub z: f32,
    /// Drehung um y. Bei 0 sitzt der Arbeiter auf der +z-Seite und schaut nach -z.
    pub rot_y: f32,
    pub art: TischArt,
    /// Nur im Hinterbüro: ab dieser Ausbaustufe vorhanden (0 = immer).
    pub fluegel: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TischArt {
    Schreibtisch,
    Sitzsack,
    Kiosk(Board),
    Besprechung,
}

/// Die Wandtafeln.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Board {
    Issues,
    Warteschlange,
    PullRequests,
    Dienste,
}

pub const TISCH_BREITE: f32 = 2.2;
pub const TISCH_TIEFE: f32 = 1.1;
pub const TISCH_HOEHE: f32 = 0.78;

fn tisch(id: String, x: f32, z: f32, rot_y: f32, art: TischArt, fluegel: u32) -> Tisch {
    Tisch {
        id,
        x,
        z,
        rot_y,
        art,
        fluegel,
    }
}

/// Die 16 Schreibtische im Großraum: zwei Inseln je Seite, Rücken an Rücken.
pub fn schreibtische() -> Vec<Tisch> {
    let mut v = Vec::new();
    let mut n = 1;
    for (hinten, vorne) in [(-4.55, -3.45), (3.45, 4.55)] {
        for cx in [-10.5f32, -1.5] {
            for (z, rot) in [(hinten, PI), (vorne, 0.0)] {
                for dx in [-TISCH_BREITE / 2.0, TISCH_BREITE / 2.0] {
                    v.push(tisch(
                        format!("desk-{n}"),
                        cx + dx,
                        z,
                        rot,
                        TischArt::Schreibtisch,
                        0,
                    ));
                    n += 1;
                }
            }
        }
    }
    v
}

/// Das Hinterbüro: eine Bucht durch die Nordwand östlich vom Gong, je Ausbaustufe eine Reihe tiefer.
pub const FLUEGEL_MIN_X: f32 = 13.4;
pub const FLUEGEL_REIHE: f32 = 4.6;
pub const FLUEGEL_REIHEN: u32 = 2;

/// Wie weit nördlich die Rückwand des Hinterbüros bei Ausbau `stufe` liegt.
pub fn fluegel_min_z(stufe: u32) -> f32 {
    BODEN.min_z - stufe.min(FLUEGEL_REIHEN) as f32 * FLUEGEL_REIHE
}

/// Ob (x, z) im Hinterbüro liegt.
pub fn im_fluegel(x: f32, z: f32, stufe: u32) -> bool {
    stufe > 0
        && x > FLUEGEL_MIN_X
        && x < BODEN.max_x
        && z <= BODEN.min_z
        && z > fluegel_min_z(stufe)
}

fn fluegel_tische() -> Vec<Tisch> {
    let x = (FLUEGEL_MIN_X + BODEN.max_x) / 2.0;
    let mut v = Vec::new();
    for i in 0..FLUEGEL_REIHEN {
        let z = BODEN.min_z - (i as f32 + 0.5) * FLUEGEL_REIHE;
        let n = 17 + 2 * i;
        v.push(tisch(
            format!("desk-{n}"),
            x,
            z - TISCH_TIEFE / 2.0,
            PI,
            TischArt::Schreibtisch,
            i + 1,
        ));
        v.push(tisch(
            format!("desk-{}", n + 1),
            x,
            z + TISCH_TIEFE / 2.0,
            0.0,
            TischArt::Schreibtisch,
            i + 1,
        ));
    }
    v
}

const SITZSAECKE: [(f32, f32, f32); 12] = [
    (15.0, -9.8, 0.0),
    (5.4, -9.8, 0.0),
    (-16.1, -9.0, PI / 2.0),
    (-16.1, -3.0, PI / 2.0),
    (-8.8, 10.2, PI),
    (0.8, 10.2, PI),
    (12.2, -5.6, -PI / 2.0),
    (12.2, 5.6, -PI / 2.0),
    (-16.1, 3.0, PI / 2.0),
    (-13.2, -9.8, 0.0),
    (-12.6, 9.2, PI / 2.0),
    (-5.4, -9.8, 0.0),
];

/// Die Kioske der Board-Agenten an der Nordwand, je links neben ihrer Tafel.
pub fn kioske() -> Vec<Tisch> {
    [
        ("station-issues", Board::Issues, -15.6),
        ("station-pulls", Board::PullRequests, 0.0),
        ("station-queue", Board::Warteschlange, -7.8),
    ]
    .into_iter()
    .map(|(id, b, x)| tisch(id.into(), x, BODEN.min_z + 1.3, PI, TischArt::Kiosk(b), 0))
    .collect()
}

/// Alle Plätze, an denen ein Arbeiter sein kann, in der Reihenfolge des Originals.
pub fn alle_tische() -> Vec<Tisch> {
    let mut v = schreibtische();
    v.extend(fluegel_tische());
    v.extend(SITZSAECKE.iter().enumerate().map(|(i, &(x, z, r))| {
        tisch(format!("beanbag-{}", i + 1), x, z, r, TischArt::Sitzsack, 0)
    }));
    v.extend(kioske());
    v.extend(sitze::besprechungs_tische());
    v
}

/// Ein Tisch nach ID.
pub fn tisch_nach_id(id: &str) -> Option<Tisch> {
    alle_tische().into_iter().find(|t| t.id == id)
}

/// Wo der Arbeiter (und wer mit ihm spricht) relativ zum Tisch steht.
pub fn tisch_sitz(t: &Tisch, abstand: f32) -> (f32, f32) {
    (t.x + t.rot_y.sin() * abstand, t.z + t.rot_y.cos() * abstand)
}

/// Eine Wandtafel oder ein Bildschirm: Mitte, Blickrichtung (0 = +z) und Größe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tafel {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_y: f32,
    pub breite: f32,
    pub hoehe: f32,
}

pub fn tafel(b: Board) -> Tafel {
    let nord = BODEN.min_z + 0.08;
    match b {
        Board::Issues => Tafel {
            x: -11.7,
            y: 2.1,
            z: nord,
            rot_y: 0.0,
            breite: 6.0,
            hoehe: 3.0,
        },
        Board::Warteschlange => Tafel {
            x: -3.9,
            y: 2.1,
            z: nord,
            rot_y: 0.0,
            breite: 6.0,
            hoehe: 3.0,
        },
        Board::PullRequests => Tafel {
            x: 3.9,
            y: 2.1,
            z: nord,
            rot_y: 0.0,
            breite: 6.0,
            hoehe: 3.0,
        },
        Board::Dienste => Tafel {
            x: BODEN.max_x - 0.08,
            y: 2.1,
            z: -8.2,
            rot_y: -PI / 2.0,
            breite: 6.0,
            hoehe: 3.0,
        },
    }
}

/// Der große Fernseher an der Ostwand (zeigt geteilte Bildschirme).
pub const TV: Tafel = Tafel {
    x: BODEN.max_x - 0.1,
    y: 2.2,
    z: 0.0,
    rot_y: -PI / 2.0,
    breite: 6.4,
    hoehe: 3.6,
};
/// Der Monitor an der Westwand: wie ausgelastet der Rechner des Office ist.
pub const RECHNER_MONITOR: Tafel = Tafel {
    x: BODEN.min_x + 0.05,
    y: 2.2,
    z: -6.0,
    rot_y: PI / 2.0,
    breite: 2.3,
    hoehe: 1.3,
};

/// Möbel an festen Stellen (x, z).
pub const JUKEBOX: (f32, f32) = (BODEN.max_x - 0.42, 5.4);
pub const ARCADE: (f32, f32) = (BODEN.max_x - 0.42, 7.05);
pub const BUECHERREGAL: (f32, f32) = (-6.5, BODEN.max_z - 0.21);
pub const GONG: (f32, f32) = (11.8, BODEN.min_z + 0.75);
pub const WHITEBOARD: (f32, f32) = (5.4, -5.4);
/// Die Küchenzeile in der Südwest-Ecke mit der Kaffeemaschine.
pub const KUECHE: Flaeche = Flaeche::neu(-17.0, -10.75, 11.7, 12.7);
/// Wo man an der Kaffeemaschine steht (E schenkt einen Becher ein).
pub const KAFFEEMASCHINE: (f32, f32) = (-15.7, 10.9);
/// Der Kühlschrank in der Küche (für sein Brummen).
pub const KUEHLSCHRANK: (f32, f32) = (-11.3, 12.2);
/// Das Teleskop auf der Empore, mit Blick über den Großraum.
pub const TELESKOP: (f32, f32) = (9.6, 8.6);
/// Wo man ohne Etage (oder nach dem Laden) steht.
pub const START: (f32, f32) = (8.0, 7.0);

/// Topfpflanzen: x, z, Größe.
pub const PFLANZEN: [(f32, f32, f32); 8] = [
    (-17.2, -12.2, 1.4),
    (17.2, -12.2, 1.5),
    (17.2, 12.2, 1.3),
    (-17.2, 8.5, 1.2),
    (14.2, -12.2, 1.1),
    (-6.0, 0.0, 1.0),
    (3.5, 0.0, 0.9),
    (8.5, 5.0, 1.1),
];

/// Die Pflanzen, die bei Ausbau `stufe` stehen (die vor dem Hinterbüro wird weggeräumt).
pub fn pflanzen(stufe: u32) -> impl Iterator<Item = (f32, f32, f32)> {
    PFLANZEN
        .into_iter()
        .filter(move |&(x, z, _)| stufe == 0 || !(x > FLUEGEL_MIN_X && z < BODEN.min_z + 1.5))
}

/// Das Chefbüro auf der Empore in der Südost-Ecke.
pub const EMPORE: Flaeche = Flaeche::neu(9.0, BODEN.max_x, 8.0, BODEN.max_z);
pub const EMPORE_Y: f32 = 3.0;
pub const EMPORE_HOEHE: f32 = 2.8;
/// Die Treppe läuft an der Südwand nach Osten hinauf zur Empore.
pub const TREPPE: Flaeche = Flaeche::neu(3.0, EMPORE.min_x, 11.2, BODEN.max_z);
pub const TREPPE_STUFEN: u32 = 15;

/// Bodenhöhe an (x, z) im Büro: Treppe und Empore heben einen an.
pub fn bodenhoehe(x: f32, z: f32, y_jetzt: f32) -> f32 {
    if TREPPE.enthaelt(x, z) {
        let t = ((x - TREPPE.min_x) / TREPPE.breite()).clamp(0.0, 1.0);
        let stufe = (t * TREPPE_STUFEN as f32).floor() / TREPPE_STUFEN as f32;
        return stufe * EMPORE_Y;
    }
    // Auf der Empore steht nur, wer schon oben ist (darunter liegt der Besprechungsraum).
    if EMPORE.enthaelt(x, z) && y_jetzt > EMPORE_Y - 0.6 {
        return EMPORE_Y;
    }
    0.0
}

/// Der Aufzug an der Nordwand zwischen PR-Tafel und Gong.
pub const AUFZUG_X: f32 = 8.5;
pub const AUFZUG_BREITE: f32 = 2.6;
pub const AUFZUG_TIEFE: f32 = 2.4;
pub const AUFZUG_WAND: f32 = 0.14;
pub const AUFZUG_TUER: f32 = 1.4;
pub const AUFZUG_TUER_HOEHE: f32 = 2.4;
pub const AUFZUG_VORNE: f32 = BODEN.min_z + AUFZUG_TIEFE;
/// Das Innere der Kabine.
pub const AUFZUG_KABINE: Flaeche = Flaeche::neu(
    AUFZUG_X - AUFZUG_BREITE / 2.0 + AUFZUG_WAND,
    AUFZUG_X + AUFZUG_BREITE / 2.0 - AUFZUG_WAND,
    BODEN.min_z,
    AUFZUG_VORNE - AUFZUG_WAND,
);
/// Wo man nach einer Fahrt ankommt: mitten in der Kabine, Blick zu den Türen (+z).
pub const AUFZUG_ANKUNFT: (f32, f32) = (AUFZUG_X, (BODEN.min_z + AUFZUG_VORNE - AUFZUG_WAND) / 2.0);

/// Eine Öffnung in einer Außenwand. `u` ist die Mitte entlang der Wand (x bei Nord/Süd, z bei Ost/West).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oeffnung {
    pub wand: Seite,
    pub u: f32,
    pub breite: f32,
    pub unten: f32,
    pub oben: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seite {
    Nord,
    Sued,
    Ost,
    West,
}

/// Glastüren zum Balkon in der Südwand.
pub const BALKON_TUER: Oeffnung = Oeffnung {
    wand: Seite::Sued,
    u: -4.0,
    breite: 3.0,
    unten: 0.0,
    oben: 2.5,
};
/// Die Tür nach draußen in der Westwand (nur auf der untersten Etage), mit Treppe zur Straße.
pub const AUSGANG: Oeffnung = Oeffnung {
    wand: Seite::West,
    u: 6.5,
    breite: 1.4,
    unten: 0.0,
    oben: 2.4,
};

/// Fenster und Türen in den Außenwänden.
pub fn oeffnungen(unterste_etage: bool) -> Vec<Oeffnung> {
    let mut v = Vec::new();
    for u in [-14.0, -9.0, 1.0] {
        v.push(Oeffnung {
            wand: Seite::Sued,
            u,
            breite: 3.0,
            unten: 1.1,
            oben: 3.3,
        });
    }
    for u in [-9.0, -3.0, 3.0] {
        v.push(Oeffnung {
            wand: Seite::West,
            u,
            breite: 3.0,
            unten: 1.1,
            oben: 3.3,
        });
    }
    v.push(Oeffnung {
        wand: Seite::Sued,
        u: EMPORE.min_x + 2.0,
        breite: 2.8,
        unten: EMPORE_Y + 0.9,
        oben: EMPORE_Y + 2.5,
    });
    v.push(Oeffnung {
        wand: Seite::Ost,
        u: (EMPORE.min_z + EMPORE.max_z) / 2.0,
        breite: 2.8,
        unten: EMPORE_Y + 0.9,
        oben: EMPORE_Y + 2.5,
    });
    v.push(BALKON_TUER);
    if unterste_etage {
        v.push(AUSGANG);
    }
    v
}

/// Der Balkon über der Garageneinfahrt.
pub const BALKON: Flaeche = Flaeche::neu(
    -10.5,
    2.5,
    BODEN.max_z + WANDSTAERKE,
    BODEN.max_z + WANDSTAERKE + 3.4,
);
/// Der Aschenbecher auf dem Balkon.
pub const ASCHENBECHER: (f32, f32) = (-8.2, BALKON.max_z - 0.55);
/// Der Golf-Abschlag auf dem Balkon und das Loch auf der anderen Straßenseite (auf Straßenhöhe).
pub const GOLF_ABSCHLAG: (f32, f32) = (-6.75, 14.75);
pub const GOLF_LOCH: (f32, f32) = (-5.0, 58.0);
/// Die Leiter an der Westwand durch Luken in Decke und Boden.
pub const LEITER: (f32, f32) = (BODEN.min_x + 0.62, 0.0);
/// Die Feuerwehrstange zwischen den Tischen und der Lounge.
pub const STANGE: (f32, f32) = (6.8, 1.6);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sechzehn_schreibtische_und_eindeutige_ids() {
        assert_eq!(schreibtische().len(), 16);
        assert_eq!(schreibtische()[0].id, "desk-1");
        let alle = alle_tische();
        let ids: std::collections::HashSet<_> = alle.iter().map(|t| t.id.clone()).collect();
        assert_eq!(ids.len(), alle.len());
        assert!(tisch_nach_id("desk-20").is_some());
        assert!(tisch_nach_id("station-pulls").is_some());
    }

    #[test]
    fn fluegel_waechst_nach_norden() {
        assert_eq!(fluegel_min_z(0), BODEN.min_z);
        assert!(fluegel_min_z(2) < fluegel_min_z(1));
        assert!(im_fluegel(15.0, -15.0, 1));
        assert!(!im_fluegel(15.0, -15.0, 0));
        assert_eq!(fluegel_min_z(9), fluegel_min_z(FLUEGEL_REIHEN));
    }

    #[test]
    fn strasse_liegt_unter_jeder_etage_tiefer() {
        assert_eq!(strasse_unter(0), STRASSE_Y);
        assert!(strasse_unter(2) < strasse_unter(1));
    }

    #[test]
    fn tisch_sitz_liegt_vor_dem_tisch() {
        let t = &schreibtische()[0];
        let (x, z) = tisch_sitz(t, 0.85);
        assert!((x - t.x).abs() < 1e-4);
        assert!((z - (t.z - 0.85)).abs() < 1e-4);
    }

    #[test]
    fn treppe_fuehrt_zur_empore() {
        assert_eq!(bodenhoehe(TREPPE.min_x + 0.01, 12.0, 0.0), 0.0);
        assert!(bodenhoehe(TREPPE.max_x - 0.01, 12.0, 0.0) > EMPORE_Y * 0.9);
        assert_eq!(bodenhoehe(15.0, 10.0, EMPORE_Y), EMPORE_Y);
        assert_eq!(
            bodenhoehe(15.0, 10.0, 0.0),
            0.0,
            "unter der Empore bleibt man unten"
        );
    }

    #[test]
    fn ankunft_liegt_in_der_kabine() {
        assert!(AUFZUG_KABINE.enthaelt(AUFZUG_ANKUNFT.0, AUFZUG_ANKUNFT.1));
    }
}
