//! Die Jukebox im Browser: spielt die eingebauten Stücke (Noten aus `crate::tonfolge`) mit Web Audio,
//! oder einen eingefügten Stream über ein `<audio>`. Alle auf der Etage hören denselben Takt, weil
//! jede Note allein aus Stück und Stelle folgt.

use std::cell::RefCell;
use std::rc::Rc;

use protokoll::medien::JukeboxZustand;
use wasm_bindgen::JsCast;
use web_sys::{
    AudioContextState, AudioNode, AudioScheduledSourceNode, BiquadFilterType, GainNode,
    HtmlAudioElement, OscillatorType,
};

use super::{Kern, Klang, Ort};
use crate::client::Takt;
use crate::jukebox::STREAM;
use crate::tonfolge::{Note, Partitur, abschnitt, midi_zu_hz};

const VORLAUF: f64 = 1.1;
/// Pegel nach dem Kompressor: bei voller Lautstärke direkt an der Jukebox etwa 0.7.
const PEGEL: f32 = 0.75;
/// Wie die Jukebox mit der Entfernung leiser wird (Panner wie im Original, beim Stream von Hand).
const REFERENZ: f64 = 2.5;
const ABFALL: f64 = 1.3;

#[derive(Default)]
struct Innen {
    zustand: Option<JukeboxZustand>,
    versatz_ms: f64,
    spieler: Option<Stueckspieler>,
    stream: Option<HtmlAudioElement>,
    eingang: Option<AudioNode>,
    bus: Option<GainNode>,
    /// Misst, was an den eigenen Ohren ankommt (nach der eigenen Musik-Lautstärke).
    messer: Option<web_sys::AnalyserNode>,
    lautstaerke: f32,
    stumm: bool,
    abstand: f32,
    _takt: Option<Takt>,
}

/// Die Jukebox der eigenen Etage. Billig zu klonen.
#[derive(Clone)]
pub struct Jukebox {
    klang: Klang,
    innen: Rc<RefCell<Innen>>,
}

impl Jukebox {
    /// `ort`: wo die Jukebox in der Lounge steht.
    pub fn neu(klang: &Klang, ort: Ort) -> Self {
        let j = Self {
            klang: klang.clone(),
            innen: Rc::new(RefCell::new(Innen {
                lautstaerke: 0.5,
                ..Default::default()
            })),
        };
        let j2 = j.clone();
        klang.beim_start(move |k| {
            // Eigener Musik-Bus direkt an die Ausgabe: eigene Lautstärke, spielt auch im verborgenen Tab.
            let panner = k.panner(ort, REFERENZ, ABFALL);
            let ton = k.filter(BiquadFilterType::Lowpass, 16000.0, 0.5);
            let bus = k.verstaerker(0.0);
            let _ = panner.connect_with_audio_node(&ton);
            let _ = ton.connect_with_audio_node(&bus);
            let _ = bus.connect_with_audio_node(&k.ctx.destination());
            let messer = k.ctx.create_analyser().ok();
            if let Some(m) = &messer {
                m.set_fft_size(2048);
                let _ = bus.connect_with_audio_node(m);
            }
            {
                let mut i = j2.innen.borrow_mut();
                i.eingang = Some(panner.unchecked_into());
                i.bus = Some(bus);
                i.messer = messer;
            }
            j2.lautstaerke_anwenden();
            j2.anwenden();
        });
        let j2 = j.clone();
        j.innen.borrow_mut()._takt = Some(Takt::neu(200, move || j2.takt()));
        j
    }

    /// Neuer Zustand vom Server; `versatz_ms` = Server-Uhr minus eigene Uhr.
    pub fn zustand_setzen(&self, zustand: Option<JukeboxZustand>, versatz_ms: f64) {
        let gleich = {
            let mut i = self.innen.borrow_mut();
            i.versatz_ms = versatz_ms;
            let alt = i.zustand.replace(zustand.clone().unwrap_or(JukeboxZustand {
                an: false,
                stueck: String::new(),
                url: None,
                von: None,
                start_ms: 0,
                vergangen_ms: 0,
            }));
            alt.zip(zustand).is_some_and(|(a, b)| {
                a.an == b.an && a.stueck == b.stueck && a.url == b.url && a.start_ms == b.start_ms
            })
        };
        if !gleich {
            self.anwenden();
        }
    }

