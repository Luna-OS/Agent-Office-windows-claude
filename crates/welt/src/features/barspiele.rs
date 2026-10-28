//! Darts und Axtwurf auf der Dachterrasse: E an der Scheibe (oder der Bahn) stellt einen an die
//! Linie, die Maus zielt, Leertaste halten holt aus (die Kraftanzeige pendelt), Loslassen wirft.
//! Im Grünen fliegt ein Dart gerade und eine Axt bleibt stecken. Eine Runde sind 3 Darts oder
//! 5 Äxte; die Würfe der anderen sieht man auch (der Server schickt sie weiter).

use std::cell::RefCell;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::praesenz::{Barspiel, Client as P};
use protokoll::spielzeug::Client as S;

use crate::api::{Klang, WeltAktion, klang, melden};
use crate::barspiele::{anzeige, gruen, landung, runde, wertung, wurfplatz, ziel};
use crate::bau::farbe;
use crate::register::{Aktivitaet, Phase, Register};
use crate::welt::Kontext;

/// So lange braucht die Kraftanzeige einmal hinauf (Sekunden).
const PERIODE: f32 = 0.9;

/// Ein Wurf, der in der Scheibe steckt (oder darunter liegt).
struct Steckt {
    wer: String,
    knoten: SceneNode3d,
}

#[derive(Default)]
struct Wuerfe {
    alle: Vec<Steckt>,
}

impl Wuerfe {
    /// Einen Wurf in die Welt setzen; der erste einer Runde räumt die alten dieses Werfers weg.
    fn setzen(
        &mut self,
        eltern: &mut SceneNode3d,
        wer: &str,
        s: Barspiel,
        (u, v, steckt, n): (f32, f32, bool, u32),
    ) {
        if n <= 1 {
            self.alle.retain_mut(|w| {
                let bleibt = w.wer != wer;
                if !bleibt {
                    w.knoten.detach();
                }
                bleibt
            });
        }
        let ([zx, zy, zz], rechts, raus) = ziel(s);
        let (x, z) = (
            zx + rechts.0 * u + raus.0 * 0.05,
            zz + rechts.1 * u + raus.1 * 0.05,
        );
        let y = if steckt { zy + v } else { 0.05 };
        let mut k = eltern.add_group();
        k.set_position(Vec3::new(x, y, z + if steckt { 0.0 } else { raus.1 * 0.4 }));
        let blick = raus.0.atan2(raus.1);
        k.set_rotation(Quat::from_axis_angle(Vec3::Y, blick));
        match s {
            Barspiel::Darts => {
                let mut schaft = k.add_cylinder(0.006, 0.14);
                schaft.set_rotation(Quat::from_axis_angle(Vec3::X, std::f32::consts::FRAC_PI_2));
                schaft.set_position(Vec3::new(0.0, 0.0, 0.07));
                schaft.set_color(farbe("#adb5bd"));
                let mut feder = k.add_cone(0.02, 0.04);
                feder.set_rotation(Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2));
                feder.set_position(Vec3::new(0.0, 0.0, 0.15));
                feder.set_color(farbe("#ef476f"));
            }
            Barspiel::Axt => {
                let mut stiel = k.add_cube(0.04, 0.04, 0.5);
                stiel.set_position(Vec3::new(0.0, 0.0, 0.25));
                stiel.set_color(farbe("#8a5a3b"));
                let mut klinge = k.add_cube(0.02, 0.14, 0.12);
                klinge.set_position(Vec3::new(0.0, 0.0, 0.02));
                klinge.set_color(farbe("#ced4da"));
                klinge.set_metallic(0.9);
            }
        }
        self.alle.push(Steckt {
            wer: wer.to_string(),
            knoten: k,
        });
    }
}

struct Werfen {
    spiel: Barspiel,
    n: u32,
    ausholen: Option<f32>,
    punkte: u32,
    wuerfe: Rc<RefCell<Wuerfe>>,
}

/// Wo der Blick die Ebene des Ziels trifft, als (u, v).
fn gezielt(kx: &Kontext, s: Barspiel) -> (f32, f32) {
    let ([zx, zy, zz], rechts, raus) = ziel(s);
    let a = kx.spieler.auge();
    let r = kx.spieler.blickrichtung();
    // Ebene durch die Zielmitte mit Normale `raus`.
    let nenner = r[0] * raus.0 + r[2] * raus.1;
    if nenner.abs() < 1e-4 {
        return (0.0, 0.0);
    }
    let t = ((zx - a[0]) * raus.0 + (zz - a[2]) * raus.1) / nenner;
    let p = [a[0] + r[0] * t, a[1] + r[1] * t, a[2] + r[2] * t];
    ((p[0] - zx) * rechts.0 + (p[2] - zz) * rechts.1, p[1] - zy)
}

/// Ein kleines, wechselndes Zittern der Hand (-1..1).
fn zittern(t: f64) -> [f32; 2] {
    let s = (t * 1000.0).sin() * 43758.545;
    let a = (s - s.floor()) as f32 * 2.0 - 1.0;
    let s2 = (t * 1731.0).cos() * 24634.634;
    [a, (s2 - s2.floor()) as f32 * 2.0 - 1.0]
}

