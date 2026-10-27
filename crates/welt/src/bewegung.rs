//! Laufen, Rennen, Springen und Fallen des eigenen Spielers (wie `player/index.ts`), ohne 3D.

use std::f32::consts::PI;

use crate::kollision::{self, GROESSE, Kasten, Koerper, STUFE};

pub const GEHEN: f32 = 4.6;
pub const RENNEN: f32 = 7.5;
pub const SPRUNG: f32 = 6.4;
pub const SCHWERKRAFT: f32 = 18.0;
/// Augenhöhe über den Füßen in der Ich-Ansicht.
pub const AUGENHOEHE: f32 = 1.4;

/// Was die Tasten gerade wollen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Steuerung {
    /// -1 zurück … +1 vor.
    pub vor: f32,
    /// -1 links … +1 rechts.
    pub seit: f32,
    pub rennen: bool,
    pub springen: bool,
}

/// Wirkungen auf die Beine (Kaffee macht schneller und lässt höher springen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wirkung {
    pub tempo: f32,
    pub sprung: f32,
}

impl Default for Wirkung {
    fn default() -> Self {
        Self { tempo: 1.0, sprung: 1.0 }
    }
}

/// Der eigene Spieler.
#[derive(Debug, Clone, PartialEq)]
pub struct Spieler {
    pub koerper: Koerper,
    pub vy: f32,
    /// Blick der Kamera um y (0 = Kamera schaut nach -z, also wie das Original: Kamera steht bei +sin/+cos).
    pub gier: f32,
    /// Neigung des Blicks (+ nach oben).
    pub neigung: f32,
    /// Wohin die Figur schaut (0 = +z).
    pub blick: f32,
    /// Ob er gerade läuft (für die Animation und das Protokoll).
    pub laeuft: bool,
    /// Tiefster Boden überhaupt (die Straße).
    pub strasse: f32,
}

impl Spieler {
    pub fn neu(x: f32, y: f32, z: f32, blick: f32, strasse: f32) -> Self {
        Self {
            koerper: Koerper { pos: [x, y, z], am_boden: true, stufen_versatz: 0.0 },
            vy: 0.0,
            gier: blick + PI,
            neigung: 0.0,
            blick,
            laeuft: false,
            strasse,
        }
    }

    pub fn pos(&self) -> [f32; 3] {
        self.koerper.pos
    }

    /// Setzt den Spieler an einen Ort (Etagenwechsel, Aufstehen …).
    pub fn versetzen(&mut self, x: f32, y: f32, z: f32, blick: f32) {
        self.koerper.pos = [x, y, z];
        self.koerper.stufen_versatz = 0.0;
        self.vy = 0.0;
        self.blick = blick;
        self.gier = blick + PI;
    }

    /// Mausbewegung in Pixeln: dreht den Blick.
    pub fn umsehen(&mut self, dx: f32, dy: f32, empfindlichkeit: f32) {
        self.gier -= dx * empfindlichkeit;
        self.neigung = (self.neigung - dy * empfindlichkeit).clamp(-1.4, 1.4);
        self.gier = winkel(self.gier);
    }

