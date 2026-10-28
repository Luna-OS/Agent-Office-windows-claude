//! Der Himmel über dem Office: Ort (für die Sonne) und Wetter. Ohne Stadt wandert das Wetter von
//! selbst, eine Wetterlage nach der anderen, Schnee nur im Winter, Gewitter im Sommer. Mit `--stadt`
//! folgt es der Vorhersage von open-meteo.com (ohne Schlüssel).

use std::sync::Mutex;

use protokoll::einstellungen::{Himmel, Wetter};
use rand::Rng;

pub struct HimmelWetter {
    pub stadt: Option<String>,
    stand: Mutex<Himmel>,
}

/// Ein grober Ort aus der Zeitzone, damit Sonnenauf- und -untergang ungefähr stimmen.
fn ort_raten(versatz_min: i32) -> (f32, f32) {
    let laenge = (versatz_min as f32 / 60.0 * 15.0).clamp(-180.0, 180.0);
    let breite = match versatz_min {
        -600..=-180 => 40.0,
        -179..=180 => 51.0,
        181..=420 => 28.0,
        _ => 35.0,
    };
    (breite, laenge)
}

/// WMO-Wettercodes (open-meteo) als unser Wetter und wie stark.
pub fn aus_wmo(code: u32) -> (Wetter, f32) {
    match code {
        0 | 1 => (Wetter::Klar, 0.0),
        2 => (Wetter::Bewoelkt, 0.4),
        3 => (Wetter::Bewoelkt, 1.0),
        45 => (Wetter::Nebel, 0.8),
        48 => (Wetter::Nebel, 1.0),
        51 => (Wetter::Regen, 0.2),
        53 | 56 => (Wetter::Regen, 0.3),
        55 => (Wetter::Regen, 0.4),
        57 | 61 => (Wetter::Regen, 0.45),
        63 => (Wetter::Regen, 0.7),
        65 | 82 => (Wetter::Regen, 1.0),
        66 | 80 => (Wetter::Regen, 0.5),
        67 => (Wetter::Regen, 0.9),
        81 => (Wetter::Regen, 0.75),
        71 => (Wetter::Schnee, 0.35),
        73 => (Wetter::Schnee, 0.65),
        75 | 86 => (Wetter::Schnee, 1.0),
        77 => (Wetter::Schnee, 0.3),
        85 => (Wetter::Schnee, 0.6),
        95 => (Wetter::Gewitter, 0.8),
        96 | 99 => (Wetter::Gewitter, 1.0),
        _ => (Wetter::Bewoelkt, 0.5),
    }
}

impl HimmelWetter {
    pub fn neu(stadt: Option<String>) -> Self {
        let versatz = super::utc_versatz();
        let (breite, laenge) = ort_raten(versatz);
        Self {
            stadt,
            stand: Mutex::new(Himmel {
                breite,
                laenge,
                utc_versatz: versatz,
                wetter: Wetter::Klar,
                staerke: 0.0,
                stadt: None,
                temperatur: None,
            }),
        }
    }

    pub fn stand(&self) -> Himmel {
        self.stand.lock().unwrap().clone()
    }

    pub fn setzen(&self, h: Himmel) {
        *self.stand.lock().unwrap() = h;
    }

    /// Die nächste ausgedachte Wetterlage: eher bleibt es, wie es ist.
    pub fn wandern(&self, monat: u32) -> Himmel {
        let mut rng = rand::rng();
        let mut h = self.stand.lock().unwrap();
        if rng.random_bool(0.55) {
            h.staerke = (h.staerke + rng.random_range(-0.15..0.15)).clamp(0.0, 1.0);
            return h.clone();
        }
        let winter = matches!(monat, 12 | 1 | 2);
        let sommer = matches!(monat, 6..=8);
        let wahl: &[(Wetter, u32)] = &[
            (Wetter::Klar, 40),
            (Wetter::Bewoelkt, 25),
            (Wetter::Regen, 15),
            (Wetter::Nebel, 6),
            (Wetter::Schnee, if winter { 14 } else { 0 }),
            (Wetter::Gewitter, if sommer { 8 } else { 1 }),
        ];
        let summe: u32 = wahl.iter().map(|(_, g)| g).sum();
        let mut los = rng.random_range(0..summe);
        for (w, g) in wahl {
            if los < *g {
                h.wetter = *w;
                break;
            }
            los -= g;
        }
        h.staerke = if h.wetter == Wetter::Klar {
            0.0
        } else {
            rng.random_range(0.2..1.0)
        };
        h.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kein_schnee_im_sommer() {
        let h = HimmelWetter::neu(None);
        for _ in 0..500 {
            assert_ne!(h.wandern(7).wetter, Wetter::Schnee);
        }
    }

    #[test]
    fn wmo() {
        assert_eq!(aus_wmo(0).0, Wetter::Klar);
        assert_eq!(aus_wmo(95).0, Wetter::Gewitter);
    }
}
