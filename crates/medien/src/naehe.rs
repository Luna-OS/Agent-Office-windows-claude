//! Lautstärke nach Entfernung: für Voice (Nähe-Sprache) und die Leinwand auf dem Dach.

/// Voice: laut in der Nähe, leiser mit Abstand, aber nie ganz still (wie im Original).
pub fn sprach_lautstaerke(abstand: f32) -> f32 {
    const NAH: f32 = 3.0;
    const FERN: f32 = 18.0;
    const MIN: f32 = 0.15;
    if abstand <= NAH {
        1.0
    } else if abstand >= FERN {
        MIN
    } else {
        let t = (abstand - NAH) / (FERN - NAH);
        1.0 - (1.0 - MIN) * t * t.sqrt()
    }
}

/// Leinwand: voll bis `nah`, dann bis `fern` auf 0 (quadratisch, damit es natürlich abklingt).
/// Ergebnis 0..=100 wie bei der YouTube-API (`setVolume`).
pub fn leinwand_lautstaerke(abstand: f32, nah: f32, fern: f32) -> u8 {
    if !abstand.is_finite() || abstand >= fern {
        return 0;
    }
    if abstand <= nah {
        return 100;
    }
    let t = 1.0 - (abstand - nah) / (fern - nah);
    (t * t * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Ab welchem Abstand das YouTube-Embed eingeblendet wird (und darunter auch hörbar ist).
pub const LEINWAND_NAH: f32 = 6.0;
pub const LEINWAND_FERN: f32 = 28.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_nie_ganz_still() {
        assert_eq!(sprach_lautstaerke(0.0), 1.0);
        assert_eq!(sprach_lautstaerke(100.0), 0.15);
        let mitte = sprach_lautstaerke(10.0);
        assert!(mitte < 1.0 && mitte > 0.15);
        assert!(sprach_lautstaerke(5.0) > sprach_lautstaerke(12.0));
    }

    #[test]
    fn leinwand_klingt_ab() {
        assert_eq!(leinwand_lautstaerke(2.0, 6.0, 28.0), 100);
        assert_eq!(leinwand_lautstaerke(28.0, 6.0, 28.0), 0);
        assert_eq!(leinwand_lautstaerke(f32::NAN, 6.0, 28.0), 0);
        let a = leinwand_lautstaerke(10.0, 6.0, 28.0);
        let b = leinwand_lautstaerke(20.0, 6.0, 28.0);
        assert!(a > b && b > 0);
    }
}
