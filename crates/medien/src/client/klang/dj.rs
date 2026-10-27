//! Der DJ auf dem Dach im Browser: spielt das Drum-and-Bass-Set aus `crate::dnb` mit Web Audio, durch die
//! Boxen am DJ-Pult, mit der eigenen Musik-Lautstärke. Auf einem Zeitgeber statt pro Frame, damit es auch
//! im Hintergrund-Tab weiterläuft.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{
    AudioContextState, AudioNode, BiquadFilterNode, BiquadFilterType, DynamicsCompressorNode, GainNode, OscillatorType,
    OverSampleType,
};

use super::kern::kette;
use super::{Kern, Klang, Ort};
use crate::client::Takt;
use crate::dnb::{SECHZEHNTEL, TAKT, Teil, dj_zeit, plan, tonleiter};
use crate::tonfolge::{midi_zu_hz, streuung};

const VORLAUF: f64 = 1.1;
const PEGEL: f32 = 0.9;

#[derive(Default)]
struct Innen {
    an: bool,
    versatz_ms: f64,
    lautstaerke: f32,
    stumm: bool,
    bus: Option<GainNode>,
    eingang: Option<AudioNode>,
    spieler: Option<Spieler>,
    _takt: Option<Takt>,
}

/// Der DJ auf dem Dach. Billig zu klonen.
#[derive(Clone)]
pub struct Dj {
    klang: Klang,
    innen: Rc<RefCell<Innen>>,
}

impl Dj {
    /// `pult`: wo das DJ-Pult steht (die Boxen hängen dort in 2,2 m Höhe).
    pub fn neu(klang: &Klang, pult: Ort) -> Self {
        let dj = Self { klang: klang.clone(), innen: Rc::new(RefCell::new(Innen { lautstaerke: 0.5, ..Default::default() })) };
        let d2 = dj.clone();
        klang.beim_start(move |k| {
            // Laut genug für das ganze Dach, am lautesten auf der Tanzfläche.
            let p = k.panner(Ort { y: 2.2, ..pult }, 7.0, 0.8);
            let bus = k.verstaerker(0.0);
            kette(&[&p, &bus, &k.ctx.destination()]);
            {
                let mut i = d2.innen.borrow_mut();
                i.eingang = Some(p.unchecked_into());
                i.bus = Some(bus);
            }
            d2.lautstaerke_anwenden();
            d2.anwenden();
        });
        dj
    }

    /// Auf dem Dach (`an`) oder nicht; `versatz_ms` = Server-Uhr minus eigene Uhr.
    pub fn setzen(&self, an: bool, versatz_ms: f64) {
        let war = {
            let mut i = self.innen.borrow_mut();
            i.versatz_ms = versatz_ms;
            std::mem::replace(&mut i.an, an)
        };
        if war != an {
            self.anwenden();
        }
    }

    /// Eigene Musik-Lautstärke (dieselbe wie bei der Jukebox).
    pub fn lautstaerke_setzen(&self, lautstaerke: f32, stumm: bool) {
        {
            let mut i = self.innen.borrow_mut();
            i.lautstaerke = lautstaerke.clamp(0.0, 1.0);
            i.stumm = stumm;
        }
        self.lautstaerke_anwenden();
    }

    /// Jemand am Pult drückt das Air-Horn.
    pub fn horn(&self) {
        if let (Some(s), Some(k)) = (&self.innen.borrow().spieler, self.klang.kern()) {
            s.horn(k.jetzt());
        }
    }

    fn lautstaerke_anwenden(&self) {
        let i = self.innen.borrow();
        let g = if i.stumm { 0.0 } else { i.lautstaerke * i.lautstaerke };
        if let (Some(bus), Some(k)) = (&i.bus, self.klang.kern()) {
            let _ = bus.gain().set_target_at_time(g, k.jetzt(), 0.05);
        }
    }

    fn anwenden(&self) {
        let (an, eingang) = {
            let mut i = self.innen.borrow_mut();
            if let Some(s) = i.spieler.take() {
                s.stoppen();
            }
            i._takt = None;
            (i.an, i.eingang.clone())
        };
        let (true, Some(eingang), Some(k)) = (an, eingang, self.klang.kern()) else { return };
        let spieler = Spieler::neu(k, &eingang);
        let d2 = self.clone();
        let mut i = self.innen.borrow_mut();
        i.spieler = Some(spieler);
        i._takt = Some(Takt::neu(150, move || d2.takt()));
    }

