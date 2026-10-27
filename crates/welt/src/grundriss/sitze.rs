//! Wo Menschen sitzen können (Couch, Sitzsäcke, Bank, Barhocker …) und der Besprechungsraum.

use std::f32::consts::PI;

use super::{BALKON, BODEN, EMPORE, EMPORE_Y, TV, Tisch, TischArt};
use super::dach::{BAR, FEUERSTELLE};

/// Der Besprechungsraum unter der Empore, mit einem langen Tisch.
pub const BESPRECHUNGS_TISCH: (f32, f32, f32, f32) = (13.7, 10.55, 3.6, 1.2);

/// Die Stühle am Besprechungstisch, in der Reihenfolge, in der eine Besprechung sie füllt.
pub fn besprechungs_tische() -> Vec<Tisch> {
    let (x, z, b, t) = BESPRECHUNGS_TISCH;
    [
        (x - b / 2.0 + 0.35, z, -PI / 2.0),
        (x - 0.6, z - t / 2.0 + 0.35, PI),
        (x - 0.6, z + t / 2.0 - 0.35, 0.0),
        (x + 1.1, z - t / 2.0 + 0.35, PI),
        (x + 1.1, z + t / 2.0 - 0.35, 0.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (x, z, rot_y))| Tisch { id: format!("meeting-{}", i + 1), x, z, rot_y, art: TischArt::Besprechung, fluegel: 0 })
    .collect()
}

/// Etwas zum Hinsetzen. Man sitzt mit Blick `rot_y` (0 = +z).
#[derive(Debug, Clone, PartialEq)]
pub struct Sitz {
    pub id: &'static str,
    /// Was der Hinweis dazu sagt.
    pub name: &'static str,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_y: f32,
    /// Wo die Plätze nebeneinander liegen, seitlich von der Mitte.
    pub plaetze: &'static [f32],
    /// Hüfthöhe über dem Boden.
    pub huefte: f32,
    /// Wie weit vor der Mitte man sitzt.
    pub tiefe: f32,
    /// Beim Aufstehen tritt man so weit nach vorn.
    pub raus: f32,
    /// Auf dem Dach (sonst im Büro).
    pub dach: bool,
    /// An der Bar: E im Sitzen bestellt ein Getränk.
    pub bar: bool,
    /// Blickt auf den TV: im Sitzen kommt der geteilte Bildschirm groß.
    pub tv: bool,
}

const fn sitz(id: &'static str, name: &'static str, x: f32, y: f32, z: f32, rot_y: f32, plaetze: &'static [f32], huefte: f32, tiefe: f32, raus: f32) -> Sitz {
    Sitz { id, name, x, y, z, rot_y, plaetze, huefte, tiefe, raus, dach: false, bar: false, tv: false }
}

