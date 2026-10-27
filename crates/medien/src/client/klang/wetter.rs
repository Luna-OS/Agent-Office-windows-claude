//! Das Wetter draußen: Regen (drinnen gedämpft, an den Fenstern prasselnd oder ringsum) und Donner.

use web_sys::{BiquadFilterNode, BiquadFilterType, GainNode};

use super::kern::{huelle, kette};
use super::{Kern, Klang, Lage, Ort, Wo, waehle, zufall, zufall_f};

/// Regen: ein Rauschen, das mit ihm an- und abschwillt, und Tropfen.
#[derive(Default)]
pub(super) struct Regen {
    knoten: Option<(GainNode, BiquadFilterNode)>,
    naechste_regel: f64,
    naechster_tropfen: f64,
}

impl Regen {
    pub fn takt(&mut self, k: &Kern, lage: &Lage, jetzt: f64) {
        let regen = lage.regen as f64;
        if regen < 0.01 && self.knoten.is_none() {
            return;
        }
        let (pegel, ton) = self.knoten.get_or_insert_with(|| {
            let q = k.quelle(&k.puffer.weiss, true);
            let hp = k.filter(BiquadFilterType::Highpass, 450.0, 0.5);
            let ton = k.filter(BiquadFilterType::Lowpass, 1300.0, 0.5);
            let pegel = k.verstaerker(0.0);
            kette(&[&q, &hp, &ton, &pegel, &k.umgebung]);
            let _ = q.start();
            (pegel, ton)
        });
        if jetzt >= self.naechste_regel {
            // Ein paar Mal pro Sekunde; der Pegel gleitet ohnehin.
            self.naechste_regel = jetzt + 0.25;
            let basis = match lage.wo {
                Wo::Draussen => 0.16,
                Wo::Garage => 0.11,
                Wo::Drinnen => 0.06,
            };
            let ziel = if regen < 0.01 { 0.0 } else { basis * regen.powf(0.8) };
            let _ = pegel.gain().set_target_at_time(ziel as f32, jetzt, 0.6);
            let f = match lage.wo {
                Wo::Draussen => 6500.0,
                Wo::Garage => 2600.0,
                Wo::Drinnen => 1300.0,
            };
            let _ = ton.frequency().set_target_at_time(f, jetzt, 0.3);
        }
        if regen > 0.05 && jetzt >= self.naechster_tropfen {
            self.naechster_tropfen = jetzt + zufall(0.03, 0.2) / regen;
            let l = lage.kopf;
            let ort = if lage.wo == Wo::Drinnen && !lage.orte.fenster.is_empty() {
                *waehle(&lage.orte.fenster)
            } else {
                Ort { x: l.x + zufall_f(-4.0, 4.0), y: l.y - 1.2, z: l.z + zufall_f(-4.0, 4.0) }
            };
            k.abspielen_bei(&k.puffer.tropfen, ort, zufall_f(0.05, 0.14), zufall_f(0.7, 1.4), 1.5, 1.3);
        }
    }
}

impl Klang {
    /// Donner, `verzoegerung` Sekunden nach dem Blitz: ein Krachen, wenn er nah ist, dann ein langes Grollen.
    pub fn donner(&self, verzoegerung: f64, laut: f32) {
        let Some(k) = self.kern() else { return };
        if k.ctx.state() != web_sys::AudioContextState::Running {
            return;
        }
        let t0 = k.jetzt() + verzoegerung;
        let spitze = 0.45 * laut * if self.lage().wo == Wo::Drinnen { 0.6 } else { 1.0 };
        let q = k.quelle(&k.puffer.braun, true);
        let ton = k.filter(BiquadFilterType::Lowpass, 700.0, 0.7);
        let _ = ton.frequency().set_value_at_time(700.0, t0);
        let _ = ton.frequency().exponential_ramp_to_value_at_time(110.0, t0 + 3.5);
        let g = k.verstaerker(0.0);
        let p = g.gain();
        let _ = p.set_value_at_time(0.0001, t0);
        let _ = p.exponential_ramp_to_value_at_time(spitze.max(0.0002), t0 + 0.08 + (1.0 - laut as f64) * 0.5);
        let _ = p.exponential_ramp_to_value_at_time((spitze * 0.35).max(0.0002), t0 + 1.3);
        let _ = p.exponential_ramp_to_value_at_time((spitze * 0.6).max(0.0002), t0 + 1.9);
        let _ = p.exponential_ramp_to_value_at_time(0.0001, t0 + 4.0 + laut as f64 * 2.5);
        kette(&[&q, &ton, &g, &k.umgebung]);
        let _ = q.start_with_when_and_grain_offset(t0, zufall(0.0, 5.0));
        let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&q, t0 + 7.0);
        if verzoegerung < 1.0 {
            let krach = k.quelle(&k.puffer.weiss, false);
            let cg = k.verstaerker(0.0);
            huelle(&cg.gain(), t0, &[(0.01, spitze * 0.5), (0.25, 0.0)]);
            let bp = k.filter(BiquadFilterType::Bandpass, 1800.0, 0.6);
            kette(&[&krach, &bp, &cg, &k.umgebung]);
            super::kern::laufen(&krach, t0, t0 + 0.3);
        }
    }
}