    fn takt(&self) {
        let mut i = self.innen.borrow_mut();
        let at = dj_zeit(js_sys::Date::now() + i.versatz_ms);
        if let Some(s) = i.spieler.as_mut() {
            s.takt(at);
        }
    }
}

struct Spieler {
    k: Rc<Kern>,
    versatz: f64,
    naechster: i64,
    blende: GainNode,
    master: DynamicsCompressorNode,
    drums: BiquadFilterNode,
    hats: AudioNode,
    /// Duckt Bass und Pads bei jeder Kick, damit sie durchkommt.
    ducken: GainNode,
    reese: AudioNode,
    pad: AudioNode,
    arp: AudioNode,
    hall: AudioNode,
}

impl Spieler {
    fn neu(k: Rc<Kern>, aus: &AudioNode) -> Self {
        let jetzt = k.jetzt();
        let blende = k.verstaerker(0.0);
        let _ = blende.gain().set_value_at_time(0.0, jetzt);
        let _ = blende.gain().linear_ramp_to_value_at_time(PEGEL, jetzt + 1.2);
        let _ = blende.connect_with_audio_node(aus);
        let master = k.ctx.create_dynamics_compressor().expect("Kompressor");
        master.threshold().set_value(-16.0);
        master.knee().set_value(8.0);
        master.ratio().set_value(4.0);
        master.attack().set_value(0.005);
        master.release().set_value(0.12);
        let _ = master.connect_with_audio_node(&blende);

        let raum = k.ctx.create_convolver().expect("Convolver");
        raum.set_buffer(Some(&k.puffer.raum));
        let nass = k.verstaerker(0.3);
        kette(&[&raum, &nass, &master]);

        let drums = k.filter(BiquadFilterType::Lowpass, 16000.0, 0.7);
        let _ = drums.connect_with_audio_node(&master);
        let hats = k.filter(BiquadFilterType::Highpass, 7000.0, 0.7);
        let _ = hats.connect_with_audio_node(&drums);

        let ducken = k.verstaerker(1.0);
        let _ = ducken.connect_with_audio_node(&master);
        // Der Reese knurrt durch etwas Verzerrung; die Tiefen bleiben dem Sub.
        let zerr = k.ctx.create_wave_shaper().expect("WaveShaper");
        let mut kurve = zerr_kurve(2.2);
        zerr.set_curve_opt_f32_slice(Some(&mut kurve));
        zerr.set_oversample(OverSampleType::N2x);
        let hp = k.filter(BiquadFilterType::Highpass, 90.0, 0.7);
        let rp = k.verstaerker(0.55);
        kette(&[&zerr, &hp, &rp, &ducken]);

        let pad = k.filter(BiquadFilterType::Lowpass, 1700.0, 0.5);
        let _ = pad.connect_with_audio_node(&ducken);
        let _ = pad.connect_with_audio_node(&raum);

        // Das Arpeggio hallt auf der punktierten Achtel nach.
        let arp = k.filter(BiquadFilterType::Lowpass, 3200.0, 0.8);
        let echo = k.ctx.create_delay_with_max_delay_time(1.0).expect("Delay");
        echo.delay_time().set_value((SECHZEHNTEL * 3.0) as f32);
        let rueck = k.verstaerker(0.38);
        let echo_pegel = k.verstaerker(0.45);
        let _ = arp.connect_with_audio_node(&master);
        let _ = arp.connect_with_audio_node(&echo);
        kette(&[&echo, &rueck, &echo]);
        kette(&[&echo, &echo_pegel, &master]);

        Self {
            versatz: f64::NAN,
            naechster: -1,
            blende,
            master,
            drums,
            hats: hats.unchecked_into(),
            ducken,
            reese: zerr.unchecked_into(),
            pad: pad.unchecked_into(),
            arp: arp.unchecked_into(),
            hall: raum.unchecked_into(),
            k,
        }
    }

