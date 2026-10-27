//! Was der Raum von selbst tut: Grundrauschen und Lüftung, der Kühlschrank, Vögel am Tag, Grillen in
//! der Nacht, Telefone an fernen Tischen, und auf dem Dach Wind und die Stadt tief unten.

use web_sys::{AudioNode, BiquadFilterType, GainNode, OscillatorType};

use super::kern::{huelle, kette};
use super::wetter::Regen;
use super::{Kern, Lage, Ort, waehle, zufall, zufall_f, zufall_ganz};

/// Etwas, das ab und zu passiert: `erst` Sekunden nach dem Start, danach alle `abstand` Sekunden.
struct AbUndZu {
    naechstes: f64,
    abstand: fn() -> f64,
}

impl AbUndZu {
    fn neu(jetzt: f64, erst: f64, abstand: fn() -> f64) -> Self {
        Self {
            naechstes: jetzt + erst,
            abstand,
        }
    }

    fn faellig(&mut self, jetzt: f64) -> bool {
        if jetzt < self.naechstes {
            return false;
        }
        self.naechstes = jetzt + (self.abstand)();
        true
    }
}

struct Kuehlschrank {
    pegel: GainNode,
    an: bool,
    naechstes: f64,
}

#[derive(Default)]
pub(super) struct Umgebung {
    kuehlschrank: Option<Kuehlschrank>,
    voegel: Option<AbUndZu>,
    grillen: Option<AbUndZu>,
    telefone: Option<AbUndZu>,
    regen: Regen,
}

impl Umgebung {
    pub fn starten(&mut self, k: &Kern, lage: &Lage) {
        raumton(k);
        wind(k);
        self.kuehlschrank = Some(kuehlschrank(k, lage.orte.kuehlschrank));
        let jetzt = k.jetzt();
        // Manchmal antwortet ein zweiter Vogel von einem anderen Fenster.
        self.voegel = Some(AbUndZu::neu(jetzt, zufall(5.0, 15.0), || {
            if js_sys::Math::random() < 0.35 {
                zufall(1.5, 4.0)
            } else {
                zufall(12.0, 35.0)
            }
        }));
        self.grillen = Some(AbUndZu::neu(jetzt, zufall(2.0, 6.0), || zufall(3.0, 8.0)));
        self.telefone = Some(AbUndZu::neu(jetzt, zufall(60.0, 150.0), || {
            zufall(90.0, 240.0)
        }));
    }

    pub fn takt(&mut self, k: &Kern, lage: &Lage) {
        let jetzt = k.jetzt();
        if let Some(f) = &mut self.kuehlschrank
            && jetzt >= f.naechstes
        {
            // Der Kompressor springt an, läuft eine Weile und klonkt wieder aus.
            f.an = !f.an;
            let _ = f.pegel.gain().set_target_at_time(
                if f.an { 0.06 } else { 0.0 },
                jetzt,
                if f.an { 0.6 } else { 0.3 },
            );
            f.naechstes = jetzt
                + if f.an {
                    zufall(25.0, 50.0)
                } else {
                    zufall(20.0, 45.0)
                };
            let p = k.panner(lage.orte.kuehlschrank, 1.0, 1.6);
            let _ = p.connect_with_audio_node(&k.drinnen);
            k.schlag(0.0, 0.25, 0.6, &p);
        }
        let fenster = &lage.orte.fenster;
        if self.voegel.as_mut().is_some_and(|v| v.faellig(jetzt))
            && lage.nacht < 0.5
            && lage.regen < 0.1
            && !fenster.is_empty()
        {
            voegel(k, *waehle(fenster), jetzt);
        }
        if self.grillen.as_mut().is_some_and(|v| v.faellig(jetzt))
            && lage.nacht > 0.6
            && lage.regen < 0.05
            && !fenster.is_empty()
        {
            grillen(k, *waehle(fenster), jetzt);
        }
        if self.telefone.as_mut().is_some_and(|v| v.faellig(jetzt))
            && !lage.draussen
            && !lage.orte.tische.is_empty()
        {
            let l = lage.kopf;
            let fern: Vec<Ort> = lage
                .orte
                .tische
                .iter()
                .copied()
                .filter(|t| (t.x - l.x).hypot(t.z - l.z) > 7.0)
                .collect();
            let tisch = *waehle(if fern.is_empty() {
                &lage.orte.tische
            } else {
                &fern
            });
            telefon(k, tisch, jetzt);
        }
        self.regen.takt(k, lage, jetzt);
    }
}

