//! Draußen und die Spielsachen mit fester Geometrie: Garage mit Autos, Straße, Basketballkorb, Hund.
//!
//! Koordinaten wie im Büro; die Garage und die Straße liegen auf Straßenhöhe
//! (`strasse_unter(index)` unter dem Boden der Etage).

use std::f32::consts::PI;

use super::{BODEN, Flaeche, WANDSTAERKE};

/// Die Straße läuft vor dem Gebäude (Süden) von Ost nach West, mit Gehwegen an beiden Seiten.
pub const STRASSE: (f32, f32) = (23.0, 31.0);
/// So weit reicht die Straße nach Osten und Westen.
pub const STRASSE_ENDE: f32 = 110.0;

/// Der Grundriss des Gebäudes mit Wänden; die Garage liegt darunter.
pub const GEBAEUDE: Flaeche = Flaeche::neu(
    BODEN.min_x - WANDSTAERKE,
    BODEN.max_x + WANDSTAERKE,
    BODEN.min_z - WANDSTAERKE,
    BODEN.max_z + WANDSTAERKE,
);
/// Der Hof vor der Garage bis zum Gehweg, und der an der Ostseite.
pub const HOF: Flaeche = Flaeche::neu(-30.0, 30.0, GEBAEUDE.max_z, 21.0);
pub const SEITENHOF: Flaeche = Flaeche::neu(
    GEBAEUDE.max_x,
    GEBAEUDE.max_x + 12.0,
    GEBAEUDE.min_z - 2.0,
    GEBAEUDE.max_z + 4.0,
);

/// Wo ein Auto fahren darf.
pub const BEFAHRBAR: [Flaeche; 7] = [
    Flaeche::neu(BODEN.min_x, GEBAEUDE.max_x, BODEN.min_z, GEBAEUDE.max_z),
    Flaeche::neu(HOF.min_x, HOF.max_x, HOF.min_z, STRASSE.0),
    SEITENHOF,
    Flaeche::neu(-STRASSE_ENDE, STRASSE_ENDE, STRASSE.0, STRASSE.1),
    RUNDKURS[0],
    RUNDKURS[1],
    RUNDKURS[2],
];

/// Der Rundkurs: von beiden Enden der Straße hinter der Nachbarschaft herum (West, Nord, Ost);
/// mit der Straße zusammen ein geschlossener Ring.
pub const RUNDKURS: [Flaeche; 3] = [
    Flaeche::neu(-STRASSE_ENDE - 8.0, -STRASSE_ENDE, -68.0, STRASSE.1),
    Flaeche::neu(-STRASSE_ENDE - 8.0, STRASSE_ENDE + 8.0, -68.0, -60.0),
    Flaeche::neu(STRASSE_ENDE, STRASSE_ENDE + 8.0, -68.0, STRASSE.1),
];

/// Ein Auto: Länge (Nase bis Heck), Breite, Höhe der Karosserie und des Dachs.
pub const AUTO_LAENGE: f32 = 4.6;
pub const AUTO_BREITE: f32 = 2.0;
pub const AUTO_KAROSSERIE: f32 = 0.82;
pub const AUTO_DACH: f32 = 1.12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoArt {
    Lambo,
    Ferrari,
}

/// Ein Auto in der Garage: Art, Farbe, Name, Parkplatz und Blick der Nase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoPlatz {
    pub art: AutoArt,
    pub farbe: &'static str,
    pub name: &'static str,
    pub x: f32,
    pub z: f32,
    pub rot_y: f32,
}

const HINTEN: f32 = GEBAEUDE.min_z + WANDSTAERKE + 0.4 + AUTO_LAENGE / 2.0;
const VORNE: f32 = GEBAEUDE.max_z - 0.5 - AUTO_LAENGE / 2.0;