    /// Eigene Musik-Lautstärke (0..1).
    pub fn lautstaerke_setzen(&self, lautstaerke: f32, stumm: bool) {
        {
            let mut i = self.innen.borrow_mut();
            i.lautstaerke = lautstaerke.clamp(0.0, 1.0);
            i.stumm = stumm;
        }
        self.lautstaerke_anwenden();
    }

    /// Abstand zur Jukebox in m (für einen Stream, der nicht durch den Panner läuft).
    pub fn abstand_setzen(&self, abstand: f32) {
        self.innen.borrow_mut().abstand = abstand;
        self.lautstaerke_anwenden();
    }

    /// Was gerade spielt (Stück-ID oder `"stream"`), `None` wenn aus.
    pub fn spielt(&self) -> Option<String> {
        let i = self.innen.borrow();
        if i.spieler.is_some() || i.stream.is_some() {
            i.zustand.as_ref().map(|z| z.stueck.clone())
        } else {
            None
        }
    }

    /// Pegel (RMS) der Musik an den eigenen Ohren; ein Stream zählt hier nicht mit (läuft am Graphen vorbei).
    pub fn pegel(&self) -> f32 {
        let i = self.innen.borrow();
        let Some(m) = &i.messer else { return 0.0 };
        let mut d = vec![0f32; m.fft_size() as usize];
        m.get_float_time_domain_data(&mut d);
        (d.iter().map(|v| v * v).sum::<f32>() / d.len().max(1) as f32).sqrt()
    }