/// Ein tiefes Grollen von Gebäude und Verkehr, und die Lüftung, die langsam an- und abschwillt.
fn raumton(k: &Kern) {
    let grollen = k.quelle(&k.puffer.braun, true);
    let gg = k.verstaerker(0.07);
    let tp = k.filter(BiquadFilterType::Lowpass, 300.0, 0.7);
    kette(&[&grollen, &tp, &gg, &k.drinnen]);
    let luft = k.quelle(&k.puffer.weiss, true);
    let lg = k.verstaerker(0.009);
    let schwellen = k.oszillator(OscillatorType::Sine);
    schwellen.frequency().set_value(0.06);
    let tiefe = k.verstaerker(0.004);
    let _ = schwellen.connect_with_audio_node(&tiefe);
    let _ = tiefe.connect_with_audio_param(&lg.gain());
    let bp = k.filter(BiquadFilterType::Bandpass, 650.0, 0.5);
    kette(&[&luft, &bp, &lg, &k.drinnen]);
    let _ = grollen.start();
    let _ = luft.start();
    let _ = schwellen.start();
}

/// Auf dem Dach: Verkehr weit unten und Wind, der in Böen kommt und höher pfeift, wenn er zulegt.
fn wind(k: &Kern) {
    let stadt = k.quelle(&k.puffer.braun, true);
    let sg = k.verstaerker(0.08);
    let tp = k.filter(BiquadFilterType::Lowpass, 420.0, 0.6);
    kette(&[&stadt, &tp, &sg, &k.draussen]);
    let wind = k.quelle(&k.puffer.weiss, true);
    let ton = k.filter(BiquadFilterType::Bandpass, 520.0, 0.8);
    let wg = k.verstaerker(0.012);
    let boe = k.oszillator(OscillatorType::Sine);
    boe.frequency().set_value(0.08);
    let boe_tiefe = k.verstaerker(0.009);
    let _ = boe.connect_with_audio_node(&boe_tiefe);
    let _ = boe_tiefe.connect_with_audio_param(&wg.gain());
    let hoehe = k.verstaerker(220.0);
    let _ = boe.connect_with_audio_node(&hoehe);
    let _ = hoehe.connect_with_audio_param(&ton.frequency());
    kette(&[&wind, &ton, &wg, &k.draussen]);
    let _ = stadt.start();
    let _ = wind.start();
    let _ = boe.start();
}

/// Der Kühlschrank in der Küche, der vor sich hin brummt.
fn kuehlschrank(k: &Kern, ort: Ort) -> Kuehlschrank {
    let brumm = k.oszillator(OscillatorType::Sawtooth);
    brumm.frequency().set_value(50.0);
    let sirren = k.oszillator(OscillatorType::Sine);
    sirren.frequency().set_value(120.0);
    let sg = k.verstaerker(0.3);
    let pegel = k.verstaerker(0.0);
    let ton = k.filter(BiquadFilterType::Lowpass, 220.0, 0.7);
    let _ = brumm.connect_with_audio_node(&ton);
    kette(&[&sirren, &sg, &ton]);
    let p = k.panner(ort, 1.0, 1.6);
    kette(&[&ton, &pegel, &p, &k.drinnen]);
    let _ = brumm.start();
    let _ = sirren.start();
    Kuehlschrank {
        pegel,
        an: false,
        naechstes: k.jetzt() + zufall(3.0, 12.0),
    }
}

