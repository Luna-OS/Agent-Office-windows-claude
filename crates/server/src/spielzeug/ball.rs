//! Der Basketball: wer ihn hat, oder wie er zuletzt geworfen wurde. Fliegen und Springen rechnet
//! jeder Browser selbst aus dem Wurf; der Server merkt sich nur Start und Zeit.

use protokoll::spielzeug::{Ball, Wurf};

/// So lange fliegt/rollt ein Ball höchstens, danach liegt er wieder unter dem Korb.
pub const WURF_DAUER_MS: u64 = 20_000;
/// Höchstgeschwindigkeit eines Wurfs in m/s (gegen Unsinn aus dem Netz).
const MAX_TEMPO: f32 = 30.0;

/// Der Ball, wie ihn ein Browser jetzt sehen soll (mit aktueller `vergangen_ms`).
pub fn jetzt(b: &Ball, seit: u64, jetzt_ms: u64) -> Ball {
    let mut b = b.clone();
    if let Some(w) = &mut b.wurf {
        let vergangen = jetzt_ms.saturating_sub(seit);
        if vergangen > WURF_DAUER_MS {
            b.wurf = None;
            b.werfer = None;
        } else {
            w.vergangen_ms = vergangen;
        }
    }
    b
}

/// Nehmen/fangen: gehört dem, wenn ihn niemand anders hat.
pub fn nehmen(b: &mut Ball, id: &str) -> bool {
    if b.halter.as_deref().is_some_and(|h| h != id) {
        return false;
    }
    if b.halter.as_deref() == Some(id) {
        return false;
    }
    *b = Ball { halter: Some(id.into()), wurf: None, werfer: None };
    true
}

/// Werfen: nur wer ihn hält. Werte werden begrenzt.
pub fn werfen(b: &mut Ball, id: &str, [x, y, z]: [f32; 3], [vx, vy, vz]: [f32; 3]) -> bool {
    if b.halter.as_deref() != Some(id) {
        return false;
    }
    let gut = |v: f32, max: f32| if v.is_finite() { v.clamp(-max, max) } else { 0.0 };
    *b = Ball {
        halter: None,
        wurf: Some(Wurf { x: gut(x, 60.0), y: gut(y, 30.0).max(0.0), z: gut(z, 60.0), vx: gut(vx, MAX_TEMPO), vy: gut(vy, MAX_TEMPO), vz: gut(vz, MAX_TEMPO), vergangen_ms: 0 }),
        werfer: Some(id.into()),
    };
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nehmen_und_werfen() {
        let mut b = Ball::default();
        assert!(nehmen(&mut b, "a"));
        assert!(!nehmen(&mut b, "b"));
        assert!(!werfen(&mut b, "b", [0.0, 1.0, 0.0], [1.0, 1.0, 1.0]));
        assert!(werfen(&mut b, "a", [0.0, 1.0, 0.0], [99.0, 1.0, f32::NAN]));
        let w = b.wurf.unwrap();
        assert_eq!(w.vx, MAX_TEMPO);
        assert_eq!(w.vz, 0.0);
        let spaeter = jetzt(&b, 1000, 1000 + WURF_DAUER_MS + 1);
        assert!(spaeter.wurf.is_none());
    }
}
