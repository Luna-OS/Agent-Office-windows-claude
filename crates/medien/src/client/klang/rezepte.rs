//! Die Rezepte: Hinweistöne, Schritte, Papier, der Gong, und das Tippen an den Schreibtischen.

use std::collections::HashMap;

use web_sys::{AudioNode, BiquadFilterType, OscillatorType};

use super::{Kern, Klang, Ort, waehle, zufall, zufall_ganz};

/// Warum ein Arbeiter klingelt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ding {
    /// Zwei Töne aufwärts: fertig.
    Fertig,
    /// Drei Töne: braucht eine Eingabe.
    Eingabe,
}

/// Warum der Gong schlägt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gong {
    /// Jemand hat draufgehauen.
    Schlag,
    /// Ein Pull Request wurde gemergt (kräftiger).
    Gemergt,
    /// Die Warteschlange ist leer (drei Schläge, jeder größer).
    Leer,
}

/// Obertöne eines Gongs: (Verhältnis zum tiefsten, Lautstärke, Sekunden bis verklungen).
const GONG_TEILTOENE: [(f32, f32, f64); 11] = [
    (1.0, 0.8, 7.0),
    (1.51, 0.75, 5.5),
    (2.13, 0.65, 4.6),
    (2.66, 0.55, 3.8),
    (3.19, 0.45, 3.1),
    (3.84, 0.38, 2.5),
    (4.48, 0.3, 2.0),
    (5.27, 0.22, 1.6),
    (6.35, 0.16, 1.2),
    (7.61, 0.1, 0.9),
    (9.08, 0.07, 0.6),
];

impl Klang {
    /// Ein Arbeiter ist fertig oder braucht eine Eingabe.
    pub fn ding(&self, art: Ding) {
        self.entsperren();
        let Some(k) = self.kern() else { return };
        let toene: &[f32] = match art {
            Ding::Fertig => &[660.0, 880.0],
            Ding::Eingabe => &[880.0, 660.0, 880.0],
        };
        for (i, &f) in toene.iter().enumerate() {
            let t0 = k.jetzt() + i as f64 * 0.12;
            ton(
                &k,
                OscillatorType::Triangle,
                f,
                t0,
                0.02,
                0.3,
                0.25,
                k.hinweise.as_ref(),
            );
        }
    }

    /// Ein eigener Schritt, oder das Aufkommen nach einem Sprung.
    pub fn schritt(&self, landen: bool) {
        let Some(k) = self.kern() else { return };
        let p = waehle(&k.puffer.schritte);
        if landen {
            k.abspielen(p, 0.0, 0.5, 0.75, k.umgebung.as_ref());
        } else {
            k.abspielen(
                p,
                0.0,
                zufall(0.16, 0.21) as f32,
                zufall(0.9, 1.1) as f32,
                k.umgebung.as_ref(),
            );
        }
    }

    /// Der Schritt eines anderen, auf dem Boden der Etage (oder in Höhe `y`).
    pub fn schritt_bei(&self, x: f32, z: f32, y: f32) {
        let Some(k) = self.kern() else { return };
        let ort = Ort { x, y: y + 0.1, z };
        k.abspielen_bei(
            waehle(&k.puffer.schritte),
            ort,
            zufall(0.3, 0.38) as f32,
            zufall(0.9, 1.1) as f32,
            1.5,
            1.4,
        );
    }

    /// Eine Karte in der Hand: vom Board genommen oder auf einen Tisch gelegt.
    pub fn papier(&self) {
        let Some(k) = self.kern() else { return };
        k.abspielen(
            &k.puffer.rascheln,
            0.0,
            0.5,
            zufall(1.1, 1.3) as f32,
            k.umgebung.as_ref(),
        );
    }

