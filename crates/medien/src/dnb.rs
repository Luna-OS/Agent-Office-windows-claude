//! Der DJ auf dem Dach: ein endloses Drum-and-Bass-Set, Stück für Stück zu je 96 Takten (Intro, Aufbau,
//! Drop, Breakdown, Aufbau, zweiter Drop). Tonart, Akkorde, Groove und Bass jedes Stücks folgen aus
//! seiner Nummer.
//!
//! Alles folgt aus der Uhr des Office ([`dj_zeit`]): alle auf dem Dach hören denselben Takt, und die
//! Lichter blitzen auf denselben Kicks und Snares ([`dj_bild`]), auch bei denen, die die Musik aus haben.
//! Der Browser spielt das in `client::klang::dj`; die Welt nimmt `dj_bild` für die Lichter.

use crate::tonfolge::mulberry32;

pub const DJ_BPM: f64 = 172.0;
/// Eine Sechzehntel, ein Schlag, ein Takt in Sekunden.
pub const SECHZEHNTEL: f64 = 60.0 / DJ_BPM / 4.0;
pub const SCHLAG: f64 = SECHZEHNTEL * 4.0;
pub const TAKT: f64 = SECHZEHNTEL * 16.0;
const STUECK_TAKTE: i64 = 96;
/// Hier begann das Set (Unix-ms, 1. 1. 2026 UTC); hält die Zahlen klein.
const EPOCHE_MS: f64 = 1_767_225_600_000.0;

/// Wie weit das Set ist (Sekunden) zu `office_ms` auf der Uhr des Office.
pub fn dj_zeit(office_ms: f64) -> f64 {
    (office_ms - EPOCHE_MS) / 1000.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Teil {
    Intro,
    Aufbau,
    Drop,
    Breakdown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Abschnitt {
    pub teil: Teil,
    /// Der Takt in diesem Teil, und wie viele er hat.
    pub takt: i64,
    pub takte: i64,
    /// Nach dem ersten Drop: Breakdown, zweiter Aufbau, zweiter Drop.
    pub zweiter: bool,
}

const ABSCHNITTE: [(Teil, i64); 6] =
    [(Teil::Intro, 16), (Teil::Aufbau, 8), (Teil::Drop, 32), (Teil::Breakdown, 16), (Teil::Aufbau, 8), (Teil::Drop, 16)];

fn abschnitt(takt_im_stueck: i64) -> Abschnitt {
    let (mut start, mut zweiter) = (0, false);
    for (teil, takte) in ABSCHNITTE {
        if takt_im_stueck < start + takte {
            return Abschnitt { teil, takt: takt_im_stueck - start, takte, zweiter };
        }
        if teil == Teil::Drop {
            zweiter = true;
        }
        start += takte;
    }
    Abschnitt { teil: Teil::Drop, takt: 0, takte: 16, zweiter: true }
}

/// Ein Takt aus Sechzehnteln: `x` voll, `o` leise (Ghost Note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Groove {
    pub kick: &'static str,
    pub snare: &'static str,
}

const GROOVES: [Groove; 5] = [
    Groove { kick: "x.........x.....", snare: "....x..o....x..o" },
    Groove { kick: "x.........x..x..", snare: "....x.......x.o." },
    Groove { kick: "x......x..x.....", snare: ".o..x..o....x..." },
    Groove { kick: "x.x.......x.....", snare: "....x....o..x..o" },
    Groove { kick: "x.........xx....", snare: "....x..o....x..." },
];

/// Akkorde als Stufen der Moll-Tonleiter, einer alle zwei Takte.
const FOLGEN: [[i32; 4]; 5] = [[0, 5, 2, 6], [0, 6, 5, 6], [0, 3, 5, 4], [5, 3, 0, 6], [0, 5, 3, 4]];
const MOLL: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const ARPEGGIEN: [[i32; 16]; 3] = [
    [0, 1, 2, 3, 2, 1, 0, 1, 2, 3, 4, 3, 2, 1, 2, 3],
    [0, 2, 1, 3, 2, 4, 3, 1, 0, 2, 1, 3, 2, 4, 3, 5],
    [0, 0, 2, 0, 3, 0, 2, 4, 0, 0, 2, 0, 3, 2, 4, 3],
];

#[derive(Debug, Clone, PartialEq)]
pub struct DjStueck {
    /// Tiefster Basston (MIDI, um F1).
    pub grundton: i32,
    pub folge: [i32; 4],
    pub groove: Groove,
    /// Lange, knurrende Noten (Reese) oder kurze, rollende.
    pub reese: bool,
    /// Wie der Bassfilter sich durch den Takt bewegt, je Achtel: `h` auf, `l` zu.
    pub wah: String,
    pub arpeggio: [i32; 16],
    /// Ein Air-Horn, wenn der erste Drop kommt.
    pub horn: bool,
    /// Farbe der Lichter für dieses Stück, 0..1 um den Farbkreis.
    pub farbton: f64,
}

/// Das Stück Nummer `n` (gleicher Zufall wie im Original).
pub fn stueck(n: i64) -> DjStueck {
    let mut r = mulberry32((n as i32).wrapping_mul(7919).wrapping_add(13) as u32);
    let mut waehle = |len: usize| (r() * len as f64) as usize;
    let grundton = [28, 29, 30, 31, 33][waehle(5)];
    let folge = FOLGEN[waehle(5)];
    let groove = GROOVES[waehle(5)];
    let reese = r() < 0.6;
    let wah = (0..8).map(|i| if i == 0 || r() < 0.45 { 'h' } else { 'l' }).collect();
    let arpeggio = ARPEGGIEN[(r() * 3.0) as usize];
    let horn = r() < 0.65;
    let farbton = r();
    DjStueck { grundton, folge, groove, reese, wah, arpeggio, horn, farbton }
}

/// Ein Ton der Moll-Tonleiter des Stücks: `stufe` Schritte über `grundton` (MIDI).
pub fn tonleiter(grundton: i32, stufe: i32) -> i32 {
    grundton + MOLL[stufe.rem_euclid(7) as usize] + 12 * stufe.div_euclid(7)
}

/// Was auf einer Sechzehntel des Sets passiert.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub stueck: DjStueck,
    pub abschnitt: Abschnitt,
    /// 0..15 im Takt.
    pub s: i64,
    /// Stufe des Akkords.
    pub akkord: i32,
    /// Wie hart jede Trommel schlägt (0 = gar nicht).
    pub kick: f64,
    pub snare: f64,
    pub hat: f64,
    pub offene_hat: bool,
    pub shaker: f64,
}

