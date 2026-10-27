//! Cartoon-Figuren: ein Mensch (Spieler, andere Spieler) oder ein Arbeiter am Tisch.
//! Kugelkopf, Kapselkörper, schwingende Arme und Beine, Frisur, ein Lämpchen über dem Kopf.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;

use crate::bau::{farbe, toenen};
use crate::farben;

/// Wie eine Figur gerade dasteht.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Haltung {
    Stehen,
    Laufen,
    /// Sitzen mit Hüfte so hoch über den Füßen.
    Sitzen(f32),
    /// Am Tisch tippen (sitzend).
    Tippen,
}

pub struct Figur {
    pub wurzel: SceneNode3d,
    koerper: SceneNode3d,
    kopf: SceneNode3d,
    arm_l: SceneNode3d,
    arm_r: SceneNode3d,
    bein_l: SceneNode3d,
    bein_r: SceneNode3d,
    lampe: SceneNode3d,
    phase: f32,
    pub haltung: Haltung,
}

/// Aussehen einer Figur.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub hemd: String,
    pub haut: u8,
    pub haare: u8,
    pub frisur: u8,
}

impl Figur {
    pub fn neu(eltern: &mut SceneNode3d, look: &Look) -> Self {
        let mut wurzel = eltern.add_group();
        let hemd = farbe(&look.hemd);
        let haut = farbe(farben::HAUT[look.haut as usize % farben::HAUT.len()]);
        let haare = farbe(farben::HAARE[look.haare as usize % farben::HAARE.len()]);
        let hose = farbe("#3d405b");

        // Beine und Arme hängen an Gelenk-Gruppen, damit sie um Hüfte/Schulter schwingen.
        let glied = |w: &mut SceneNode3d, x: f32, y: f32, laenge: f32, r: f32, c: Color| {
            let mut g = w.add_group();
            g.set_position(Vec3::new(x, y, 0.0));
            let mut k = g.add_capsule(r, laenge);
            k.set_position(Vec3::new(0.0, -laenge / 2.0 - r * 0.5, 0.0));
            k.set_color(c);
            k.set_roughness(0.8);
            g
        };
        let bein_l = glied(&mut wurzel, -0.11, 0.78, 0.5, 0.075, hose);
        let bein_r = glied(&mut wurzel, 0.11, 0.78, 0.5, 0.075, hose);
        let schuhe = |b: &SceneNode3d| {
            let mut s = b.clone().add_cube(0.14, 0.08, 0.22);
            s.set_position(Vec3::new(0.0, -0.74, 0.04));
            s.set_color(farbe("#2b2d42"));
        };
        schuhe(&bein_l);
        schuhe(&bein_r);
        let mut koerper = wurzel.add_capsule(0.21, 0.42);
        koerper.set_position(Vec3::new(0.0, 1.06, 0.0));
        koerper.set_color(hemd);
        koerper.set_roughness(0.8);
        let arm_l = glied(&mut wurzel, -0.29, 1.32, 0.42, 0.06, hemd);
        let arm_r = glied(&mut wurzel, 0.29, 1.32, 0.42, 0.06, hemd);
        for a in [&arm_l, &arm_r] {
            let mut hand = a.clone().add_sphere(0.065);
            hand.set_position(Vec3::new(0.0, -0.58, 0.0));
            hand.set_color(haut);
        }
        let mut kopf = wurzel.add_group();
        kopf.set_position(Vec3::new(0.0, 1.62, 0.0));
        let mut schaedel = kopf.add_sphere(0.22);
        schaedel.set_color(haut);
        schaedel.set_roughness(0.7);
        // Augen nach vorn (+z).
        for x in [-0.075, 0.075] {
            let mut auge = kopf.add_sphere(0.03);
            auge.set_position(Vec3::new(x, 0.03, 0.2));
            auge.set_color(farbe("#1b1b1b"));
        }
        frisur(&mut kopf, look.frisur, haare);
        let mut lampe = wurzel.add_sphere(0.09);
        lampe.set_position(Vec3::new(0.0, 2.08, 0.0));
        lampe.set_visible(false);
        Self { wurzel, koerper, kopf, arm_l, arm_r, bein_l, bein_r, lampe, phase: 0.0, haltung: Haltung::Stehen }
    }

    /// Ort der Füße und Blickrichtung (0 = +z).
    pub fn setzen(&mut self, x: f32, y: f32, z: f32, rot_y: f32) {
        self.wurzel.set_position(Vec3::new(x, y, z));
        self.wurzel.set_rotation(Quat::from_axis_angle(Vec3::Y, rot_y));
    }

    pub fn sichtbar(&mut self, an: bool) {
        self.wurzel.set_visible(an);
    }

    /// Das Lämpchen über dem Kopf: Farbe oder aus.
    pub fn lampe(&mut self, c: Option<&str>) {
        match c {
            Some(c) => {
                self.lampe.set_visible(true);
                self.lampe.set_color(farbe(c));
                self.lampe.set_emissive(toenen(farbe(c), -0.2));
            }
            None => {
                self.lampe.set_visible(false);
            }
        }
    }

