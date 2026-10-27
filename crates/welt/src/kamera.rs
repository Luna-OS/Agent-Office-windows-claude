//! Die Kamera: in der Ich-Ansicht an den Augen, sonst hinter dem Spieler (Mausrad zoomt).
//!
//! kiss3ds `FirstPersonCamera3d` liefert die Matrizen; seine eigene Steuerung (Tasten, Maus) ist
//! abgeschaltet, wir setzen Auge und Ziel selbst.

use kiss3d::camera::{Camera3d, FirstPersonCamera3d};
use kiss3d::event::WindowEvent;
use kiss3d::prelude::*;
use kiss3d::window::Canvas;

pub struct Kamera {
    innen: FirstPersonCamera3d,
    /// 0 = Ich-Ansicht; sonst Abstand hinter dem Kopf.
    pub abstand: f32,
    groesse: Vec2,
}

impl Kamera {
    pub fn neu() -> Self {
        let mut innen = FirstPersonCamera3d::new_with_frustum(70f32.to_radians(), 0.05, 900.0, Vec3::new(0.0, 1.6, 5.0), Vec3::ZERO);
        innen.rebind_rotate_button(None);
        innen.rebind_drag_button(None);
        innen.unbind_movement_keys();
        Self { innen, abstand: 0.0, groesse: Vec2::new(1.0, 1.0) }
    }

    /// Mausrad: rein bis zur Ich-Ansicht, raus bis 6 m.
    pub fn zoomen(&mut self, rad: f32) {
        if rad == 0.0 {
            return;
        }
        self.abstand = (self.abstand + rad * 0.004).clamp(0.0, 6.0);
        if self.abstand < 0.6 {
            self.abstand = if rad > 0.0 { 0.6 } else { 0.0 };
        }
    }

    /// Setzt Auge und Blick. `begrenzen` hält eine Kamera hinter dem Kopf vor Wänden (Abstand kürzen).
    pub fn setzen(&mut self, auge: [f32; 3], blick: [f32; 3], begrenzen: impl Fn([f32; 3], [f32; 3]) -> f32) {
        let a = Vec3::from(auge);
        let b = Vec3::from(blick).normalize_or(Vec3::NEG_Z);
        let eye = if self.abstand > 0.0 {
            let max = begrenzen(auge, (-b).into());
            a - b * self.abstand.min(max - 0.2).max(0.0)
        } else {
            a
        };
        self.innen.look_at(eye, eye + b);
    }

    pub fn auge(&self) -> Vec3 {
        self.innen.eye()
    }

    /// Ein Weltpunkt in CSS-Pixeln (oben links 0, 0) und ob er vor der Kamera liegt.
    pub fn projizieren(&self, p: [f32; 3], css: Vec2) -> ([f32; 2], bool) {
        let h = self.innen.transformation() * Vec3::from(p).extend(1.0);
        let vorn = h.w > 0.0;
        let n = h.truncate() / h.w.abs().max(1e-6);
        ([(1.0 + n.x) * css.x / 2.0, (1.0 - n.y) * css.y / 2.0], vorn)
    }

    pub fn bildgroesse(&self) -> Vec2 {
        self.groesse
    }
}

impl Camera3d for Kamera {
    fn handle_event(&mut self, canvas: &Canvas, event: &WindowEvent) {
        // Nur die Fenstergröße interessiert (fürs Seitenverhältnis); Maus und Tasten machen wir selbst.
        if let WindowEvent::FramebufferSize(w, h) = *event {
            self.groesse = Vec2::new(w as f32, h as f32);
            self.innen.handle_event(canvas, event);
        }
    }
    fn eye(&self) -> Vec3 {
        self.innen.eye()
    }
    fn view_transform(&self) -> Pose3 {
        self.innen.view_transform()
    }
    fn transformation(&self) -> Mat4 {
        self.innen.transformation()
    }
    fn inverse_transformation(&self) -> Mat4 {
        self.innen.inverse_transformation()
    }
    fn clip_planes(&self) -> (f32, f32) {
        self.innen.clip_planes()
    }
    fn update(&mut self, _canvas: &Canvas) {}
    fn view_transform_pair(&self, pass: usize) -> (Pose3, Mat4) {
        self.innen.view_transform_pair(pass)
    }
}
