//! Der Hund der Etage: läuft seine Wegstücke ab (der Server plant sie), schnüffelt, schläft im Korb
//! oder unter einem Tisch, bellt bei Arbeitern, die warten, und wedelt, wenn man ihn streichelt.
//! E streichelt ihn, Shift+E benennt ihn um.

use std::cell::RefCell;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::spielzeug::{Client as S, Hund, HundTun, Rasse};

use crate::api::{Klang, WeltAktion, klang, melden};
use crate::bau::{farbe, farbe_srgb};
use crate::register::{Nutzbar, Phase, Register};
use crate::spiellogik::{FELLE, hund_bei};
use crate::welt::Kontext;

struct Modell {
    wurzel: SceneNode3d,
    beine: [SceneNode3d; 4],
    schwanz: SceneNode3d,
    kopf: SceneNode3d,
    art: (u32, Rasse),
}

#[derive(Default)]
struct Stand {
    modell: Option<Modell>,
    /// Das laufende Wegstück und wann (Weltzeit) es nach Rechnung begann.
    stueck: Option<(Hund, f64)>,
    gebellt: f64,
    phase: f32,
}

/// Maße je Rasse: Körperlänge, Beinlänge, Größe.
fn masse(r: Rasse) -> (f32, f32, f32) {
    match r {
        Rasse::Welpe => (0.45, 0.18, 0.8),
        Rasse::Corgi => (0.6, 0.12, 1.0),
        Rasse::Dackel => (0.8, 0.1, 0.95),
        Rasse::Mops => (0.42, 0.16, 0.95),
        Rasse::Shiba => (0.6, 0.24, 1.0),
    }
}