    /// Ein Schritt der Animation.
    pub fn animieren(&mut self, dt: f32) {
        let rot = |n: &mut SceneNode3d, w: f32| {
            n.set_rotation(Quat::from_axis_angle(Vec3::X, w));
        };
        match self.haltung {
            Haltung::Laufen => {
                self.phase += dt * 9.0;
                let s = self.phase.sin() * 0.7;
                rot(&mut self.bein_l, s);
                rot(&mut self.bein_r, -s);
                rot(&mut self.arm_l, -s * 0.8);
                rot(&mut self.arm_r, s * 0.8);
                self.koerper.set_position(Vec3::new(0.0, 1.06 + (self.phase * 2.0).sin().abs() * 0.03, 0.0));
            }
            Haltung::Stehen => {
                self.phase += dt;
                for n in [&mut self.bein_l, &mut self.bein_r, &mut self.arm_l, &mut self.arm_r] {
                    rot(n, 0.0);
                }
                self.koerper.set_position(Vec3::new(0.0, 1.06 + (self.phase * 1.5).sin() * 0.008, 0.0));
            }
            Haltung::Sitzen(_) | Haltung::Tippen => {
                self.phase += dt * 14.0;
                rot(&mut self.bein_l, -FRAC_PI_2);
                rot(&mut self.bein_r, -FRAC_PI_2);
                if self.haltung == Haltung::Tippen {
                    rot(&mut self.arm_l, -1.1 + self.phase.sin() * 0.08);
                    rot(&mut self.arm_r, -1.1 - self.phase.sin() * 0.08);
                } else {
                    rot(&mut self.arm_l, -0.3);
                    rot(&mut self.arm_r, -0.3);
                }
                self.koerper.set_position(Vec3::new(0.0, 1.06, 0.0));
            }
        }
        let wippen = if self.haltung == Haltung::Tippen { (self.phase * 0.3).sin() * 0.05 } else { 0.0 };
        self.kopf.set_rotation(Quat::from_axis_angle(Vec3::X, wippen));
    }

    /// Wie weit die Füße beim Sitzen absinken: die Figur ist so gebaut, dass die Hüfte bei 0.78 liegt.
    pub fn sitz_versatz(huefte: f32) -> f32 {
        huefte - 0.78
    }

    pub fn entfernen(&mut self) {
        self.wurzel.detach();
    }

    /// Kopfposition in der Welt (für Namensschilder).
    pub fn kopf_welt(&self) -> Vec3 {
        self.wurzel.position() + Vec3::new(0.0, 2.15, 0.0)
    }
}

fn frisur(kopf: &mut SceneNode3d, art: u8, c: Color) {
    let teil = |k: &mut SceneNode3d, r: f32, pos: Vec3, skala: Vec3| {
        let mut s = k.add_sphere(r);
        s.set_position(pos);
        s.set_local_scale(skala.x, skala.y, skala.z);
        s.set_color(c);
        s.set_roughness(0.9);
    };
    match art % 7 {
        // Kurz: eine Kappe.
        0 => teil(kopf, 0.23, Vec3::new(0.0, 0.06, -0.02), Vec3::new(1.0, 0.7, 1.0)),
        // Pony.
        1 => {
            teil(kopf, 0.23, Vec3::new(0.0, 0.07, -0.03), Vec3::new(1.02, 0.72, 1.0));
            teil(kopf, 0.12, Vec3::new(0.06, 0.13, 0.15), Vec3::new(1.4, 0.5, 0.6));
        }
        // Lang, bis auf die Schultern.
        2 => {
            teil(kopf, 0.235, Vec3::new(0.0, 0.05, -0.04), Vec3::new(1.05, 0.75, 1.0));
            teil(kopf, 0.2, Vec3::new(0.0, -0.18, -0.1), Vec3::new(1.1, 1.2, 0.6));
        }
        // Dutt.
        3 => {
            teil(kopf, 0.23, Vec3::new(0.0, 0.06, -0.02), Vec3::new(1.0, 0.7, 1.0));
            teil(kopf, 0.1, Vec3::new(0.0, 0.25, -0.12), Vec3::ONE);
        }
        // Afro.
        4 => teil(kopf, 0.3, Vec3::new(0.0, 0.1, -0.03), Vec3::new(1.0, 0.85, 1.0)),
        // Irokese.
        5 => teil(kopf, 0.16, Vec3::new(0.0, 0.2, -0.02), Vec3::new(0.35, 0.8, 1.3)),
        // Glatze mit Haarkranz.
        _ => teil(kopf, 0.225, Vec3::new(0.0, -0.02, -0.04), Vec3::new(1.02, 0.5, 1.0)),
    }
}

/// Ein Winkel weich zu einem anderen hin (kürzester Weg).
pub fn winkel_hin(von: f32, nach: f32, t: f32) -> f32 {
    let mut d = nach - von;
    while d > PI {
        d -= 2.0 * PI;
    }
    while d < -PI {
        d += 2.0 * PI;
    }
    von + d * t.clamp(0.0, 1.0)
}