pub fn plan(k: i64) -> Plan {
    let takt = k.div_euclid(16);
    let s = k.rem_euclid(16);
    let n = takt.div_euclid(STUECK_TAKTE);
    let b = takt - n * STUECK_TAKTE;
    let stueck = stueck(n);
    let ab = abschnitt(b);
    let akkord = stueck.folge[(b / 2) as usize % 4];
    let schlag = |p: &str| match p.as_bytes()[s as usize] {
        b'x' => 1.0,
        b'o' => 0.35,
        _ => 0.0,
    };
    let g = stueck.groove;
    let mut p = Plan { stueck: stueck.clone(), abschnitt: ab, s, akkord, kick: 0.0, snare: 0.0, hat: 0.0, offene_hat: false, shaker: 0.0 };
    let sb = ab.takt;
    match ab.teil {
        Teil::Intro => {
            // Erst nur Hats, dann der ganze Break, gedämpft (der Filter geht mit der Zeit auf).
            p.hat = if s % 2 == 0 { if s % 4 == 2 { 0.8 } else { 0.5 } } else { 0.0 };
            if sb >= 4 {
                p.kick = schlag(g.kick);
                p.snare = schlag(g.snare);
            }
        }
        Teil::Aufbau => {
            // Four-to-the-floor unter einem Snare-Wirbel, der alle paar Takte schneller wird, dann ein Schlag Stille.
            let letzt = sb == ab.takte - 1 && s >= 12;
            if !letzt {
                if sb < 6 && s % 4 == 0 {
                    p.kick = 0.8;
                }
                let alle = if sb < 3 { 4 } else if sb < 5 { 2 } else { 1 };
                if s % alle == 0 {
                    p.snare = 0.35 + 0.65 * ((sb * 16 + s) as f64 / (ab.takte * 16) as f64);
                }
                p.hat = if s % 2 == 0 { 0.4 } else { 0.0 };
            }
        }
        Teil::Drop => {
            let fill = sb % 8 == 7;
            p.kick = if fill && s >= 10 { 0.0 } else { schlag(g.kick) };
            p.snare = if fill && s >= 12 { 0.55 + (s - 12) as f64 * 0.15 } else { schlag(g.snare) };
            p.hat = if s % 2 == 0 { if s % 4 == 2 { 1.0 } else { 0.6 } } else { 0.0 };
            p.offene_hat = s == 14 && sb % 2 == 1;
            p.shaker = if s % 2 == 1 { 0.7 } else { 0.4 };
        }
        Teil::Breakdown => {
            // Pads und Arpeggio allein, dann kriecht ein Halftime-Beat zurück.
            if sb >= 8 {
                p.kick = if s == 0 { 0.8 } else { 0.0 };
                p.snare = if s == 8 { 0.8 } else { 0.0 };
                p.hat = if s % 4 == 2 { 0.6 } else { 0.0 };
            }
        }
    }
    p
}

