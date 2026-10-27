//! Himmel, Sonne und Wolken. Die Sonne wandert mit der Uhrzeit des Browsers über den Himmel
//! (wie `shared/sun.ts`, vereinfacht): morgens Osten, mittags Süden, abends Westen, nachts Mond.

use std::f32::consts::PI;

use kiss3d::prelude::*;

use super::{Bau, farbe};

/// Wo die Sonne steht und wie hell es ist, zu einer Tageszeit in Stunden (0..24).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sonnenstand {
    /// Richtung zur Sonne (Einheitsvektor).
    pub richtung: [f32; 3],
    /// 0 = Nacht, 1 = voller Tag.
    pub tag: f32,
}

pub fn sonnenstand(stunde: f32) -> Sonnenstand {
    // Sonnenaufgang 6 Uhr, Untergang 20 Uhr; dazwischen ein Bogen über den Süden (+z).
    let t = ((stunde - 6.0) / 14.0).clamp(-0.2, 1.2);
    let w = t * PI;
    let hoehe = w.sin();
    let richtung = [w.cos(), hoehe.max(0.05), 0.6];
    let l = (richtung[0] * richtung[0] + richtung[1] * richtung[1] + richtung[2] * richtung[2]).sqrt();
    let tag = ((hoehe + 0.15) / 0.35).clamp(0.0, 1.0);
    Sonnenstand { richtung: richtung.map(|v| v / l), tag }
}

/// Die Himmelsfarbe zwischen Nacht und Tag.
pub fn himmelsfarbe(tag: f32) -> Color {
    let nacht = farbe("#0b1d3a");
    let mittag = farbe("#8ecae6");
    let m = |a: f32, b: f32| a + (b - a) * tag;
    Color::new(m(nacht.r, mittag.r), m(nacht.g, mittag.g), m(nacht.b, mittag.b), 1.0)
}

/// Wolken hoch über der Stadt.
pub fn wolken(b: &mut Bau) {
    let weiss = Color::new(1.0, 1.0, 1.0, 1.0);
    for (i, (x, z)) in [(-60.0, -40.0), (30.0, -70.0), (80.0, 20.0), (-90.0, 60.0), (10.0, 90.0), (-20.0, -110.0)].into_iter().enumerate() {
        let y = 55.0 + (i as f32 * 7.0) % 20.0;
        for (dx, dz, r) in [(0.0, 0.0, 6.0), (6.0, 1.0, 4.5), (-5.5, -1.0, 4.0), (2.0, -3.0, 3.5)] {
            let mut k = b.kugel(x + dx, y, z + dz, r, weiss);
            k.set_local_scale(1.4, 0.55, 1.0);
            k.set_emissive(Color::new(0.35, 0.35, 0.35, 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mittags_hoch_nachts_dunkel() {
        let m = sonnenstand(13.0);
        assert!(m.richtung[1] > 0.6 && m.tag == 1.0);
        assert_eq!(sonnenstand(2.0).tag, 0.0);
        assert!(sonnenstand(8.0).richtung[0] > 0.0, "morgens im Osten (+x)");
    }
}
