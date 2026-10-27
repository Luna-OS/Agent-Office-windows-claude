//! Rund ums Office: der Hund, die Kaffeemaschine, Leiter und Feuerwehrstangen, Luken und Zellentüren.

use web_sys::{AudioNode, AudioScheduledSourceNode, BiquadFilterType, OscillatorType};

use super::kern::{huelle, kette, laufen};
use super::{Kern, Klang, Ort, zufall, zufall_f};

impl Klang {
    /// Ein paar raue Wuffs, von wo der Hund ist.
    pub fn bellen(&self, x: f32, z: f32, mal: u32) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(Ort { x, y: 0.5, z }), 2.0, 1.0, &k.umgebung);
        let mut t = k.jetzt() + 0.03;
        let tonhoehe = zufall_f(0.95, 1.05);
        for _ in 0..mal {
            wuff(&k, &aus, t, 300.0 * tonhoehe * zufall_f(0.95, 1.05), 0.17, 0.55);
            t += zufall(0.3, 0.42);
        }
    }

    /// Ein kurzes, hohes, fröhliches Jaulen: jemand hat den Hund gestreichelt.
    pub fn jaulen(&self, x: f32, z: f32) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(Ort { x, y: 0.5, z }), 1.5, 1.0, &k.umgebung);
        wuff(&k, &aus, k.jetzt() + 0.02, 620.0, 0.09, 0.3);
    }

    /// Mahlen, Gluckern, Tropfen an der Kaffeemaschine.
    pub fn kaffee(&self) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(self.lage().orte.kaffeemaschine), 1.2, 1.0, &k.umgebung);
        let t0 = k.jetzt() + 0.05;
        // Mahlwerk: ein brummender Motor, in dem Bohnen knirschen.
        let motor = k.oszillator(OscillatorType::Sawtooth);
        let f = motor.frequency();
        let _ = f.set_value_at_time(70.0, t0);
        let _ = f.linear_ramp_to_value_at_time(118.0, t0 + 0.25);
        let _ = f.set_value_at_time(118.0, t0 + 1.2);
        let _ = f.linear_ramp_to_value_at_time(60.0, t0 + 1.5);
        let motor_ton = k.filter(BiquadFilterType::Lowpass, 1100.0, 0.8);
        let knirschen = k.quelle(&k.puffer.weiss, false);
        let knirsch_ton = k.filter(BiquadFilterType::Bandpass, 2600.0, 1.2);
        let mahlen = k.verstaerker(0.0);
        huelle(&mahlen.gain(), t0, &[(0.08, 0.13), (1.25, 0.13), (1.5, 0.0)]);
        let knirsch_amp = k.verstaerker(0.5);
        let rattern = k.quelle(&k.puffer.gluckern, true);
        rattern.playback_rate().set_value(3.0);
        let _ = rattern.connect_with_audio_param(&knirsch_amp.gain());
        kette(&[&motor, &motor_ton, &mahlen]);
        kette(&[&knirschen, &knirsch_ton, &knirsch_amp, &mahlen, &aus]);
        for q in [&*motor as &AudioScheduledSourceNode, &knirschen, &rattern] {
            laufen(q, t0, t0 + 1.6);
        }
        // Brühen: ein zischender, gluckernder Strahl, Blasen platzen.
        let t1 = t0 + 1.8;
        let strahl = k.quelle(&k.puffer.weiss, false);
        let strahl_ton = k.filter(BiquadFilterType::Bandpass, 850.0, 0.9);
        let gluck = k.verstaerker(0.25);
        let wackeln = k.quelle(&k.puffer.gluckern, true);
        let _ = wackeln.connect_with_audio_param(&gluck.gain());
        let bruehen = k.verstaerker(0.0);
        huelle(&bruehen.gain(), t1, &[(0.2, 0.3), (2.4, 0.26), (3.0, 0.0)]);
        kette(&[&strahl, &strahl_ton, &gluck, &bruehen, &aus]);
        for _ in 0..14 {
            k.blip(&aus, t1 + zufall(0.2, 2.6), zufall_f(350.0, 800.0), zufall_f(1.6, 2.4), 0.05, 0.1, OscillatorType::Sine);
        }
        // Die letzten Tropfen in die Tasse.
        for dt in [3.3, 3.9, 4.7] {
            k.blip(&aus, t1 + dt + zufall(-0.1, 0.1), zufall_f(1100.0, 1400.0), 0.55, 0.05, 0.11, OscillatorType::Sine);
        }
        laufen(&strahl, t1, t1 + 3.2);
        laufen(&wackeln, t1, t1 + 3.2);
    }

    /// Die Hand schließt sich um eine Stahlsprosse (oder die Stange: `weich`).
    pub fn sprosse(&self, weich: bool) {
        let Some(k) = self.kern() else { return };
        let t0 = k.jetzt() + 0.005;
        let f = zufall_f(820.0, 980.0) * if weich { 0.7 } else { 1.0 };
        k.blip(&k.umgebung, t0, f, 0.97, 0.12, if weich { 0.05 } else { 0.08 }, OscillatorType::Sine);
        k.blip(&k.umgebung, t0, f * 2.71, 0.98, 0.06, 0.03, OscillatorType::Sine);
        k.schlag(0.0, 0.12, zufall_f(1.6, 1.9), &k.umgebung);
    }

    /// Eine Falltür an der Leiter: knarrt auf oder knallt zu.
    pub fn luke(&self, ort: Ort, auf: bool) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 2.0, 1.1, &k.umgebung);
        if auf {
            knarren(&k, &aus, k.jetzt() + 0.01);
        } else {
            k.schlag(0.0, 0.5, 0.62, &aus);
        }
    }

    /// Kopf an die Decke: höher geht die Leiter nicht.
    pub fn bonk(&self) {
        let Some(k) = self.kern() else { return };
        k.schlag(0.0, 0.35, 0.5, &k.umgebung);
        k.blip(&k.umgebung, k.jetzt() + 0.01, 180.0, 0.6, 0.18, 0.12, OscillatorType::Sine);
    }

    /// Wusch: Luft und quietschende Hände die Feuerwehrstange hinunter.
    pub fn rutschen(&self, sekunden: f64) {
        let Some(k) = self.kern() else { return };
        let t0 = k.jetzt() + 0.01;
        let ende = t0 + sekunden;
        let luft = k.quelle(&k.puffer.weiss, true);
        let ton = k.filter(BiquadFilterType::Bandpass, 500.0, 0.9);
        let _ = ton.frequency().set_value_at_time(400.0, t0);
        let _ = ton.frequency().exponential_ramp_to_value_at_time(2200.0, ende);
        let ag = k.verstaerker(0.0);
        huelle(&ag.gain(), t0, &[(0.25, 0.22), (sekunden * 0.85, 0.3), (sekunden, 0.0)]);
        kette(&[&luft, &ton, &ag, &k.umgebung]);
        laufen(&luft, t0, ende + 0.05);
        let quietschen = k.oszillator(OscillatorType::Triangle);
        let _ = quietschen.frequency().set_value_at_time(1100.0, t0 + 0.1);
        let _ = quietschen.frequency().exponential_ramp_to_value_at_time(1900.0, ende);
        let vib = k.oszillator(OscillatorType::Sine);
        vib.frequency().set_value(9.0);
        let vd = k.verstaerker(35.0);
        let _ = vib.connect_with_audio_node(&vd);
        let _ = vd.connect_with_audio_param(&quietschen.frequency());
        let sg = k.verstaerker(0.0);
        huelle(&sg.gain(), t0, &[(0.15, 0.0), (0.3, 0.035), (sekunden * 0.8, 0.045), (sekunden, 0.0)]);
        kette(&[&quietschen, &sg, &k.umgebung]);
        laufen(&quietschen, t0, ende + 0.05);
        laufen(&vib, t0, ende + 0.05);
    }

    /// Ein kleines „Juhuu“-Pfeifen beim Drehen um die Stange.
    pub fn drehen(&self) {
        let Some(k) = self.kern() else { return };
        let t0 = k.jetzt() + 0.01;
        let o = k.oszillator(OscillatorType::Sine);
        let _ = o.frequency().set_value_at_time(600.0, t0);
        let _ = o.frequency().exponential_ramp_to_value_at_time(1500.0, t0 + 0.45);
        let _ = o.frequency().exponential_ramp_to_value_at_time(900.0, t0 + 1.0);
        let g = k.verstaerker(0.0);
        huelle(&g.gain(), t0, &[(0.08, 0.09), (0.8, 0.07), (1.05, 0.0)]);
        kette(&[&o, &g, &k.umgebung]);
        laufen(&o, t0, t0 + 1.1);
    }

    /// Unten an der Stange auf der Matte: ein Plumps und die Feuerwehrglocke. `ort` für jemand anderen.
    pub fn stange_landen(&self, tempo: f32, ort: Option<Ort>) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(ort, 2.5, 0.9, &k.umgebung);
        k.schlag(0.0, (0.35 + tempo * 0.07).min(0.9), 0.55, &aus);
        let t0 = k.jetzt() + 0.08;
        for i in 0..3 {
            let wann = t0 + i as f64 * 0.16;
            for (verh, amp, len) in [(1.0, 0.16, 1.1), (2.76, 0.07, 0.6), (5.4, 0.035, 0.3)] {
                k.blip(&aus, wann, 1320.0 * verh, 1.0, len, amp, OscillatorType::Sine);
            }
        }
    }

    /// Eine Zellentür im Verlies: Scharniere knarren auf oder sie kracht mit Eisenklang zu.
    pub fn zellentuer(&self, ort: Ort, auf: bool) {
        if auf {
            return self.luke(ort, true);
        }
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 3.0, 1.0, &k.umgebung);
        let t0 = k.jetzt() + 0.005;
        k.schlag(0.0, 0.9, 0.45, &aus);
        k.blip(&aus, t0, 95.0, 0.6, 0.25, 0.22, OscillatorType::Sine);
        for (f, amp, len) in [(310.0, 0.07, 1.4), (742.0, 0.05, 1.1), (1270.0, 0.035, 0.8), (2115.0, 0.02, 0.5)] {
            k.blip(&aus, t0, f * zufall_f(0.97, 1.03), 0.995, len, amp, OscillatorType::Triangle);
        }
        k.klirren(&aus, t0 + 0.02, zufall_f(1600.0, 1900.0), 0.06);
    }

    /// Der Aufzug ist da: ein heller Zweiklang an der Tür.
    pub fn aufzug(&self, ort: Ort) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 2.0, 1.0, &k.umgebung);
        let t0 = k.jetzt() + 0.01;
        k.blip(&aus, t0, 880.0, 1.0, 0.6, 0.09, OscillatorType::Triangle);
        k.blip(&aus, t0 + 0.28, 698.0, 1.0, 0.9, 0.08, OscillatorType::Triangle);
    }

    /// Jemand landet im Stroh.
    pub fn plumps(&self, ort: Ort) {
        let Some(k) = self.kern() else { return };
        let aus = k.ausgang(Some(ort), 2.0, 1.1, &k.umgebung);
        k.schlag(0.0, 0.7, 0.5, &aus);
        k.blip(&aus, k.jetzt() + 0.005, 120.0, 0.5, 0.16, 0.14, OscillatorType::Sine);
    }
}