/// Wonach die Lichter gehen: wo das Set ist und was gerade geschlagen hat.
#[derive(Debug, Clone, PartialEq)]
pub struct DjBild {
    /// Schläge seit Beginn des Sets.
    pub schlaege: f64,
    /// 1 auf jedem Schlag, bis zum nächsten auf 0 fallend.
    pub schlag: f64,
    /// 1, wenn eine Kick oder Snare landet, schnell abfallend.
    pub kick: f64,
    pub snare: f64,
    /// Wie heftig es ist: niedrig im Breakdown, 1 im Drop.
    pub energie: f64,
    pub teil: Teil,
    /// 0 → 1 durch einen Aufbau.
    pub anstieg: f64,
    /// Sekunden seit der Drop gelandet ist (unendlich außerhalb eines Drops).
    pub seit_drop: f64,
    pub stueck: i64,
    pub farbton: f64,
}

/// Wo das Set zur Zeit `at` (siehe [`dj_zeit`]) ist, und was gerade geschlagen hat.
pub fn dj_bild(at: f64) -> DjBild {
    let k = (at / SECHZEHNTEL).floor() as i64;
    let p = plan(k);
    let Abschnitt { teil, takt, takte, zweiter } = p.abschnitt;
    let schlaege = at / SCHLAG;
    let schlag = (1.0 - schlaege.fract().rem_euclid(1.0)).powi(3);
    let (mut kick, mut snare) = (0.0, 0.0);
    // Die letzte Kick und Snare innerhalb eines Schlags, abklingend seit sie kamen.
    for j in 0..4 {
        if kick != 0.0 && snare != 0.0 {
            break;
        }
        let q = if j == 0 { p.clone() } else { plan(k - j) };
        let seit = at - (k - j) as f64 * SECHZEHNTEL;
        if kick == 0.0 && q.kick >= 0.5 {
            kick = (-seit * 9.0).exp();
        }
        if snare == 0.0 && q.snare >= 0.5 {
            snare = (-seit * 11.0).exp();
        }
    }
    let durch = (takt as f64 + k.rem_euclid(16) as f64 / 16.0) / takte as f64;
    let energie = match teil {
        Teil::Drop => 1.0,
        Teil::Aufbau => 0.45 + 0.5 * durch,
        Teil::Intro => 0.3 + 0.2 * durch,
        Teil::Breakdown if takt >= 8 => 0.35,
        Teil::Breakdown => 0.18,
    };
    DjBild {
        schlaege,
        schlag,
        kick,
        snare,
        energie: if zweiter && teil == Teil::Drop { 1.0 } else { energie },
        teil,
        anstieg: if teil == Teil::Aufbau { durch } else { 0.0 },
        seit_drop: if teil == Teil::Drop {
            (takt * 16 + k.rem_euclid(16)) as f64 * SECHZEHNTEL + (at - k as f64 * SECHZEHNTEL)
        } else {
            f64::INFINITY
        },
        stueck: k.div_euclid(16).div_euclid(STUECK_TAKTE),
        farbton: p.stueck.farbton,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_baut_sich_auf() {
        // Takt 0: Intro, nur Hats.
        let p = plan(0);
        assert_eq!(p.abschnitt.teil, Teil::Intro);
        assert_eq!(p.kick, 0.0);
        assert!(p.hat > 0.0);
        // Takt 24: erster Drop.
        let d = plan(24 * 16);
        assert_eq!(d.abschnitt.teil, Teil::Drop);
        assert!(!d.abschnitt.zweiter);
        assert_eq!(plan(56 * 16).abschnitt.teil, Teil::Breakdown);
        assert!(plan(88 * 16).abschnitt.zweiter);
        // Nächstes Stück nach 96 Takten.
        assert_eq!(plan(96 * 16).abschnitt.teil, Teil::Intro);
    }

    #[test]
    fn tonleiter_und_stueck() {
        assert_eq!(tonleiter(28, 0), 28);
        assert_eq!(tonleiter(28, 7), 40);
        assert_eq!(tonleiter(28, -1), 26);
        let s = stueck(3);
        assert_eq!(s, stueck(3));
        assert_eq!(s.wah.len(), 8);
        assert!(s.wah.starts_with('h'));
    }

    /// Werte aus dem Original (`src/client/dnb.ts`, mit Node gerechnet).
    #[test]
    fn wie_im_original() {
        let erwartet = [
            (0.0, 1.0, 0.0, 0.3831601137548761, 0.3, Teil::Intro, 0.0, 0, 0.16639575641602278),
            (33.5, 0.9032962962963016, 0.9006386440656572, 0.0, 1.0, Teil::Drop, 0.0, 0, 0.16639575641602278),
            (200.1, 0.05487199999999803, 0.0, 0.09263671112675777, 1.0, Teil::Drop, 0.0, 1, 0.052869664039462805),
            (1234.56, 0.7991787520008741, 0.797680999362157, 0.7586011985598623, 0.746875, Teil::Aufbau, 0.59375, 9, 0.5090907937847078),
            (5000.0, 0.29629629629791315, 0.0, 0.27829606013791186, 1.0, Teil::Drop, 0.0, 37, 0.5976990393828601),
            (98765.4, 0.14060800001510979, 0.04610913529734715, 0.1585222265371637, 1.0, Teil::Drop, 0.0, 737, 0.784559135325253),
        ];
        let nah = |a: f64, b: f64| (a - b).abs() < 1e-9;
        for (at, schlag, kick, snare, energie, teil, anstieg, stueck, farbton) in erwartet {
            let b = dj_bild(at);
            assert!(nah(b.schlag, schlag) && nah(b.kick, kick) && nah(b.snare, snare), "{at}: {b:?}");
            assert!(nah(b.energie, energie) && nah(b.anstieg, anstieg) && nah(b.farbton, farbton), "{at}: {b:?}");
            assert_eq!((b.teil, b.stueck), (teil, stueck), "{at}");
        }
        for (n, grundton, folge, reese, wah, horn) in [
            (0, 30, [0, 6, 5, 6], true, "hhhhhlll", true),
            (1, 30, [0, 3, 5, 4], false, "hhllllhl", true),
            (7, 28, [0, 5, 2, 6], false, "hhllhlll", false),
            (42, 30, [5, 3, 0, 6], false, "hhlhlhhl", true),
        ] {
            let s = stueck(n);
            assert_eq!((s.grundton, s.folge, s.reese, s.wah.as_str(), s.horn), (grundton, folge, reese, wah, horn), "Stück {n}");
        }
        assert_eq!(EPOCHE_MS, 1_767_225_600_000.0);
    }

    #[test]
    fn bild_ist_stetig() {
        let b = dj_bild(dj_zeit(EPOCHE_MS + 5000.0));
        assert!((b.schlaege - 5.0 / SCHLAG).abs() < 1e-9);
        assert!((0.0..=1.0).contains(&b.schlag));
        assert!(b.seit_drop.is_infinite());
    }
}