    fn takt(&mut self, at: f64) {
        let k = self.k.clone();
        if k.ctx.state() != AudioContextState::Running {
            return;
        }
        let versatz = k.jetzt() - at;
        let drift = (versatz - self.versatz).abs();
        if drift.is_nan() || drift >= 0.03 {
            self.versatz = versatz;
            if drift.is_nan() || drift >= 0.5 {
                self.naechster = -1;
            }
        }
        if self.naechster < 0 {
            self.naechster = (at / SECHZEHNTEL).ceil() as i64;
        }
        while (self.naechster as f64) * SECHZEHNTEL < at + VORLAUF {
            let t = self.naechster as f64 * SECHZEHNTEL;
            if t >= at - 0.02 {
                self.spielen(self.naechster, k.jetzt().max(t + self.versatz));
            }
            self.naechster += 1;
        }
    }

    fn stoppen(self) {
        let jetzt = self.k.jetzt();
        let g = self.blende.gain();
        let _ = g.cancel_scheduled_values(jetzt);
        let _ = g.set_value_at_time(g.value(), jetzt);
        let _ = g.linear_ramp_to_value_at_time(0.0, jetzt + 0.5);
    }

    fn spielen(&self, k: i64, wann: f64) {
        let p = plan(k);
        let (st, ab, s, akkord) = (&p.stueck, p.abschnitt, p.s, p.akkord);
        let len = |n: f64| n * SECHZEHNTEL;
        // Die Drums, und wie gedämpft: im Intro geht der Filter Takt für Takt auf.
        if s == 0 {
            let f = if ab.teil == Teil::Intro { 500.0 * 24f64.powf(ab.takt as f64 / 15.0) } else { 16000.0 };
            let _ = self.drums.frequency().set_target_at_time(f as f32, wann, 0.2);
        }
        if p.kick > 0.0 {
            self.kick(wann, p.kick as f32);
        }
        if p.snare > 0.0 {
            self.snare(wann, p.snare as f32);
        }
        if p.hat > 0.0 {
            self.hat(wann, (p.hat * (0.75 + 0.25 * streuung(k as u32, 3))) as f32, false);
        }
        if p.offene_hat {
            self.hat(wann, 0.7, true);
        }
        if p.shaker > 0.0 {
            self.shaker(wann, p.shaker as f32);
        }
        let pad_toene: Vec<i32> = [0, 2, 4, 6].iter().map(|i| tonleiter(st.grundton + 24, akkord + i)).collect();
        let zwei_takte = ab.takt % 2 == 0 && s == 0;
        let sub_ton = || {
            let n = tonleiter(st.grundton, akkord);
            if n > st.grundton + 6 { n - 12 } else { n }
        };
        match ab.teil {
            Teil::Intro | Teil::Breakdown => {
                if zwei_takte {
                    self.pad(wann, &pad_toene, len(32.0), if ab.teil == Teil::Breakdown { 1.0 } else { 0.8 });
                }
                if ab.teil == Teil::Intro && ab.takt >= 8 && s == 0 {
                    self.sub(wann, sub_ton(), len(16.0));
                }
            }
            Teil::Aufbau => {
                if zwei_takte && ab.takt < 6 {
                    self.pad(wann, &pad_toene, len(32.0), 0.6);
                }
                if s == 0 {
                    self.anstieg(wann, ab.takt as f64 / ab.takte as f64, (ab.takt + 1) as f64 / ab.takte as f64);
                }
            }
            Teil::Drop => {
                if ab.takt == 0 && s == 0 {
                    self.einschlag(wann);
                    if !ab.zweiter && st.horn {
                        self.horn(wann + SECHZEHNTEL * 2.0);
                    }
                }
                if zwei_takte {
                    self.pad(wann, &pad_toene, len(32.0), 0.4);
                }
                if s == 0 {
                    self.sub(wann, sub_ton(), len(16.0));
                }
                let r = tonleiter(st.grundton + 12, akkord);
                if st.reese {
                    // Ein langes Knurren über den Takt, jeden zweiten Takt für die letzten Sechzehntel eine Oktave höher.
                    if s == 0 {
                        self.reese(wann, r, len(if ab.takt % 2 == 1 { 10.0 } else { 16.0 }), &st.wah);
                    }
                    if s == 10 && ab.takt % 2 == 1 {
                        self.reese(wann, r + 12, len(6.0), &st.wah[5..]);
                    }
                } else {
                    for (bei, n, hoch) in [(0, 3.0, 0), (3, 2.0, 0), (6, 3.0, 12), (10, 2.0, 0), (12, 3.0, 7)] {
                        if bei == s {
                            self.reese(wann, r + hoch, len(n), "hl");
                        }
                    }
                }
            }
        }
        // Das Arpeggio: im Breakdown, sobald er läuft, und über dem zweiten Drop.
        if (ab.teil == Teil::Breakdown && ab.takt >= 4) || (ab.teil == Teil::Drop && ab.zweiter) {
            let i = st.arpeggio[s as usize];
            let ton = tonleiter(st.grundton + 48, akkord + i * 2 - if i > 3 { 7 } else { 0 });
            self.arpeggio(wann, ton, if ab.teil == Teil::Drop { 0.8 } else { 1.0 });
        }
    }

