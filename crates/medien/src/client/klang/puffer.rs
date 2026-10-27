//! Klang-Proben, einmal erzeugt, sobald Audio startet: Tastenklicks, Schritte, Papier, Rauschen,
//! Plattenknistern und ein kleiner dunkler Raum für den Hall der Jukebox.

use std::f64::consts::PI;

use web_sys::{AudioBuffer, BaseAudioContext};

use super::zufall;

pub struct Puffer {
    pub tasten: Vec<AudioBuffer>,
    pub leertasten: Vec<AudioBuffer>,
    pub maus: AudioBuffer,
    pub schritte: Vec<AudioBuffer>,
    pub rascheln: AudioBuffer,
    pub weiss: AudioBuffer,
    pub knistern: AudioBuffer,
    pub raum: AudioBuffer,
    /// Ein Regentropfen am Glas.
    pub tropfen: AudioBuffer,
    pub braun: AudioBuffer,
    /// Ein langsames, klumpiges 0..1-Signal, um die Lautstärke anderer Klänge wackeln zu lassen.
    pub gluckern: AudioBuffer,
}

impl Puffer {
    pub fn neu(ctx: &BaseAudioContext) -> Option<Self> {
        Some(Self {
            tasten: (0..6)
                .map(|_| tastenklick(ctx, zufall(190.0, 300.0), zufall(0.7, 1.0), zufall(0.06, 0.09), 95.0, 0.12))
                .collect::<Option<_>>()?,
            leertasten: (0..2)
                .map(|_| tastenklick(ctx, zufall(105.0, 130.0), 0.55, zufall(0.09, 0.12), 55.0, 0.2))
                .collect::<Option<_>>()?,
            maus: tastenklick(ctx, 900.0, 1.0, 0.07, 400.0, 0.1)?,
            schritte: (0..3).map(|_| schritt(ctx)).collect::<Option<_>>()?,
            rascheln: rascheln(ctx)?,
            weiss: probe(ctx, 5.0, None, |_| zufall(-1.0, 1.0))?,
            knistern: knistern(ctx)?,
            raum: raum(ctx)?,
            tropfen: probe(ctx, 0.06, Some(0.9), |t| {
                ((2.0 * PI * 2400.0 * t * (1.0 - t * 5.0)).sin() * 0.6 + zufall(-1.0, 1.0) * 0.4) * (-t * 110.0).exp()
            })?,
            braun: schleifbar(ctx, 6.0, braunes_rauschen())?,
            gluckern: schleifbar(ctx, 4.0, klumpig(ctx.sample_rate() as f64, 0.03, 0.11))?,
        })
    }
}

/// Eine Probe aus einer Funktion der Zeit, auf `spitze` normiert (falls angegeben).
fn probe(ctx: &BaseAudioContext, sekunden: f64, spitze: Option<f32>, mut f: impl FnMut(f64) -> f64) -> Option<AudioBuffer> {
    let sr = ctx.sample_rate();
    let n = (sr as f64 * sekunden).ceil() as u32;
    let b = ctx.create_buffer(1, n, sr).ok()?;
    let mut d: Vec<f32> = (0..n).map(|i| f(i as f64 / sr as f64) as f32).collect();
    if let Some(p) = spitze {
        let max = d.iter().fold(0f32, |m, v| m.max(v.abs()));
        if max > 0.0 {
            d.iter_mut().for_each(|v| *v *= p / max);
        }
    }
    b.copy_to_channel(&d, 0).ok()?;
    Some(b)
}

/// Ein Puffer, dessen Ende weich in den Anfang übergeht, damit er ohne Knacken in Schleife läuft.
fn schleifbar(ctx: &BaseAudioContext, sekunden: f64, mut naechstes: impl FnMut() -> f64) -> Option<AudioBuffer> {
    let sr = ctx.sample_rate();
    let n = (sr as f64 * sekunden).ceil() as usize;
    let blende = (sr as f64 * 0.25) as usize;
    let roh: Vec<f64> = (0..n + blende).map(|_| naechstes()).collect();
    let mut d: Vec<f32> = roh[..n].iter().map(|&v| v as f32).collect();
    for i in 0..blende {
        let k = i as f64 / blende as f64;
        d[i] = (roh[i] * k.sqrt() + roh[n + i] * (1.0 - k).sqrt()) as f32;
    }
    let b = ctx.create_buffer(1, n as u32, sr).ok()?;
    b.copy_to_channel(&d, 0).ok()?;
    Some(b)
}

fn braunes_rauschen() -> impl FnMut() -> f64 {
    let mut letzt = 0.0;
    move || {
        letzt = (letzt + 0.02 * zufall(-1.0, 1.0)) / 1.02;
        letzt * 3.5
    }
}

