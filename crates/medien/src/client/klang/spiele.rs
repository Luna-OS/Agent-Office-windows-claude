//! Spiele: Golf vom Balkon, Basketball, Darts und Äxte an der Dachbar, der Arcade-Automat.

use web_sys::{AudioNode, BiquadFilterType, OscillatorType};

use super::kern::{huelle, kette, laufen};
use super::{Kern, Klang, Ort, zufall_f};

/// Ein Golfball.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GolfKlang {
    /// Der Schläger durch den Ball.
    Schlag,
    /// Aufkommen auf Gras oder Straße.
    Aufprall,
    /// Liegen bleiben in Sand oder Rough.
    Plumps,
    /// Gegen das Glas des Geländers.
    Gelaender,
    Wand,
    /// Ins Loch.
    Loch,
    /// Fanfare für ein Hole-in-one.
    Jubel,
}

/// Der Basketball.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BallKlang {
    Aufprall,
    Ring,
    Brett,
    Treffer,
}

/// Werfen an der Dachbar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WurfKlang {
    /// Ein Dart verlässt die Hand.
    Dart,
    /// Eine Axt dreht sich durch die Luft.
    Axt,
    /// Dart in die Scheibe.
    Scheibe,
    /// Dart ins Holz daneben.
    Holz,
    /// Axt beißt ins Ziel.
    Einschlag,
    /// Axt prallt ab.
    Abpraller,
    /// Landet auf der Matte.
    Fallen,
    /// 180 oder Killshot.
    Jubel,
}

const SINUS: OscillatorType = OscillatorType::Sine;
const DREIECK: OscillatorType = OscillatorType::Triangle;

impl Klang {
    /// Golf: `ort` für den Ball eines anderen (den eigenen hört man überall), `tempo` in m/s.
    pub fn golf(&self, art: GolfKlang, ort: Option<Ort>, tempo: f32) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(ort, 3.0, 1.0, &k.umgebung);
        let t0 = k.jetzt() + 0.005;
        let hart = (tempo / 15.0).min(1.0);
        match art {
            GolfKlang::Schlag => {
                k.blip(&aus, t0, 1900.0, 0.55, 0.06, 0.3, DREIECK);
                k.schlag(0.0, 0.45, 2.6, &aus);
                let wisch = k.quelle(&k.puffer.weiss, false);
                let g = k.verstaerker(0.0);
                huelle(&g.gain(), t0, &[(0.03, 0.06), (0.16, 0.0)]);
                let bp = k.filter(BiquadFilterType::Bandpass, 2400.0, 1.2);
                kette(&[&wisch, &bp, &g, &aus]);
                laufen(&wisch, t0, t0 + 0.2);
            }
            GolfKlang::Aufprall => {
                k.schlag(0.0, 0.08 + hart * 0.3, zufall_f(1.7, 2.0), &aus);
                k.blip(&aus, t0, zufall_f(620.0, 700.0), 0.7, 0.05, 0.03 + hart * 0.06, SINUS);
            }
            GolfKlang::Plumps => k.schlag(0.0, 0.08 + hart * 0.2, zufall_f(1.1, 1.3), &aus),
            GolfKlang::Gelaender => {
                k.klirren(&aus, t0, zufall_f(1250.0, 1400.0), 0.05 + hart * 0.08);
                k.schlag(0.0, 0.25, 2.2, &aus);
            }
            GolfKlang::Wand => k.schlag(0.0, 0.15 + hart * 0.3, 1.9, &aus),
            GolfKlang::Loch => {
                k.blip(&aus, t0, 520.0, 0.6, 0.12, 0.14, DREIECK);
                for i in 1..=3 {
                    let i = i as f32;
                    k.blip(&aus, t0 + 0.08 + i as f64 * 0.06, 900.0 - i * 90.0, 0.8, 0.04, 0.05 / i, SINUS);
                }
            }
            GolfKlang::Jubel => fanfare(&k, &aus, t0 + 0.25, 0.13, 0.9, 0.2),
        }
    }

    /// Der Basketball, von wo er ist, mit `tempo` m/s.
    pub fn ball(&self, art: BallKlang, ort: Ort, tempo: f32) {
        let Some(k) = self.kern() else { return };
        let laut = (tempo / 7.0).min(1.0);
        let aus = k.ausgang(Some(ort), 2.0, 1.1, &k.umgebung);
        let t0 = k.jetzt() + 0.005;
        match art {
            BallKlang::Aufprall => {
                k.blip(&aus, t0, zufall_f(150.0, 175.0), 0.7, 0.16, 0.05 + 0.3 * laut, SINUS);
                k.schlag(0.0, 0.15 + 0.5 * laut, zufall_f(1.25, 1.4), &aus);
            }
            BallKlang::Ring => {
                let f = zufall_f(520.0, 600.0);
                for (verh, amp, len) in [(1.0, 0.1, 0.5), (2.43, 0.06, 0.35), (4.1, 0.03, 0.2)] {
                    k.blip(&aus, t0, f * verh, 0.99, len, amp * (0.3 + laut), SINUS);
                }
                k.schlag(0.0, 0.2 * laut, 1.9, &aus);
            }
            BallKlang::Brett => {
                k.schlag(0.0, 0.25 + 0.5 * laut, 0.8, &aus);
                k.blip(&aus, t0, 240.0, 0.8, 0.12, 0.05 + 0.1 * laut, SINUS);
            }
            BallKlang::Treffer => {
                let n = k.quelle(&k.puffer.weiss, false);
                let ton = k.filter(BiquadFilterType::Bandpass, 2400.0, 1.2);
                let _ = ton.frequency().set_value_at_time(1800.0, t0);
                let _ = ton.frequency().linear_ramp_to_value_at_time(4200.0, t0 + 0.28);
                let g = k.verstaerker(0.0);
                huelle(&g.gain(), t0, &[(0.03, 0.22), (0.18, 0.14), (0.34, 0.0)]);
                kette(&[&n, &ton, &g, &aus]);
                laufen(&n, t0, t0 + 0.4);
            }
        }
    }

    /// Darts und Äxte an der Dachbar, von `ort`.
    pub fn werfen(&self, art: WurfKlang, ort: Ort) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 2.0, 1.1, &k.umgebung);
        let t0 = k.jetzt() + 0.005;
        match art {
            WurfKlang::Dart => luft(&k, &aus, t0, 2600.0, 3600.0, 0.12, 0.05, 0),
            WurfKlang::Axt => luft(&k, &aus, t0, 500.0, 900.0, 0.6, 0.14, 3),
            WurfKlang::Scheibe => {
                k.schlag(0.0, 0.35, 2.3, &aus);
                k.blip(&aus, t0, 950.0, 0.7, 0.03, 0.05, SINUS);
            }
            WurfKlang::Holz => {
                k.schlag(0.0, 0.3, 1.7, &aus);
                k.blip(&aus, t0, 430.0, 0.8, 0.06, 0.06, SINUS);
            }
            WurfKlang::Einschlag => {
                k.schlag(0.0, 0.8, 0.85, &aus);
                k.blip(&aus, t0, 160.0, 0.6, 0.18, 0.22, SINUS);
                k.blip(&aus, t0 + 0.01, 310.0, 0.8, 0.09, 0.07, DREIECK);
            }
            WurfKlang::Abpraller => {
                k.klirren(&aus, t0, zufall_f(1900.0, 2300.0), 0.12);
                k.schlag(0.0, 0.45, 1.4, &aus);
            }
            WurfKlang::Fallen => {
                k.schlag(0.0, 0.4, 1.05, &aus);
                k.klirren(&aus, t0 + 0.02, zufall_f(1300.0, 1500.0), 0.04);
            }
            WurfKlang::Jubel => fanfare(&k, &aus, t0 + 0.1, 0.12, 0.8, 0.18),
        }
    }

    /// Die Chip-Piepser des Arcade-Automaten: ein Teil landet, Reihen weg (`reihen`), Spiel vorbei.
    pub fn arcade_landen(&self) {
        self.arcade(|k, aus, t0| k.blip(aus, t0, 160.0, 0.55, 0.07, 0.1, OscillatorType::Square));
    }

    pub fn arcade_reihen(&self, reihen: usize) {
        self.arcade(move |k, aus, t0| {
            for (i, f) in [523.0, 659.0, 784.0, 1047.0, 1319.0].into_iter().take(reihen + 1).enumerate() {
                k.blip(aus, t0 + i as f64 * 0.07, f, 1.02, 0.1, 0.09, OscillatorType::Square);
            }
        });
    }

    pub fn arcade_vorbei(&self) {
        self.arcade(|k, aus, t0| {
            for (i, f) in [392.0, 330.0, 262.0, 196.0].into_iter().enumerate() {
                k.blip(aus, t0 + i as f64 * 0.18, f, 0.97, 0.17, 0.14, DREIECK);
            }
        });
    }

    fn arcade(&self, f: impl FnOnce(&Kern, &AudioNode, f64)) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(self.lage().orte.arcade), 1.5, 1.2, &k.umgebung);
        f(&k, &aus, k.jetzt() + 0.02);
    }
}

