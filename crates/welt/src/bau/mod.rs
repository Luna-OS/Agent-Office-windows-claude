// Bau-Hilfen nehmen Ort und Größe als einzelne Zahlen (x, y, z, Breite, Höhe, Tiefe, Farbe):
// so lesen sich die Bauanleitungen wie Maße auf einem Plan.
#![allow(clippy::too_many_arguments)]
//! Der Baukasten: einfache Formen in Cartoon-Farben, und was beim Bauen nebenbei anfällt
//! (Kollisionskästen, benutzbare Dinge, benannte Teile, die sich später ändern).

pub mod aussen;
pub mod buero;
pub mod dach;
pub mod einrichtung;
pub mod huelle;
pub mod himmel;

use std::collections::HashMap;

use kiss3d::prelude::*;

use crate::kollision::Kasten;
use crate::register::Nutzbar;

/// Eine Farbe aus "#rrggbb" (ungültig: Magenta, damit es auffällt). Die Hex-Werte sind sRGB wie im
/// Original; kiss3d rechnet linear, darum wird umgerechnet.
pub fn farbe(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let k = |i: usize| linear(u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255) as f32 / 255.0);
    if h.len() != 6 {
        return Color::new(1.0, 0.0, 1.0, 1.0);
    }
    Color::new(k(0), k(2), k(4), 1.0)
}

/// Eine Farbe aus "#rrggbb" ohne Umrechnung (für Text, den kiss3d direkt zeichnet).
pub fn farbe_srgb(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let k = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255) as f32 / 255.0;
    if h.len() != 6 {
        return Color::new(1.0, 0.0, 1.0, 1.0);
    }
    Color::new(k(0), k(2), k(4), 1.0)
}

fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Eine Farbe heller (t > 0) oder dunkler (t < 0) machen.
pub fn toenen(c: Color, t: f32) -> Color {
    let f = |v: f32| if t >= 0.0 { v + (1.0 - v) * t } else { v * (1.0 + t) };
    Color::new(f(c.r), f(c.g), f(c.b), c.a)
}

/// Sammelt beim Bauen ein, was die Welt danach braucht.
#[derive(Default)]
pub struct Gebaut {
    pub kaesten: Vec<Kasten>,
    pub nutzbar: Vec<Nutzbar>,
    /// Teile, die sich zur Laufzeit ändern (Bildschirme, Tafeln, Türen …), nach Namen.
    pub teile: HashMap<String, SceneNode3d>,
}

/// Baut in einen Knoten hinein.
pub struct Bau<'a> {
    pub knoten: SceneNode3d,
    pub gebaut: &'a mut Gebaut,
    /// Wo der Ursprung dieser Gruppe in der Welt liegt, und wie sie um y gedreht ist.
    ursprung: [f32; 3],
    drehung: f32,
}

impl<'a> Bau<'a> {
    pub fn neu(knoten: SceneNode3d, gebaut: &'a mut Gebaut) -> Self {
        Self { knoten, gebaut, ursprung: [0.0; 3], drehung: 0.0 }
    }