fn bauen(eltern: &mut SceneNode3d, fell: u32, rasse: Rasse) -> Modell {
    let [koerper_c, bauch_c, ohr_c] = FELLE[fell as usize % FELLE.len()].map(farbe);
    let (laenge, bein, s) = masse(rasse);
    let mut wurzel = eltern.add_group();
    let y = bein + 0.12;
    let mut koerper = wurzel.add_capsule(0.14 * s, laenge * s);
    koerper.set_rotation(Quat::from_axis_angle(Vec3::X, std::f32::consts::FRAC_PI_2));
    koerper.set_position(Vec3::new(0.0, y, 0.0));
    koerper.set_color(koerper_c);
    let mut bauch = wurzel.add_sphere(0.12 * s);
    bauch.set_local_scale(1.0, 0.7, laenge * 2.6);
    bauch.set_position(Vec3::new(0.0, y - 0.05, 0.0));
    bauch.set_color(bauch_c);
    let mut kopf = wurzel.add_group();
    kopf.set_position(Vec3::new(0.0, y + 0.14 * s, (laenge / 2.0 + 0.16) * s));
    kopf.add_sphere(0.15 * s).set_color(koerper_c);
    let schnauze = if rasse == Rasse::Mops { 0.06 } else { 0.12 };
    kopf.add_sphere(0.08 * s)
        .set_position(Vec3::new(0.0, -0.04 * s, schnauze * s))
        .set_color(bauch_c);
    kopf.add_sphere(0.025 * s)
        .set_position(Vec3::new(0.0, -0.02 * s, (schnauze + 0.08) * s))
        .set_color(farbe("#1b1b1b"));
    for x in [-0.07, 0.07] {
        kopf.add_sphere(0.022 * s)
            .set_position(Vec3::new(x * s, 0.04 * s, 0.13 * s))
            .set_color(farbe("#1b1b1b"));
        let mut ohr = kopf.add_cone(0.05 * s, 0.12 * s);
        ohr.set_position(Vec3::new(x * 1.3 * s, 0.14 * s, -0.02));
        ohr.set_color(ohr_c);
        if matches!(rasse, Rasse::Dackel | Rasse::Mops | Rasse::Welpe) {
            ohr.set_rotation(Quat::from_axis_angle(
                Vec3::Z,
                if x < 0.0 { 2.4 } else { -2.4 },
            ));
        }
    }
    let mut beine = Vec::new();
    for (bx, bz) in [(-0.08, 0.35), (0.08, 0.35), (-0.08, -0.35), (0.08, -0.35)] {
        let mut g = wurzel.add_group();
        g.set_position(Vec3::new(bx * s, bein + 0.04, bz * laenge * s));
        let mut b = g.add_capsule(0.035 * s, bein);
        b.set_position(Vec3::new(0.0, -bein / 2.0, 0.0));
        b.set_color(koerper_c);
        beine.push(g);
    }
    let mut schwanz = wurzel.add_group();
    schwanz.set_position(Vec3::new(0.0, y + 0.05, -(laenge / 2.0 + 0.12) * s));
    let mut sch = schwanz.add_capsule(0.03 * s, 0.18 * s);
    sch.set_position(Vec3::new(0.0, 0.09 * s, 0.0));
    sch.set_color(koerper_c);
    Modell {
        wurzel,
        beine: beine.try_into().unwrap_or_else(|_| unreachable!()),
        schwanz,
        kopf,
        art: (fell, rasse),
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let stand = Rc::new(RefCell::new(Stand::default()));

    let s = stand.clone();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, dt| {
        let mut st = s.borrow_mut();
        if kx.neu_gebaut {
            if let Some(mut m) = st.modell.take() {
                m.wurzel.detach();
            }
            st.stueck = None;
        }
        // Ein neues Wegstück?
        let hund = kx.zustand.spielzeug.hund.clone();
        let neu = match (&hund, &st.stueck) {
            (Some(h), Some((alt, _))) => h != alt,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if neu && let Some(h) = &hund {
            if h.tun == HundTun::Bellen {
                st.gebellt = 0.0;
            }
            st.stueck = Some((h.clone(), kx.zeit - h.vergangen_ms as f64 / 1000.0));
        }
        let Some((h, start)) = st.stueck.clone() else {
            if let Some(mut m) = st.modell.take() {
                m.wurzel.detach();
            }
            return;
        };
        if st
            .modell
            .as_ref()
            .is_none_or(|m| m.art != (h.fell, h.rasse))
        {
            if let Some(mut m) = st.modell.take() {
                m.wurzel.detach();
            }
            st.modell = Some(bauen(&mut kx.etage, h.fell, h.rasse));
        }
        let lage = hund_bei(&h.weg, h.tempo, h.blick, (kx.zeit - start) as f32);
        st.phase += dt * if lage.unterwegs { 12.0 } else { 3.0 };
        let phase = st.phase;
        let m = st.modell.as_mut().unwrap_or_else(|| unreachable!());
        m.wurzel.set_position(Vec3::new(lage.x, 0.0, lage.z));
        m.wurzel
            .set_rotation(Quat::from_axis_angle(Vec3::Y, lage.blick));
        let liegt = !lage.unterwegs && matches!(h.tun, HundTun::Liegen | HundTun::Schlafen);
        let sitzt = !lage.unterwegs && h.tun == HundTun::Sitzen;
        for (i, b) in m.beine.iter_mut().enumerate() {
            let w = if lage.unterwegs {
                (phase
                    + if i % 2 == 0 {
                        0.0
                    } else {
                        std::f32::consts::PI
                    })
                .sin()
                    * 0.6
            } else if liegt {
                1.4
            } else {
                0.0
            };
            b.set_rotation(Quat::from_axis_angle(
                Vec3::X,
                if i >= 2 && sitzt { -1.2 } else { w },
            ));
        }
        m.wurzel
            .set_local_scale(1.0, if liegt { 0.55 } else { 1.0 }, 1.0);
        let wedeln = matches!(h.tun, HundTun::Wedeln | HundTun::Bellen) || lage.unterwegs;
        m.schwanz.set_rotation(
            Quat::from_axis_angle(
                Vec3::Y,
                if wedeln {
                    (phase * 3.0).sin() * 0.7
                } else {
                    0.0
                },
            ) * Quat::from_axis_angle(Vec3::X, -0.5),
        );
        let schnueffeln = h.tun == HundTun::Schnueffeln && !lage.unterwegs;
        m.kopf.set_rotation(Quat::from_axis_angle(
            Vec3::X,
            if schnueffeln {
                0.5 + (phase * 4.0).sin() * 0.1
            } else if liegt {
                0.3
            } else {
                0.0
            },
        ));
        let pos = [lage.x, 0.5, lage.z];
        // Bellen: gleich am Anfang, dann alle 14 Sekunden.
        if h.tun == HundTun::Bellen && !lage.unterwegs && kx.zeit - st.gebellt > 14.0 {
            st.gebellt = kx.zeit;
            klang(Klang::Bellen { pos });
        }
        let mut name = h.name.clone();
        if h.tun == HundTun::Schlafen && !lage.unterwegs {
            name.push_str(" · zzz");
        } else if h.tun == HundTun::Bellen {
            name.push_str(" · Wuff!");
        }
        kx.beschriften(name, [lage.x, 0.95, lage.z], farbe_srgb("#ffffff"));
        kx.dinge.push(Nutzbar {
            art: "hund",
            id: "hund".into(),
            pos,
            reichweite: 1.5,
        });
    });

    reg.interaktion(
        "hund",
        |kx: &Kontext, _| {
            let name = kx
                .zustand
                .spielzeug
                .hund
                .as_ref()
                .map(|h| h.name.clone())
                .unwrap_or_else(|| "den Hund".into());
            Some(format!("E: {name} streicheln · Shift+E: umbenennen"))
        },
        |kx: &mut Kontext, _| {
            let g = &kx.eingabe.gehalten;
            if g.contains("ShiftLeft") || g.contains("ShiftRight") {
                melden(WeltAktion::HundUmbenennen);
            } else {
                kx.senden(ClientNachricht::Spielzeug(S::HundStreicheln));
            }
        },
    );
}
