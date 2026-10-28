//! Laufen und Umsehen: WASD/Pfeile, Shift rennt, Leertaste springt, Maus dreht (bei gefangenem
//! Zeiger), Mausrad zoomt die Kamera hinter den Kopf. Schickt die eigene Position an den Server.

use protokoll::ClientNachricht;
use protokoll::praesenz::Client as P;

use crate::bewegung::{AUGENHOEHE, Steuerung};
use crate::register::{Phase, Register};
use crate::welt::Kontext;

/// Wie stark die Maus dreht (Bogenmaß pro Pixel).
const EMPFINDLICHKEIT: f32 = 0.0022;
/// So oft geht die Position an den Server, solange man sich bewegt.
const SENDE_TAKT: f32 = 0.1;

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.tick(Phase::Lenken, |kx: &mut Kontext, _| {
        let e = &kx.eingabe;
        if e.gefangen && !e.gesperrt {
            let (dx, dy) = e.maus;
            kx.spieler.umsehen(dx, dy, EMPFINDLICHKEIT);
        }
        kx.kamera.zoomen(e.rad);
    });

    reg.tick(Phase::Bewegen, |kx: &mut Kontext, dt| {
        if kx.spieler_fest {
            kx.spieler.laeuft = false;
            return;
        }
        let g = &kx.eingabe.gehalten;
        let an = |a: &str, b: &str| g.contains(a) || g.contains(b);
        let s = if kx.eingabe.gesperrt {
            Steuerung::default()
        } else {
            Steuerung {
                vor: an("KeyW", "ArrowUp") as i32 as f32 - an("KeyS", "ArrowDown") as i32 as f32,
                seit: an("KeyD", "ArrowRight") as i32 as f32
                    - an("KeyA", "ArrowLeft") as i32 as f32,
                rennen: an("ShiftLeft", "ShiftRight"),
                springen: g.contains("Space"),
            }
        };
        let kaesten = kx.kaesten();
        let w = kx.wirkung;
        kx.spieler.schritt(s, w, &kaesten, dt);
    });

    // Position senden: oft genug, solange man läuft, und einmal, wenn man stehen bleibt.
    let mut seit = 0.0f32;
    let mut zuletzt: Option<[f32; 4]> = None;
    reg.tick(Phase::Ich, move |kx: &mut Kontext, dt| {
        seit += dt;
        let p = kx.spieler.pos();
        let jetzt = [p[0], p[1], p[2], kx.spieler.blick];
        let anders = zuletzt.is_none_or(|z| {
            z.iter()
                .zip(jetzt.iter())
                .any(|(a, b)| (a - b).abs() > 0.01)
        });
        if anders && seit >= SENDE_TAKT {
            seit = 0.0;
            zuletzt = Some(jetzt);
            kx.senden(ClientNachricht::Praesenz(P::Bewegen {
                x: p[0],
                y: p[1],
                z: p[2],
                rot_y: kx.spieler.blick,
                laeuft: kx.spieler.laeuft,
            }));
        }
    });

    // Kamera ans Auge (oder dahinter), ohne durch Wände zu schauen.
    reg.tick(Phase::Zielen, |kx: &mut Kontext, _| {
        let auge = kx.spieler.auge();
        let blick = kx.spieler.blickrichtung();
        let kaesten = kx.kaesten();
        let ziel = kx.kamera_ziel.take();
        let (kopf, richtung) = ziel.unwrap_or((auge, blick));
        match ziel {
            Some((a, b)) => kx.kamera.setzen(a, b, |_, _| f32::INFINITY),
            None => kx
                .kamera
                .setzen(auge, blick, |von, richtung| strahl(&kaesten, von, richtung)),
        }
        let umgebung = umgebung(kx.zustand.auf_dem_dach(), kx.spieler.pos());
        crate::api::hoerer_melden(crate::api::Hoerer {
            kopf,
            blick: richtung,
            umgebung,
        });
        let _ = AUGENHOEHE;
    });
}

/// Drinnen, in der Garage, draußen oder auf dem Dach.
pub fn umgebung(dach: bool, p: [f32; 3]) -> crate::api::Umgebung {
    use crate::api::Umgebung as U;
    use crate::grundriss::{BODEN, PLATTE, draussen::GEBAEUDE, im_fluegel};
    if dach {
        return U::Dach;
    }
    if p[1] < -PLATTE - 0.5 {
        return if GEBAEUDE.enthaelt(p[0], p[2]) {
            U::Garage
        } else {
            U::Draussen
        };
    }
    if BODEN.enthaelt(p[0], p[2]) || im_fluegel(p[0], p[2], crate::grundriss::FLUEGEL_REIHEN) {
        U::Drinnen
    } else {
        U::Draussen
    }
}

/// Wie weit ein Strahl von `von` in `richtung` frei ist (für die Kamera hinter dem Kopf).
fn strahl(kaesten: &[crate::kollision::Kasten], von: [f32; 3], richtung: [f32; 3]) -> f32 {
    let mut t = 0.0;
    while t < 6.0 {
        t += 0.15;
        let p = [
            von[0] + richtung[0] * t,
            von[1] + richtung[1] * t,
            von[2] + richtung[2] * t,
        ];
        let drin = kaesten.iter().any(|k| {
            let unten = if k.unten.is_finite() {
                k.unten
            } else {
                -1000.0
            };
            !k.zaun
                && p[0] > k.min_x
                && p[0] < k.max_x
                && p[2] > k.min_z
                && p[2] < k.max_z
                && p[1] < k.oben
                && p[1] > unten
        });
        if drin {
            return t;
        }
    }
    t
}