    /// 0..1 auf jedem Schlag, für die Lichter der Jukebox.
    pub fn puls(&self) -> f64 {
        let i = self.innen.borrow();
        match (&i.spieler, &i.zustand) {
            (Some(s), Some(z)) => s.partitur.puls(stelle_s(z, i.versatz_ms)),
            _ => 0.0,
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
        if let Some(a) = &i.stream {
            let d = i.abstand.max(REFERENZ as f32) as f64;
            let faktor = REFERENZ / (REFERENZ + ABFALL * (d - REFERENZ));
            a.set_volume((g as f64 * faktor).clamp(0.0, 1.0));
        }
    }

    /// Startet, stoppt oder wechselt, was gespielt wird.
    fn anwenden(&self) {
        let (zustand, versatz) = {
            let mut i = self.innen.borrow_mut();
            if let Some(s) = i.spieler.take() {
                s.stoppen();
            }
            if let Some(a) = i.stream.take() {
                let _ = a.pause();
                a.set_src("");
            }
            (i.zustand.clone(), i.versatz_ms)
        };
        let Some(z) = zustand.filter(|z| z.an) else {
            return;
        };
        if z.stueck == STREAM {
            let Some(url) = &z.url else { return };
            let Ok(a) = HtmlAudioElement::new_with_src(url) else {
                return;
            };
            a.set_cross_origin(None);
            a.set_current_time(stelle_s(&z, versatz));
            let _ = a.play();
            self.innen.borrow_mut().stream = Some(a);
            self.lautstaerke_anwenden();
            return;
        }
        let (Some(k), Some(eingang)) = (self.klang.kern(), self.innen.borrow().eingang.clone())
        else {
            return;
        };
        let spieler = Stueckspieler::neu(k, &eingang, &z.stueck);
        self.innen.borrow_mut().spieler = Some(spieler);
        self.takt();
    }

    fn takt(&self) {
        let mut i = self.innen.borrow_mut();
        let Some(z) = i.zustand.clone() else { return };
        let at = stelle_s(&z, i.versatz_ms);
        if let Some(s) = i.spieler.as_mut() {
            s.takt(at);
        }
    }
}

fn stelle_s(z: &JukeboxZustand, versatz_ms: f64) -> f64 {
    ((js_sys::Date::now() + versatz_ms - z.start_ms as f64) / 1000.0).max(0.0)
}

/// Spielt ein eingebautes Stück: ein Lo-Fi-Sampler mit Hall, Tremolo, Bandwackeln und Plattenknistern.
struct Stueckspieler {
    kern: Rc<Kern>,
    partitur: Partitur,
    /// Audio-Uhr minus Stück-Zeit.
    versatz: f64,
    naechster: i64,
    blende: GainNode,
    trocken: GainNode,
    hall: AudioNode,
    tasten: AudioNode,
    melodie: AudioNode,
    bass: AudioNode,
    hat: AudioNode,
    wackeln: GainNode,
    vibrato: GainNode,
    schleifen: Vec<AudioScheduledSourceNode>,
}

impl Stueckspieler {
    fn neu(k: Rc<Kern>, aus: &AudioNode, stueck: &str) -> Self {
        let jetzt = k.jetzt();
        let blende = k.verstaerker(0.0);
        let _ = blende.gain().set_value_at_time(0.0, jetzt);
        let _ = blende
            .gain()
            .linear_ramp_to_value_at_time(PEGEL, jetzt + 1.5);
        let _ = blende.connect_with_audio_node(aus);
        let komp = k.ctx.create_dynamics_compressor().expect("Kompressor");
        komp.threshold().set_value(-20.0);
        komp.ratio().set_value(3.0);
        komp.attack().set_value(0.01);
        komp.release().set_value(0.2);
        let trocken = k.verstaerker(0.8);
        let dumpf = k.filter(BiquadFilterType::Lowpass, 5200.0, 0.5);
        let _ = trocken.connect_with_audio_node(&dumpf);
        let _ = dumpf.connect_with_audio_node(&komp);
        let _ = komp.connect_with_audio_node(&blende);

        let conv = k.ctx.create_convolver().expect("Convolver");
        conv.set_buffer(Some(&k.puffer.raum));
        let nass = k.verstaerker(0.35);
        let _ = conv.connect_with_audio_node(&nass);
        let _ = nass.connect_with_audio_node(&trocken);

        let mut schleifen = Vec::new();
        let mut lfo = |freq: f32, tiefe: f32| {
            let o = k.ctx.create_oscillator().expect("LFO");
            o.frequency().set_value(freq);
            let g = k.verstaerker(tiefe);
            let _ = o.connect_with_audio_node(&g);
            let _ = o.start();
            schleifen.push(o.unchecked_into::<AudioScheduledSourceNode>());
            g
        };
        // Die Tasten schwanken etwas in der Lautstärke, wie das Tremolo eines Rhodes.
        let trem = k.verstaerker(0.85);
        let _ = lfo(3.2, 0.15).connect_with_audio_param(&trem.gain());
        let tasten = k.filter(BiquadFilterType::Lowpass, 2400.0, 0.6);
        let _ = tasten.connect_with_audio_node(&trem);
        let _ = trem.connect_with_audio_node(&trocken);
        let _ = trem.connect_with_audio_node(&conv);
        let melodie = k.filter(BiquadFilterType::Lowpass, 3000.0, 0.7);
        let _ = melodie.connect_with_audio_node(&trocken);
        let _ = melodie.connect_with_audio_node(&conv);
        let bass = k.filter(BiquadFilterType::Lowpass, 420.0, 0.8);
        let _ = bass.connect_with_audio_node(&trocken);
        let hat = k.filter(BiquadFilterType::Highpass, 6500.0, 0.7);
        let _ = hat.connect_with_audio_node(&trocken);
        let wackeln = lfo(0.37, 9.0);
        let vibrato = lfo(5.2, 7.0);

        // Die Nadel in der Rille: Knistern und leises Rauschen, die ganze Zeit.
        if let Ok(q) = k.ctx.create_buffer_source() {
            q.set_buffer(Some(&k.puffer.knistern));
            q.set_loop(true);
            let g = k.verstaerker(0.05);
            let hp = k.filter(BiquadFilterType::Highpass, 900.0, 0.7);
            let _ = q.connect_with_audio_node(&hp);
            let _ = hp.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(&blende);
            let _ = q.start();
            schleifen.push(q.unchecked_into());
        }
        Self {
            partitur: Partitur::neu(stueck),
            versatz: f64::NAN,
            naechster: -1,
            blende,
            trocken,
            hall: conv.unchecked_into(),
            tasten: tasten.unchecked_into(),
            melodie: melodie.unchecked_into(),
            bass: bass.unchecked_into(),
            hat: hat.unchecked_into(),
            wackeln,
            vibrato,
            schleifen,
            kern: k,
        }
    }

