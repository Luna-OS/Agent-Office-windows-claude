//! Was man im Büro benutzen kann, das ein Fenster der Oberfläche öffnet oder eine kleine Wirkung
//! hat: Tafeln, TV, Whiteboard, Bücherregal, Jukebox, Arcade, Leinwand, Bar, Gong, Kaffee,
//! Aschenbecher, Leiter und Feuerwehrstange. Dazu F (Bild aufhängen) und H (Hilfe).

use std::cell::Cell;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::praesenz::Client as P;

use crate::api::{Board, WeltAktion, melden};
use crate::bewegung::Wirkung;
use crate::register::{Phase, Register};
use crate::welt::Kontext;

/// Wie lange ein Kaffee wirkt (Sekunden), und wie sehr.
const KAFFEE_DAUER: f64 = 60.0;
const KAFFEE: Wirkung = Wirkung {
    tempo: 1.35,
    sprung: 1.3,
};

fn board_nach_name(n: &str) -> Option<Board> {
    Some(match n {
        "Issues" => Board::Issues,
        "Warteschlange" => Board::Warteschlange,
        "Pull Requests" => Board::PullRequests,
        "Dienste" => Board::Dienste,
        _ => return None,
    })
}

/// Eine Art, die nur ein Fenster öffnet.
fn fenster(
    reg: &mut Register<Kontext>,
    art: &'static str,
    hinweis: &'static str,
    aktion: WeltAktion,
) {
    reg.interaktion(
        art,
        move |_, _| Some(hinweis.into()),
        move |_, _| melden(aktion.clone()),
    );
}

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.interaktion(
        "tafel",
        |_, n| Some(format!("E: {} ansehen", n.id)),
        |_, n| {
            if let Some(board) = board_nach_name(&n.id) {
                melden(WeltAktion::BoardOeffnen { board });
            }
        },
    );
    fenster(
        reg,
        "tv",
        "E: geteilten Bildschirm ansehen",
        WeltAktion::TvOeffnen,
    );
    fenster(
        reg,
        "whiteboard",
        "E: am Whiteboard zeichnen",
        WeltAktion::WhiteboardOeffnen,
    );
    fenster(
        reg,
        "regal",
        "E: die Doku des Projekts lesen",
        WeltAktion::BuecherregalOeffnen,
    );
    fenster(reg, "jukebox", "E: Jukebox", WeltAktion::JukeboxOeffnen);
    fenster(
        reg,
        "arcade",
        "E: am Automaten spielen",
        WeltAktion::ArcadeOeffnen,
    );
    fenster(
        reg,
        "leinwand",
        "E: Video auf die Leinwand legen",
        WeltAktion::LeinwandOeffnen,
    );
    fenster(reg, "bar", "E: an der Bar bestellen", WeltAktion::BarKarte);

    // Der Gong schwingt (und alle hören ihn, sobald die Medien ihn spielen).
    let schwung = Rc::new(Cell::new(0.0f32));
    let s = schwung.clone();
    reg.interaktion(
        "gong",
        |_, _| Some("E: Gong schlagen".into()),
        move |_, n| {
            s.set(1.0);
            crate::api::klang(crate::api::Klang::Gong { pos: n.pos });
        },
    );
    // Die Ruhelage des Gongs (Burg und Büro hängen ihn verschieden auf), beim ersten Schlag gemerkt.
    let mut ruhe: Option<Quat> = None;
    reg.tick(Phase::Umgebung, move |kx: &mut Kontext, dt| {
        if kx.neu_gebaut {
            ruhe = None;
        }
        let w = schwung.get();
        if w <= 0.0 {
            return;
        }
        schwung.set((w - dt * 0.6).max(0.0));
        let winkel = (kx.zeit as f32 * 14.0).sin() * 0.12 * w;
        if let Some(g) = kx.teil("gong") {
            let basis = *ruhe.get_or_insert_with(|| g.rotation());
            g.set_rotation(Quat::from_axis_angle(Vec3::Y, winkel) * basis);
        }
    });

    // Kaffee: eine Minute schneller laufen und höher springen.
    let bis = Rc::new(Cell::new(0.0f64));
    let b = bis.clone();
    reg.interaktion(
        "kaffee",
        |_, _| Some("E: Kaffee holen".into()),
        move |kx: &mut Kontext, _| {
            b.set(kx.zeit + KAFFEE_DAUER);
            let (x, z) = crate::grundriss::KAFFEEMASCHINE;
            crate::api::klang(crate::api::Klang::Kaffee {
                pos: [x, 1.2, z + 1.0],
            });
            melden(WeltAktion::Meldung {
                text: "☕ Eine Minute lang schneller unterwegs.".into(),
            });
        },
    );
    reg.tick(Phase::Vor, move |kx: &mut Kontext, _| {
        kx.wirkung = if kx.zeit < bis.get() {
            KAFFEE
        } else {
            Wirkung::default()
        };
    });

    // Raucherpause auf dem Balkon.
    let raucht = Rc::new(Cell::new(false));
    let r = raucht.clone();
    reg.interaktion(
        "aschenbecher",
        move |_, _| {
            Some(if r.get() {
                "E: Zigarette ausdrücken".into()
            } else {
                "E: Raucherpause".into()
            })
        },
        move |kx: &mut Kontext, _| {
            let an = !raucht.get();
            raucht.set(an);
            kx.senden(ClientNachricht::Praesenz(P::Handlung {
                rauchen: Some(an),
                golfen: None,
                getraenk: None,
                wirft: None,
            }));
        },
    );

    // Leiter hoch, Stange runter: eine Etage weiter, mit dem Aufzug-Weg (der Server setzt einen ab).
    reg.interaktion(
        "leiter",
        |_, _| Some("E: Leiter hinauf zur Etage darüber".into()),
        |kx: &mut Kontext, _| nachbar_etage(kx, 1),
    );
    reg.interaktion(
        "stange",
        |_, _| Some("E: Feuerwehrstange hinunter".into()),
        |kx: &mut Kontext, _| nachbar_etage(kx, -1),
    );
    reg.interaktion(
        "dj",
        |_, _| Some("E: Lufthorn!".into()),
        |_, _| {
            let (x, z) = crate::grundriss::dach::DJ;
            crate::api::klang(crate::api::Klang::Lufthorn { pos: [x, 1.6, z] });
        },
    );

    reg.interaktion(
        "durchbruch",
        |_, _| Some("🚧 Platz zum Wachsen – E: durch die Wand brechen (2 Tische mehr)".into()),
        |kx: &mut Kontext, _| {
            kx.senden(ClientNachricht::Etagen(
                protokoll::etagen::Client::Erweitern,
            ));
        },
    );

    reg.taste("KeyF", None, |kx: &mut Kontext| {
        match wandplatz(kx) {
            Some(platz) => melden(WeltAktion::BildAufhaengen { platz }),
            None => melden(WeltAktion::Meldung {
                text: "Schau auf eine Wand, um dort ein Bild aufzuhängen.".into(),
            }),
        }
        true
    });
    reg.taste("KeyH", None, |_| {
        melden(WeltAktion::HilfeOeffnen);
        true
    });
}