impl Aktivitaet<Kontext> for Werfen {
    fn name(&self) -> &'static str {
        "barspiel"
    }
    fn taste(&mut self, kx: &mut Kontext, code: &str, gedrueckt: bool) -> bool {
        match (code, gedrueckt) {
            ("Space", true) => {
                self.ausholen.get_or_insert(0.0);
                true
            }
            ("Space", false) => {
                let Some(t) = self.ausholen.take() else {
                    return true;
                };
                let kraft = anzeige(t, PERIODE);
                let (gu, gv) = gezielt(kx, self.spiel);
                let (u, v, steckt) = landung(self.spiel, gu, gv, kraft, zittern(kx.zeit));
                self.n += 1;
                kx.senden(ClientNachricht::Spielzeug(S::Barwurf {
                    spiel: self.spiel,
                    u,
                    v,
                    steckt,
                    n: self.n,
                }));
                let ich = kx.zustand.ich.clone().unwrap_or_default();
                self.wuerfe.borrow_mut().setzen(
                    &mut kx.etage,
                    &ich,
                    self.spiel,
                    (u, v, steckt, self.n),
                );
                let w = wertung(self.spiel, u, v, steckt);
                self.punkte += w.punkte;
                let ([x, y, z], _, _) = ziel(self.spiel);
                klang(match self.spiel {
                    Barspiel::Darts => Klang::Dart { pos: [x, y, z] },
                    Barspiel::Axt => Klang::Axt { pos: [x, y, z] },
                });
                let ende = self.n >= runde(self.spiel);
                melden(WeltAktion::Meldung {
                    text: if ende {
                        format!("{} – Runde vorbei: {} Punkte", w.text, self.punkte)
                    } else {
                        format!("{} ({} Punkte)", w.text, w.punkte)
                    },
                });
                if ende {
                    self.n = 0;
                    self.punkte = 0;
                }
                true
            }
            ("KeyE", true) | ("Escape", true) => {
                kx.alle_stoppen = true;
                true
            }
            _ => false,
        }
    }
    fn tick(&mut self, kx: &mut Kontext, dt: f32) -> bool {
        if let Some(t) = self.ausholen.as_mut() {
            *t += dt;
        }
        // An der Linie stehen bleiben, die Maus zielt frei.
        let (x, z, _) = wurfplatz(self.spiel);
        kx.spieler.koerper.pos = [x, 0.0, z];
        true
    }
    fn hinweis(&self, _kx: &Kontext) -> Option<String> {
        let n = runde(self.spiel);
        Some(match self.ausholen {
            Some(t) => {
                let k = anzeige(t, PERIODE);
                let (m, b) = gruen(self.spiel);
                let balken: String = (0..20)
                    .map(|i| {
                        if (i as f32 / 20.0) < k {
                            if (k - m).abs() <= b / 2.0 {
                                '█'
                            } else {
                                '▓'
                            }
                        } else {
                            '░'
                        }
                    })
                    .collect();
                format!("Loslassen zum Werfen {balken} {}%", (k * 100.0) as u32)
            }
            None => format!(
                "Maus: zielen · Leertaste halten: ausholen · Wurf {} von {n} · E: aufhören",
                self.n + 1
            ),
        })
    }
    fn stoppen(&mut self, kx: &mut Kontext) {
        kx.senden(ClientNachricht::Praesenz(P::Handlung {
            rauchen: None,
            golfen: None,
            getraenk: None,
            wirft: Some(None),
        }));
    }
}

fn beginnen(kx: &mut Kontext, spiel: Barspiel, wuerfe: Rc<RefCell<Wuerfe>>) {
    let (x, z, blick) = wurfplatz(spiel);
    kx.spieler.versetzen(x, 0.0, z, blick);
    kx.senden(ClientNachricht::Praesenz(P::Handlung {
        rauchen: None,
        golfen: None,
        getraenk: None,
        wirft: Some(Some(spiel)),
    }));
    kx.aktivitaet(Werfen {
        spiel,
        n: 0,
        ausholen: None,
        punkte: 0,
        wuerfe,
    });
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let wuerfe: Rc<RefCell<Wuerfe>> = Rc::default();
    let w = wuerfe.clone();
    reg.interaktion(
        "darts",
        |_, _| Some("E: Darts werfen (3 pro Runde)".into()),
        move |kx: &mut Kontext, _| beginnen(kx, Barspiel::Darts, w.clone()),
    );
    let w = wuerfe.clone();
    reg.interaktion(
        "axt",
        |_, _| Some("E: Äxte werfen (5 pro Runde)".into()),
        move |kx: &mut Kontext, _| beginnen(kx, Barspiel::Axt, w.clone()),
    );
    // Die Würfe der anderen.
    reg.tick(Phase::Welt, move |kx: &mut Kontext, _| {
        let mut w = wuerfe.borrow_mut();
        if kx.neu_gebaut {
            w.alle.clear();
        }
        for (wer, spiel, u, v, steckt, n) in std::mem::take(&mut kx.aenderungen.barwuerfe) {
            w.setzen(&mut kx.etage, &wer, spiel, (u, v, steckt, n));
            let ([x, y, z], _, _) = ziel(spiel);
            klang(match spiel {
                Barspiel::Darts => Klang::Dart { pos: [x, y, z] },
                Barspiel::Axt => Klang::Axt { pos: [x, y, z] },
            });
        }
    });
}
