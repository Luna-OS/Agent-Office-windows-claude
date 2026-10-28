//! Die Autos in der Garage (Motoren, Hupe, Tür, Unfall) und die Dachbar (Einschenken, Schluckauf).

use std::collections::HashMap;

use web_sys::{
    AudioScheduledSourceNode, BiquadFilterNode, BiquadFilterType, GainNode, OscillatorNode,
    OscillatorType, PannerNode,
};

use super::kern::{huelle, kette, laufen};
use super::{Kern, Klang, Ort, zufall, zufall_f};

/// Ein Auto, das gerade jemand fährt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motor {
    pub auto: u32,
    pub ort: Ort,
    /// m/s.
    pub tempo: f32,
    /// Gaspedal −1..1.
    pub gas: f32,
}

/// Ein Motorklang: Sägezahn und eine Rechteckwelle eine Oktave tiefer, durch einen Filter, der beim Gasgeben aufgeht.
struct Laufend {
    saege: OscillatorNode,
    tief: OscillatorNode,
    ton: BiquadFilterNode,
    pegel: GainNode,
    panner: PannerNode,
    geboren: f64,
}

#[derive(Default)]
pub(super) struct Motoren {
    laufend: HashMap<u32, Laufend>,
}

impl Motoren {
    pub fn setzen(&mut self, k: &Kern, motoren: &[Motor]) {
        let jetzt = k.jetzt();
        for m in motoren {
            let l = self
                .laufend
                .entry(m.auto)
                .or_insert_with(|| anlassen(k, m.ort));
            l.panner
                .set_position(m.ort.x as f64, m.ort.y as f64, m.ort.z as f64);
            // Springt noch an, mit seinem Aufheulen.
            if jetzt - l.geboren < 0.7 {
                continue;
            }
            let v = m.tempo.abs();
            let druck = m.gas.abs();
            // Durch die Gänge: die Drehzahl steigt in jedem und fällt beim Hochschalten zurück.
            let gang = (v / 5.5).floor().min(3.0);
            let f = 44.0 + gang * 7.0 + ((v - gang * 5.5) / 5.5).min(1.5) * 46.0 + druck * 5.0;
            let _ = l.saege.frequency().set_target_at_time(f, jetzt, 0.06);
            let _ = l.tief.frequency().set_target_at_time(f / 2.0, jetzt, 0.06);
            let _ =
                l.ton
                    .frequency()
                    .set_target_at_time(240.0 + f * 5.0 + druck * 450.0, jetzt, 0.08);
            let ziel = 0.035 + 0.04 * druck + 0.025 * (v / 20.0).min(1.0);
            let _ = l.pegel.gain().set_target_at_time(ziel, jetzt, 0.1);
        }
        let weg: Vec<u32> = self
            .laufend
            .keys()
            .filter(|a| !motoren.iter().any(|m| m.auto == **a))
            .copied()
            .collect();
        for a in weg {
            if let Some(l) = self.laufend.remove(&a) {
                let _ = l.pegel.gain().cancel_scheduled_values(jetzt);
                let _ = l.pegel.gain().set_target_at_time(0.0, jetzt, 0.12);
                let _ = l.saege.stop_with_when(jetzt + 0.8);
                let _ = l.tief.stop_with_when(jetzt + 0.8);
            }
        }
    }
}

/// Ein Motor springt an: fängt mit einem Aufheulen an und blubbert dann.
fn anlassen(k: &Kern, ort: Ort) -> Laufend {
    let jetzt = k.jetzt();
    let panner = k.panner(ort, 3.0, 1.0);
    let _ = panner.connect_with_audio_node(&k.umgebung);
    let pegel = k.verstaerker(0.0);
    let _ = pegel.gain().set_value_at_time(0.0, jetzt);
    let _ = pegel
        .gain()
        .linear_ramp_to_value_at_time(0.11, jetzt + 0.12);
    let _ = pegel.gain().set_target_at_time(0.035, jetzt + 0.45, 0.15);
    let ton = k.filter(BiquadFilterType::Lowpass, 900.0, 2.0);
    let saege = k.oszillator(OscillatorType::Sawtooth);
    let tief = k.oszillator(OscillatorType::Square);
    for (o, faktor) in [(&saege, 1.0), (&tief, 0.5)] {
        let _ = o.frequency().set_value_at_time(30.0 * faktor, jetzt);
        let _ = o
            .frequency()
            .linear_ramp_to_value_at_time(115.0 * faktor, jetzt + 0.3);
        let _ = o
            .frequency()
            .set_target_at_time(44.0 * faktor, jetzt + 0.35, 0.12);
    }
    let leise = k.verstaerker(0.5);
    let _ = saege.connect_with_audio_node(&ton);
    kette(&[&tief, &leise, &ton, &pegel, &panner]);
    let _ = saege.start_with_when(jetzt);
    let _ = tief.start_with_when(jetzt);
    Laufend {
        saege,
        tief,
        ton,
        pegel,
        panner,
        geboren: jetzt,
    }
}