/// Wandert zwischen zufälligen Pegeln und hält jeden `min`–`max` Sekunden.
fn klumpig(sr: f64, min: f64, max: f64) -> impl FnMut() -> f64 {
    let (mut pegel, mut ziel, mut halten) = (0.0, 0.0, 0i64);
    move || {
        halten -= 1;
        if halten <= 0 {
            ziel = zufall(0.0, 1.0).powi(2);
            halten = (zufall(min, max) * sr) as i64;
        }
        pegel += (ziel - pegel) * (100.0 / sr);
        pegel
    }
}

/// Eine Taste schlägt auf: ein heller Tick über einem kurzen, hölzernen Klopfen, dann ein leiserer Tick beim Zurückfedern.
fn tastenklick(ctx: &BaseAudioContext, koerper: f64, hell: f64, loslassen: f64, abklingen: f64, laenge: f64) -> Option<AudioBuffer> {
    let (mut vorher, mut zischen, mut tief) = (0.0, 0.0, 0.0);
    probe(ctx, laenge, Some(0.9), |t| {
        let w = zufall(-1.0, 1.0);
        zischen += (w - vorher - zischen) * 0.5;
        vorher = w;
        tief += (w - tief) * 0.15;
        let mut v = zischen * (-t * 700.0).exp() * hell + ((2.0 * PI * koerper * t).sin() * 0.5 + tief) * (-t * abklingen).exp();
        let r = t - loslassen;
        if r > 0.0 {
            v += zischen * (-r * 900.0).exp() * hell * 0.45;
        }
        v
    })
}

/// Ein weicher Schuh auf Teppich: ein gedämpfter Tritt und ein kleines Schlurfen.
fn schritt(ctx: &BaseAudioContext) -> Option<AudioBuffer> {
    let (mut tief, mut vorher) = (0.0, 0.0);
    let schlurf = zufall(0.025, 0.045);
    probe(ctx, 0.25, Some(0.9), |t| {
        let w = zufall(-1.0, 1.0);
        tief += (w - tief) * 0.03;
        let anschlag = (t / 0.004).min(1.0);
        let mut v = (tief * 3.0 + (2.0 * PI * 62.0 * t).sin() * 0.5) * anschlag * (-t * 30.0).exp();
        let s = t - schlurf;
        if s > 0.0 {
            v += (w - vorher) * 0.06 * (-s * 50.0).exp();
        }
        vorher = w;
        v
    })
}

/// Papier wird geblättert: knisterndes Mittenrauschen in unregelmäßigen Schüben.
fn rascheln(ctx: &BaseAudioContext) -> Option<AudioBuffer> {
    let laenge = 0.8;
    let sr = ctx.sample_rate() as f64;
    let (mut a, mut b, mut amp, mut ziel, mut halten) = (0.0, 0.0, 0.0, 0.0, 0i64);
    probe(ctx, laenge, Some(0.8), |t| {
        let w = zufall(-1.0, 1.0);
        a += (w - a) * 0.5;
        b += (w - b) * 0.05;
        halten -= 1;
        if halten <= 0 {
            ziel = zufall(0.0, 1.0).powi(3);
            halten = (zufall(0.008, 0.03) * sr) as i64;
        }
        amp += (ziel - amp) * 0.02;
        (a - b) * amp * (PI * t / laenge).sin()
    })
}

/// Knackser jeder Größe, ein paar Dutzend pro Sekunde, über leisem Rauschen (die Nadel in der Rille).
fn knistern(ctx: &BaseAudioContext) -> Option<AudioBuffer> {
    let sr = ctx.sample_rate();
    let n = (sr * 5.0) as usize;
    let mut d: Vec<f32> = (0..n).map(|_| (zufall(-1.0, 1.0) * 0.04) as f32).collect();
    for _ in 0..5 * 28 {
        let bei = (zufall(0.0, 1.0) * (n - 200) as f64) as usize;
        let amp = zufall(0.0, 1.0).powi(3) * if zufall(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        let breite = 2.0 + (zufall(0.0, 30.0)).floor();
        for i in 0..(breite as usize * 4) {
            if let Some(v) = d.get_mut(bei + i) {
                *v += (amp * (-(i as f64) / breite).exp() * if i % 2 == 1 { -0.6 } else { 1.0 }) as f32;
            }
        }
    }
    let b = ctx.create_buffer(1, n as u32, sr).ok()?;
    b.copy_to_channel(&d, 0).ok()?;
    Some(b)
}

/// Ein kleiner, dunkler Raum (Impulsantwort für den Hall).
fn raum(ctx: &BaseAudioContext) -> Option<AudioBuffer> {
    let sr = ctx.sample_rate();
    let n = (sr * 1.6) as u32;
    let b = ctx.create_buffer(2, n, sr).ok()?;
    for kanal in 0..2 {
        let mut tief = 0.0;
        let d: Vec<f32> = (0..n)
            .map(|i| {
                tief += (zufall(-1.0, 1.0) - tief) * 0.25;
                (tief * (-(i as f64) / sr as f64 * 3.4).exp()) as f32
            })
            .collect();
        b.copy_to_channel(&d, kanal).ok()?;
    }
    Some(b)
}