    // ---- Instrumente ----

    fn rauschen(&self, wann: f64, bis: f64) -> web_sys::AudioBufferSourceNode {
        let n = self.k.quelle(&self.k.puffer.weiss, false);
        let _ = n.start_with_when_and_grain_offset(wann, js_sys::Math::random() * 2.0);
        let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&n, bis);
        n
    }

    fn huelle(&self, wann: f64, spitze: f32, an: f64, ab: f64) -> GainNode {
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0001, wann);
        let _ = g.gain().exponential_ramp_to_value_at_time(spitze.max(0.0002), wann + an);
        let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, wann + an + ab);
        g
    }

    fn osz(&self, art: OscillatorType, wann: f64, bis: f64) -> web_sys::OscillatorNode {
        let o = self.k.oszillator(art);
        let _ = o.start_with_when(wann);
        let _ = o.stop_with_when(bis);
        o
    }

    fn kick(&self, wann: f64, st: f32) {
        let o = self.osz(OscillatorType::Sine, wann, wann + 0.4);
        let _ = o.frequency().set_value_at_time(190.0, wann);
        let _ = o.frequency().exponential_ramp_to_value_at_time(55.0, wann + 0.045);
        let _ = o.frequency().exponential_ramp_to_value_at_time(42.0, wann + 0.25);
        kette(&[&o, &self.huelle(wann, 0.95 * st, 0.002, 0.34), &self.drums]);
        // Der Klick des Schlägels.
        let hp = self.k.filter(BiquadFilterType::Highpass, 2500.0, 0.7);
        kette(&[&self.rauschen(wann, wann + 0.02), &hp, &self.huelle(wann, 0.25 * st, 0.001, 0.012), &self.drums]);
        let _ = self.ducken.gain().set_value_at_time(0.35, wann);
        let _ = self.ducken.gain().set_target_at_time(1.0, wann + 0.02, 0.07);
    }

    fn snare(&self, wann: f64, st: f32) {
        let g = self.huelle(wann, 0.55 * st, 0.002, 0.17);
        let hp = self.k.filter(BiquadFilterType::Highpass, 900.0, 0.7);
        let spitze = self.k.filter(BiquadFilterType::Peaking, 2400.0, 1.0);
        kette(&[&self.rauschen(wann, wann + 0.22), &hp, &spitze, &g]);
        let o = self.osz(OscillatorType::Triangle, wann, wann + 0.14);
        let _ = o.frequency().set_value_at_time(230.0, wann);
        let _ = o.frequency().exponential_ramp_to_value_at_time(180.0, wann + 0.06);
        kette(&[&o, &self.huelle(wann, 0.4 * st, 0.002, 0.1), &g]);
        let _ = g.connect_with_audio_node(&self.drums);
        if st > 0.5 {
            let _ = g.connect_with_audio_node(&self.hall);
        }
    }

    fn hat(&self, wann: f64, st: f32, offen: bool) {
        let n = self.rauschen(wann, wann + if offen { 0.3 } else { 0.06 });
        let g = self.huelle(wann, (if offen { 0.12 } else { 0.16 }) * st, 0.001, if offen { 0.24 } else { 0.035 });
        kette(&[&n, &g, &self.hats]);
    }

    fn shaker(&self, wann: f64, st: f32) {
        let bp = self.k.filter(BiquadFilterType::Bandpass, 9000.0, 1.2);
        kette(&[&self.rauschen(wann, wann + 0.05), &bp, &self.huelle(wann, 0.07 * st, 0.006, 0.03), &self.drums]);
    }

    /// Ein tiefer Sinus unter allem, den ganzen Takt.
    fn sub(&self, wann: f64, midi: i32, len: f64) {
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
    fn reese(&self, wann: f64, midi: i32, len: f64, wah: &str) {
        let tp = self.k.filter(BiquadFilterType::Lowpass, 300.0, 6.0);
        let achtel = SECHZEHNTEL * 2.0;
        let wah = wah.as_bytes();
        let mut i = 0;
        while (i as f64) * achtel < len {
            let f = if wah[i % wah.len()] == b'h' { 1600.0 } else { 260.0 };
            let _ = tp.frequency().set_target_at_time(f, wann + i as f64 * achtel, achtel * 0.3);
            i += 1;
        }
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0, wann);
        let _ = g.gain().linear_ramp_to_value_at_time(0.3, wann + 0.01);
        let _ = g.gain().set_value_at_time(0.3, wann + (len - 0.03).max(0.02));
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
    fn pad(&self, wann: f64, toene: &[i32], len: f64, pegel: f32) {
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0, wann);
        let _ = g.gain().linear_ramp_to_value_at_time(0.045 * pegel, wann + 0.5);
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

    fn arpeggio(&self, wann: f64, midi: i32, pegel: f32) {
        let o = self.osz(OscillatorType::Square, wann, wann + 0.2);
        o.frequency().set_value(midi_zu_hz(midi as f64) as f32);
        kette(&[&o, &self.huelle(wann, 0.05 * pegel, 0.003, 0.16), &self.arp]);
    }

    /// Ein Takt des Rausch-Anstiegs im Aufbau, von `von` bis `bis` des Weges.
    fn anstieg(&self, wann: f64, von: f64, bis: f64) {
        let bp = self.k.filter(BiquadFilterType::Bandpass, 300.0, 1.4);
        let f = |p: f64| (300.0 * 28f64.powf(p)) as f32;
        let _ = bp.frequency().set_value_at_time(f(von), wann);
        let _ = bp.frequency().exponential_ramp_to_value_at_time(f(bis), wann + TAKT);
        let g = self.k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time((0.02 + 0.3 * von * von) as f32, wann);
        let ziel = if bis >= 1.0 { 0.0 } else { 0.02 + 0.3 * bis * bis };
        let _ = g.gain().linear_ramp_to_value_at_time(ziel as f32, wann + TAKT);
        kette(&[&self.rauschen(wann, wann + TAKT + 0.01), &bp, &g, &self.master]);
        let _ = g.connect_with_audio_node(&self.hall);
    }

    /// Der Drop landet: ein Crash und ein Boom.
    fn einschlag(&self, wann: f64) {
        let crash = self.huelle(wann, 0.3, 0.004, 2.0);
        let hp = self.k.filter(BiquadFilterType::Highpass, 4500.0, 0.6);
        kette(&[&self.rauschen(wann, wann + 2.2), &hp, &crash, &self.master]);
        let _ = crash.connect_with_audio_node(&self.hall);
        let o = self.osz(OscillatorType::Sine, wann, wann + 1.3);
        let _ = o.frequency().set_value_at_time(95.0, wann);
        let _ = o.frequency().exponential_ramp_to_value_at_time(30.0, wann + 0.9);
        kette(&[&o, &self.huelle(wann, 0.7, 0.005, 1.2), &self.master]);
    }

    /// Das Air-Horn: BAAP, bap bap, BAAAAAP.
    fn horn(&self, wann: f64) {
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

/// Eine weiche Sättigungskurve für den Dreck im Bass.
fn zerr_kurve(menge: f32) -> Vec<f32> {
    (0..1024).map(|i| ((i as f32 / 1023.0 * 2.0 - 1.0) * menge).tanh() / menge.tanh()).collect()
}