/// Ein Bellen: eine schnarrende Stimme, die hochspringt und abfällt, zu einem „Wuh“ geformt, mit Atem.
fn wuff(k: &Kern, aus: &AudioNode, t: f64, f: f32, len: f64, laut: f32) {
    let stimme = k.oszillator(OscillatorType::Sawtooth);
    let _ = stimme.frequency().set_value_at_time(f * 0.75, t);
    let _ = stimme.frequency().exponential_ramp_to_value_at_time(f * 1.45, t + len * 0.22);
    let _ = stimme.frequency().exponential_ramp_to_value_at_time(f * 0.6, t + len);
    let mund = k.filter(BiquadFilterType::Bandpass, 950.0, 1.1);
    let _ = mund.frequency().set_value_at_time(700.0, t);
    let _ = mund.frequency().linear_ramp_to_value_at_time(1300.0, t + len * 0.3);
    let _ = mund.frequency().linear_ramp_to_value_at_time(600.0, t + len);
    let g = k.verstaerker(0.0);
    let _ = g.gain().set_value_at_time(0.0001, t);
    let _ = g.gain().exponential_ramp_to_value_at_time(laut, t + 0.012);
    let _ = g.gain().exponential_ramp_to_value_at_time(laut * 0.45, t + len * 0.5);
    let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, t + len);
    kette(&[&stimme, &mund, &g, aus]);
    let atem = k.quelle(&k.puffer.weiss, false);
    let bp = k.filter(BiquadFilterType::Bandpass, 1800.0, 0.8);
    let rau = k.verstaerker(0.35);
    kette(&[&atem, &bp, &rau, &g]);
    laufen(&stimme, t, t + len + 0.02);
    let _ = atem.start_with_when_and_grain_offset(t, zufall(0.0, 4.0));
    let _ = AudioScheduledSourceNode::stop_with_when(&atem, t + len + 0.02);
}

/// Ein knarrendes Scharnier: ein rauer, wackelnder Quietscher.
fn knarren(k: &Kern, aus: &AudioNode, t0: f64) {
    let o = k.oszillator(OscillatorType::Sawtooth);
    let _ = o.frequency().set_value_at_time(420.0, t0);
    let _ = o.frequency().linear_ramp_to_value_at_time(640.0, t0 + 0.18);
    let _ = o.frequency().linear_ramp_to_value_at_time(380.0, t0 + 0.36);
    let wackeln = k.oszillator(OscillatorType::Sine);
    wackeln.frequency().set_value(23.0);
    let tiefe = k.verstaerker(40.0);
    let _ = wackeln.connect_with_audio_node(&tiefe);
    let _ = tiefe.connect_with_audio_param(&o.frequency());
    let g = k.verstaerker(0.0);
    huelle(&g.gain(), t0, &[(0.04, 0.05), (0.3, 0.04), (0.4, 0.0)]);
    let bp = k.filter(BiquadFilterType::Bandpass, 1400.0, 2.5);
    kette(&[&o, &bp, &g, aus]);
    laufen(&o, t0, t0 + 0.45);
    laufen(&wackeln, t0, t0 + 0.45);
}