    /// Plant, was kommt. `at`: wie weit das Stück jetzt ist, in Sekunden.
    fn takt(&mut self, at: f64) {
        let k = self.kern.clone();
        // Ein angehaltener Kontext hat eine stehende Uhr: alles käme auf einmal.
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
            self.naechster = (at / self.partitur.sechzehntel).ceil().max(0.0) as i64;
        }
        let bis = at + VORLAUF;
        while self.partitur.zeit(self.naechster as u64) < bis {
            let t = self.partitur.zeit(self.naechster as u64);
            if t >= at - 0.02 {
                let wann = k.jetzt().max(t + self.versatz);
                for n in self.partitur.noten(self.naechster as u64) {
                    self.spielen(&n, wann);
                }
            }
            self.naechster += 1;
        }
    }

    fn stoppen(self) {
        let jetzt = self.kern.jetzt();
        let g = self.blende.gain();
        let _ = g.cancel_scheduled_values(jetzt);
        let _ = g.set_value_at_time(g.value(), jetzt);
        let _ = g.linear_ramp_to_value_at_time(0.0, jetzt + 0.4);
        for s in &self.schleifen {
            let _ = s.stop_with_when(jetzt + 1.5);
        }
    }

    fn spielen(&self, n: &Note, wann: f64) {
        let k = &self.kern;
        match *n {
            Note::Kick { staerke } => {
                let o = k.ctx.create_oscillator().expect("Osz");
                let _ = o.frequency().set_value_at_time(115.0, wann);
                let _ = o
                    .frequency()
                    .exponential_ramp_to_value_at_time(44.0, wann + 0.12);
                self.huelle(&o, &self.trocken, wann, 0.006, 0.85 * staerke as f32, 0.42);
            }
            Note::Snare { staerke } => {
                let g = k.verstaerker(0.0);
                exp_huelle(&g, wann, 0.004, 0.32 * staerke as f32, 0.22);
                if let Ok(q) = k.ctx.create_buffer_source() {
                    q.set_buffer(Some(&k.puffer.weiss));
                    let bp = k.filter(BiquadFilterType::Bandpass, 1900.0, 0.9);
                    let _ = q.connect_with_audio_node(&bp);
                    let _ = bp.connect_with_audio_node(&g);
                    let _ = q.start_with_when_and_grain_offset(wann, js_sys::Math::random() * 2.0);
                    let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&q, wann + 0.25);
                }
                let o = k.ctx.create_oscillator().expect("Osz");
                o.set_type(OscillatorType::Triangle);
                let _ = o.frequency().set_value_at_time(190.0, wann);
                let _ = o
                    .frequency()
                    .exponential_ramp_to_value_at_time(140.0, wann + 0.08);
                self.huelle(&o, &g, wann, 0.003, 0.25 * staerke as f32, 0.1);
                let _ = g.connect_with_audio_node(&self.trocken);
                let _ = g.connect_with_audio_node(&self.hall);
            }
            Note::Hat { staerke } => {
                let g = k.verstaerker(0.0);
                exp_huelle(&g, wann, 0.002, 0.13 * staerke as f32, 0.045);
                if let Ok(q) = k.ctx.create_buffer_source() {
                    q.set_buffer(Some(&k.puffer.weiss));
                    let _ = q.connect_with_audio_node(&g);
                    let _ = q.start_with_when_and_grain_offset(wann, js_sys::Math::random() * 2.0);
                    let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&q, wann + 0.06);
                }
                let _ = g.connect_with_audio_node(&self.hat);
            }
            Note::Bass { midi, laenge } => {
                let f = midi_zu_hz(midi as f64) as f32;
                let g = k.verstaerker(0.0);
                let _ = g.gain().set_value_at_time(0.0, wann);
                let _ = g.gain().linear_ramp_to_value_at_time(0.34, wann + 0.012);
                let _ = g.gain().set_target_at_time(0.24, wann + 0.012, 0.3);
                let _ = g.gain().set_target_at_time(0.0, wann + laenge, 0.05);
                for (art, mal, lvl) in [
                    (OscillatorType::Sine, 1.0, 1.0),
                    (OscillatorType::Triangle, 2.0, 0.18),
                ] {
                    let o = k.ctx.create_oscillator().expect("Osz");
                    o.set_type(art);
                    o.frequency().set_value(f * mal);
                    let og = k.verstaerker(lvl);
                    let _ = o.connect_with_audio_node(&og);
                    let _ = og.connect_with_audio_node(&g);
                    let _ = o.start_with_when(wann);
                    let _ = o.stop_with_when(wann + laenge + 0.4);
                }
                let _ = g.connect_with_audio_node(&self.bass);
            }
            Note::Taste {
                midi,
                laenge,
                staerke,
                versatz,
            } => {
                // E-Piano: ein Sinus, kurz aufgehellt von einem zweiten in gleicher Tonhöhe (FM).
                let wann = wann + versatz;
                let f = midi_zu_hz(midi as f64) as f32;
                let traeger = k.ctx.create_oscillator().expect("Osz");
                traeger.frequency().set_value(f);
                traeger
                    .detune()
                    .set_value(((crate::tonfolge::streuung(midi as u32, 5) - 0.5) * 8.0) as f32);
                let modul = k.ctx.create_oscillator().expect("Osz");
                modul.frequency().set_value(f);
                let tiefe = k.verstaerker(0.0);
                let _ = tiefe
                    .gain()
                    .set_value_at_time(f * 1.4 * staerke as f32, wann);
                let _ = tiefe
                    .gain()
                    .exponential_ramp_to_value_at_time(f * 0.12, wann + 0.5);
                let _ = modul.connect_with_audio_node(&tiefe);
                let _ = tiefe.connect_with_audio_param(&traeger.frequency());
                let g = k.verstaerker(0.0);
                let _ = g.gain().set_value_at_time(0.0, wann);
                let _ = g
                    .gain()
                    .linear_ramp_to_value_at_time(0.085 * staerke as f32, wann + 0.006);
                let _ = g
                    .gain()
                    .set_target_at_time(0.035 * staerke as f32, wann + 0.006, 0.5);
                let _ = g.gain().set_target_at_time(0.0, wann + laenge, 0.12);
                let _ = traeger.connect_with_audio_node(&g);
                let _ = g.connect_with_audio_node(&self.tasten);
                let _ = self.wackeln.connect_with_audio_param(&traeger.detune());
                let ende = wann + laenge + 0.8;
                let _ = traeger.start_with_when(wann);
                let _ = modul.start_with_when(wann);
                let _ = traeger.stop_with_when(ende);
                let _ = modul.stop_with_when(ende);
            }
            Note::Melodie { midi, laenge } => {
                let o = k.ctx.create_oscillator().expect("Osz");
                o.set_type(OscillatorType::Triangle);
                o.frequency().set_value(midi_zu_hz(midi as f64) as f32);
                let g = k.verstaerker(0.0);
                let _ = g.gain().set_value_at_time(0.0, wann);
                let _ = g.gain().linear_ramp_to_value_at_time(0.075, wann + 0.03);
                let _ = g.gain().set_target_at_time(0.05, wann + 0.03, 0.4);
                let _ = g.gain().set_target_at_time(0.0, wann + laenge, 0.1);
                let _ = o.connect_with_audio_node(&g);
                let _ = g.connect_with_audio_node(&self.melodie);
                let _ = self.wackeln.connect_with_audio_param(&o.detune());
                let _ = self.vibrato.connect_with_audio_param(&o.detune());
                let _ = o.start_with_when(wann);
                let _ = o.stop_with_when(wann + laenge + 0.6);
            }
        }
        let _ = abschnitt; // Abschnitte stecken schon in den Noten.
    }

    /// Oszillator durch eine exponentielle Hüllkurve nach `ziel`.
    fn huelle(
        &self,
        o: &web_sys::OscillatorNode,
        ziel: &AudioNode,
        wann: f64,
        an: f64,
        spitze: f32,
        aus: f64,
    ) {
        let g = self.kern.verstaerker(0.0);
        exp_huelle(&g, wann, an, spitze, aus);
        let _ = o.connect_with_audio_node(&g);
        let _ = g.connect_with_audio_node(ziel);
        let _ = o.start_with_when(wann);
        let _ = o.stop_with_when(wann + aus + 0.03);
    }
}

fn exp_huelle(g: &GainNode, wann: f64, an: f64, spitze: f32, aus: f64) {
    let p = g.gain();
    let _ = p.set_value_at_time(0.0001, wann);
    let _ = p.exponential_ramp_to_value_at_time(spitze.max(0.0002), wann + an);
    let _ = p.exponential_ramp_to_value_at_time(0.0001, wann + aus);
}