    /// Eine Buchseite wird umgeblättert: ein weiches Wischen, dann ein leichtes Auflegen.
    pub fn seite_umblaettern(&self) {
        let Some(k) = self.kern() else { return };
        let t0 = k.jetzt() + 0.005;
        let len = zufall(0.24, 0.32);
        let Ok(q) = k.ctx.create_buffer_source() else {
            return;
        };
        q.set_buffer(Some(&k.puffer.weiss));
        let band = k.filter(BiquadFilterType::Bandpass, 1000.0, 0.8);
        let f = band.frequency();
        let _ = f.set_value_at_time(zufall(800.0, 1100.0) as f32, t0);
        let _ = f.exponential_ramp_to_value_at_time(zufall(2400.0, 3000.0) as f32, t0 + len * 0.6);
        let _ = f.exponential_ramp_to_value_at_time(1400.0, t0 + len);
        let g = k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0, t0);
        let _ = g.gain().linear_ramp_to_value_at_time(0.075, t0 + len * 0.3);
        let _ = g.gain().linear_ramp_to_value_at_time(0.13, t0 + len * 0.6);
        let _ = g.gain().linear_ramp_to_value_at_time(0.0, t0 + len);
        let tief = k.filter(BiquadFilterType::Lowpass, 4500.0, 0.7);
        let _ = q.connect_with_audio_node(&band);
        let _ = band.connect_with_audio_node(&tief);
        let _ = tief.connect_with_audio_node(&g);
        let _ = g.connect_with_audio_node(&k.umgebung);
        let _ = q.start_with_when_and_grain_offset(t0, zufall(0.0, 4.5));
        let _ = web_sys::AudioScheduledSourceNode::stop_with_when(&q, t0 + len + 0.02);
        k.abspielen(
            waehle(&k.puffer.schritte),
            t0 + len * 0.85,
            0.08,
            zufall(2.4, 2.8) as f32,
            k.umgebung.as_ref(),
        );
    }

    /// Der Gong am PR-Board, von dort, wo er hängt.
    pub fn gong(&self, warum: Gong, ort: Ort) {
        self.entsperren();
        let Some(k) = self.kern() else { return };
        let (referenz, abfall, bus) = match warum {
            Gong::Schlag => (4.0, 0.6, &k.umgebung),
            _ => (8.0, 0.45, &k.hinweise),
        };
        let aus = k.panner(ort, referenz, abfall);
        let _ = aus.connect_with_audio_node(bus);
        let t0 = k.jetzt() + 0.03;
        match warum {
            Gong::Leer => {
                for (i, s) in [0.7, 0.85, 1.1].into_iter().enumerate() {
                    gong_schlag(&k, aus.as_ref(), t0 + i as f64 * 0.85, s);
                }
            }
            Gong::Gemergt => gong_schlag(&k, aus.as_ref(), t0, 1.0),
            Gong::Schlag => gong_schlag(&k, aus.as_ref(), t0, zufall(0.6, 0.8) as f32),
        }
    }
}

/// Ein einfacher Ton mit Hüllkurve (exponentiell rauf und wieder runter).
#[allow(clippy::too_many_arguments)]
fn ton(
    k: &Kern,
    art: OscillatorType,
    f: f32,
    t0: f64,
    an: f64,
    spitze: f32,
    aus: f64,
    ziel: &AudioNode,
) {
    let Ok(o) = k.ctx.create_oscillator() else {
        return;
    };
    o.set_type(art);
    o.frequency().set_value(f);
    let g = k.verstaerker(0.0);
    let _ = g.gain().set_value_at_time(0.0001, t0);
    let _ = g.gain().exponential_ramp_to_value_at_time(spitze, t0 + an);
    let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, t0 + aus);
    let _ = o.connect_with_audio_node(&g);
    let _ = g.connect_with_audio_node(ziel);
    let _ = o.start_with_when(t0);
    let _ = o.stop_with_when(t0 + aus + 0.05);
}

/// Ein Schlag mit dem Klöppel: das Metall klingt in nicht-harmonischen Obertönen, leicht schwebend.
fn gong_schlag(k: &Kern, aus: &AudioNode, t0: f64, staerke: f32) {
    let f0 = 118.0 * zufall(0.98, 1.02) as f32;
    let ring = k.verstaerker(0.3 * staerke);
    let _ = ring.connect_with_audio_node(aus);
    let lang = 0.6 + 0.4 * staerke as f64;
    for (verh, amp, abkl) in GONG_TEILTOENE {
        let f = f0 * verh;
        let ende = t0 + abkl * lang;
        for cent in [-1.0, 1.0] {
            let Ok(o) = k.ctx.create_oscillator() else {
                continue;
            };
            let _ = o
                .frequency()
                .set_value_at_time(f * (1.0 + 0.012 * staerke), t0);
            let _ = o.frequency().exponential_ramp_to_value_at_time(f, t0 + 1.2);
            o.detune().set_value(cent * zufall(2.0, 5.0) as f32);
            let g = k.verstaerker(0.0);
            let _ = g.gain().set_value_at_time(0.0001, t0);
            let _ = g
                .gain()
                .exponential_ramp_to_value_at_time(amp * 0.5, t0 + 0.01 + verh as f64 * 0.004);
            let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, ende);
            let _ = o.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(&ring);
            let _ = o.start_with_when(t0);
            let _ = o.stop_with_when(ende + 0.05);
        }
    }
    // Der Filzklöppel: ein kurzer, tiefer Stoß.
    let Ok(o) = k.ctx.create_oscillator() else {
        return;
    };
    let _ = o.frequency().set_value_at_time(80.0, t0);
    let _ = o
        .frequency()
        .exponential_ramp_to_value_at_time(45.0, t0 + 0.15);
    let g = k.verstaerker(0.0);
    let _ = g.gain().set_value_at_time(0.0001, t0);
    let _ = g
        .gain()
        .exponential_ramp_to_value_at_time(0.5 * staerke, t0 + 0.004);
    let _ = g
        .gain()
        .exponential_ramp_to_value_at_time(0.0001, t0 + 0.25);
    let _ = o.connect_with_audio_node(&g);
    let _ = g.connect_with_audio_node(aus);
    let _ = o.start_with_when(t0);
    let _ = o.stop_with_when(t0 + 0.3);
}

