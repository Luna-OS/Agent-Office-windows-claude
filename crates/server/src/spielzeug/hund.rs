//! Der Hund einer Etage: läuft Wegstück für Wegstück herum, schnüffelt, legt sich hin, trottet
//! jemandem hinterher und wedelt, wenn man ihn streichelt. Name, Fell und Rasse folgen aus der Etagen-ID.

use protokoll::spielzeug::{Hund as HundZustand, HundTun, Rasse};
use rand::Rng;

const NAMEN: [&str; 10] = ["Keks", "Pfannkuchen", "Erdnuss", "Pfeffer", "Krümel", "Brezel", "Ziggy", "Bagel", "Ahorn", "Scout"];
const RASSEN: [Rasse; 5] = [Rasse::Welpe, Rasse::Corgi, Rasse::Dackel, Rasse::Mops, Rasse::Shiba];
const FELLE: u32 = 6;
const TEMPO: f32 = 1.2;
use welt::grundriss::draussen::{HUND_KORB, HUND_PUNKTE};
const WEDELN_MS: u64 = 3000;

pub struct Hund {
    pub standard_name: String,
    pub eigener_name: Option<String>,
    fell: u32,
    rasse: Rasse,
    weg: Vec<[f32; 2]>,
    tun: HundTun,
    /// Beginn des Wegstücks.
    seit: u64,
    /// Wann das nächste Stück beginnt.
    bis: u64,
    folgt: Option<String>,
    gestreichelt_von: Option<String>,
}

/// Name, Fell und Rasse aus der Etagen-ID (immer dieselben für dieselbe Etage).
pub fn vorgaben(etage: &str) -> (String, u32, Rasse) {
    let h = etage.chars().fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32));
    let gemischt = ((h ^ (h >> 16)).wrapping_mul(0x45d9f3b)) >> 16;
    (NAMEN[h as usize % NAMEN.len()].into(), (h >> 8) % FELLE, RASSEN[gemischt as usize % RASSEN.len()])
}

fn laenge(weg: &[[f32; 2]]) -> f32 {
    weg.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum()
}

impl Hund {
    pub fn neu(etage: &str, eigener_name: Option<String>) -> Self {
        let (name, fell, rasse) = vorgaben(etage);
        Self { standard_name: name, eigener_name, fell, rasse, weg: vec![[HUND_KORB.0, HUND_KORB.1]], tun: HundTun::Schlafen, seit: 0, bis: 0, folgt: None, gestreichelt_von: None }
    }

    pub fn name(&self) -> String {
        self.eigener_name.clone().unwrap_or_else(|| self.standard_name.clone())
    }

