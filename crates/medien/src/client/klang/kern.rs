//! Der Audio-Kern: Kontext, Busse, Proben und die Handgriffe, aus denen jedes Rezept gebaut ist
//! (Panner, Probe abspielen, Blip, Klirren, Rauschquelle, Hüllkurve).

use wasm_bindgen::JsCast;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioNode, AudioParam,
    AudioScheduledSourceNode, BiquadFilterNode, BiquadFilterType, DistanceModelType, GainNode,
    OscillatorNode, OscillatorType, PannerNode, PanningModelType,
};

use super::Ort;
use super::puffer::Puffer;

pub(crate) struct Kern {
    pub ctx: AudioContext,
    pub master: GainNode,
    /// Der Raum selbst; wird leise, wenn der Tab verborgen ist.
    pub umgebung: GainNode,
    /// Hinweise der Arbeiter, die man auch aus einem anderen Tab hören will.
    pub hinweise: GainNode,
    /// Das Summen des Office (Raum, Kühlschrank); bleibt unten, wenn man aufs Dach geht …
    pub drinnen: GainNode,
    /// … wo Wind ist und die Stadt weit unten.
    pub draussen: GainNode,
    pub puffer: Puffer,
}

impl Kern {
    pub fn neu() -> Option<Self> {
        let ctx = AudioContext::new().ok()?;
        let puffer = Puffer::neu(&ctx)?;
        let komp = ctx.create_dynamics_compressor().ok()?;
        komp.threshold().set_value(-14.0);
        komp.knee().set_value(12.0);
        komp.ratio().set_value(4.0);
        komp.attack().set_value(0.004);
        komp.release().set_value(0.25);
        let master = ctx.create_gain().ok()?;
        master.gain().set_value(0.0);
        master.connect_with_audio_node(&komp).ok()?;
        komp.connect_with_audio_node(&ctx.destination()).ok()?;
        let umgebung = ctx.create_gain().ok()?;
        umgebung.connect_with_audio_node(&master).ok()?;
        let hinweise = ctx.create_gain().ok()?;
        hinweise.connect_with_audio_node(&master).ok()?;
        let drinnen = ctx.create_gain().ok()?;
        drinnen.connect_with_audio_node(&umgebung).ok()?;
        let draussen = ctx.create_gain().ok()?;
        draussen.gain().set_value(0.0);
        draussen.connect_with_audio_node(&umgebung).ok()?;
        let _ = ctx.resume();
        Some(Self {
            ctx,
            master,
            umgebung,
            hinweise,
            drinnen,
            draussen,
            puffer,
        })
    }

    pub fn jetzt(&self) -> f64 {
        self.ctx.current_time()
    }

    pub fn verstaerker(&self, wert: f32) -> GainNode {
        let g = self.ctx.create_gain().expect("GainNode");
        g.gain().set_value(wert);
        g
    }

    pub fn filter(&self, art: BiquadFilterType, frequenz: f32, q: f32) -> BiquadFilterNode {
        let f = self.ctx.create_biquad_filter().expect("BiquadFilterNode");
        f.set_type(art);
        f.frequency().set_value(frequenz);
        f.q().set_value(q);
        f
    }

    pub fn oszillator(&self, art: OscillatorType) -> OscillatorNode {
        let o = self.ctx.create_oscillator().expect("OscillatorNode");
        o.set_type(art);
        o
    }

    /// Ein Panner an `ort`: so hört man, woher etwas kommt.
    pub fn panner(&self, ort: Ort, referenz: f64, abfall: f64) -> PannerNode {
        let p = self.ctx.create_panner().expect("PannerNode");
        p.set_panning_model(PanningModelType::Equalpower);
        p.set_distance_model(DistanceModelType::Inverse);
        p.set_ref_distance(referenz);
        p.set_rolloff_factor(abfall);
        p.set_position(ort.x as f64, ort.y as f64, ort.z as f64);
        p
    }

    /// Ausgang eines Rezepts: von `ort` her (Panner) oder, ohne Ort, direkt; hängt an `bus`.
    pub fn ausgang(
        &self,
        ort: Option<Ort>,
        referenz: f64,
        abfall: f64,
        bus: &AudioNode,
    ) -> AudioNode {
        let n: AudioNode = match ort {
            Some(o) => self.panner(o, referenz, abfall).unchecked_into(),
            None => self.verstaerker(1.0).unchecked_into(),
        };
        let _ = n.connect_with_audio_node(bus);
        n
    }