struct Tippend {
    ort: Ort,
    an: bool,
    naechster: f64,
    uebrig: i32,
    wort: i32,
    panner: Option<web_sys::PannerNode>,
}

/// Das Tippen an den Schreibtischen: Schübe von Anschlägen, Leertasten, Enter, ab und zu die Maus.
#[derive(Default)]
pub(super) struct Tipper {
    tippende: HashMap<String, Tippend>,
}

#[derive(Clone, Copy)]
enum Taste {
    Anschlag,
    Leer,
    Enter,
    Maus,
}

impl Tipper {
    pub fn setzen(&mut self, id: &str, x: f32, z: f32, an: bool) {
        let t = self.tippende.entry(id.into()).or_insert(Tippend {
            ort: Ort { x, y: 0.9, z },
            an,
            naechster: 0.0,
            uebrig: 0,
            wort: 0,
            panner: None,
        });
        t.an = an;
        if t.ort.x != x || t.ort.z != z {
            t.ort = Ort { x, y: 0.9, z };
            t.panner = None;
        }
    }

    pub fn entfernen(&mut self, id: &str) {
        self.tippende.remove(id);
    }

    /// Plant etwas voraus auf der Audio-Uhr, damit der Rhythmus nicht mit der Bildrate wackelt.
    pub fn planen(&mut self, k: &Kern) {
        let jetzt = k.jetzt();
        let horizont = jetzt + 0.12;
        for t in self.tippende.values_mut().filter(|t| t.an) {
            if t.panner.is_none() {
                let p = k.panner(t.ort, 1.2, 1.3);
                let _ = p.connect_with_audio_node(&k.umgebung);
                t.panner = Some(p);
            }
            if t.naechster < jetzt - 0.25 {
                t.naechster = jetzt + zufall(0.05, 0.8);
                t.uebrig = 0;
            }
            while t.naechster < horizont {
                let wann = t.naechster;
                if t.uebrig == 0 {
                    t.uebrig = zufall_ganz(6, 36);
                    t.wort = zufall_ganz(2, 8);
                    if zufall(0.0, 1.0) < 0.3 {
                        anschlag(k, t, wann, Taste::Maus);
                        if zufall(0.0, 1.0) < 0.5 {
                            anschlag(k, t, wann + zufall(0.1, 0.16), Taste::Maus);
                        }
                        t.naechster = wann + zufall(0.4, 1.2);
                        continue;
                    }
                }
                t.uebrig -= 1;
                if t.uebrig == 0 {
                    anschlag(
                        k,
                        t,
                        wann,
                        if zufall(0.0, 1.0) < 0.4 {
                            Taste::Enter
                        } else {
                            Taste::Anschlag
                        },
                    );
                    t.naechster = wann
                        + if zufall(0.0, 1.0) < 0.15 {
                            zufall(4.0, 9.0)
                        } else {
                            zufall(0.6, 3.0)
                        };
                } else {
                    t.wort -= 1;
                    if t.wort <= 0 {
                        anschlag(k, t, wann, Taste::Leer);
                        t.wort = zufall_ganz(2, 8);
                        t.naechster = wann + zufall(0.1, 0.22);
                    } else {
                        anschlag(k, t, wann, Taste::Anschlag);
                        t.naechster = wann + zufall(0.065, 0.16);
                    }
                }
            }
        }
    }
}

fn anschlag(k: &Kern, t: &Tippend, wann: f64, art: Taste) {
    let Some(ziel) = &t.panner else { return };
    let b = &k.puffer;
    let (puffer, laut) = match art {
        Taste::Anschlag => (waehle(&b.tasten), zufall(0.24, 0.34) as f32),
        Taste::Leer => (waehle(&b.leertasten), 0.4),
        Taste::Enter => (waehle(&b.leertasten), 0.55),
        Taste::Maus => (&b.maus, 0.3),
    };
    k.abspielen(puffer, wann, laut, zufall(0.93, 1.07) as f32, ziel.as_ref());
}
