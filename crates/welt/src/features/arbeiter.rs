//! Die Arbeiter an den Tischen: eine Figur je Arbeiter (tippt, wenn er arbeitet; schläft, wenn er
//! beendet ist), ein Lämpchen und ein Schild über dem Kopf mit seinem Status, der Monitor leuchtet
//! in der Statusfarbe. E am Tisch öffnet sein Terminal (oder stellt einen ein), dazu die Tasten
//! P (Auftrag), C (Änderungen), B (Shell), R (fortsetzen), X (nach Hause), O (Pull Request),
//! L (Schild) und N (zum nächsten, der wartet).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::arbeiter::{ArbeiterInfo, ArbeiterStatus, Client as A};

use crate::api::{WeltAktion, melden};
use crate::bau::{Bau, farbe};
use crate::farben::{self, status};
use crate::figur::{Figur, Haltung, Look};
use crate::grundriss::{TischArt, tisch_sitz};
use crate::register::{Phase, Register};
use crate::welt::Kontext;

struct Sitzend {
    figur: Figur,
    look: Look,
    tisch: String,
    sack: Option<SceneNode3d>,
}

pub fn statusfarbe(s: ArbeiterStatus) -> &'static str {
    match s {
        ArbeiterStatus::Arbeitet => status::ARBEITET,
        ArbeiterStatus::Wartet => status::WARTET,
        ArbeiterStatus::Fertig => status::FERTIG,
        ArbeiterStatus::Bereit | ArbeiterStatus::Startet => status::BEREIT,
        ArbeiterStatus::Beendet | ArbeiterStatus::Offline => status::SCHLAEFT,
    }
}

pub fn statustext(s: ArbeiterStatus) -> &'static str {
    match s {
        ArbeiterStatus::Startet => "startet",
        ArbeiterStatus::Bereit => "bereit",
        ArbeiterStatus::Arbeitet => "arbeitet",
        ArbeiterStatus::Wartet => "wartet auf dich",
        ArbeiterStatus::Fertig => "fertig",
        ArbeiterStatus::Beendet => "beendet",
        ArbeiterStatus::Offline => "schläft",
    }
}

/// Bauernkittel in der Halle (die Farbe des Arbeiters bleibt am Namen).
const KITTEL: &str = "#a68a64";

fn look(a: &ArbeiterInfo, halle: bool) -> Look {
    // Arbeiter haben kein Aussehen im Protokoll: aus der ID gewürfelt, aber immer gleich.
    let h =
        a.id.bytes()
            .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    Look {
        hemd: if halle {
            KITTEL.to_string()
        } else {
            a.farbe.clone()
        },
        haut: (h % 8) as u8,
        haare: (h / 8 % 10) as u8,
        frisur: (h / 80 % 7) as u8,
    }
}

/// In der Burg: wer vor dem Thron ansteht (fertig oder wartend, noch nicht angesehen), wer am
/// längsten wartet zuerst, höchstens `SCHLANGE_PLAETZE`.
pub fn schlange(kx: &Kontext) -> Vec<String> {
    if kx.halle().is_none_or(|p| p.thron.is_none()) {
        return Vec::new();
    }
    let mut v: Vec<&ArbeiterInfo> = kx
        .zustand
        .arbeiter
        .values()
        .filter(|a| {
            matches!(a.status, ArbeiterStatus::Wartet | ArbeiterStatus::Fertig)
                && !a.gesehen
                && !a.tisch.starts_with("station-")
        })
        .collect();
    v.sort_by_key(|a| a.wartet_seit.unwrap_or(u64::MAX));
    v.into_iter()
        .take(crate::grundriss::burg::SCHLANGE_PLAETZE)
        .map(|a| a.id.clone())
        .collect()
}

