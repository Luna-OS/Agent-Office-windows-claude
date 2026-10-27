//! Der Basketball: liegt unter dem Korb, bis ihn jemand nimmt (E). Wer ihn hält, trägt ihn vor
//! sich; E gedrückt halten holt aus, Loslassen wirft in Blickrichtung (je länger, desto weiter),
//! Q lässt ihn fallen. Jeder Wurf wird auf jeder Seite gleich nachgerechnet (`spiellogik::Ball`).

use std::cell::RefCell;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::spielzeug::{Ball as BallStand, Client as S};

use crate::api::{Klang, WeltAktion, klang, melden};
use crate::bau::farbe;
use crate::grundriss::draussen::{BALL_MAX_TEMPO, BALL_RADIUS, BALL_RUHE};
use crate::register::{Aktivitaet, Nutzbar, Phase, Register};
use crate::spiellogik::Ball;
use crate::welt::Kontext;

/// So lange ausholen für den weitesten Wurf (Sekunden).
const AUSHOLEN: f32 = 1.05;

#[derive(Default)]
struct Stand {
    knoten: Option<SceneNode3d>,
    /// Der zuletzt gesehene Stand und die lokale Rechnung dazu.
    gesehen: Option<BallStand>,
    flug: Option<Ball>,
    rest: f32,
}

struct Halten {
    ausholen: Option<f32>,
}

impl Aktivitaet<Kontext> for Halten {
    fn name(&self) -> &'static str {
        "ball"
    }
    fn haelt_fest(&self) -> bool {
        false
    }
    fn taste(&mut self, kx: &mut Kontext, code: &str, gedrueckt: bool) -> bool {
        match (code, gedrueckt) {
            ("KeyE", true) => {
                self.ausholen = Some(0.0);
                true
            }
            ("KeyE", false) => {
                let kraft = (self.ausholen.take().unwrap_or(0.0) / AUSHOLEN).min(1.0);
                werfen(kx, 3.0 + kraft * (BALL_MAX_TEMPO - 3.0));
                false
            }
            ("KeyQ", true) => {
                werfen(kx, 0.5);
                true
            }
            _ => false,
        }
    }
    fn tick(&mut self, kx: &mut Kontext, dt: f32) -> bool {
        if let Some(a) = self.ausholen.as_mut() {
            *a += dt;
        }
        ich_halte(kx)
    }
    fn hinweis(&self, _kx: &Kontext) -> Option<String> {
        Some(match self.ausholen {
            Some(a) => format!(
                "Loslassen zum Werfen · Kraft {}%",
                ((a / AUSHOLEN).min(1.0) * 100.0) as u32
            ),
            None => "E halten: ausholen und werfen · Q: fallen lassen".into(),
        })
    }
}

fn ich_halte(kx: &Kontext) -> bool {
    kx.zustand.spielzeug.ball.halter.is_some() && kx.zustand.spielzeug.ball.halter == kx.zustand.ich
}

fn hand(kx: &Kontext) -> [f32; 3] {
    let a = kx.spieler.auge();
    let r = kx.spieler.blickrichtung();
    [
        a[0] + r[0] * 0.5,
        a[1] - 0.35 + r[1] * 0.5,
        a[2] + r[2] * 0.5,
    ]
}

fn werfen(kx: &mut Kontext, tempo: f32) {
    let p = hand(kx);
    let r = kx.spieler.blickrichtung();
    // Etwas Bogen nach oben, damit ein Wurf auf den Korb natürlich wirkt.
    let v = [r[0] * tempo, (r[1] + 0.35) * tempo, r[2] * tempo];
    kx.senden(ClientNachricht::Spielzeug(S::BallWerfen {
        x: p[0],
        y: p[1],
        z: p[2],
        vx: v[0],
        vy: v[1],
        vz: v[2],
    }));
    kx.alle_stoppen = true;
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let stand = Rc::new(RefCell::new(Stand::default()));
    let s = stand.clone();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, dt| {
        let mut st = s.borrow_mut();
        let st = &mut *st;
        if kx.neu_gebaut || st.knoten.is_none() {
            if let Some(mut k) = st.knoten.take() {
                k.detach();
            }
            let mut k = kx.etage.add_sphere(BALL_RADIUS);
            k.set_color(farbe("#ff7b00"));
            k.set_roughness(0.9);
            st.knoten = Some(k);
            st.gesehen = None;
        }
        if kx.zustand.auf_dem_dach() {
            if let Some(k) = st.knoten.as_mut() {
                k.set_visible(false);
            }
            return;
        }
        let ball = kx.zustand.spielzeug.ball.clone();
        if st.gesehen.as_ref() != Some(&ball) {
            st.flug = ball.wurf.map(|w| {
                Ball::nach(
                    [w.x, w.y, w.z],
                    [w.vx, w.vy, w.vz],
                    w.vergangen_ms as f32 / 1000.0,
                )
            });
            st.rest = 0.0;
            st.gesehen = Some(ball.clone());
        }
        let pos = if let Some(h) = &ball.halter {
            if Some(h) == kx.zustand.ich.as_ref() {
                hand(kx)
            } else if let Some(p) = kx.zustand.personen.get(h) {
                [
                    p.x + p.rot_y.sin() * 0.4,
                    p.y + 1.1,
                    p.z + p.rot_y.cos() * 0.4,
                ]
            } else {
                [BALL_RUHE.0, BALL_RADIUS, BALL_RUHE.1]
            }
        } else if let Some(f) = st.flug.as_mut() {
            st.rest += dt;
            while st.rest >= crate::spiellogik::BALL_SCHRITT {
                st.rest -= crate::spiellogik::BALL_SCHRITT;
                let e = f.schritt();
                if e.korb {
                    klang(Klang::BallKorb { pos: f.p });
                    melden(WeltAktion::Meldung {
                        text: "🏀 Korb!".into(),
                    });
                } else if e.ring {
                    klang(Klang::BallRing { pos: f.p });
                } else if e.aufprall {
                    klang(Klang::BallAufprall { pos: f.p });
                }
            }
            f.p
        } else {
            [BALL_RUHE.0, BALL_RADIUS, BALL_RUHE.1]
        };
        if let Some(k) = st.knoten.as_mut() {
            k.set_visible(true);
            k.set_position(Vec3::from(pos));
        }
        if ball.halter.is_none() {
            kx.dinge.push(Nutzbar {
                art: "ball",
                id: "ball".into(),
                pos,
                reichweite: 1.4,
            });
        }
        // Wer den Ball bekommt, hält ihn (Aktivität), bis er ihn wirft.
        if ich_halte(kx) && kx.aktiv != Some("ball") && kx.beginnen.is_none() {
            kx.aktivitaet(Halten { ausholen: None });
        }
    });

    reg.interaktion(
        "ball",
        |_, _| Some("E: Basketball nehmen".into()),
        |kx: &mut Kontext, _| {
            kx.senden(ClientNachricht::Spielzeug(S::BallNehmen));
        },
    );
}
