//! Cartoon-Figuren: ein Mensch (Spieler, andere Spieler) oder ein Arbeiter am Tisch.
//! Kugelkopf, Kapselkörper, schwingende Arme und Beine, Frisur, ein Lämpchen über dem Kopf.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;
use protokoll::praesenz::Emote;

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

/// Was eine Figur in der rechten Hand halten kann.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Halt {
    Zigarette,
    Golfschlaeger,
    /// Ein Getränk: Farbe des Inhalts, großes oder kleines Glas.
    Getraenk(&'static str, bool),
    Dart,
    Axt,
    /// Eine Issue-Karte vom Board.
    Karte,
}

/// Was eine Person laut Protokoll gerade in der Hand hat.
pub fn halt_von(p: &protokoll::praesenz::Person) -> Option<Halt> {
    use protokoll::praesenz::{Barspiel, Getraenk as G};
    if let Some(w) = p.wirft {
        return Some(match w {
            Barspiel::Darts => Halt::Dart,
            Barspiel::Axt => Halt::Axt,
        });
    }
    if p.golft {
        return Some(Halt::Golfschlaeger);
    }
    if p.raucht {
        return Some(Halt::Zigarette);
    }
    if let Some(g) = p.getraenk {
        return Some(match g {
            G::Bier => Halt::Getraenk("#f2b134", true),
            G::Wein => Halt::Getraenk("#8e1c3c", false),
            G::Martini => Halt::Getraenk("#e6f0c8", false),
            G::MaiTai => Halt::Getraenk("#ff8c42", true),
            G::Kurzer => Halt::Getraenk("#f7d488", false),
            G::Mojito => Halt::Getraenk("#b7e4a0", true),
            G::Wasser => Halt::Getraenk("#d6f1ff", true),
        });
    }
    p.traegt.as_ref().map(|_| Halt::Karte)
}

