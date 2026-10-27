//! Reine Rechnungen der Spielsachen (ohne 3D): wo der Hund auf seinem Weg ist, wie ein geworfener
//! Basketball fliegt und springt, wie ein Golfball fliegt. Jede Seite rechnet aus demselben Wurf
//! dasselbe aus, darum sieht jeder auf der Etage denselben Ball.

use crate::grundriss::draussen::{BALL_RADIUS, RING, SCHWERKRAFT};
use crate::grundriss::{BODEN, WANDHOEHE};

/// Fellfarben: Körper, Bauch und Schnauze, Ohren (Index `Hund::fell`).
pub const FELLE: [[&str; 3]; 6] = [
    ["#e0a458", "#fff1d6", "#b36f35"],
    ["#3b3d4f", "#ffffff", "#23242f"],
    ["#8a5a3b", "#f0d2b0", "#5e3a24"],
    ["#f3dcb0", "#fffaf0", "#d9a066"],
    ["#a4acb6", "#f4f6f8", "#6f7884"],
    ["#cf6a45", "#fbe1d2", "#9c4527"],
];

/// Wo der Hund auf seinem Wegstück ist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HundLage {
    pub x: f32,
    pub z: f32,
    pub blick: f32,
    pub unterwegs: bool,
}

/// Der Hund `t` Sekunden nach Beginn seines Wegstücks (wie `dogAt` im Original).
pub fn hund_bei(weg: &[[f32; 2]], tempo: f32, blick_am_ziel: Option<f32>, t: f32) -> HundLage {
    let mut rest = t.max(0.0) * tempo;
    let mut blick = blick_am_ziel.unwrap_or(0.0);
    for w in weg.windows(2) {
        let (dx, dz) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let laenge = dx.hypot(dz);
        if laenge < 1e-6 {
            continue;
        }
        blick = dx.atan2(dz);
        if rest < laenge {
            let k = rest / laenge;
            return HundLage { x: w[0][0] + dx * k, z: w[0][1] + dz * k, blick, unterwegs: true };
        }
        rest -= laenge;
    }
    let [x, z] = weg.last().copied().unwrap_or([0.0, 0.0]);
    HundLage { x, z, blick: blick_am_ziel.unwrap_or(blick), unterwegs: false }
}

/// Ein fliegender oder rollender Basketball.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ball {
    pub p: [f32; 3],
    pub v: [f32; 3],
    /// Ist durch den Ring gefallen (einmal pro Wurf).
    pub treffer: bool,
}

/// Was beim Schritt passiert ist (für Klänge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BallEreignis {
    pub aufprall: bool,
    pub ring: bool,
    pub korb: bool,
}

pub const BALL_SCHRITT: f32 = 1.0 / 120.0;
const SPRUNG: f32 = 0.72;
const REIBUNG: f32 = 0.985;

impl Ball {
    pub fn neu(p: [f32; 3], v: [f32; 3]) -> Self {
        Self { p, v, treffer: false }
    }

    /// Ob er liegt (kaum noch Bewegung am Boden).
    pub fn ruht(&self) -> bool {
        self.p[1] <= BALL_RADIUS + 0.001 && self.v.iter().all(|v| v.abs() < 0.05)
    }

    /// Ein fester Zeitschritt: Schwerkraft, Boden, Wände, Decke, Brett und Ring.
    pub fn schritt(&mut self) -> BallEreignis {
        let mut e = BallEreignis::default();
        let dt = BALL_SCHRITT;
        let vorher = self.p;
        self.v[1] -= SCHWERKRAFT * dt;
        for i in 0..3 {
            self.p[i] += self.v[i] * dt;
        }
        let r = BALL_RADIUS;
        // Boden und Decke.
        if self.p[1] < r {
            self.p[1] = r;
            if self.v[1] < -0.8 {
                e.aufprall = true;
            }
            self.v[1] = -self.v[1] * SPRUNG;
            if self.v[1] < 0.3 {
                self.v[1] = 0.0;
            }
            self.v[0] *= REIBUNG;
            self.v[2] *= REIBUNG;
        }
        if self.p[1] > WANDHOEHE - r {
            self.p[1] = WANDHOEHE - r;
            self.v[1] = -self.v[1].abs() * SPRUNG;
        }
        // Wände des Raums.
        for (achse, min, max) in [(0, BODEN.min_x, BODEN.max_x), (2, BODEN.min_z, BODEN.max_z)] {
            if self.p[achse] < min + r {
                self.p[achse] = min + r;
                self.v[achse] = self.v[achse].abs() * SPRUNG;
                e.aufprall = true;
            } else if self.p[achse] > max - r {
                self.p[achse] = max - r;
                self.v[achse] = -self.v[achse].abs() * SPRUNG;
                e.aufprall = true;
            }
        }
        // Das Brett (eine Platte vor der Westwand).
        let brett_x = crate::grundriss::draussen::KORB_BRETT_X;
        if self.p[0] < brett_x + 0.05 + r && vorher[0] >= brett_x + 0.05 + r && (2.88..3.93).contains(&self.p[1]) && (self.p[2] - RING.2).abs() < 0.7 {
            self.p[0] = brett_x + 0.05 + r;
            self.v[0] = self.v[0].abs() * SPRUNG;
            e.aufprall = true;
        }
        // Ring: Treffer, wenn er von oben durch die Ringebene fällt, innen; sonst prallt er am Rand ab.
        let (rx, ry, rz, rr) = RING;
        let d = (self.p[0] - rx).hypot(self.p[2] - rz);
        if vorher[1] >= ry && self.p[1] < ry {
            if d < rr - r * 0.6 && !self.treffer {
                self.treffer = true;
                e.korb = true;
            } else if (d - rr).abs() < r {
                let nx = (self.p[0] - rx) / d.max(1e-4);
                let nz = (self.p[2] - rz) / d.max(1e-4);
                self.v[0] = nx * self.v[0].abs().max(1.0) * 0.6;
                self.v[2] = nz * self.v[2].abs().max(1.0) * 0.6;
                self.v[1] = self.v[1].abs() * 0.4;
                e.ring = true;
            }
        }
        e
    }

