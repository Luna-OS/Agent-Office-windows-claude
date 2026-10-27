//! Die eingebauten Jukebox-Stücke als Noten: Lo-Fi-Beats, die der Browser mit Web Audio spielt
//! (siehe `client::klang::musik`). Vier Takte Akkorde auf einem warmen E-Piano über geswingten Drums
//! und einem runden Bass, dazu eine kleine Melodie; in Runden zu 32 Takten (erst nur die Tasten, dann
//! der Beat, eine Melodie, ein Breakdown).
//!
//! Jede Note folgt allein aus dem Stück und der Stelle darin: wer zur selben Zeit startet, hört
//! genau denselben Takt – auf jedem Rechner (gleicher Zufall wie im Original, Mulberry32).

/// Ein Stück: Tempo, Swing, Akkorde, Rhythmen.
pub struct Tonart {
    pub bpm: f64,
    /// Wie spät jede zweite Sechzehntel kommt (Anteil einer Sechzehntel).
    pub swing: f64,
    /// Pro Takt: Grundton des Basses, dann die Töne der Tasten (MIDI).
    pub akkorde: &'static [&'static [u8]],
    /// Ein Takt aus Sechzehnteln: `x` voll, `o` leise.
    pub kick: &'static str,
    pub snare: &'static str,
    pub hat: &'static str,
    /// (Sechzehntel, Länge in Sechzehnteln, Halbtöne über dem Grundton).
    pub bass: &'static [(u8, u8, u8)],
    /// Wann die Tasten den Akkord anschlagen: (Sechzehntel, Länge, Stärke).
    pub anschlag: &'static [(u8, u8, f64)],
    /// Wählt die Melodie.
    pub saat: u32,
}

pub fn tonart(id: &str) -> &'static Tonart {
    match id {
        "coffee-break" => &KAFFEEPAUSE,
        "late-commit" => &SPAETER_COMMIT,
        "green-build" => &GRUENER_BUILD,
        _ => &REGEN,
    }
}

static REGEN: Tonart = Tonart {
    bpm: 74.0,
    swing: 0.16,
    akkorde: &[&[41, 57, 60, 64, 67], &[40, 55, 59, 62, 64], &[38, 53, 57, 60, 64], &[36, 55, 58, 62, 64]],
    kick: "x.........x.....",
    snare: "....x.......x...",
    hat: "x.o.x.o.x.o.x.oo",
    bass: &[(0, 7, 0), (10, 4, 0), (14, 2, 7)],
    anschlag: &[(0, 10, 1.0), (11, 5, 0.6)],
    saat: 7,
};
static KAFFEEPAUSE: Tonart = Tonart {
    bpm: 88.0,
    swing: 0.22,
    akkorde: &[&[38, 53, 57, 60, 64], &[43, 53, 59, 64, 69], &[36, 52, 55, 59, 62], &[45, 55, 61, 65]],
    kick: "x......x..x.....",
    snare: "....x.......x..o",
    hat: "x.oxx.o.x.oxx.o.",
    bass: &[(0, 5, 0), (6, 2, 7), (8, 5, 0), (14, 2, 12)],
    anschlag: &[(0, 6, 1.0), (6, 3, 0.65), (10, 6, 0.85)],
    saat: 21,
};
static SPAETER_COMMIT: Tonart = Tonart {
    bpm: 70.0,
    swing: 0.12,
    akkorde: &[&[45, 60, 64, 67, 71], &[41, 57, 59, 64], &[38, 53, 57, 60, 64], &[40, 56, 59, 62, 65]],
    kick: "x......ox.x.....",
    snare: "....x.......x...",
    hat: "x.o.x.o.x.o.x.o.",
    bass: &[(0, 6, 0), (8, 2, 0), (10, 5, 0)],
    anschlag: &[(0, 10, 1.0), (12, 4, 0.6)],
    saat: 3,
};
static GRUENER_BUILD: Tonart = Tonart {
    bpm: 94.0,
    swing: 0.1,
    akkorde: &[&[43, 59, 62, 66, 69], &[42, 57, 61, 64], &[40, 55, 59, 62, 66], &[45, 55, 61, 66]],
    kick: "x.....x...x.....",
    snare: "....x.......x...",
    hat: "x.xox.xox.xox.xo",
    bass: &[(0, 3, 0), (3, 2, 12), (6, 3, 0), (10, 3, 7), (14, 2, 0)],
    anschlag: &[(0, 3, 0.9), (4, 2, 0.6), (8, 3, 0.8), (11, 4, 0.7)],
    saat: 11,
};