/// Die Autos auf ihren Parkplätzen (Index = Auto-Nummer im Protokoll).
pub const AUTOS: [AutoPlatz; 9] = [
    AutoPlatz {
        art: AutoArt::Lambo,
        farbe: "#8ac926",
        name: "Limetten-Lambo",
        x: -14.4,
        z: HINTEN,
        rot_y: PI,
    },
    AutoPlatz {
        art: AutoArt::Lambo,
        farbe: "#ff7b00",
        name: "Oranger Lambo",
        x: -8.0,
        z: HINTEN,
        rot_y: PI,
    },
    AutoPlatz {
        art: AutoArt::Lambo,
        farbe: "#ffd000",
        name: "Gelber Lambo",
        x: 1.6,
        z: HINTEN,
        rot_y: PI,
    },
    AutoPlatz {
        art: AutoArt::Lambo,
        farbe: "#7b2cbf",
        name: "Lila Lambo",
        x: 11.2,
        z: HINTEN,
        rot_y: PI,
    },
    AutoPlatz {
        art: AutoArt::Ferrari,
        farbe: "#d90429",
        name: "Roter Ferrari",
        x: -14.4,
        z: VORNE,
        rot_y: 0.0,
    },
    AutoPlatz {
        art: AutoArt::Ferrari,
        farbe: "#d90429",
        name: "Rosso-Ferrari",
        x: -4.8,
        z: VORNE,
        rot_y: 0.0,
    },
    AutoPlatz {
        art: AutoArt::Ferrari,
        farbe: "#ffc300",
        name: "Giallo-Ferrari",
        x: 4.8,
        z: VORNE,
        rot_y: 0.0,
    },
    AutoPlatz {
        art: AutoArt::Ferrari,
        farbe: "#e5383b",
        name: "Scharlach-Ferrari",
        x: 14.4,
        z: VORNE,
        rot_y: 0.0,
    },
    AutoPlatz {
        art: AutoArt::Lambo,
        farbe: "#00b4d8",
        name: "Blauer Lambo",
        x: 9.0,
        z: 18.2,
        rot_y: PI / 2.0,
    },
];

/// Wo Fahrer und Beifahrer sitzen, im Rahmen des Autos (x quer, +x links; z zur Nase).
pub const FAHRER: (f32, f32) = (0.42, -0.5);
pub const BEIFAHRER: (f32, f32) = (-0.42, -0.5);
pub const SITZ_HUEFTE: f32 = 0.45;

/// Wie ein Auto gerade steht und fährt.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AutoLage {
    pub x: f32,
    pub z: f32,
    pub rot_y: f32,
    /// m/s entlang der Nase (rückwärts negativ).
    pub tempo: f32,
    /// Einschlag der Vorderräder (+ ist links).
    pub lenkung: f32,
}

/// Pedale und Lenkrad: `gas` 1 vor, -1 zurück; `lenken` +1 voll links.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pedale {
    pub gas: f32,
    pub lenken: f32,
    pub bremse: bool,
}

const SPITZE: f32 = 20.0;
const RUECKWAERTS: f32 = 7.0;
const BESCHLEUNIGUNG: f32 = 8.0;
const RUECK_BESCHL: f32 = 5.0;
const BREMSE: f32 = 20.0;
const ROLLEN: f32 = 2.5;
const RADSTAND: f32 = 2.8;
const EINSCHLAG: f32 = 0.6;
const LENK_TEMPO: f32 = 2.8;

/// Das Auto `dt` Sekunden später (Fahrradmodell, kein Rutschen; wie im Original).
pub fn fahren(p: AutoLage, pedale: Pedale, dt: f32) -> AutoLage {
    let grenze = EINSCHLAG / (1.0 + p.tempo.abs() / 9.0);
    let ziel = pedale.lenken.clamp(-1.0, 1.0) * grenze;
    let lenkung = p.lenkung + (ziel - p.lenkung).clamp(-LENK_TEMPO * dt, LENK_TEMPO * dt);
    let mut v = p.tempo;
    let hin = |v: &mut f32, ziel: f32, rate: f32| *v += (ziel - *v).clamp(-rate * dt, rate * dt);
    let gas = pedale.gas.clamp(-1.0, 1.0);
    if pedale.bremse {
        hin(&mut v, 0.0, BREMSE);
    } else if gas > 0.0 {
        if v < 0.0 {
            hin(&mut v, 0.0, BREMSE)
        } else {
            v = (v + BESCHLEUNIGUNG * gas * dt).min(SPITZE)
        }
    } else if gas < 0.0 {
        if v > 0.0 {
            hin(&mut v, 0.0, BREMSE)
        } else {
            v = (v + RUECK_BESCHL * gas * dt).max(-RUECKWAERTS)
        }
    } else {
        hin(&mut v, 0.0, ROLLEN);
    }
    let gier = v * lenkung.tan() / RADSTAND;
    let mitte = p.rot_y + gier * dt / 2.0;
    let neu = p.rot_y + gier * dt;
    AutoLage {
        x: p.x + mitte.sin() * v * dt,
        z: p.z + mitte.cos() * v * dt,
        rot_y: neu.sin().atan2(neu.cos()),
        tempo: v,
        lenkung,
    }
}