impl Klang {
    /// Die Hupe: zwei Töne eine Terz auseinander, ein Lambo höher als ein Ferrari.
    pub fn hupen(&self, ort: Ort, hoch: bool) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 4.0, 0.9, &k.umgebung);
        let t0 = k.jetzt() + 0.005;
        let g = k.verstaerker(0.0);
        huelle(&g.gain(), t0, &[(0.02, 0.09), (0.42, 0.08), (0.5, 0.0)]);
        let ton = k.filter(BiquadFilterType::Lowpass, 2200.0, 0.7);
        kette(&[&ton, &g, &aus]);
        for f in if hoch { [440.0, 554.0] } else { [392.0, 494.0] } {
            let o = k.oszillator(OscillatorType::Square);
            o.frequency().set_value(f);
            let _ = o.connect_with_audio_node(&ton);
            laufen(&o, t0, t0 + 0.55);
        }
    }

    /// Eine Autotür fällt zu.
    pub fn autotuer(&self, ort: Ort) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 2.0, 1.1, &k.umgebung);
        k.schlag(0.0, 0.6, 0.55, &aus);
        k.blip(
            &aus,
            k.jetzt() + 0.005,
            120.0,
            0.6,
            0.09,
            0.12,
            OscillatorType::Sine,
        );
    }

    /// Ein Auto fährt mit `tempo` m/s irgendwo dagegen: ein Rums und scheppernde Bleche.
    pub fn crash(&self, ort: Ort, tempo: f32) {
        let Some(k) = self.kern() else { return };
        let laut = (tempo / 12.0).min(1.0);
        let aus = k.ausgang(Some(ort), 3.0, 1.0, &k.umgebung);
        let t0 = k.jetzt() + 0.005;
        k.schlag(0.0, 0.4 + 0.8 * laut, 0.4, &aus);
        k.blip(
            &aus,
            t0,
            90.0,
            0.5,
            0.3,
            0.1 + 0.25 * laut,
            OscillatorType::Sine,
        );
        for (f, amp) in [(520.0, 0.05), (1270.0, 0.03)] {
            k.blip(
                &aus,
                t0,
                f * zufall_f(0.9, 1.1),
                0.98,
                0.25,
                amp * laut,
                OscillatorType::Triangle,
            );
        }
    }

    /// Ein Drink an der Bar: Eis ins Glas, Einschenken, Klirren.
    pub fn einschenken(&self, ort: Ort) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 1.2, 1.0, &k.umgebung);
        let t0 = k.jetzt() + 0.05;
        for i in 0..3 {
            k.klirren(
                &aus,
                t0 + i as f64 * zufall(0.07, 0.12),
                zufall_f(2200.0, 3200.0),
                0.05,
            );
        }
        // Das Einschenken: gefiltertes Rauschen, das höher wird, wie sich das Glas füllt.
        let strahl = k.quelle(&k.puffer.weiss, false);
        let ton = k.filter(BiquadFilterType::Bandpass, 700.0, 1.4);
        let _ = ton.frequency().set_value_at_time(700.0, t0 + 0.35);
        let _ = ton
            .frequency()
            .linear_ramp_to_value_at_time(1500.0, t0 + 1.15);
        let g = k.verstaerker(0.0);
        huelle(
            &g.gain(),
            t0 + 0.35,
            &[(0.06, 0.09), (0.7, 0.08), (0.85, 0.0)],
        );
        let wackeln = k.quelle(&k.puffer.gluckern, true);
        wackeln.playback_rate().set_value(4.0);
        let amp = k.verstaerker(0.6);
        let _ = wackeln.connect_with_audio_param(&amp.gain());
        kette(&[&strahl, &ton, &amp, &g, &aus]);
        laufen(&strahl, t0 + 0.35, t0 + 1.3);
        laufen(&wackeln, t0 + 0.35, t0 + 1.3);
        // Über den Tresen geschoben.
        k.klirren(&aus, t0 + 1.45, 3900.0, 0.08);
    }

    /// Hicks! Einer zu viel.
    pub fn schluckauf(&self) {
        let Some(k) = self.kern() else { return };
        let t0 = k.jetzt() + 0.02;
        let o = k.oszillator(OscillatorType::Sawtooth);
        let _ = o.frequency().set_value_at_time(260.0, t0);
        let _ = o
            .frequency()
            .exponential_ramp_to_value_at_time(420.0, t0 + 0.06);
        let g = k.verstaerker(0.0);
        huelle(&g.gain(), t0, &[(0.008, 0.12), (0.05, 0.08), (0.11, 0.0)]);
        let bp = k.filter(BiquadFilterType::Bandpass, 1100.0, 2.5);
        kette(&[&o, &bp, &g, &k.umgebung]);
        laufen(&o, t0, t0 + 0.14);
        // Das Stocken in der Kehle, kurz davor.
        let n = k.quelle(&k.puffer.weiss, false);
        let ng = k.verstaerker(0.0);
        huelle(&ng.gain(), t0 - 0.015, &[(0.004, 0.08), (0.02, 0.0)]);
        let bp2 = k.filter(BiquadFilterType::Bandpass, 1800.0, 1.0);
        kette(&[&n, &bp2, &ng, &k.umgebung]);
        laufen(&n as &AudioScheduledSourceNode, t0 - 0.015, t0 + 0.02);
    }
}