/// Ta-da-da-DAAA.
fn fanfare(k: &Kern, aus: &AudioNode, start: f64, schritt: f64, lang: f64, kurz: f64) {
    for (i, f) in [523.0, 659.0, 784.0, 1047.0].into_iter().enumerate() {
        let wann = start + i as f64 * schritt;
        let len = if i == 3 { lang } else { kurz };
        k.blip(aus, wann, f, 1.0, len, 0.1, DREIECK);
        k.blip(aus, wann, f * 2.0, 1.0, len * 0.7, 0.03, SINUS);
    }
}

/// Luft, durch die etwas fliegt; mit `pulse` > 0 eine Klinge, die sich dreht.
#[allow(clippy::too_many_arguments)]
fn luft(k: &Kern, aus: &AudioNode, t0: f64, von: f32, bis: f32, len: f64, pegel: f32, pulse: u32) {
    let n = k.quelle(&k.puffer.weiss, false);
    let ton = k.filter(BiquadFilterType::Bandpass, von, 1.4);
    let _ = ton.frequency().set_value_at_time(von, t0);
    let _ = ton.frequency().linear_ramp_to_value_at_time(bis, t0 + len);
    let g = k.verstaerker(0.0);
    if pulse > 0 {
        let _ = g.gain().set_value_at_time(0.0, t0);
        let stueck = len / pulse as f64;
        for i in 0..pulse {
            let bei = t0 + (i as f64 + 0.5) * stueck;
            let _ = g.gain().linear_ramp_to_value_at_time(pegel, bei);
            let _ = g.gain().linear_ramp_to_value_at_time(pegel * 0.15, bei + stueck / 2.0);
        }
        let _ = g.gain().linear_ramp_to_value_at_time(0.0, t0 + len + 0.05);
    } else {
        huelle(&g.gain(), t0, &[(len * 0.3, pegel), (len, 0.0)]);
    }
    kette(&[&n, &ton, &g, aus]);
    laufen(&n, t0, t0 + len + 0.1);
}