/// Ein Punkt im Rahmen des Autos, in Weltkoordinaten.
pub fn auto_punkt(p: &AutoLage, lx: f32, lz: f32) -> (f32, f32) {
    let (s, c) = p.rot_y.sin_cos();
    (p.x + lx * c + lz * s, p.z - lx * s + lz * c)
}

/// Ob das ganze Auto auf befahrbarem Grund steht.
pub fn auf_dem_asphalt(p: &AutoLage) -> bool {
    let (w, l) = (AUTO_BREITE / 2.0, AUTO_LAENGE / 2.0);
    [
        (w, l),
        (-w, l),
        (w, -l),
        (-w, -l),
        (w, 0.0),
        (-w, 0.0),
        (0.0, l),
        (0.0, -l),
    ]
    .iter()
    .all(|&(lx, lz)| {
        let (x, z) = auto_punkt(p, lx, lz);
        BEFAHRBAR
            .iter()
            .any(|b| x >= b.min_x && x <= b.max_x && z >= b.min_z && z <= b.max_z)
    })
}

/// Der Basketballkorb an der Westwand zwischen Ausgang und Küche, mit Blick in den Raum (+x).
pub const KORB_Z: f32 = 10.1;
pub const KORB_BRETT_X: f32 = BODEN.min_x + 0.62;
/// Mitte des Rings (x, y, z) und sein Radius.
pub const RING: (f32, f32, f32, f32) = (KORB_BRETT_X + 0.4, 3.05, KORB_Z, 0.25);
pub const BALL_RADIUS: f32 = 0.12;
/// Wo der Ball liegt, wenn ihn keiner hat.
pub const BALL_RUHE: (f32, f32) = (KORB_BRETT_X + 0.7, KORB_Z - 0.8);
pub const BALL_MAX_TEMPO: f32 = 16.0;
pub const SCHWERKRAFT: f32 = 9.81;

/// Wo ein geworfener Ball nach `t` Sekunden ist, ohne Aufprall (für die Flugbahn).
pub fn ballflug(start: [f32; 3], v: [f32; 3], t: f32) -> [f32; 3] {
    [
        start[0] + v[0] * t,
        start[1] + v[1] * t - 0.5 * SCHWERKRAFT * t * t,
        start[2] + v[2] * t,
    ]
}

/// Punkte, an denen der Hund gern vorbeischnüffelt (der Server wählt daraus seine Wege).
pub const HUND_PUNKTE: [(f32, f32); 8] = [
    (6.0, 2.0),
    (-4.0, 0.0),
    (10.0, -2.0),
    (-12.0, 0.5),
    (2.0, 8.5),
    (-8.0, 8.0),
    (14.0, 2.0),
    (0.0, -8.0),
];
/// Das Hundekörbchen in der Lounge.
pub const HUND_KORB: (f32, f32) = (15.6, -1.6);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_beschleunigt_und_rollt_aus() {
        let mut p = AutoLage {
            x: 0.0,
            z: 0.0,
            rot_y: 0.0,
            ..Default::default()
        };
        for _ in 0..60 {
            p = fahren(
                p,
                Pedale {
                    gas: 1.0,
                    ..Default::default()
                },
                1.0 / 60.0,
            );
        }
        assert!(p.tempo > 7.0 && p.z > 3.0 && p.x.abs() < 1e-3);
        for _ in 0..600 {
            p = fahren(p, Pedale::default(), 1.0 / 60.0);
        }
        assert_eq!(p.tempo, 0.0);
    }

    #[test]
    fn bremsen_vor_rueckwaerts() {
        let p = AutoLage {
            tempo: 5.0,
            ..Default::default()
        };
        let q = fahren(
            p,
            Pedale {
                gas: -1.0,
                ..Default::default()
            },
            0.1,
        );
        assert!(q.tempo < 5.0 && q.tempo >= 0.0);
    }

    #[test]
    fn parkplaetze_sind_befahrbar() {
        for a in AUTOS {
            let p = AutoLage {
                x: a.x,
                z: a.z,
                rot_y: a.rot_y,
                ..Default::default()
            };
            assert!(
                auf_dem_asphalt(&p),
                "{} steht nicht auf dem Asphalt",
                a.name
            );
        }
        assert!(!auf_dem_asphalt(&AutoLage {
            x: 0.0,
            z: 60.0,
            ..Default::default()
        }));
    }

    #[test]
    fn ball_faellt() {
        let p = ballflug([0.0, 2.0, 0.0], [1.0, 0.0, 0.0], 0.5);
        assert!(p[1] < 2.0 && (p[0] - 0.5).abs() < 1e-5);
    }
}
