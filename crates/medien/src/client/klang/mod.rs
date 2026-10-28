//! Prozedurale Klänge mit Web Audio, wie im Original (`src/client/sound`): nichts wird geladen, alles
//! entsteht aus Oszillatoren, Rauschen und Filtern.
//!
//! [`Klang`] hält den gemeinsamen Audio-Kontext (startet beim ersten Klick oder Tastendruck, wie Browser
//! es wollen), die Lautstärke, wo die Ohren sind und was draußen los ist. Die Rezepte stehen in eigenen
//! Modulen (`rezepte`, `spiele`, `buero`, `autos`, `wetter`, `umgebung`), die Jukebox in `musik`,
//! der DJ auf dem Dach in `dj`.

mod autos;
mod buero;
mod dj;
mod kern;
mod musik;
mod puffer;
mod rezepte;
mod spiele;
mod umgebung;
mod wetter;

use std::cell::RefCell;
use std::rc::Rc;

use web_sys::AudioContextState;

pub use autos::Motor;
pub use dj::Dj;
pub use musik::Jukebox;
pub use rezepte::{Ding, Gong};
pub use spiele::{BallKlang, GolfKlang, WurfKlang};

use super::{Lauscher, dokument, fenster};
pub(crate) use kern::Kern;

/// Ein Ort in der Welt (Meter).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ort {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Wo die Ohren gerade sind (entscheidet, wie gedämpft Regen und Donner klingen).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Wo {
    /// Im Office: Regen gedämpft hinter Glas.
    #[default]
    Drinnen,
    /// In der Garage unter dem Office.
    Garage,
    /// Draußen (Dach, Balkon, Straße).
    Draussen,
}

/// Wo im Office die Geräusche herkommen (aus der Welt; Standard sind die Orte des Originals).
#[derive(Debug, Clone, PartialEq)]
pub struct Orte {
    pub kaffeemaschine: Ort,
    pub kuehlschrank: Ort,
    pub arcade: Ort,
    /// Knapp vor den Fenstern: Vögel, Grillen, Regentropfen.
    pub fenster: Vec<Ort>,
    /// Schreibtische, an denen Telefone klingeln.
    pub tische: Vec<Ort>,
}

impl Default for Orte {
    fn default() -> Self {
        Self {
            kaffeemaschine: Ort {
                x: -15.7,
                y: 1.4,
                z: 12.2,
            },
            kuehlschrank: Ort {
                x: -11.3,
                y: 1.1,
                z: 12.2,
            },
            arcade: Ort {
                x: -0.2,
                y: 1.2,
                z: 0.0,
            },
            fenster: Vec::new(),
            tische: Vec::new(),
        }
    }
}

/// Lage der Ohren und der Welt, für die Umgebungsgeräusche.
#[derive(Debug, Clone, Default)]
pub(crate) struct Lage {
    pub kopf: Ort,
    pub wo: Wo,
    pub draussen: bool,
    /// Wie stark es regnet (0..1) und wie dunkel es ist (1 = Nacht).
    pub regen: f32,
    pub nacht: f32,
    pub orte: Orte,
}

/// Läuft, sobald Audio startet.
type Starter = Rc<dyn Fn(&Rc<Kern>)>;

#[derive(Default)]
struct Innen {
    kern: Option<Rc<Kern>>,
    lautstaerke: f32,
    stumm: bool,
    beim_start: Vec<Starter>,
    tipper: rezepte::Tipper,
    lage: Lage,
    umgebung: umgebung::Umgebung,
    motoren: autos::Motoren,
    _lauscher: Vec<Lauscher>,
}

/// Alle Klänge des Office. Billig zu klonen.
#[derive(Clone)]
pub struct Klang(Rc<RefCell<Innen>>);

impl Default for Klang {
    fn default() -> Self {
        Self::neu()
    }
}

impl Klang {
    /// Meldet sich für den ersten Klick/Tastendruck an (erst dann darf Audio starten).
    pub fn neu() -> Self {
        let k = Self(Rc::new(RefCell::new(Innen {
            lautstaerke: 0.7,
            ..Default::default()
        })));
        let mut l = Vec::new();
        for art in ["pointerdown", "keydown"] {
            let k2 = k.clone();
            l.push(Lauscher::neu(&fenster(), art, move |_| k2.entsperren()));
        }
        let k2 = k.clone();
        l.push(Lauscher::neu(&dokument(), "visibilitychange", move |_| {
            k2.sichtbarkeit()
        }));
        k.0.borrow_mut()._lauscher = l;
        k
    }

    /// Startet Audio (oder weckt es wieder auf). Wird beim ersten Klick von selbst gerufen.
    pub fn entsperren(&self) {
        if let Some(k) = self.kern() {
            if k.ctx.state() == AudioContextState::Suspended {
                let _ = k.ctx.resume();
            }
            return;
        }
        let Some(kern) = Kern::neu() else { return };
        let kern = Rc::new(kern);
        let starter = {
            let mut i = self.0.borrow_mut();
            i.kern = Some(kern.clone());
            let lage = i.lage.clone();
            i.umgebung.starten(&kern, &lage);
            i.beim_start.clone()
        };
        self.lautstaerke_anwenden();
        self.draussen_anwenden();
        for f in starter {
            f(&kern);
        }
    }

    pub(crate) fn kern(&self) -> Option<Rc<Kern>> {
        self.0.borrow().kern.clone()
    }

