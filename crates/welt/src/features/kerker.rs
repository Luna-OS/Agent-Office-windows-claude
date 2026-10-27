//! Die Burg: Thron, Hand des Königs und der Kerker mit seinen Gefangenen.
//!
//! E am Thron: der Erste in der Schlange (sein Terminal), ohne Schlange die Hand des Königs.
//! E bei der Hand (oder K): einen neuen Arbeiter an den ersten freien Platz schicken.
//! E am Treppenloch: das Kerker-Fenster der Oberfläche. Im Kerker liegen alle Heimgeschickten,
//! werden mit den Stunden dünner und blasser, verhungern nach einem Tag und verwesen danach.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

use kiss3d::prelude::*;

use crate::api::{WeltAktion, melden};
use crate::bau::farbe_srgb;
use crate::figur::{Figur, Haltung, Look};
use crate::grundriss::burg::{self, KERKER_TIEFE, Verfall, gefangenen_platz, verfall};
use crate::register::{Phase, Register};
use crate::welt::Kontext;

/// Der erste freie Platz an den Tafeln (in der Reihenfolge, in der sie sich füllen).
fn erster_freier_platz(kx: &Kontext) -> Option<String> {
    burg::tische(kx.halle()?)
        .into_iter()
        .filter(|t| t.id.starts_with("desk-") || t.id.starts_with("beanbag-"))
        .find(|t| kx.zustand.arbeiter_am_tisch(&t.id).is_none())
        .map(|t| t.id)
}

fn hand_fragen(kx: &Kontext) {
    match erster_freier_platz(kx) {
        Some(tisch) => melden(WeltAktion::Einstellen { tisch }),
        None => melden(WeltAktion::Meldung {
            text: "Alle Plätze an den Tafeln sind besetzt, Majestät.".into(),
        }),
    }
}

/// Was E auf dem Thron tut: der Erste in der Schlange, sonst die Hand des Königs.
pub fn vom_thron(kx: &mut Kontext) {
    match crate::features::arbeiter::schlange(kx).into_iter().next() {
        Some(arbeiter) => melden(WeltAktion::TerminalOeffnen { arbeiter }),
        None => hand_fragen(kx),
    }
}

/// Der Hinweis auf dem Thron.
pub fn thron_hinweis(kx: &Kontext) -> String {
    let erster = crate::features::arbeiter::schlange(kx).into_iter().next();
    match erster.and_then(|id| kx.zustand.arbeiter.get(&id)) {
        Some(a) => format!(
            "E: {} anhören ({}) · K: Hand des Königs",
            a.name,
            crate::features::arbeiter::statustext(a.status)
        ),
        None => "E oder K: die Hand des Königs".into(),
    }
}

struct Zelleninsasse {
    figur: Figur,
    look: Look,
}

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.interaktion(
        "thron",
        |_, _| Some("E: auf den Thron setzen".into()),
        |kx: &mut Kontext, _| {
            crate::features::sitzen::hinsetzen(kx, burg::THRON_SITZ);
        },
    );
    reg.interaktion(
        "hand",
        |_, _| Some("E: Hand des Königs – einen neuen Arbeiter aussenden".into()),
        |kx: &mut Kontext, _| hand_fragen(kx),
    );
    reg.taste(
        "KeyK",
        Some(Box::new(|kx: &Kontext| kx.in_der_burg())),
        |kx| {
            hand_fragen(kx);
            true
        },
    );
    reg.interaktion(
        "kerker",
        |_, _| Some("E: in den Kerker schauen".into()),
        |_, _| melden(WeltAktion::KerkerOeffnen),
    );

    let insassen: Rc<RefCell<BTreeMap<String, Zelleninsasse>>> = Rc::default();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, dt| {
        let mut ins = insassen.borrow_mut();
        if kx.neu_gebaut || !kx.in_der_burg() {
            for (_, mut z) in std::mem::take(&mut *ins) {
                z.figur.entfernen();
            }
            if !kx.in_der_burg() {
                return;
            }
        }
        let gefangene = kx.zustand.kerker.gefangene.clone();
        ins.retain(|id, z| {
            let bleibt = gefangene.iter().any(|g| &g.id == id);
            if !bleibt {
                z.figur.entfernen();
            }
            bleibt
        });
        let jetzt = crate::grafik::jetzt_ms();
        let y = -KERKER_TIEFE;
        let dz = kx.halle().map(|p| p.kerker_versatz()).unwrap_or(0.0);
        for (i, g) in gefangene.iter().enumerate() {
            let stunden = jetzt.saturating_sub(g.um) as f32 / 3_600_000.0;
            let zustand = verfall(stunden);
            let look = Look {
                hemd: if matches!(zustand, Verfall::Lebt(_)) {
                    g.farbe.clone()
                } else {
                    "#6b705c".into()
                },
                haut: 5,
                haare: 7,
                frisur: 6,
            };
            let z = ins.entry(g.id.clone()).or_insert_with(|| Zelleninsasse {
                figur: Figur::neu(&mut kx.etage, &look),
                look: look.clone(),
            });
            if z.look != look {
                z.figur.entfernen();
                *z = Zelleninsasse {
                    figur: Figur::neu(&mut kx.etage, &look),
                    look,
                };
            }
            let (x, zz) = gefangenen_platz(i);
            let blick = if zz < burg::GANG_Z { 0.0 } else { PI };
            let zz = zz + dz;
            match zustand {
                Verfall::Lebt(a) => {
                    z.figur.haltung = Haltung::Sitzen(0.3);
                    z.figur.setzen(x, y + Figur::sitz_versatz(0.3), zz, blick);
                    // Ausgezehrt: schmaler.
                    z.figur
                        .wurzel
                        .set_local_scale(1.0 - 0.45 * a, 1.0, 1.0 - 0.45 * a);
                    z.figur.animieren(dt);
                    kx.beschriften(g.name.clone(), [x, y + 1.6, zz], farbe_srgb(&g.farbe));
                }
                Verfall::Tot(_) | Verfall::Knochen => {
                    z.figur.haltung = Haltung::Stehen;
                    z.figur.setzen(x, y + 0.25, zz, blick);
                    z.figur.wurzel.set_rotation(
                        Quat::from_axis_angle(Vec3::Y, blick)
                            * Quat::from_axis_angle(Vec3::X, -FRAC_PI_2),
                    );
                    z.figur.wurzel.set_local_scale(0.55, 1.0, 0.55);
                    let wie = if zustand == Verfall::Knochen {
                        "Gebeine"
                    } else {
                        "verhungert"
                    };
                    kx.beschriften(
                        format!("{} ({wie})", g.name),
                        [x, y + 0.9, zz],
                        farbe_srgb("#adb5bd"),
                    );
                }
            }
        }
        if kx.zustand.kerker.knochen > 0 {
            let (kx_, kz) = (burg::KNOCHEN.0, burg::KNOCHEN.1 + dz);
            kx.beschriften(
                format!("{} Knochen", kx.zustand.kerker.knochen),
                [kx_, y + 1.0, kz],
                farbe_srgb("#f1e9d2"),
            );
        }
    });
}