    /// Eine Untergruppe, verschoben und um y gedreht.
    pub fn gruppe(&mut self, x: f32, y: f32, z: f32, rot_y: f32) -> Bau<'_> {
        let mut g = self.knoten.add_group();
        g.set_position(Vec3::new(x, y, z));
        if rot_y != 0.0 {
            g.set_rotation(Quat::from_axis_angle(Vec3::Y, rot_y));
        }
        let [wx, wy, wz] = self.welt(x, y, z);
        Bau { knoten: g, gebaut: self.gebaut, ursprung: [wx, wy, wz], drehung: self.drehung + rot_y }
    }

    /// Ein Punkt dieser Gruppe in Weltkoordinaten.
    pub fn welt(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let (s, c) = self.drehung.sin_cos();
        [self.ursprung[0] + x * c + z * s, self.ursprung[1] + y, self.ursprung[2] - x * s + z * c]
    }

    /// Ein Quader mit Mitte (x, y, z) und Größe (b, h, t).
    pub fn quader(&mut self, x: f32, y: f32, z: f32, b: f32, h: f32, t: f32, c: Color) -> SceneNode3d {
        let mut n = self.knoten.add_cube(b.max(0.001), h.max(0.001), t.max(0.001));
        n.set_position(Vec3::new(x, y, z));
        n.set_color(c);
        n.set_roughness(0.85);
        n
    }

    /// Ein Quader, der auf dem Boden steht: Grundfläche um (x, z), Unterkante bei `unten`.
    pub fn block(&mut self, x: f32, unten: f32, z: f32, b: f32, h: f32, t: f32, c: Color) -> SceneNode3d {
        self.quader(x, unten + h / 2.0, z, b, h, t, c)
    }

    /// Ein senkrechter Zylinder, der auf `unten` steht.
    pub fn saeule(&mut self, x: f32, unten: f32, z: f32, r: f32, h: f32, c: Color) -> SceneNode3d {
        let mut n = self.knoten.add_cylinder(r, h);
        n.set_position(Vec3::new(x, unten + h / 2.0, z));
        n.set_color(c);
        n.set_roughness(0.8);
        n
    }

    pub fn kugel(&mut self, x: f32, y: f32, z: f32, r: f32, c: Color) -> SceneNode3d {
        let mut n = self.knoten.add_sphere(r);
        n.set_position(Vec3::new(x, y, z));
        n.set_color(c);
        n.set_roughness(0.7);
        n
    }

    /// Eine flache Bildfläche (Quad) mit Mitte (x, y, z), die nach `rot_y` schaut (0 = +z).
    pub fn flaeche(&mut self, x: f32, y: f32, z: f32, b: f32, h: f32, rot_y: f32, c: Color) -> SceneNode3d {
        let mut n = self.knoten.add_quad(b, h, 1, 1);
        n.set_position(Vec3::new(x, y, z));
        n.set_rotation(Quat::from_axis_angle(Vec3::Y, rot_y));
        n.set_color(c);
        n
    }

    /// Ein Kollisionskasten in Weltkoordinaten.
    pub fn kasten(&mut self, k: Kasten) {
        self.gebaut.kaesten.push(k);
    }

    /// Ein Quader auf dem Boden, gegen den man stößt. Der Kollisionskasten umschließt ihn in der Welt
    /// (bei gedrehten Gruppen das umgebende Rechteck).
    pub fn fester_block(&mut self, x: f32, unten: f32, z: f32, b: f32, h: f32, t: f32, c: Color) -> SceneNode3d {
        self.fest(x, unten, z, b, h, t);
        self.block(x, unten, z, b, h, t, c)
    }

    /// Nur der Kollisionskasten zu einem Quader dieser Gruppe.
    pub fn fest(&mut self, x: f32, unten: f32, z: f32, b: f32, h: f32, t: f32) {
        let ecken = [(-b, -t), (b, -t), (b, t), (-b, t)].map(|(dx, dz)| self.welt(x + dx / 2.0, unten, z + dz / 2.0));
        let min_x = ecken.iter().map(|e| e[0]).fold(f32::INFINITY, f32::min);
        let max_x = ecken.iter().map(|e| e[0]).fold(f32::NEG_INFINITY, f32::max);
        let min_z = ecken.iter().map(|e| e[2]).fold(f32::INFINITY, f32::min);
        let max_z = ecken.iter().map(|e| e[2]).fold(f32::NEG_INFINITY, f32::max);
        let y = ecken[0][1];
        self.gebaut.kaesten.push(Kasten::neu(min_x, max_x, min_z, max_z, y + h).mit_unten(y));
    }

    /// Etwas Benutzbares; `pos` in Koordinaten dieser Gruppe.
    pub fn nutzbar(&mut self, art: &'static str, id: impl Into<String>, pos: [f32; 3], reichweite: f32) {
        let pos = self.welt(pos[0], pos[1], pos[2]);
        self.gebaut.nutzbar.push(Nutzbar { art, id: id.into(), pos, reichweite });
    }

    pub fn merken(&mut self, name: impl Into<String>, n: SceneNode3d) {
        self.gebaut.teile.insert(name.into(), n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexfarben() {
        let c = farbe("#ff8000");
        assert_eq!((c.r, c.b), (1.0, 0.0));
        assert!((c.g - 0.2158605).abs() < 1e-4, "sRGB 128 ist linear etwa 0.216");
        assert_eq!(farbe("kaputt").g, 0.0);
        assert!(toenen(c, 0.5).b > 0.4);
    }
}