    /// Spielt eine Probe ab: `wann` auf der Audio-Uhr (0 = sofort), Lautstärke, Tempo, Ziel.
    pub fn abspielen(
        &self,
        puffer: &AudioBuffer,
        wann: f64,
        lautstaerke: f32,
        tempo: f32,
        ziel: &AudioNode,
    ) {
        let Ok(q) = self.ctx.create_buffer_source() else {
            return;
        };
        q.set_buffer(Some(puffer));
        q.playback_rate().set_value(tempo);
        let g = self.verstaerker(lautstaerke);
        let _ = q
            .connect_with_audio_node(&g)
            .and_then(|_| g.connect_with_audio_node(ziel));
        let _ = q.start_with_when(wann.max(self.jetzt()));
    }

    /// Wie [`abspielen`](Self::abspielen), aber aus einer Richtung, auf den Umgebungs-Bus.
    pub fn abspielen_bei(
        &self,
        puffer: &AudioBuffer,
        ort: Ort,
        lautstaerke: f32,
        tempo: f32,
        referenz: f64,
        abfall: f64,
    ) {
        let p = self.panner(ort, referenz, abfall);
        let _ = p.connect_with_audio_node(&self.umgebung);
        self.abspielen(puffer, 0.0, lautstaerke, tempo, p.unchecked_ref());
    }

    /// Ein zufälliger Schritt-Puffer (dumpfer Schlag, Grundbaustein vieler Geräusche).
    pub fn schlag(&self, wann: f64, lautstaerke: f32, tempo: f32, ziel: &AudioNode) {
        self.abspielen(
            super::waehle(&self.puffer.schritte),
            wann,
            lautstaerke,
            tempo,
            ziel,
        );
    }

    /// Ein kurzer gestimmter Blip: eine Blase, wenn `verh` > 1, ein Tropfen, wenn < 1.
    #[allow(clippy::too_many_arguments)]
    pub fn blip(
        &self,
        ziel: &AudioNode,
        wann: f64,
        f: f32,
        verh: f32,
        laenge: f64,
        laut: f32,
        art: OscillatorType,
    ) {
        let o = self.oszillator(art);
        let _ = o.frequency().set_value_at_time(f, wann);
        let _ = o
            .frequency()
            .exponential_ramp_to_value_at_time(f * verh, wann + laenge);
        let g = self.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0001, wann);
        let _ = g
            .gain()
            .exponential_ramp_to_value_at_time(laut.max(0.0002), wann + 0.006);
        let _ = g
            .gain()
            .exponential_ramp_to_value_at_time(0.0001, wann + laenge);
        let _ = o.connect_with_audio_node(&g);
        let _ = g.connect_with_audio_node(ziel);
        let _ = o.start_with_when(wann);
        let _ = o.stop_with_when(wann + laenge + 0.02);
    }

    /// Ein Glas klingt: zwei hohe Teiltöne, gleich wieder weg.
    pub fn klirren(&self, ziel: &AudioNode, wann: f64, f: f32, laut: f32) {
        for (mal, lvl) in [(1.0, 1.0), (2.76, 0.4)] {
            let o = self.oszillator(OscillatorType::Sine);
            o.frequency().set_value(f * mal);
            let g = self.verstaerker(0.0);
            let _ = g.gain().set_value_at_time(0.0001, wann);
            let _ = g
                .gain()
                .exponential_ramp_to_value_at_time((laut * lvl).max(0.0002), wann + 0.003);
            let _ = g
                .gain()
                .exponential_ramp_to_value_at_time(0.0001, wann + 0.25);
            let _ = o.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(ziel);
            let _ = o.start_with_when(wann);
            let _ = o.stop_with_when(wann + 0.3);
        }
    }

    /// Eine Rausch- (oder sonstige Puffer-)Quelle, noch nicht gestartet.
    pub fn quelle(&self, puffer: &AudioBuffer, schleife: bool) -> AudioBufferSourceNode {
        let q = self.ctx.create_buffer_source().expect("BufferSource");
        q.set_buffer(Some(puffer));
        q.set_loop(schleife);
        q
    }
}

/// Rampt `param` ab `t0` von 0 durch (Sekunden nach t0, Wert)-Punkte.
pub(crate) fn huelle(param: &AudioParam, t0: f64, punkte: &[(f64, f32)]) {
    let _ = param.set_value_at_time(0.0, t0);
    for &(dt, v) in punkte {
        let _ = param.linear_ramp_to_value_at_time(v, t0 + dt);
    }
}

/// Startet und stoppt eine Quelle (Puffer oder Oszillator).
pub(crate) fn laufen(q: &AudioScheduledSourceNode, von: f64, bis: f64) {
    let _ = q.start_with_when(von);
    let _ = q.stop_with_when(bis);
}

/// Verkettet Knoten: a → b → c …
pub(crate) fn kette(knoten: &[&AudioNode]) {
    for paar in knoten.windows(2) {
        let _ = paar[0].connect_with_audio_node(paar[1]);
    }
}
