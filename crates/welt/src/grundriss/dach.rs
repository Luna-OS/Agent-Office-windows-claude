//! Die Dachterrasse über der obersten Etage: Bühne mit DJ, Tanzfläche, Bar, Feuerstelle,
//! Liegen, Darts und Axtwurf – und die große Leinwand für YouTube.
//!
//! Das Dach liegt (wie jede Etage) bei y = 0 und hat dieselbe Fläche, damit der Aufzug an
//! derselben Stelle ankommt. Die Straße liegt `dach_tiefe(etagen)` darunter.

use super::{BODEN, Flaeche, strasse_unter};

/// Die Etagen-ID, unter der der Server das Dach führt (`protokoll::etagen::DACH`).
pub const DACH_ID: &str = "@dach";

/// Wie weit die Straße unter dem Dach liegt, bei `etagen` Etagen darunter.
pub fn dach_tiefe(etagen: usize) -> f32 {
    -strasse_unter(etagen.max(1))
}

/// Die Bühne des DJs an der Nordkante westlich vom Aufzug, mit der Tanzfläche davor.
pub const BUEHNE: Flaeche = Flaeche::neu(-8.0, 2.0, BODEN.min_z, -9.2);
pub const BUEHNE_HOEHE: f32 = 0.6;
/// Wo der DJ hinter dem Pult steht.
pub const DJ: (f32, f32) = (-3.0, -11.3);
/// LED-Kacheln, je ein Meter, die zur Musik leuchten.
pub const TANZFLAECHE: Flaeche = Flaeche::neu(-8.0, 2.0, -9.2, -2.2);

/// Die Bar an der Ostseite: x ist die Mitte der Theke.
pub struct Bar {
    pub x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub tiefe: f32,
    pub hoehe: f32,
}
pub const BAR: Bar = Bar {
    x: 12.95,
    min_z: -6.0,
    max_z: 4.0,
    tiefe: 0.7,
    hoehe: 1.1,
};

/// Die Feuerstelle in der Lounge (Südwest), mit Sofas an drei Seiten: x, z, Radius.
pub const FEUERSTELLE: (f32, f32, f32) = (-12.0, 8.2, 0.9);
/// Stehtische zwischen Aufzug und Bar.
pub const STEHTISCHE: [(f32, f32); 3] = [(6.6, 5.2), (9.6, 8.8), (6.0, 10.8)];

/// Die Axtwurfbahn in der Nordwest-Ecke und die Dartscheibe außen an ihrer Ostwand.
pub const AXTBAHN: Flaeche = Flaeche::neu(
    BODEN.min_x + 0.15,
    BODEN.min_x + 2.95,
    BODEN.min_z,
    BODEN.min_z + 3.4,
);
pub const AXT_ZIEL: (f32, f32, f32) = (BODEN.min_x + 1.55, 1.5, BODEN.min_z + 0.25);
pub const AXT_LINIE: f32 = 3.66;
pub const DARTSCHEIBE: (f32, f32, f32) =
    (BODEN.min_x + 2.95 + 0.15 + 0.08, 1.73, BODEN.min_z + 1.8);
pub const DART_ABSTAND: f32 = 2.37;

/// Die große Leinwand über der Bühne, mit Blick nach Süden über die Tanzfläche.
/// Mitte (x, y, z), Breite und Höhe der Bildfläche in Metern (16:9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leinwand {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub breite: f32,
    pub hoehe: f32,
}
pub const LEINWAND: Leinwand = Leinwand {
    x: -3.0,
    y: 5.0,
    z: BODEN.min_z + 0.35,
    breite: 8.0,
    hoehe: 4.5,
};

impl Leinwand {
    /// Die vier Ecken der Bildfläche: links oben, rechts oben, rechts unten, links unten
    /// (von vorn gesehen, also von Süden).
    pub fn ecken(&self) -> [[f32; 3]; 4] {
        let (b, h) = (self.breite / 2.0, self.hoehe / 2.0);
        [
            [self.x - b, self.y + h, self.z],
            [self.x + b, self.y + h, self.z],
            [self.x + b, self.y - h, self.z],
            [self.x - b, self.y - h, self.z],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Die Leinwand liegt über der Bühne, und der DJ passt darunter.
    const _: () = assert!(LEINWAND.x - LEINWAND.breite / 2.0 >= BUEHNE.min_x - 0.01);
    const _: () = assert!(LEINWAND.x + LEINWAND.breite / 2.0 <= BUEHNE.max_x + 0.01);
    const _: () = assert!(LEINWAND.y - LEINWAND.hoehe / 2.0 > BUEHNE_HOEHE + 1.8);

    #[test]
    fn leinwand_ecken_im_uhrzeigersinn() {
        let e = LEINWAND.ecken();
        assert!(e[0][0] < e[1][0] && e[0][1] > e[3][1]);
    }

    #[test]
    fn dach_ist_hoeher_mit_mehr_etagen() {
        assert!(dach_tiefe(3) > dach_tiefe(1));
        assert_eq!(dach_tiefe(0), dach_tiefe(1));
    }
}
