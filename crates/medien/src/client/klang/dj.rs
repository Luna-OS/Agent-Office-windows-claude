//! Der DJ auf dem Dach im Browser: spielt das Drum-and-Bass-Set aus `crate::dnb` mit Web Audio, durch die
//! Boxen am DJ-Pult, mit der eigenen Musik-Lautstärke. Auf einem Zeitgeber statt pro Frame, damit es auch
//! im Hintergrund-Tab weiterläuft.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{
    AudioContextState, AudioNode, BiquadFilterNode, BiquadFilterType, DynamicsCompressorNode,
    GainNode, OverSampleType,
};

use super::kern::kette;
use super::{Kern, Klang, Ort};
use crate::client::Takt;
use crate::dnb::{SECHZEHNTEL, TAKT, Teil, dj_zeit, plan, tonleiter};
use crate::tonfolge::streuung;

mod instrumente;

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
        let dj = Self {
            klang: klang.clone(),
            innen: Rc::new(RefCell::new(Innen {
                lautstaerke: 0.5,
                ..Default::default()
            })),
        };
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
        let g = if i.stumm {
            0.0
        } else {
            i.lautstaerke * i.lautstaerke
        };
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
        let (true, Some(eingang), Some(k)) = (an, eingang, self.klang.kern()) else {
            return;
        };
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
        let _ = blende
            .gain()
            .linear_ramp_to_value_at_time(PEGEL, jetzt + 1.2);
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
            let f = if ab.teil == Teil::Intro {
                500.0 * 24f64.powf(ab.takt as f64 / 15.0)
            } else {
                16000.0
            };
            let _ = self
                .drums
                .frequency()
                .set_target_at_time(f as f32, wann, 0.2);
        }
        if p.kick > 0.0 {
            self.kick(wann, p.kick as f32);
        }
        if p.snare > 0.0 {
            self.snare(wann, p.snare as f32);
        }
        if p.hat > 0.0 {
            self.hat(
                wann,
                (p.hat * (0.75 + 0.25 * streuung(k as u32, 3))) as f32,
                false,
            );
        }
        if p.offene_hat {
            self.hat(wann, 0.7, true);
        }
        if p.shaker > 0.0 {
            self.shaker(wann, p.shaker as f32);
        }
        let pad_toene: Vec<i32> = [0, 2, 4, 6]
            .iter()
            .map(|i| tonleiter(st.grundton + 24, akkord + i))
            .collect();
        let zwei_takte = ab.takt % 2 == 0 && s == 0;
        let sub_ton = || {
            let n = tonleiter(st.grundton, akkord);
            if n > st.grundton + 6 { n - 12 } else { n }
        };
        match ab.teil {
            Teil::Intro | Teil::Breakdown => {
                if zwei_takte {
                    self.pad(
                        wann,
                        &pad_toene,
                        len(32.0),
                        if ab.teil == Teil::Breakdown { 1.0 } else { 0.8 },
                    );
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
                    self.anstieg(
                        wann,
                        ab.takt as f64 / ab.takte as f64,
                        (ab.takt + 1) as f64 / ab.takte as f64,
                    );
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
                        self.reese(
                            wann,
                            r,
                            len(if ab.takt % 2 == 1 { 10.0 } else { 16.0 }),
                            &st.wah,
                        );
                    }
                    if s == 10 && ab.takt % 2 == 1 {
                        self.reese(wann, r + 12, len(6.0), &st.wah[5..]);
                    }
                } else {
                    for (bei, n, hoch) in [
                        (0, 3.0, 0),
                        (3, 2.0, 0),
                        (6, 3.0, 12),
                        (10, 2.0, 0),
                        (12, 3.0, 7),
                    ] {
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
}

/// Eine weiche Sättigungskurve für den Dreck im Bass.
fn zerr_kurve(menge: f32) -> Vec<f32> {
    (0..1024)
        .map(|i| ((i as f32 / 1023.0 * 2.0 - 1.0) * menge).tanh() / menge.tanh())
        .collect()
}