/// Durchs Glas gehört: ein Panner am Fenster, hinter einem Tiefpass.
fn am_fenster(k: &Kern, ort: Ort, tp: f32) -> AudioNode {
    let p = k.panner(ort, 2.0, 1.2);
    let f = k.filter(BiquadFilterType::Lowpass, tp, 0.7);
    kette(&[&p, &f, &k.umgebung]);
    p.into()
}

/// Ein paar Zwitscherer vor einem Fenster.
fn voegel(k: &Kern, ort: Ort, jetzt: f64) {
    let aus = am_fenster(k, ort, 5000.0);
    let basis = zufall_f(2400.0, 4200.0);
    let form = js_sys::Math::random();
    let mut t = jetzt + 0.05;
    for _ in 0..zufall_ganz(2, 6) {
        let len = zufall(0.06, 0.14);
        let o = k.oszillator(OscillatorType::Sine);
        let _ = o
            .frequency()
            .set_value_at_time(basis * zufall_f(0.9, 1.05), t);
        if form < 0.5 {
            let _ = o
                .frequency()
                .exponential_ramp_to_value_at_time(basis * zufall_f(1.25, 1.6), t + len * 0.6);
            let _ = o
                .frequency()
                .exponential_ramp_to_value_at_time(basis * zufall_f(0.8, 1.0), t + len);
        } else {
            let _ = o
                .frequency()
                .exponential_ramp_to_value_at_time(basis * zufall_f(0.6, 0.75), t + len);
        }
        let g = k.verstaerker(0.0);
        let _ = g.gain().set_value_at_time(0.0001, t);
        let _ = g.gain().exponential_ramp_to_value_at_time(0.06, t + 0.012);
        let _ = g.gain().exponential_ramp_to_value_at_time(0.0001, t + len);
        kette(&[&o, &g, &aus]);
        super::kern::laufen(&o, t, t + len + 0.02);
        t += len + zufall(0.04, 0.2);
    }
}

/// Eine Grille vor dem Fenster, ein paar Sekunden lang.
fn grillen(k: &Kern, ort: Ort, jetzt: f64) {
    let aus = am_fenster(k, ort, 6000.0);
    let f = zufall_f(4200.0, 5200.0);
    let mut t = jetzt + 0.05;
    for _ in 0..zufall_ganz(4, 9) {
        for _ in 0..3 {
            k.blip(&aus, t, f, 1.0, 0.022, 0.022, OscillatorType::Sine);
            t += 0.035;
        }
        t += zufall(0.35, 0.6);
    }
}

/// Ein Tischtelefon trillert ein paar Mal irgendwo im Raum, dann nimmt jemand ab.
fn telefon(k: &Kern, tisch: Ort, jetzt: f64) {
    let p = k.panner(
        Ort {
            x: tisch.x,
            y: 0.9,
            z: tisch.z,
        },
        1.5,
        1.2,
    );
    let tp = k.filter(BiquadFilterType::Lowpass, 3000.0, 0.7);
    kette(&[&p, &tp, &k.umgebung]);
    for r in 0..zufall_ganz(2, 3) {
        let t = jetzt + 0.05 + r as f64 * 2.4;
        let o = k.oszillator(OscillatorType::Triangle);
        for i in 0..18 {
            let _ = o.frequency().set_value_at_time(
                if i % 2 == 1 { 1450.0 } else { 1150.0 },
                t + i as f64 / 18.0,
            );
        }
        let g = k.verstaerker(0.0);
        huelle(&g.gain(), t, &[(0.02, 0.045), (0.95, 0.045), (1.0, 0.0)]);
        kette(&[&o, &g, &p]);
        super::kern::laufen(&o, t, t + 1.05);
    }
}