/// Wo ein Bild hängen würde: der Punkt der Außenwand, auf den man schaut (bis 8 m weit).
fn wandplatz(kx: &Kontext) -> Option<protokoll::spielzeug::DekorPlatz> {
    use crate::grundriss::{BODEN, WANDHOEHE};
    use protokoll::spielzeug::{DekorPlatz, Wand};
    if kx.zustand.auf_dem_dach() {
        return None;
    }
    let a = kx.spieler.auge();
    let r = kx.spieler.blickrichtung();
    let mut bestes: Option<(f32, Wand, f32, f32)> = None;
    let mut pruefe = |t: f32, wand: Wand| {
        if t <= 0.0 || t > 8.0 || bestes.is_some_and(|b| b.0 < t) {
            return;
        }
        let (x, y, z) = (a[0] + r[0] * t, a[1] + r[1] * t, a[2] + r[2] * t);
        let u = match wand {
            Wand::Nord | Wand::Sued => x,
            Wand::Ost | Wand::West => z,
        };
        let innen = BODEN.min_x < x + 0.01
            && x < BODEN.max_x + 0.01
            && BODEN.min_z < z + 0.01
            && z < BODEN.max_z + 0.01;
        if innen && (0.8..WANDHOEHE - 0.8).contains(&y) {
            bestes = Some((t, wand, u, y));
        }
    };
    if r[2].abs() > 1e-4 {
        pruefe((BODEN.min_z - a[2]) / r[2], Wand::Nord);
        pruefe((BODEN.max_z - a[2]) / r[2], Wand::Sued);
    }
    if r[0].abs() > 1e-4 {
        pruefe((BODEN.min_x - a[0]) / r[0], Wand::West);
        pruefe((BODEN.max_x - a[0]) / r[0], Wand::Ost);
    }
    let (_, wand, u, y) = bestes?;
    Some(DekorPlatz {
        url: String::new(),
        titel: None,
        wand,
        u,
        y,
        b: 1.2,
        h: 0.9,
        rahmen: 0,
    })
}

fn nachbar_etage(kx: &mut Kontext, schritt: i32) {
    let i = kx.zustand.etagen_index() as i32 + schritt;
    let ziel = kx
        .zustand
        .etagen
        .get(i.max(0) as usize)
        .filter(|_| i >= 0)
        .map(|e| e.id.clone());
    match ziel {
        Some(id) => crate::features::aufzug::fahren(kx, &id),
        None => melden(WeltAktion::Meldung {
            text: if schritt > 0 {
                "Darüber ist keine Etage mehr.".into()
            } else {
                "Das ist schon die unterste Etage.".into()
            },
        }),
    }
}