/// Rhythmen für einen Takt Melodie: (Sechzehntel, Länge).
const ZELLEN: [&[(u8, u8)]; 5] = [
    &[(0, 4), (6, 2), (8, 6)],
    &[(2, 2), (4, 4), (10, 4)],
    &[(0, 6), (8, 2), (10, 2), (12, 4)],
    &[(4, 2), (6, 2), (8, 8)],
    &[(0, 3), (3, 3), (6, 6)],
];
/// Der letzte Takt jeder Phrasenhälfte ruht auf ein, zwei Tönen.
const ENDEN: [&[(u8, u8)]; 2] = [&[(0, 12)], &[(2, 2), (4, 10)]];

/// Mulberry32 wie im Original: dieselben Zahlen 0..1 für dieselbe Saat.
pub fn mulberry32(saat: u32) -> impl FnMut() -> f64 {
    let mut a = saat;
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4_294_967_296.0
    }
}

/// Dieselbe Zahl 0..1 für dieselben Zahlen, auf jedem Rechner.
pub fn streuung(a: u32, b: u32) -> f64 {
    let mut h = (a ^ 0x9e37_79b9).wrapping_mul(0x85eb_ca6b) ^ b.wrapping_add(0x632b_e5ab).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h as f64 / 4_294_967_296.0
}

pub fn midi_zu_hz(m: f64) -> f64 {
    440.0 * 2f64.powf((m - 69.0) / 12.0)
}

/// Acht Takte Melodie: (Sechzehntel, Länge, MIDI).
pub fn melodie(t: &Tonart) -> Vec<Vec<(u8, u8, u8)>> {
    let mut zufall = mulberry32(t.saat);
    let mut vorher: i32 = 74;
    let mut takte = Vec::with_capacity(8);
    for b in 0..8 {
        let akkord = t.akkorde[b % t.akkorde.len()];
        let toene: Vec<i32> =
            (67..=83).filter(|m: &i32| akkord.iter().any(|&c| (c as i32 - m) % 12 == 0)).collect();
        let zellen: &[&[(u8, u8)]] = if b % 4 == 3 { &ENDEN } else { &ZELLEN };
        let zelle = zellen[(zufall() * zellen.len() as f64) as usize];
        let mut takt = Vec::new();
        for &(stelle, laenge) in zelle {
            // Meist ein, zwei Schritte vom letzten Ton, ab und zu ein Sprung (ein Zufall je Ton, wie im Original).
            let nah: Vec<i32> = toene
                .iter()
                .copied()
                .filter(|&m| {
                    let weite = if zufall() < 0.2 { 7 } else { 4 };
                    (m - vorher).abs() <= weite && m != vorher
                })
                .collect();
            let topf = if nah.is_empty() { &toene } else { &nah };
            vorher = topf[(zufall() * topf.len() as f64) as usize];
            takt.push((stelle, laenge, vorher as u8));
        }
        takte.push(takt);
    }
    takte
}

/// Was in einem Takt einer 32-Takt-Runde mitspielt.
pub fn abschnitt(takt: u64) -> (bool, bool, bool) {
    let b = takt % 32;
    let drums = b >= 4 && !(24..28).contains(&b);
    let bass = b >= 2;
    let melodie = (8..24).contains(&b) || b >= 28;
    (drums, bass, melodie)
}

/// Eine Note, die zu einer Sechzehntel gespielt wird. Längen in Sekunden.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    Kick { staerke: f64 },
    Snare { staerke: f64 },
    Hat { staerke: f64 },
    Bass { midi: u8, laenge: f64 },
    /// Ein E-Piano-Ton; `versatz` streut den Anschlag ein paar Millisekunden.
    Taste { midi: u8, laenge: f64, staerke: f64, versatz: f64 },
    Melodie { midi: u8, laenge: f64 },
}

/// Ein Stück mit vorberechneter Melodie.
pub struct Partitur {
    pub tonart: &'static Tonart,
    melodie: Vec<Vec<(u8, u8, u8)>>,
    /// Dauer einer Sechzehntel in Sekunden.
    pub sechzehntel: f64,
}

impl Partitur {
    pub fn neu(stueck_id: &str) -> Self {
        let tonart = tonart(stueck_id);
        Self { tonart, melodie: melodie(tonart), sechzehntel: 60.0 / tonart.bpm / 4.0 }
    }

