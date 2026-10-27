//! Die Instrumente des DJ-Sets: Kick, Snare, Hats, Shaker, Sub, Reese-Bass, Pads, Arpeggio,
//! Rausch-Anstieg, Einschlag und Air-Horn.

use web_sys::{BiquadFilterType, GainNode, OscillatorType};

use super::super::kern::kette;
use super::{SECHZEHNTEL, Spieler, TAKT};
use crate::tonfolge::midi_zu_hz;

impl Spieler {
    pub(super) fn rauschen(&self, wann: f64, bis: f64) -> web_sys::AudioBufferSourceNode {
        let n = self.k.quelle(&self.k.puffer.weiss, false);
        let _ = n.start_with_when_and_grain_offset(wann, js_sys::Math::random() * 2.0);
        let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&n, bis);
        n
    }

    pub(super) fn huelle(&self, wann: f64, spitze: f32, an: f64, ab: f64) -> GainNode {
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0001, wann);
        let _ = g
            .gain()
            .exponential_ramp_to_value_at_time(spitze.max(0.0002), wann + an);
        let _ = g
            .gain()
            .exponential_ramp_to_value_at_time(0.0001, wann + an + ab);
        g
    }

    pub(super) fn osz(&self, art: OscillatorType, wann: f64, bis: f64) -> web_sys::OscillatorNode {
        let o = self.k.oszillator(art);
        let _ = o.start_with_when(wann);
        let _ = o.stop_with_when(bis);
        o
    }

    pub(super) fn kick(&self, wann: f64, st: f32) {
        let o = self.osz(OscillatorType::Sine, wann, wann + 0.4);
        let _ = o.frequency().set_value_at_time(190.0, wann);
        let _ = o
            .frequency()
            .exponential_ramp_to_value_at_time(55.0, wann + 0.045);
        let _ = o
            .frequency()
            .exponential_ramp_to_value_at_time(42.0, wann + 0.25);
        kette(&[&o, &self.huelle(wann, 0.95 * st, 0.002, 0.34), &self.drums]);
        // Der Klick des Schlägels.
        let hp = self.k.filter(BiquadFilterType::Highpass, 2500.0, 0.7);
        kette(&[
            &self.rauschen(wann, wann + 0.02),
            &hp,
            &self.huelle(wann, 0.25 * st, 0.001, 0.012),
            &self.drums,
        ]);
        let _ = self.ducken.gain().set_value_at_time(0.35, wann);
        let _ = self
            .ducken
            .gain()
            .set_target_at_time(1.0, wann + 0.02, 0.07);
    }

    pub(super) fn snare(&self, wann: f64, st: f32) {
        let g = self.huelle(wann, 0.55 * st, 0.002, 0.17);
        let hp = self.k.filter(BiquadFilterType::Highpass, 900.0, 0.7);
        let spitze = self.k.filter(BiquadFilterType::Peaking, 2400.0, 1.0);
        kette(&[&self.rauschen(wann, wann + 0.22), &hp, &spitze, &g]);
        let o = self.osz(OscillatorType::Triangle, wann, wann + 0.14);
        let _ = o.frequency().set_value_at_time(230.0, wann);
        let _ = o
            .frequency()
            .exponential_ramp_to_value_at_time(180.0, wann + 0.06);
        kette(&[&o, &self.huelle(wann, 0.4 * st, 0.002, 0.1), &g]);
        let _ = g.connect_with_audio_node(&self.drums);
        if st > 0.5 {
            let _ = g.connect_with_audio_node(&self.hall);
        }
    }

    pub(super) fn hat(&self, wann: f64, st: f32, offen: bool) {
        let n = self.rauschen(wann, wann + if offen { 0.3 } else { 0.06 });
        let g = self.huelle(
            wann,
            (if offen { 0.12 } else { 0.16 }) * st,
            0.001,
            if offen { 0.24 } else { 0.035 },
        );
        kette(&[&n, &g, &self.hats]);
    }

    pub(super) fn shaker(&self, wann: f64, st: f32) {
        let bp = self.k.filter(BiquadFilterType::Bandpass, 9000.0, 1.2);
        kette(&[
            &self.rauschen(wann, wann + 0.05),
            &bp,
            &self.huelle(wann, 0.07 * st, 0.006, 0.03),
            &self.drums,
        ]);
    }

    /// Ein tiefer Sinus unter allem, den ganzen Takt.
    pub(super) fn sub(&self, wann: f64, midi: i32, len: f64) {
        let o = self.osz(OscillatorType::Sine, wann, wann + len + 0.02);
        o.frequency().set_value(midi_zu_hz(midi as f64) as f32);
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0, wann);
        let _ = g.gain().linear_ramp_to_value_at_time(0.42, wann + 0.02);
        let _ = g.gain().set_value_at_time(0.42, wann + len - 0.04);
        let _ = g.gain().linear_ramp_to_value_at_time(0.0, wann + len);
        kette(&[&o, &g, &self.ducken]);
    }

    /// Verstimmte Sägezähne durch einen resonanten Filter, der auf den Achteln auf- und zugeht.
    pub(super) fn reese(&self, wann: f64, midi: i32, len: f64, wah: &str) {
        let tp = self.k.filter(BiquadFilterType::Lowpass, 300.0, 6.0);
        let achtel = SECHZEHNTEL * 2.0;
        let wah = wah.as_bytes();
        let mut i = 0;
        while (i as f64) * achtel < len {
            let f = if wah[i % wah.len()] == b'h' {
                1600.0
            } else {
                260.0
            };
            let _ = tp
                .frequency()
                .set_target_at_time(f, wann + i as f64 * achtel, achtel * 0.3);
            i += 1;
        }
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0, wann);
        let _ = g.gain().linear_ramp_to_value_at_time(0.3, wann + 0.01);
        let _ = g
            .gain()
            .set_value_at_time(0.3, wann + (len - 0.03).max(0.02));
        let _ = g.gain().linear_ramp_to_value_at_time(0.0, wann + len);
        kette(&[&tp, &g, &self.reese]);
        for det in [-17.0, 0.0, 17.0] {
            let o = self.osz(OscillatorType::Sawtooth, wann, wann + len + 0.02);
            o.frequency().set_value(midi_zu_hz(midi as f64) as f32);
            o.detune().set_value(det);
            let _ = o.connect_with_audio_node(&tp);
        }
    }

    /// Ein langsamer, breiter Akkord: zwei verstimmte Sägezähne je Ton.
    pub(super) fn pad(&self, wann: f64, toene: &[i32], len: f64, pegel: f32) {
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0, wann);
        let _ = g
            .gain()
            .linear_ramp_to_value_at_time(0.045 * pegel, wann + 0.5);
        let _ = g.gain().set_value_at_time(0.045 * pegel, wann + len);
        let _ = g.gain().linear_ramp_to_value_at_time(0.0, wann + len + 0.9);
        let _ = g.connect_with_audio_node(&self.pad);
        for &m in toene {
            for det in [-9.0, 9.0] {
                let o = self.osz(OscillatorType::Sawtooth, wann, wann + len + 1.0);
                o.frequency().set_value(midi_zu_hz(m as f64) as f32);
                o.detune().set_value(det);
                let _ = o.connect_with_audio_node(&g);
            }
        }
    }

    pub(super) fn arpeggio(&self, wann: f64, midi: i32, pegel: f32) {
        let o = self.osz(OscillatorType::Square, wann, wann + 0.2);
        o.frequency().set_value(midi_zu_hz(midi as f64) as f32);
        kette(&[&o, &self.huelle(wann, 0.05 * pegel, 0.003, 0.16), &self.arp]);
    }

    /// Ein Takt des Rausch-Anstiegs im Aufbau, von `von` bis `bis` des Weges.
    pub(super) fn anstieg(&self, wann: f64, von: f64, bis: f64) {
        let bp = self.k.filter(BiquadFilterType::Bandpass, 300.0, 1.4);
        let f = |p: f64| (300.0 * 28f64.powf(p)) as f32;
        let _ = bp.frequency().set_value_at_time(f(von), wann);
        let _ = bp
            .frequency()
            .exponential_ramp_to_value_at_time(f(bis), wann + TAKT);
        let g = self.k.verstaerker(0.0);
        let _ = g
            .gain()
            .set_value_at_time((0.02 + 0.3 * von * von) as f32, wann);
        let ziel = if bis >= 1.0 {
            0.0
        } else {
            0.02 + 0.3 * bis * bis
        };
        let _ = g
            .gain()
            .linear_ramp_to_value_at_time(ziel as f32, wann + TAKT);
        kette(&[
            &self.rauschen(wann, wann + TAKT + 0.01),
            &bp,
            &g,
            &self.master,
        ]);
        let _ = g.connect_with_audio_node(&self.hall);
    }

    /// Der Drop landet: ein Crash und ein Boom.
    pub(super) fn einschlag(&self, wann: f64) {
        let crash = self.huelle(wann, 0.3, 0.004, 2.0);
        let hp = self.k.filter(BiquadFilterType::Highpass, 4500.0, 0.6);
        kette(&[&self.rauschen(wann, wann + 2.2), &hp, &crash, &self.master]);
        let _ = crash.connect_with_audio_node(&self.hall);
        let o = self.osz(OscillatorType::Sine, wann, wann + 1.3);
        let _ = o.frequency().set_value_at_time(95.0, wann);
        let _ = o
            .frequency()
            .exponential_ramp_to_value_at_time(30.0, wann + 0.9);
        kette(&[&o, &self.huelle(wann, 0.7, 0.005, 1.2), &self.master]);
    }

    /// Das Air-Horn: BAAP, bap bap, BAAAAAP.
    pub(super) fn horn(&self, wann: f64) {
        let aus = self.k.filter(BiquadFilterType::Bandpass, 1300.0, 0.6);
        let g = self.k.verstaerker(0.9);
        kette(&[&aus, &g, &self.master]);
        let _ = g.connect_with_audio_node(&self.hall);
        for (t, len) in [(0.0, 0.2), (0.26, 0.09), (0.4, 0.09), (0.55, 0.62)] {
            let w = wann + t;
            let h = self.k.verstaerker(0.0);
            let _ = h.gain().set_value_at_time(0.0, w);
            let _ = h.gain().linear_ramp_to_value_at_time(0.22, w + 0.015);
            let _ = h.gain().set_value_at_time(0.22, w + len - 0.03);
            let _ = h.gain().linear_ramp_to_value_at_time(0.0, w + len);
            let _ = h.connect_with_audio_node(&aus);
            for (f, det) in [(415.0, 0.0), (415.0, 14.0), (523.0, -8.0), (830.0, 6.0)] {
                let o = self.osz(OscillatorType::Sawtooth, w, w + len + 0.02);
                o.detune().set_value(det);
                // Jeder Stoß schleift sich in den Ton hinein.
                let _ = o.frequency().set_value_at_time(f * 0.9, w);
                let _ = o.frequency().exponential_ramp_to_value_at_time(f, w + 0.06);
                let _ = o.connect_with_audio_node(&h);
            }
        }
    }
}