/// Der Arbeiter am Tisch, auf den man zielt.
fn am_ziel(kx: &Kontext) -> Option<(String, Option<ArbeiterInfo>)> {
    let z = kx.ziel.as_ref().filter(|z| z.art == "tisch")?;
    Some((z.id.clone(), kx.zustand.arbeiter_am_tisch(&z.id).cloned()))
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let figuren: Rc<RefCell<BTreeMap<String, Sitzend>>> = Rc::default();

    let f = figuren.clone();
    let mut abgaenge: Vec<crate::features::abgang::Abgang> = Vec::new();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, dt| {
        let mut figuren = f.borrow_mut();
        let halle = kx.in_der_burg();
        let jetzt = crate::grafik::jetzt_ms();
        if kx.neu_gebaut {
            abgaenge.clear();
        }
        if kx.neu_gebaut {
            for (_, mut s) in std::mem::take(&mut *figuren) {
                s.figur.entfernen();
            }
        }
        // Weg, wer nicht mehr da ist oder umgezogen ist.
        let weg: Vec<String> = figuren
            .iter()
            .filter(|(id, s)| {
                !kx.zustand
                    .arbeiter
                    .get(*id)
                    .is_some_and(|a| a.tisch == s.tisch && look(a, halle) == s.look)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in weg {
            let Some(mut s) = figuren.remove(&id) else {
                continue;
            };
            if let Some(mut n) = s.sack.take() {
                n.detach();
            }
            if kx.zustand.arbeiter.contains_key(&id) {
                // Umgezogen oder neu eingekleidet: gleich neu bauen.
                s.figur.entfernen();
            } else {
                // Nach Hause geschickt: packt einen Karton und geht (Aufzug, Kerker oder Fallschirm).
                abgaenge.push(crate::features::abgang::Abgang::neu(kx, s.figur, &s.tisch));
            }
        }
        abgaenge.retain_mut(|a| a.schritt(dt));
        for a in &abgaenge {
            for (text, ort) in a.sprechblasen() {
                kx.beschriften(text, ort, Color::new(1.0, 0.95, 0.85, 1.0));
            }
        }
        let arbeiter: Vec<ArbeiterInfo> = kx.zustand.arbeiter.values().cloned().collect();
        let schlange = schlange(kx);
        for a in &arbeiter {
            let Some(t) = kx.tisch(&a.tisch) else {
                continue;
            };
            let s = figuren.entry(a.id.clone()).or_insert_with(|| {
                let l = look(a, halle);
                let figur = Figur::neu(&mut kx.etage, &l);
                let sack = (t.art == TischArt::Sitzsack).then(|| {
                    let mut gebaut = crate::bau::Gebaut::default();
                    let mut g = kx.etage.add_group();
                    let mut b = Bau::neu(g.clone(), &mut gebaut);
                    crate::bau::einrichtung::sitzsack(
                        &mut b,
                        t.x,
                        t.z,
                        farben::STUEHLE[a.tisch.len() % 6],
                    );
                    g.set_visible(true);
                    g
                });
                Sitzend {
                    figur,
                    look: l,
                    tisch: a.tisch.clone(),
                    sack,
                }
            });
            let steht = matches!(t.art, TischArt::Kiosk(_));
            let (x, z) = match t.art {
                TischArt::Sitzsack => (t.x, t.z),
                TischArt::Kiosk(_) => tisch_sitz(&t, 0.55),
                _ => tisch_sitz(&t, 0.85),
            };
            let schlaeft = matches!(a.status, ArbeiterStatus::Beendet | ArbeiterStatus::Offline);
            let huefte = if t.art == TischArt::Sitzsack {
                0.38
            } else {
                0.5
            };
            s.figur.haltung = if steht {
                Haltung::Stehen
            } else if a.status == ArbeiterStatus::Arbeitet {
                Haltung::Tippen
            } else {
                Haltung::Sitzen(huefte)
            };
            let y = if steht {
                0.0
            } else {
                Figur::sitz_versatz(huefte)
            };
            match schlange.iter().position(|id| id == &a.id) {
                // In der Burg: vor dem Thron anstehen.
                Some(i) => {
                    let (sx, sz) = kx
                        .halle()
                        .and_then(crate::grundriss::burg::schlange)
                        .unwrap_or((0.0, 0.0));
                    s.figur.haltung = Haltung::Stehen;
                    s.figur.setzen(
                        sx,
                        0.0,
                        sz + i as f32 * crate::grundriss::burg::SCHLANGE_SCHRITT,
                        PI,
                    );
                }
                None => s.figur.setzen(x, y, z, t.rot_y + PI),
            }
            s.figur.animieren(dt);
            let verbraucht =
                crate::grundriss::verschleiss::verbraucht(a.gearbeitet_ms, a.arbeitet_seit, jetzt);
            s.figur.altern(halle.then_some(verbraucht));
            // Wartet: das Lämpchen blinkt.
            let blink = a.status != ArbeiterStatus::Wartet || (kx.zeit * 3.0).fract() < 0.6;
            s.figur.lampe(blink.then(|| statusfarbe(a.status)));
            let kopf = s.figur.kopf_welt();
            let text = if schlaeft {
                format!("{} · zzz", a.name)
            } else {
                format!("{} · {}", a.name, statustext(a.status))
            };
            kx.beschriften(
                text,
                [kopf.x, kopf.y + 0.15, kopf.z],
                crate::bau::farbe_srgb(&a.farbe),
            );
            if let Some(m) = kx.teil(&format!("monitor:{}", a.tisch)) {
                let c = farbe(if schlaeft {
                    "#14213d"
                } else {
                    statusfarbe(a.status)
                });
                m.set_emissive(Color::new(c.r * 0.6, c.g * 0.6, c.b * 0.6, 1.0));
            }
        }
    });

    reg.interaktion(
        "tisch",
        |kx: &Kontext, n| {
            let t = kx.tisch(&n.id)?;
            if matches!(t.art, TischArt::Kiosk(_)) {
                let wer = kx
                    .zustand
                    .arbeiter_am_tisch(&n.id)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "den Board-Agenten".into());
                return Some(format!("E: {wer} fragen"));
            }
            Some(match kx.zustand.arbeiter_am_tisch(&n.id) {
                Some(a) => {
                    let mut h = format!(
                        "E: Terminal von {} · P: Auftrag · C: Änderungen · X: nach Hause",
                        a.name
                    );
                    if matches!(a.status, ArbeiterStatus::Beendet | ArbeiterStatus::Offline) {
                        h.push_str(" · R: fortsetzen");
                    }
                    if a.worktree.is_some() {
                        h.push_str(" · O: Pull Request");
                    }
                    h
                }
                None if matches!(t.art, TischArt::Kiosk(_)) => "E: Board-Agent einstellen".into(),
                None => "E: Arbeiter einstellen · B: Shell öffnen · L: Schild".into(),
            })
        },
        |kx: &mut Kontext, n| match kx.zustand.arbeiter_am_tisch(&n.id) {
            _ if n.id.starts_with("station-") => melden(WeltAktion::StationOeffnen {
                tisch: n.id.clone(),
            }),
            Some(a) => melden(WeltAktion::TerminalOeffnen {
                arbeiter: a.id.clone(),
            }),
            None => melden(WeltAktion::Einstellen {
                tisch: n.id.clone(),
            }),
        },
    );

    let am_tisch: fn(&Kontext) -> bool = |kx| kx.ziel.as_ref().is_some_and(|z| z.art == "tisch");
    let mit = |tun: fn(&mut Kontext, String, Option<ArbeiterInfo>) -> bool| {
        move |kx: &mut Kontext| match am_ziel(kx) {
            Some((tisch, a)) => tun(kx, tisch, a),
            None => false,
        }
    };
    reg.taste(
        "KeyP",
        Some(Box::new(am_tisch)),
        mit(|_, tisch, a| {
            melden(match a {
                Some(a) => WeltAktion::PromptOeffnen { arbeiter: a.id },
                None => WeltAktion::Einstellen { tisch },
            });
            true
        }),
    );
    reg.taste(
        "KeyC",
        Some(Box::new(am_tisch)),
        mit(|_, _, a| {
            a.map(|a| melden(WeltAktion::AenderungenOeffnen { arbeiter: a.id }))
                .is_some()
        }),
    );
    reg.taste(
        "KeyB",
        Some(Box::new(am_tisch)),
        mit(|_, tisch, a| {
            a.is_none() && {
                melden(WeltAktion::ShellOeffnen { tisch });
                true
            }
        }),
    );
    reg.taste(
        "KeyR",
        Some(Box::new(am_tisch)),
        mit(|kx, _, a| match a {
            Some(a) => {
                kx.senden(ClientNachricht::Arbeiter(A::Fortsetzen { arbeiter: a.id }));
                true
            }
            None => false,
        }),
    );
    reg.taste(
        "KeyX",
        Some(Box::new(am_tisch)),
        mit(|_, _, a| {
            a.map(|a| melden(WeltAktion::NachHauseSchicken { arbeiter: a.id }))
                .is_some()
        }),
    );
    reg.taste(
        "KeyO",
        Some(Box::new(am_tisch)),
        mit(|_, _, a| {
            a.map(|a| melden(WeltAktion::PullRequest { arbeiter: a.id }))
                .is_some()
        }),
    );
    reg.taste(
        "KeyL",
        Some(Box::new(am_tisch)),
        mit(|_, tisch, _| {
            melden(WeltAktion::SchildOeffnen { tisch });
            true
        }),
    );
    reg.taste("KeyN", None, |kx: &mut Kontext| {
        let Some(a) = kx.zustand.laengster_wartender().cloned() else {
            melden(WeltAktion::Meldung {
                text: "Gerade wartet niemand auf dich.".into(),
            });
            return true;
        };
        if let Some(t) = kx.tisch(&a.tisch) {
            let (x, z) = tisch_sitz(&t, 1.7);
            let blick = (t.x - x).atan2(t.z - z);
            kx.spieler.versetzen(x, 0.0, z, blick);
        }
        true
    });
}