    /// Wann Sechzehntel `k` erklingt (Sekunden ab Stückbeginn, mit Swing).
    pub fn zeit(&self, k: u64) -> f64 {
        (k as f64 + if k % 2 == 1 { self.tonart.swing } else { 0.0 }) * self.sechzehntel
    }

    /// 1 auf jedem Schlag, bis zum nächsten auf 0 fallend (für die Lichter der Jukebox).
    pub fn puls(&self, sekunden: f64) -> f64 {
        let schlaege = sekunden / (self.sechzehntel * 4.0);
        let p = (1.0 - schlaege.fract()).powi(2);
        if abschnitt((schlaege / 4.0) as u64).0 { p } else { p * 0.35 }
    }

    /// Alle Noten der Sechzehntel `k`.
    pub fn noten(&self, k: u64) -> Vec<Note> {
        let t = self.tonart;
        let takt = k / 16;
        let s = (k % 16) as usize;
        let (drums, bass, melodie) = abschnitt(takt);
        let akkord = t.akkorde[takt as usize % t.akkorde.len()];
        let laenge = |n: u8| n as f64 * self.sechzehntel;
        let mut aus = Vec::new();
        if drums {
            let schlag = |muster: &str| match muster.as_bytes()[s] {
                b'x' => 1.0,
                b'o' => 0.55,
                _ => 0.0,
            };
            if schlag(t.kick) > 0.0 {
                aus.push(Note::Kick { staerke: schlag(t.kick) });
            }
            if schlag(t.snare) > 0.0 {
                aus.push(Note::Snare { staerke: schlag(t.snare) });
            }
            if schlag(t.hat) > 0.0 {
                aus.push(Note::Hat { staerke: schlag(t.hat) * (0.7 + 0.3 * streuung(k as u32, t.saat)) });
            }
        }
        if bass {
            for &(bei, n, hoch) in t.bass {
                if bei as usize == s {
                    aus.push(Note::Bass { midi: akkord[0] + hoch, laenge: laenge(n) });
                }
            }
        }
        for &(bei, n, staerke) in t.anschlag {
            if bei as usize == s {
                for &m in &akkord[1..] {
                    let versatz = streuung(k as u32, m as u32) * 0.012;
                    aus.push(Note::Taste { midi: m, laenge: laenge(n), staerke, versatz });
                }
            }
        }
        if melodie {
            for &(bei, n, m) in &self.melodie[takt as usize % 8] {
                if bei as usize == s {
                    aus.push(Note::Melodie { midi: m, laenge: laenge(n) });
                }
            }
        }
        aus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zufall_wie_im_original() {
        let mut r = mulberry32(7);
        assert!((r() - 0.011704753153026104).abs() < 1e-15);
        assert!((r() - 0.06195825757458806).abs() < 1e-15);
        assert!((r() - 0.97690763277933).abs() < 1e-15);
        assert!((streuung(5, 7) - 0.07245543226599693).abs() < 1e-15);
        assert!((streuung(100, 3) - 0.088976816739887).abs() < 1e-15);
    }

    #[test]
    fn runde_baut_sich_auf() {
        let p = Partitur::neu("rainy-window");
        // Takt 0: nur die Tasten.
        assert!(p.noten(0).iter().all(|n| matches!(n, Note::Taste { .. })));
        assert!(!p.noten(0).is_empty());
        // Takt 4: Kick auf der Eins.
        assert!(p.noten(4 * 16).iter().any(|n| matches!(n, Note::Kick { .. })));
        // Breakdown in Takt 24: keine Drums.
        assert!(!p.noten(24 * 16).iter().any(|n| matches!(n, Note::Kick { .. })));
    }

    #[test]
    fn melodie_bleibt_in_den_akkordtoenen() {
        for id in ["rainy-window", "coffee-break", "late-commit", "green-build"] {
            let t = tonart(id);
            let m = melodie(t);
            assert_eq!(m.len(), 8);
            for (b, takt) in m.iter().enumerate() {
                let akkord = t.akkorde[b % 4];
                for &(_, _, ton) in takt {
                    assert!((67..=83).contains(&ton));
                    assert!(akkord.iter().any(|&c| (c as i32 - ton as i32) % 12 == 0), "{id} Takt {b}");
                }
            }
        }
    }

    #[test]
    fn swing_und_puls() {
        let p = Partitur::neu("coffee-break");
        assert!(p.zeit(1) > p.sechzehntel);
        assert_eq!(p.zeit(2), 2.0 * p.sechzehntel);
        assert!((p.puls(0.0) - 0.35).abs() < 1e-9);
        assert_eq!(midi_zu_hz(69.0), 440.0);
    }
}
