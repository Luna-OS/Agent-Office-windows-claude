//! Wie verbraucht ein Arbeiter in der Halle aussieht (wie `agents.ageMinutes` im Original): je
//! länger er gearbeitet hat, desto länger und grauer der Bart, desto krummer und langsamer.
//! Nach `VOLL_MIN` Minuten Arbeit ist er ganz verbraucht. Nur Arbeitszeit zählt.

/// Nach so vielen Minuten Arbeit ist ein Arbeiter ganz verbraucht.
pub const VOLL_MIN: f32 = 30.0;

/// Wie verbraucht (0 = frisch … 1 = ganz), aus der abgeschlossenen Arbeitszeit und einer laufenden
/// Strecke seit `seit` (beides ms), zur Zeit `jetzt` (ms).
pub fn verbraucht(gearbeitet_ms: u64, seit: Option<u64>, jetzt: u64) -> f32 {
    let laufend = seit.map(|s| jetzt.saturating_sub(s)).unwrap_or(0);
    ((gearbeitet_ms + laufend) as f32 / 60_000.0 / VOLL_MIN).clamp(0.0, 1.0)
}

/// Die Bartfarbe: braun, dann grau, dann weiß (RGB 0..1, sRGB).
pub fn bartfarbe(v: f32) -> [f32; 3] {
    let braun = [0.36, 0.24, 0.14];
    let grau = [0.62, 0.62, 0.6];
    let weiss = [0.95, 0.95, 0.93];
    let misch = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    if v < 0.5 {
        misch(braun, grau, v * 2.0)
    } else {
        misch(grau, weiss, (v - 0.5) * 2.0)
    }
}

/// Wie lang der Bart hängt (m unter dem Kinn): von einem Stoppel bis fast zum Boden.
pub fn bartlaenge(v: f32) -> f32 {
    0.04 + v * 1.25
}

/// Wie krumm (Neigung nach vorn, Bogenmaß) und wie langsam (Faktor auf das Lauftempo).
pub fn haltung(v: f32) -> (f32, f32) {
    (v * 0.35, 1.0 - v * 0.45)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verschleiss_waechst_mit_der_arbeit() {
        assert_eq!(verbraucht(0, None, 0), 0.0);
        assert!((verbraucht(15 * 60_000, None, 0) - 0.5).abs() < 1e-6);
        assert_eq!(
            verbraucht(10 * 60_000, Some(0), 60 * 60_000),
            1.0,
            "laufende Strecke zählt mit, gedeckelt"
        );
        assert!(bartlaenge(1.0) > 1.0 && bartlaenge(0.0) < 0.1);
        assert!(bartfarbe(1.0)[0] > bartfarbe(0.0)[0]);
        assert!(haltung(1.0).1 < 1.0);
    }
}