/// Alle Sitzgelegenheiten im Büro, auf dem Balkon und auf dem Dach.
pub fn sitze() -> Vec<Sitz> {
    let blick_tv = |x: f32, z: f32| (TV.x - x).atan2(TV.z - z);
    let mut v = vec![
        Sitz { tv: true, ..sitz("couch", "🛋️ Couch", 10.5, 0.0, 0.0, PI / 2.0, &[-1.2, 0.0, 1.2], 0.5, -0.05, 0.9) },
        sitz("lounge-beanbag-1", "🫘 Sitzsack", 12.5, 0.0, 3.5, blick_tv(12.5, 3.5), &[0.0], 0.42, -0.1, 1.2),
        sitz("lounge-beanbag-2", "🫘 Sitzsack", 14.5, 0.0, -3.4, blick_tv(14.5, -3.4), &[0.0], 0.42, -0.1, 1.2),
        sitz("loft-couch", "🛋️ Couch", BODEN.max_x - 0.65, EMPORE_Y, (EMPORE.min_z + EMPORE.max_z) / 2.0, -PI / 2.0, &[-0.5, 0.5], 0.5, -0.05, 0.9),
        sitz("boss-chair", "🪑 Chefsessel", (EMPORE.min_x + EMPORE.max_x) / 2.0 + 0.5, EMPORE_Y, (EMPORE.min_z + EMPORE.max_z) / 2.0 + 0.7, PI, &[0.0], 0.62, -0.05, -0.8),
        sitz("bench", "🪑 Bank", -9.0, 0.0, BALKON.min_z + 0.3, 0.0, &[-0.5, 0.5], 0.47, 0.0, 0.8),
        sitz("stool-1", "🪑 Hocker", -0.6, 0.0, (BALKON.min_z + BALKON.max_z) / 2.0 + 0.2, PI / 2.0, &[0.0], 0.5, 0.0, -0.7),
        sitz("stool-2", "🪑 Hocker", 1.0, 0.0, (BALKON.min_z + BALKON.max_z) / 2.0 + 0.2, -PI / 2.0, &[0.0], 0.5, 0.0, -0.7),
    ];
    const HOCKER: [&str; 6] = ["roof-stool-1", "roof-stool-2", "roof-stool-3", "roof-stool-4", "roof-stool-5", "roof-stool-6"];
    for (i, id) in HOCKER.into_iter().enumerate() {
        let z = BAR.min_z + 0.9 + i as f32 * 1.64;
        v.push(Sitz { dach: true, bar: true, ..sitz(id, "🪑 Barhocker", BAR.x - BAR.tiefe / 2.0 - 0.45, 0.0, z, PI / 2.0, &[0.0], 0.78, 0.0, -0.75) });
    }
    let (fx, fz) = (FEUERSTELLE.0, FEUERSTELLE.1);
    v.push(Sitz { dach: true, ..sitz("roof-sofa-1", "🛋️ Sofa", fx, 0.0, fz - 2.3, 0.0, &[-1.1, 0.0, 1.1], 0.5, -0.05, 0.8) });
    v.push(Sitz { dach: true, ..sitz("roof-sofa-2", "🛋️ Sofa", fx - 2.9, 0.0, fz + 0.4, PI / 2.0, &[-0.6, 0.6], 0.5, -0.05, 0.8) });
    v.push(Sitz { dach: true, ..sitz("roof-sofa-3", "🛋️ Sofa", fx + 2.9, 0.0, fz + 0.4, -PI / 2.0, &[-0.6, 0.6], 0.5, -0.05, 0.8) });
    for (id, x) in [("roof-lounger-1", -2.2), ("roof-lounger-2", 0.6), ("roof-lounger-3", 3.4)] {
        v.push(Sitz { dach: true, ..sitz(id, "🏖️ Liege", x, 0.0, BODEN.max_z - 1.5, 0.0, &[0.0], 0.42, -0.2, -1.0) });
    }
    v
}

/// Ein einzelner Platz auf einem Sitz: wo die Füße stehen, Blick, Hüfthöhe.
#[derive(Debug, Clone, PartialEq)]
pub struct Platz {
    /// Wie im Protokoll (`Person::sitz`): "couch:1".
    pub schluessel: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_y: f32,
    pub huefte: f32,
    pub raus: f32,
}

pub fn platz(s: &Sitz, i: usize) -> Platz {
    let (fx, fz) = (s.rot_y.sin(), s.rot_y.cos());
    let seitlich = s.plaetze.get(i).copied().unwrap_or(0.0);
    Platz {
        schluessel: format!("{}:{i}", s.id),
        x: s.x + fx * s.tiefe + fz * seitlich,
        y: s.y,
        z: s.z + fz * s.tiefe - fx * seitlich,
        rot_y: s.rot_y,
        huefte: s.huefte,
        raus: s.raus,
    }
}

/// Der Platz zu einem Schlüssel wie "couch:1".
pub fn platz_nach_schluessel(schluessel: &str) -> Option<Platz> {
    let (id, i) = schluessel.rsplit_once(':')?;
    let i: usize = i.parse().ok()?;
    let s = sitze().into_iter().find(|s| s.id == id)?;
    (i < s.plaetze.len()).then(|| platz(&s, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schluessel_rundreise() {
        let p = platz_nach_schluessel("couch:2").unwrap();
        assert_eq!(p.schluessel, "couch:2");
        assert!(platz_nach_schluessel("couch:3").is_none());
        assert!(platz_nach_schluessel("gibtsnicht:0").is_none());
        assert!(platz_nach_schluessel("couch").is_none());
    }

    #[test]
    fn fuenf_besprechungsstuehle() {
        assert_eq!(besprechungs_tische().len(), 5);
        assert_eq!(besprechungs_tische()[0].id, "meeting-1");
    }

    #[test]
    fn sitz_ids_eindeutig() {
        let s = sitze();
        let ids: std::collections::HashSet<_> = s.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), s.len());
    }
}