    /// `f` läuft, sobald Audio startet (oder sofort, falls es schon läuft).
    pub(crate) fn beim_start(&self, f: impl Fn(&Rc<Kern>) + 'static) {
        if let Some(k) = self.kern() {
            f(&k);
        }
        self.0.borrow_mut().beim_start.push(Rc::new(f));
    }

    /// Lautstärke 0..1; `stumm` schaltet alles aus, ohne den Pegel zu vergessen.
    pub fn lautstaerke_setzen(&self, lautstaerke: f32, stumm: bool) {
        {
            let mut i = self.0.borrow_mut();
            i.lautstaerke = lautstaerke.clamp(0.0, 1.0);
            i.stumm = stumm;
        }
        self.lautstaerke_anwenden();
    }

    fn lautstaerke_anwenden(&self) {
        let i = self.0.borrow();
        let Some(k) = &i.kern else { return };
        let g = if i.stumm {
            0.0
        } else {
            i.lautstaerke * i.lautstaerke
        };
        let _ = k.master.gain().set_target_at_time(g, k.jetzt(), 0.04);
    }

    fn sichtbarkeit(&self) {
        let Some(k) = self.kern() else { return };
        let verborgen = dokument().hidden();
        if !verborgen && k.ctx.state() == AudioContextState::Suspended {
            let _ = k.ctx.resume();
        }
        let _ = k.umgebung.gain().set_target_at_time(
            if verborgen { 0.0 } else { 1.0 },
            k.jetzt(),
            0.15,
        );
    }

    /// Wo die Ohren sind (Kopf, Blickrichtung) und ob drinnen/Garage/draußen, einmal pro Frame.
    /// Plant auch Tippen, Regen und was im Raum ab und zu passiert.
    pub fn hoerer_setzen(&self, kopf: Ort, blick_x: f32, blick_z: f32, wo: Wo) {
        let Some(k) = self.kern() else { return };
        if k.ctx.state() != AudioContextState::Running {
            return;
        }
        let l = k.ctx.listener();
        let len = blick_x.hypot(blick_z).max(1e-6);
        l.set_position(kopf.x as f64, kopf.y as f64, kopf.z as f64);
        l.set_orientation(
            (blick_x / len) as f64,
            0.0,
            (blick_z / len) as f64,
            0.0,
            1.0,
            0.0,
        );
        let mut i = self.0.borrow_mut();
        i.lage.kopf = kopf;
        i.lage.wo = wo;
        i.tipper.planen(&k);
        let lage = i.lage.clone();
        i.umgebung.takt(&k, &lage);
    }

    /// Das Wetter draußen: Regen 0..1, Nacht 0..1 (1 = dunkel).
    pub fn wetter_setzen(&self, regen: f32, nacht: f32) {
        let mut i = self.0.borrow_mut();
        i.lage.regen = regen.clamp(0.0, 1.0);
        i.lage.nacht = nacht.clamp(0.0, 1.0);
    }

    /// Wo im Office Kaffeemaschine, Kühlschrank, Fenster und Tische sind.
    pub fn orte_setzen(&self, orte: Orte) {
        self.0.borrow_mut().lage.orte = orte;
    }

    /// Auf dem Dach (`true`) oder drinnen auf einer Etage: das Summen des Office weicht Wind und Stadt.
    pub fn draussen_setzen(&self, draussen: bool) {
        self.0.borrow_mut().lage.draussen = draussen;
        self.draussen_anwenden();
    }

    fn draussen_anwenden(&self) {
        let i = self.0.borrow();
        let Some(k) = &i.kern else { return };
        let d = i.lage.draussen;
        let _ = k
            .drinnen
            .gain()
            .set_target_at_time(if d { 0.0 } else { 1.0 }, k.jetzt(), 0.3);
        let _ = k
            .draussen
            .gain()
            .set_target_at_time(if d { 1.0 } else { 0.0 }, k.jetzt(), 0.3);
    }

    pub(crate) fn lage(&self) -> Lage {
        self.0.borrow().lage.clone()
    }

    /// Ein Arbeiter tippt (oder hört auf) an seinem Schreibtisch.
    pub fn tippen_setzen(&self, id: &str, x: f32, z: f32, an: bool) {
        self.0.borrow_mut().tipper.setzen(id, x, z, an);
    }

    pub fn tipper_entfernen(&self, id: &str) {
        self.0.borrow_mut().tipper.entfernen(id);
    }

    /// Die Motoren der Autos, die gerade gefahren werden; wer fehlt, verklingt.
    pub fn motoren_setzen(&self, laufend: &[Motor]) {
        let Some(k) = self.kern() else { return };
        self.0.borrow_mut().motoren.setzen(&k, laufend);
    }
}

/// Zufall a..b (Math.random, damit kein Tastenklick wie der andere klingt).
pub(crate) fn zufall(a: f64, b: f64) -> f64 {
    a + js_sys::Math::random() * (b - a)
}

pub(crate) fn zufall_f(a: f32, b: f32) -> f32 {
    zufall(a as f64, b as f64) as f32
}

pub(crate) fn zufall_ganz(a: i32, b: i32) -> i32 {
    zufall(a as f64, b as f64 + 1.0).floor() as i32
}

pub(crate) fn waehle<T>(xs: &[T]) -> &T {
    &xs[(js_sys::Math::random() * xs.len() as f64) as usize % xs.len()]
}