    /// Der Ball nach `sekunden`, in festen Schritten (alle rechnen gleich).
    pub fn nach(start: [f32; 3], v: [f32; 3], sekunden: f32) -> Self {
        let mut b = Self::neu(start, v);
        let n = (sekunden.max(0.0) / BALL_SCHRITT) as usize;
        for _ in 0..n.min(120 * 30) {
            b.schritt();
            if b.ruht() {
                break;
            }
        }
        b
    }
}

/// Wo ein Golfball `t` Sekunden nach dem Abschlag ist: Abschlag vom Balkon (Höhe 0) hinunter zur
/// Straße (`boden`, negativ). Gier um y (0 = +z), Neigung nach oben, Kraft 0..1. `None`, wenn er liegt.
pub fn golfflug(start: [f32; 3], gier: f32, neigung: f32, kraft: f32, boden: f32, t: f32) -> ([f32; 3], bool) {
    let tempo = 12.0 + kraft.clamp(0.0, 1.0) * 30.0;
    let (vx, vy, vz) = (gier.sin() * neigung.cos() * tempo, neigung.sin() * tempo, gier.cos() * neigung.cos() * tempo);
    // Wann er unten aufkommt (y = boden).
    let a = -0.5 * SCHWERKRAFT;
    let (b, c) = (vy, start[1] - boden);
    let landung = (-b - (b * b - 4.0 * a * c).sqrt()) / (2.0 * a);
    if t < landung {
        return ([start[0] + vx * t, start[1] + vy * t + a * t * t, start[2] + vz * t], true);
    }
    // Danach rollt er aus.
    let roll = ((t - landung) * 1.5).min(1.0);
    let weit = 0.25 * roll;
    ([start[0] + vx * (landung + weit), boden + 0.03, start[2] + vz * (landung + weit)], false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hund_laeuft_den_weg_ab() {
        let weg = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]];
        let a = hund_bei(&weg, 1.0, Some(1.0), 2.0);
        assert!((a.x - 2.0).abs() < 1e-5 && a.unterwegs);
        let b = hund_bei(&weg, 1.0, Some(1.0), 5.0);
        assert!((b.z - 1.0).abs() < 1e-5 && (b.x - 4.0).abs() < 1e-5);
        let c = hund_bei(&weg, 1.0, Some(1.0), 99.0);
        assert!(!c.unterwegs && c.blick == 1.0 && c.z == 3.0);
        assert!(!hund_bei(&[[1.0, 2.0]], 1.0, None, 3.0).unterwegs);
    }

    #[test]
    fn ball_kommt_zur_ruhe_und_bleibt_im_raum() {
        let b = Ball::nach([0.0, 2.0, 0.0], [5.0, 4.0, 3.0], 30.0);
        assert!(b.ruht(), "{b:?}");
        assert!(BODEN.enthaelt(b.p[0], b.p[2]));
    }

    #[test]
    fn senkrecht_in_den_ring_ist_ein_korb() {
        let (rx, ry, rz, _) = RING;
        let mut b = Ball::neu([rx, ry + 1.0, rz], [0.0, 0.0, 0.0]);
        let mut korb = false;
        for _ in 0..240 {
            korb |= b.schritt().korb;
        }
        assert!(korb);
    }

    #[test]
    fn golfball_landet_auf_der_strasse() {
        let (p, fliegt) = golfflug([0.0, 0.0, 15.0], 0.0, 0.6, 1.0, -3.6, 0.5);
        assert!(fliegt && p[2] > 15.0 && p[1] > 0.0);
        let (q, fliegt) = golfflug([0.0, 0.0, 15.0], 0.0, 0.6, 1.0, -3.6, 30.0);
        assert!(!fliegt && (q[1] + 3.57).abs() < 0.01);
    }
}