    /// Wo er gerade ist.
    pub fn ort(&self, jetzt_ms: u64) -> [f32; 2] {
        let mut rest = jetzt_ms.saturating_sub(self.seit) as f32 / 1000.0 * TEMPO;
        for w in self.weg.windows(2) {
            let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            if rest < l && l > 0.0 {
                let t = rest / l;
                return [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
            }
            rest -= l;
        }
        *self.weg.last().expect("Weg ist nie leer")
    }

    pub fn zustand(&self, jetzt_ms: u64) -> HundZustand {
        HundZustand {
            name: self.name(),
            fell: self.fell,
            rasse: self.rasse,
            weg: self.weg.clone(),
            tempo: TEMPO,
            vergangen_ms: jetzt_ms.saturating_sub(self.seit),
            tun: self.tun,
            blick: None,
            arbeiter: None,
            folgt: self.folgt.clone(),
            gestreichelt_von: if self.tun == HundTun::Wedeln { self.gestreichelt_von.clone() } else { None },
        }
    }

    fn neues_stueck(&mut self, jetzt_ms: u64, ziel: Option<[f32; 2]>, tun: HundTun, pause_ms: u64) {
        let start = self.ort(jetzt_ms);
        self.weg = match ziel {
            Some(z) => vec![start, z],
            None => vec![start],
        };
        self.tun = tun;
        self.seit = jetzt_ms;
        self.bis = jetzt_ms + (laenge(&self.weg) / TEMPO * 1000.0) as u64 + pause_ms;
    }

    /// Weiter im Tagesablauf. `leute` sind die Orte der Menschen auf der Etage (id, x, z).
    /// Gibt `true` zurück, wenn ein neues Stück begann (dann an alle senden).
    pub fn ticken(&mut self, jetzt_ms: u64, leute: &[(String, f32, f32)]) -> bool {
        if jetzt_ms < self.bis {
            return false;
        }
        let mut rng = rand::rng();
        let hier = self.ort(jetzt_ms);
        if let Some(f) = self.folgt.clone() {
            match leute.iter().find(|(id, ..)| *id == f) {
                Some((_, x, z)) if rng.random_bool(0.85) => {
                    let (dx, dz) = (x - hier[0], z - hier[1]);
                    let d = (dx * dx + dz * dz).sqrt();
                    let ziel = if d > 1.2 { [x - dx / d, z - dz / d] } else { hier };
                    self.neues_stueck(jetzt_ms, Some(ziel), HundTun::Stehen, 600);
                    return true;
                }
                _ => self.folgt = None,
            }
        }
        if !leute.is_empty() && rng.random_bool(0.12) {
            self.folgt = Some(leute[rng.random_range(0..leute.len())].0.clone());
        }
        let (px, pz) = HUND_PUNKTE[rng.random_range(0..HUND_PUNKTE.len())];
        let mut ziel = [px + rng.random_range(-0.8..0.8), pz + rng.random_range(-0.8..0.8)];
        let (tun, pause) = match rng.random_range(0..10) {
            0..=2 => (HundTun::Schnueffeln, rng.random_range(2000..5000)),
            3..=4 => (HundTun::Sitzen, rng.random_range(4000..10_000)),
            5 => {
                // Ein Nickerchen im Körbchen.
                ziel = [HUND_KORB.0, HUND_KORB.1];
                (HundTun::Schlafen, rng.random_range(20_000..60_000))
            }
            6 => (HundTun::Liegen, rng.random_range(10_000..30_000)),
            _ => (HundTun::Stehen, rng.random_range(1000..3000)),
        };
        self.neues_stueck(jetzt_ms, Some(ziel), tun, pause);
        true
    }

    /// Streicheln: nur in Reichweite (2 m). `true`, wenn er sich freut.
    pub fn streicheln(&mut self, jetzt_ms: u64, wer: &str, x: f32, z: f32) -> bool {
        let o = self.ort(jetzt_ms);
        if (o[0] - x).powi(2) + (o[1] - z).powi(2) > 4.0 {
            return false;
        }
        self.gestreichelt_von = Some(wer.into());
        self.neues_stueck(jetzt_ms, None, HundTun::Wedeln, WEDELN_MS);
        true
    }

    /// "" gibt ihm seinen ersten Namen zurück.
    pub fn umbenennen(&mut self, name: &str) {
        let n: String = name.trim().chars().filter(|c| !c.is_control()).take(20).collect();
        self.eigener_name = (!n.is_empty()).then_some(n);
    }

    /// Jemand ging: nicht mehr hinterherlaufen.
    pub fn vergessen(&mut self, id: &str) -> bool {
        if self.folgt.as_deref() == Some(id) {
            self.folgt = None;
            self.bis = 0;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immer_derselbe_hund() {
        assert_eq!(vorgaben("office"), vorgaben("office"));
    }

    #[test]
    fn laufen_und_streicheln() {
        let mut h = Hund::neu("x", None);
        assert!(h.ticken(1000, &[]));
        assert!(!h.ticken(1001, &[]));
        let o = h.ort(1000);
        assert!(h.streicheln(1000, "Luna", o[0] + 0.5, o[1]));
        assert_eq!(h.zustand(1000).tun, HundTun::Wedeln);
        assert!(!h.streicheln(1000, "Luna", o[0] + 50.0, o[1]));
        h.umbenennen("  Bello ");
        assert_eq!(h.name(), "Bello");
        h.umbenennen("");
        assert_eq!(h.name(), h.standard_name);
    }
}
