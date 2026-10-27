//! Golf vom Balkon: E am Abschlag nimmt den Schläger, die Maus zielt, Leertaste halten holt aus,
//! Loslassen schlägt. Jeder Abschlag (auch die der anderen) fliegt für alle gleich über die Straße
//! zum Loch auf der anderen Seite. Teleskop: E auf der Empore schaut durchs Rohr (Maus schwenkt).

use std::cell::RefCell;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::praesenz::Client as P;
use protokoll::spielzeug::Client as S;

use crate::api::{Klang, WeltAktion, klang, melden};
use crate::bau::farbe;
use crate::grundriss::{EMPORE_Y, GOLF_ABSCHLAG, GOLF_LOCH, TELESKOP};
use crate::register::{Aktivitaet, Phase, Register};
use crate::spiellogik::golfflug;
use crate::welt::Kontext;

const AUSHOLEN: f32 = 1.2;

struct Golfen {
    ausholen: Option<f32>,
}

impl Aktivitaet<Kontext> for Golfen {
    fn name(&self) -> &'static str {
        "golf"
    }
    fn taste(&mut self, kx: &mut Kontext, code: &str, gedrueckt: bool) -> bool {
        match (code, gedrueckt) {
            ("Space", true) => {
                self.ausholen.get_or_insert(0.0);
                true
            }
            ("Space", false) => {
                if let Some(a) = self.ausholen.take() {
                    let kraft = (a / AUSHOLEN).min(1.0);
                    let gier = crate::bewegung::winkel(kx.spieler.gier + std::f32::consts::PI);
                    let neigung = (kx.spieler.neigung + 0.45).clamp(0.15, 1.1);
                    kx.senden(ClientNachricht::Spielzeug(S::Golf {
                        gier,
                        neigung,
                        kraft,
                    }));
                }
                true
            }
            ("KeyE", true) | ("Escape", true) => {
                kx.alle_stoppen = true;
                true
            }
            (c, _) => ["KeyW", "KeyA", "KeyS", "KeyD"].contains(&c),
        }
    }
    fn tick(&mut self, kx: &mut Kontext, dt: f32) -> bool {
        if let Some(a) = self.ausholen.as_mut() {
            *a += dt;
        }
        let (x, z) = GOLF_ABSCHLAG;
        kx.spieler.versetzen(x + 0.5, 0.0, z, kx.spieler.blick);
        true
    }
    fn hinweis(&self, _kx: &Kontext) -> Option<String> {
        Some(match self.ausholen {
            Some(a) => format!(
                "Loslassen zum Schlagen · Kraft {}%",
                ((a / AUSHOLEN).min(1.0) * 100.0) as u32
            ),
            None => "Maus: zielen · Leertaste halten: ausholen · E: Schläger weglegen".into(),
        })
    }
    fn stoppen(&mut self, kx: &mut Kontext) {
        kx.senden(ClientNachricht::Praesenz(P::Handlung {
            rauchen: None,
            golfen: Some(false),
            getraenk: None,
            wirft: None,
        }));
    }
}

struct Flug {
    knoten: SceneNode3d,
    start: f64,
    gier: f32,
    neigung: f32,
    kraft: f32,
    gelandet: bool,
}

struct Teleskop;

impl Aktivitaet<Kontext> for Teleskop {
    fn name(&self) -> &'static str {
        "teleskop"
    }
    fn taste(&mut self, kx: &mut Kontext, code: &str, gedrueckt: bool) -> bool {
        if gedrueckt && (code == "KeyE" || code == "Escape" || code == "Space") {
            kx.alle_stoppen = true;
            return true;
        }
        false
    }
    fn tick(&mut self, kx: &mut Kontext, _dt: f32) -> bool {
        // Das Rohr schwenkt nur in einem Bereich über den Großraum.
        kx.spieler.neigung = kx.spieler.neigung.clamp(-0.5, 0.08);
        let auge = [8.45, EMPORE_Y + 0.95, 7.75];
        kx.kamera_ziel = Some((auge, kx.spieler.blickrichtung()));
        kx.kamera.sichtfeld(16.0);
        true
    }
    fn hinweis(&self, _kx: &Kontext) -> Option<String> {
        Some("Maus: schwenken · E: vom Teleskop weg".into())
    }
    fn stoppen(&mut self, kx: &mut Kontext) {
        kx.kamera.sichtfeld(70.0);
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.interaktion(
        "golf",
        |_, _| Some("E: Golfschläger nehmen".into()),
        |kx: &mut Kontext, _| {
            kx.senden(ClientNachricht::Praesenz(P::Handlung {
                rauchen: None,
                golfen: Some(true),
                getraenk: None,
                wirft: None,
            }));
            // Blick zum Loch über der Straße.
            let (x, z) = GOLF_ABSCHLAG;
            let gier = (GOLF_LOCH.0 - x).atan2(GOLF_LOCH.1 - z);
            kx.spieler.versetzen(x + 0.5, 0.0, z, gier);
            kx.aktivitaet(Golfen { ausholen: None });
        },
    );
    reg.interaktion(
        "teleskop",
        |_, _| Some("E: durchs Teleskop schauen".into()),
        |kx: &mut Kontext, _| {
            let (x, z) = TELESKOP;
            kx.spieler
                .versetzen(x, EMPORE_Y, z, (-3.5f32 - 8.45).atan2(-1.5 - 7.75));
            kx.aktivitaet(Teleskop);
        },
    );

    let fluege: Rc<RefCell<Vec<Flug>>> = Rc::default();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, _| {
        let mut fl = fluege.borrow_mut();
        if kx.neu_gebaut {
            for mut f in fl.drain(..) {
                f.knoten.detach();
            }
        }
        for (_, gier, neigung, kraft) in std::mem::take(&mut kx.aenderungen.golf) {
            let mut knoten = kx.etage.add_sphere(0.06);
            knoten.set_color(farbe("#ffffff"));
            klang(Klang::Golfschlag {
                pos: [GOLF_ABSCHLAG.0, 0.1, GOLF_ABSCHLAG.1],
            });
            fl.push(Flug {
                knoten,
                start: kx.zeit,
                gier,
                neigung,
                kraft,
                gelandet: false,
            });
        }
        let boden = kx.strasse();
        let start = [GOLF_ABSCHLAG.0 - 0.2, 0.05, GOLF_ABSCHLAG.1];
        fl.retain_mut(|f| {
            let t = (kx.zeit - f.start) as f32;
            let (p, fliegt) = golfflug(start, f.gier, f.neigung, f.kraft, boden, t);
            f.knoten.set_position(Vec3::from(p));
            if !fliegt && !f.gelandet {
                f.gelandet = true;
                let weit = (p[0] - GOLF_LOCH.0).hypot(p[2] - GOLF_LOCH.1);
                if weit < 1.0 {
                    klang(Klang::GolfLoch { pos: p });
                    melden(WeltAktion::Meldung {
                        text: "⛳ Eingelocht!".into(),
                    });
                } else if weit < 5.5 {
                    melden(WeltAktion::Meldung {
                        text: format!("Auf dem Grün, {weit:.1} m vom Loch."),
                    });
                }
            }
            let bleibt = t < 20.0;
            if !bleibt {
                f.knoten.detach();
            }
            bleibt
        });
    });
}