    /// Ein Schritt der Physik.
    pub fn schritt(&mut self, s: Steuerung, w: Wirkung, kaesten: &[Kasten], dt: f32) {
        let dt = dt.min(0.1);
        let (mut ix, mut iz) = (s.seit, -s.vor);
        let lenkt = ix != 0.0 || iz != 0.0;
        // In der Ich-Ansicht schaut die Figur, wohin die Kamera schaut.
        self.blick = winkel(self.gier + PI);
        if lenkt {
            let laenge = ix.hypot(iz);
            ix /= laenge;
            iz /= laenge;
            let (sin, cos) = self.gier.sin_cos();
            let dx = ix * cos + iz * sin;
            let dz = -ix * sin + iz * cos;
            let tempo = if s.rennen { RENNEN } else { GEHEN } * w.tempo;
            let (x, z) = (self.koerper.pos[0], self.koerper.pos[2]);
            kollision::schritt(&mut self.koerper, kaesten, x + dx * tempo * dt, z);
            let x = self.koerper.pos[0];
            kollision::schritt(&mut self.koerper, kaesten, x, z + dz * tempo * dt);
        }

        let p = &mut self.koerper.pos;
        let boden = kollision::boden(kaesten, p[0], p[2], p[1]).max(self.strasse);
        if s.springen && self.koerper.am_boden {
            self.vy = SPRUNG * w.sprung;
            self.koerper.am_boden = false;
        } else if self.koerper.am_boden && p[1] > boden && p[1] - boden <= STUFE + 0.02 {
            // Eine Stufe hinunter: auf den Füßen bleiben statt zu fallen.
            self.koerper.stufen_versatz += p[1] - boden;
            p[1] = boden;
        }
        self.vy -= SCHWERKRAFT * dt;
        p[1] += self.vy * dt;
        if p[1] <= boden {
            p[1] = boden;
            self.vy = 0.0;
            self.koerper.am_boden = true;
        } else if p[1] > boden + 0.02 {
            self.koerper.am_boden = false;
        }
        let decke = kollision::decke(kaesten, p[0], p[2], p[1]);
        if p[1] + GROESSE > decke {
            p[1] = boden.max(decke - GROESSE);
            self.vy = self.vy.min(0.0);
        }
        self.koerper.stufen_versatz *= (-dt * 16.0).exp();
        self.laeuft = lenkt && self.koerper.am_boden;
    }

    /// Wo die Augen sind.
    pub fn auge(&self) -> [f32; 3] {
        let p = self.koerper.pos;
        [p[0], p[1] + AUGENHOEHE + self.koerper.stufen_versatz, p[2]]
    }

    /// Wohin man schaut (Einheitsvektor).
    pub fn blickrichtung(&self) -> [f32; 3] {
        let c = self.neigung.cos();
        [-self.gier.sin() * c, self.neigung.sin(), -self.gier.cos() * c]
    }
}

/// Ein Winkel in -π..π.
pub fn winkel(w: f32) -> f32 {
    w.sin().atan2(w.cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boden() -> Vec<Kasten> {
        vec![Kasten::neu(-50.0, 50.0, -50.0, 50.0, 0.0)]
    }

    #[test]
    fn w_geht_in_blickrichtung() {
        let mut s = Spieler::neu(0.0, 0.0, 0.0, PI, -10.0); // Blick nach -z
        let vor = s.blickrichtung();
        for _ in 0..30 {
            s.schritt(Steuerung { vor: 1.0, ..Default::default() }, Wirkung::default(), &boden(), 1.0 / 30.0);
        }
        let p = s.pos();
        assert!(p[2] < -4.0, "{p:?}");
        assert!(p[0].abs() < 1e-3);
        assert!(vor[2] < -0.99);
        assert!(s.laeuft);
    }

    #[test]
    fn springen_und_landen() {
        let mut s = Spieler::neu(0.0, 0.0, 0.0, 0.0, -10.0);
        s.schritt(Steuerung { springen: true, ..Default::default() }, Wirkung::default(), &boden(), 0.05);
        assert!(s.pos()[1] > 0.0 && !s.koerper.am_boden);
        for _ in 0..100 {
            s.schritt(Steuerung::default(), Wirkung::default(), &boden(), 0.02);
        }
        assert_eq!(s.pos()[1], 0.0);
        assert!(s.koerper.am_boden);
    }

    #[test]
    fn faellt_bis_zur_strasse() {
        let mut s = Spieler::neu(0.0, 5.0, 0.0, 0.0, -3.6);
        for _ in 0..200 {
            s.schritt(Steuerung::default(), Wirkung::default(), &[], 0.02);
        }
        assert_eq!(s.pos()[1], -3.6);
    }

    #[test]
    fn umsehen_klemmt_die_neigung() {
        let mut s = Spieler::neu(0.0, 0.0, 0.0, 0.0, 0.0);
        s.umsehen(0.0, -100000.0, 0.002);
        assert!(s.neigung <= 1.4);
    }
}