/// So lange läuft ein Emote (Sekunden).
pub const EMOTE_DAUER: f32 = 3.0;

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
    /// Ein laufendes Emote und wie lange schon (Sekunden).
    emote: Option<(Emote, f32)>,
    /// Was die rechte Hand hält, und sein Knoten.
    hand: Option<(Halt, SceneNode3d)>,
    /// Bart (in der Halle, wächst mit der Arbeit) und wie verbraucht (0..1).
    bart: Option<(SceneNode3d, f32)>,
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
        Self {
            wurzel,
            koerper,
            kopf,
            arm_l,
            arm_r,
            bein_l,
            bein_r,
            lampe,
            phase: 0.0,
            emote: None,
            hand: None,
            bart: None,
            haltung: Haltung::Stehen,
        }
    }

    /// Ort der Füße und Blickrichtung (0 = +z).
    pub fn setzen(&mut self, x: f32, y: f32, z: f32, rot_y: f32) {
        self.wurzel.set_position(Vec3::new(x, y, z));
        self.wurzel
            .set_rotation(Quat::from_axis_angle(Vec3::Y, rot_y));
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
                self.koerper.set_position(Vec3::new(
                    0.0,
                    1.06 + (self.phase * 2.0).sin().abs() * 0.03,
                    0.0,
                ));
            }
            Haltung::Stehen => {
                self.phase += dt;
                for n in [
                    &mut self.bein_l,
                    &mut self.bein_r,
                    &mut self.arm_l,
                    &mut self.arm_r,
                ] {
                    rot(n, 0.0);
                }
                self.koerper.set_position(Vec3::new(
                    0.0,
                    1.06 + (self.phase * 1.5).sin() * 0.008,
                    0.0,
                ));
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
        let wippen = if self.haltung == Haltung::Tippen {
            (self.phase * 0.3).sin() * 0.05
        } else {
            0.0
        };
        self.kopf
            .set_rotation(Quat::from_axis_angle(Vec3::X, wippen));
        if self.hand.is_some() && self.emote.is_none() && self.haltung != Haltung::Tippen {
            self.arm_r
                .set_rotation(Quat::from_axis_angle(Vec3::X, -0.9));
        }
        self.emote_spielen(dt);
    }

    /// Wie verbraucht die Figur aussieht (0 frisch … 1 ganz): Bart, Farbe, krummer Rücken.
    /// `None` nimmt den Bart ab (im Büro).
    pub fn altern(&mut self, v: Option<f32>) {
        use crate::grundriss::verschleiss::{bartfarbe, bartlaenge, haltung};
        let Some(v) = v else {
            if let Some((mut b, _)) = self.bart.take() {
                b.detach();
            }
            return;
        };
        if self
            .bart
            .as_ref()
            .is_none_or(|(_, alt)| (alt - v).abs() > 0.01)
        {
            if let Some((mut b, _)) = self.bart.take() {
                b.detach();
            }
            let mut g = self.kopf.add_group();
            let l = bartlaenge(v);
            let [r, gg, bb] = bartfarbe(v);
            let mut bart = g.add_cone(0.16, l);
            bart.set_rotation(Quat::from_axis_angle(Vec3::X, PI));
            bart.set_position(Vec3::new(0.0, -0.12 - l / 2.0, 0.12));
            bart.set_color(Color::new(r * r, gg * gg, bb * bb, 1.0));
            self.bart = Some((g, v));
        }
        let (krumm, _) = haltung(v);
        self.koerper
            .set_rotation(Quat::from_axis_angle(Vec3::X, krumm));
        self.kopf
            .set_position(Vec3::new(0.0, 1.62 - krumm * 0.25, krumm * 0.45));
    }

    /// Etwas in die rechte Hand geben (oder nichts). Baut nur neu, wenn es sich ändert.
    pub fn halten(&mut self, h: Option<Halt>) {
        if self.hand.as_ref().map(|(alt, _)| *alt) == h {
            return;
        }
        if let Some((_, mut k)) = self.hand.take() {
            k.detach();
        }
        let Some(h) = h else { return };
        let mut g = self.arm_r.add_group();
        g.set_position(Vec3::new(0.0, -0.6, 0.06));
        match h {
            Halt::Zigarette => {
                let mut z = g.add_cylinder(0.008, 0.08);
                z.set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
                z.set_position(Vec3::new(0.0, 0.0, 0.05));
                z.set_color(farbe("#f8f9fa"));
                let mut glut = g.add_sphere(0.01);
                glut.set_position(Vec3::new(0.0, 0.0, 0.09));
                glut.set_color(farbe("#ff6b35"));
                glut.set_emissive(farbe("#ff6b35"));
            }
            Halt::Golfschlaeger => {
                let mut s = g.add_cylinder(0.012, 0.95);
                s.set_position(Vec3::new(0.0, -0.45, 0.0));
                s.set_color(farbe("#adb5bd"));
                let mut kopf = g.add_cube(0.05, 0.04, 0.12);
                kopf.set_position(Vec3::new(0.0, -0.93, 0.04));
                kopf.set_color(farbe("#343a40"));
            }
            Halt::Getraenk(farbe_glas, gross) => {
                let (r, hh) = if gross { (0.045, 0.16) } else { (0.03, 0.06) };
                let mut glas = g.add_cylinder(r, hh);
                glas.set_position(Vec3::new(0.0, 0.02, 0.05));
                glas.set_color(Color::new(0.85, 0.95, 1.0, 0.5));
                let mut inhalt = g.add_cylinder(r * 0.85, hh * 0.75);
                inhalt.set_position(Vec3::new(0.0, 0.0, 0.05));
                inhalt.set_color(farbe(farbe_glas));
            }
            Halt::Dart => {
                let mut d = g.add_cylinder(0.006, 0.15);
                d.set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
                d.set_position(Vec3::new(0.0, 0.0, 0.06));
                d.set_color(farbe("#ef476f"));
            }
            Halt::Axt => {
                let mut stiel = g.add_cube(0.035, 0.45, 0.035);
                stiel.set_position(Vec3::new(0.0, 0.15, 0.04));
                stiel.set_color(farbe("#8a5a3b"));
                let mut klinge = g.add_cube(0.02, 0.12, 0.14);
                klinge.set_position(Vec3::new(0.0, 0.34, 0.1));
                klinge.set_color(farbe("#ced4da"));
            }
            Halt::Karte => {
                let mut k = g.add_cube(0.16, 0.12, 0.01);
                k.set_position(Vec3::new(0.0, 0.0, 0.08));
                k.set_color(farbe("#ffd166"));
            }
        }
        self.hand = Some((h, g));
    }

    /// Ein Emote beginnen (läuft `EMOTE_DAUER` Sekunden über die Haltung).
    pub fn emote(&mut self, e: Emote) {
        self.emote = Some((e, 0.0));
    }

    /// Arme, Kopf und Körper für das laufende Emote (nach der Haltung, überschreibt sie).
    fn emote_spielen(&mut self, dt: f32) {
        let Some((e, t)) = self.emote.as_mut() else {
            return;
        };
        *t += dt;
        let (e, t) = (*e, *t);
        if t > EMOTE_DAUER {
            self.emote = None;
            return;
        }
        let arm = |n: &mut SceneNode3d, x: f32, z: f32| {
            n.set_rotation(Quat::from_axis_angle(Vec3::Z, z) * Quat::from_axis_angle(Vec3::X, x));
        };
        // Hinein und wieder heraus blenden, damit es nicht ruckt.
        let k = (t / 0.25).min(1.0).min((EMOTE_DAUER - t) / 0.25);
        match e {
            Emote::Winken => {
                arm(&mut self.arm_r, -2.8 * k, (t * 9.0).sin() * 0.45 * k);
            }
            Emote::Daumen => {
                arm(&mut self.arm_r, -1.5 * k, 0.0);
            }
            Emote::Zeigen => {
                arm(&mut self.arm_r, -1.6 * k, 0.15 * k);
            }
            Emote::Klatschen => {
                let auf = (t * 12.0).sin().abs() * 0.35;
                arm(&mut self.arm_l, -1.3 * k, (-0.25 - auf) * k);
                arm(&mut self.arm_r, -1.3 * k, (0.25 + auf) * k);
            }
            Emote::Tanzen => {
                let s = (t * 7.0).sin();
                arm(&mut self.arm_l, -2.6 * k * (0.5 + 0.5 * s), -0.3 * k);
                arm(&mut self.arm_r, -2.6 * k * (0.5 - 0.5 * s), 0.3 * k);
                self.koerper.set_position(Vec3::new(
                    s * 0.06 * k,
                    1.06 + (t * 14.0).sin().abs() * 0.08 * k,
                    0.0,
                ));
                self.kopf
                    .set_rotation(Quat::from_axis_angle(Vec3::Z, s * 0.2 * k));
            }
            Emote::Facepalm => {
                arm(&mut self.arm_r, -2.3 * k, 0.55 * k);
                self.kopf
                    .set_rotation(Quat::from_axis_angle(Vec3::X, 0.35 * k));
            }
        }
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
        0 => teil(
            kopf,
            0.23,
            Vec3::new(0.0, 0.06, -0.02),
            Vec3::new(1.0, 0.7, 1.0),
        ),
        // Pony.
        1 => {
            teil(
                kopf,
                0.23,
                Vec3::new(0.0, 0.07, -0.03),
                Vec3::new(1.02, 0.72, 1.0),
            );
            teil(
                kopf,
                0.12,
                Vec3::new(0.06, 0.13, 0.15),
                Vec3::new(1.4, 0.5, 0.6),
            );
        }
        // Lang, bis auf die Schultern.
        2 => {
            teil(
                kopf,
                0.235,
                Vec3::new(0.0, 0.05, -0.04),
                Vec3::new(1.05, 0.75, 1.0),
            );
            teil(
                kopf,
                0.2,
                Vec3::new(0.0, -0.18, -0.1),
                Vec3::new(1.1, 1.2, 0.6),
            );
        }
        // Dutt.
        3 => {
            teil(
                kopf,
                0.23,
                Vec3::new(0.0, 0.06, -0.02),
                Vec3::new(1.0, 0.7, 1.0),
            );
            teil(kopf, 0.1, Vec3::new(0.0, 0.25, -0.12), Vec3::ONE);
        }
        // Afro.
        4 => teil(
            kopf,
            0.3,
            Vec3::new(0.0, 0.1, -0.03),
            Vec3::new(1.0, 0.85, 1.0),
        ),
        // Irokese.
        5 => teil(
            kopf,
            0.16,
            Vec3::new(0.0, 0.2, -0.02),
            Vec3::new(0.35, 0.8, 1.3),
        ),
        // Glatze mit Haarkranz.
        _ => teil(
            kopf,
            0.225,
            Vec3::new(0.0, -0.02, -0.04),
            Vec3::new(1.02, 0.5, 1.0),
        ),
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
