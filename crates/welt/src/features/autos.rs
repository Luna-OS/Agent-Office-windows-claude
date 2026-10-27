//! Die Autos in der Garage: Lambos und Ferraris, die jeder fahren kann. E steigt ein (ans Steuer,
//! oder daneben, wenn schon jemand fährt), im Auto: W Gas, S Bremse/Rückwärts, A/D lenken,
//! Leertaste Bremse, H Hupe, E aussteigen. Wer fährt, rechnet die Physik und schickt die Lage;
//! alle anderen sehen das Auto, wo sein Fahrer es meldet.

use std::cell::RefCell;
use std::f32::consts::PI;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::ClientNachricht;
use protokoll::spielzeug::{AutoSitz, Client as S};

use crate::api::{Klang, klang};
use crate::bau::{farbe, toenen};
use crate::grundriss::draussen::*;
use crate::kollision::Kasten;
use crate::register::{Aktivitaet, Nutzbar, Phase, Register};
use crate::welt::Kontext;

struct Modell {
    wurzel: SceneNode3d,
    dach: SceneNode3d,
    raeder: Vec<SceneNode3d>,
}

fn bauen(eltern: &mut SceneNode3d, p: &AutoPlatz) -> Modell {
    let mut w = eltern.add_group();
    let lack = farbe(p.farbe);
    let flach = p.art == AutoArt::Lambo;
    let h = AUTO_KAROSSERIE;
    let mut unten = w.add_cube(AUTO_BREITE, h * 0.55, AUTO_LAENGE);
    unten.set_position(Vec3::new(0.0, 0.25 + h * 0.275, 0.0));
    unten.set_color(lack);
    unten.set_metallic(0.4);
    unten.set_roughness(0.35);
    // Keilförmige Nase: ein flacher Block vorne.
    let mut nase = w.add_cube(AUTO_BREITE * 0.96, h * 0.25, AUTO_LAENGE * 0.3);
    nase.set_position(Vec3::new(0.0, 0.25 + h * 0.62, AUTO_LAENGE * 0.32));
    nase.set_color(toenen(lack, -0.1));
    let mut dach = w.add_cube(AUTO_BREITE * 0.82, AUTO_DACH - h + 0.1, AUTO_LAENGE * if flach { 0.36 } else { 0.42 });
    dach.set_position(Vec3::new(0.0, AUTO_DACH - (AUTO_DACH - h) / 2.0 + 0.2, -0.25));
    dach.set_color(Color::new(0.1, 0.12, 0.16, 0.85));
    let mut raeder = Vec::new();
    for (x, z) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let mut r = w.add_group();
        r.set_position(Vec3::new(x * AUTO_BREITE / 2.0, 0.34, z * AUTO_LAENGE * 0.32));
        let mut rad = r.add_cylinder(0.34, 0.26);
        rad.set_rotation(Quat::from_axis_angle(Vec3::Z, PI / 2.0));
        rad.set_color(farbe("#1b1b1b"));
        raeder.push(r);
    }
    for x in [-0.65, 0.65] {
        let mut l = w.add_sphere(0.1);
        l.set_position(Vec3::new(x, 0.25 + h * 0.5, AUTO_LAENGE / 2.0));
        l.set_color(farbe("#fffbe0"));
        l.set_emissive(farbe("#fff3c4"));
        let mut r = w.add_cube(0.4, 0.08, 0.04);
        r.set_position(Vec3::new(x, 0.25 + h * 0.5, -AUTO_LAENGE / 2.0));
        r.set_color(farbe("#d00000"));
        r.set_emissive(farbe("#6a040f"));
    }
    Modell { wurzel: w, dach, raeder }
}

#[derive(Default)]
struct Stand {
    modelle: Vec<Modell>,
    /// Die Lage des Autos, das man selbst fährt (die eigene Rechnung geht vor).
    eigen: Option<(usize, AutoLage)>,
}

struct Fahren {
    auto: usize,
    fahrer: bool,
    stand: Rc<RefCell<Stand>>,
    seit: f32,
}

impl Aktivitaet<Kontext> for Fahren {
    fn name(&self) -> &'static str {
        "auto"
    }
    fn taste(&mut self, kx: &mut Kontext, code: &str, gedrueckt: bool) -> bool {
        match (code, gedrueckt) {
            ("KeyE", true) => {
                kx.alle_stoppen = true;
                true
            }
            ("KeyH", true) => {
                kx.senden(ClientNachricht::Spielzeug(S::Hupen));
                true
            }
            (c, _) => self.fahrer && ["KeyW", "KeyA", "KeyS", "KeyD", "Space", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].contains(&c),
        }
    }
    fn tick(&mut self, kx: &mut Kontext, dt: f32) -> bool {
        let strasse = kx.strasse();
        let Some(a) = kx.zustand.spielzeug.autos.get(self.auto).cloned() else { return false };
        let ich = kx.zustand.ich.clone();
        let (sitz, ist_drin) = if self.fahrer { (FAHRER, a.fahrer == ich) } else { (BEIFAHRER, a.beifahrer == ich) };
        // Der Server hat uns (noch) nicht eingetragen: kurz warten, dann aufgeben.
        self.seit += dt;
        if !ist_drin && self.seit > 2.0 {
            return false;
        }
        let mut st = self.stand.borrow_mut();
        let mut lage = match st.eigen {
            Some((i, l)) if i == self.auto => l,
            _ => AutoLage { x: a.x, z: a.z, rot_y: a.rot_y, tempo: a.tempo, lenkung: a.lenkung },
        };
        if self.fahrer {
            let g = &kx.eingabe.gehalten;
            let an = |a: &str, b: &str| g.contains(a) || g.contains(b);
            let pedale = Pedale {
                gas: an("KeyW", "ArrowUp") as i32 as f32 - an("KeyS", "ArrowDown") as i32 as f32,
                lenken: an("KeyA", "ArrowLeft") as i32 as f32 - an("KeyD", "ArrowRight") as i32 as f32,
                bremse: g.contains("Space"),
            };
            let neu = fahren(lage, pedale, dt);
            lage = if auf_dem_asphalt(&neu) { neu } else { AutoLage { tempo: 0.0, ..lage } };
            st.eigen = Some((self.auto, lage));
            kx.senden(ClientNachricht::Spielzeug(S::AutoFahren { auto: self.auto as u32, x: lage.x, z: lage.z, rot_y: lage.rot_y, tempo: lage.tempo, lenkung: lage.lenkung }));
            if lage.tempo.abs() > 0.5 && (kx.zeit * 2.0).fract() < dt as f64 * 2.0 {
                klang(Klang::Motor { auto: self.auto as u32, pos: [lage.x, strasse + 0.6, lage.z], tempo: lage.tempo });
            }
        }
        // Man sitzt im Auto: Kamera auf Kopfhöhe am Sitz, Blick mit der Maus frei.
        let (px, pz) = auto_punkt(&lage, sitz.0, sitz.1);
        kx.spieler.versetzen(px, strasse + SITZ_HUEFTE, pz, lage.rot_y);
        kx.ich_sitze = Some(SITZ_HUEFTE);
        let auge = [px, strasse + SITZ_HUEFTE + 0.84, pz];
        kx.kamera_ziel = Some((auge, kx.spieler.blickrichtung()));
        true
    }
    fn hinweis(&self, _kx: &Kontext) -> Option<String> {
        Some(if self.fahrer { "W: Gas · S: Bremse/rückwärts · A/D: lenken · H: Hupe · E: aussteigen".into() } else { "E: aussteigen · H: Hupe".into() })
    }
    fn stoppen(&mut self, kx: &mut Kontext) {
        kx.ich_sitze = None;
        self.stand.borrow_mut().eigen = None;
        kx.senden(ClientNachricht::Spielzeug(S::AutoAussteigen));
        // Neben der Tür aussteigen.
        if let Some(a) = kx.zustand.spielzeug.autos.get(self.auto) {
            let lage = AutoLage { x: a.x, z: a.z, rot_y: a.rot_y, ..Default::default() };
            let seite = if self.fahrer { 1.0 } else { -1.0 };
            let (x, z) = auto_punkt(&lage, seite * (AUTO_BREITE / 2.0 + 0.6), 0.0);
            let y = kx.strasse();
            kx.spieler.versetzen(x, y, z, a.rot_y);
        }
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let stand = Rc::new(RefCell::new(Stand::default()));
    let s = stand.clone();
    reg.tick(Phase::Fahrzeuge, move |kx: &mut Kontext, dt| {
        let mut st = s.borrow_mut();
        if kx.neu_gebaut {
            for mut m in st.modelle.drain(..) {
                m.wurzel.detach();
            }
        }
        let autos = kx.zustand.spielzeug.autos.clone();
        if kx.zustand.auf_dem_dach() || autos.is_empty() {
            for m in st.modelle.iter_mut() {
                m.wurzel.set_visible(false);
            }
            return;
        }
        while st.modelle.len() < autos.len().min(AUTOS.len()) {
            let i = st.modelle.len();
            let m = bauen(&mut kx.etage, &AUTOS[i]);
            st.modelle.push(m);
        }
        let strasse = kx.strasse();
        let eigen = st.eigen;
        for (i, (a, m)) in autos.iter().zip(st.modelle.iter_mut()).enumerate() {
            let lage = match eigen {
                Some((j, l)) if j == i => l,
                _ => AutoLage { x: a.x, z: a.z, rot_y: a.rot_y, tempo: a.tempo, lenkung: a.lenkung },
            };
            m.wurzel.set_visible(true);
            m.wurzel.set_position(Vec3::new(lage.x, strasse, lage.z));
            m.wurzel.set_rotation(Quat::from_axis_angle(Vec3::Y, lage.rot_y));
            // Mit jemandem drin ist das Dach offen.
            m.dach.set_visible(a.fahrer.is_none() && a.beifahrer.is_none());
            for (k, r) in m.raeder.iter_mut().enumerate() {
                let lenk = if k < 2 { lage.lenkung } else { 0.0 };
                r.set_rotation(Quat::from_axis_angle(Vec3::Y, lenk));
            }
            let _ = dt;
            kx.bewegliche.push(Kasten::um(lage.x, lage.z, AUTO_BREITE.max(AUTO_LAENGE * lage.rot_y.cos().abs()), AUTO_LAENGE.max(AUTO_BREITE), strasse + AUTO_KAROSSERIE).mit_unten(strasse));
            let tuer = auto_punkt(&lage, AUTO_BREITE / 2.0 + 0.3, 0.0);
            kx.dinge.push(Nutzbar { art: "auto", id: i.to_string(), pos: [tuer.0, strasse + 1.0, tuer.1], reichweite: 2.2 });
        }
    });

    let s = stand.clone();
    reg.interaktion(
        "auto",
        |kx: &Kontext, n| {
            let i: usize = n.id.parse().ok()?;
            let name = AUTOS.get(i)?.name;
            let a = kx.zustand.spielzeug.autos.get(i)?;
            Some(match (&a.fahrer, &a.beifahrer) {
                (None, _) => format!("E: {name} fahren"),
                (Some(_), None) => format!("E: im {name} mitfahren"),
                (Some(_), Some(_)) => format!("Der {name} ist voll."),
            })
        },
        move |kx: &mut Kontext, n| {
            let Ok(i) = n.id.parse::<usize>() else { return };
            let Some(a) = kx.zustand.spielzeug.autos.get(i) else { return };
            let fahrer = a.fahrer.is_none();
            if !fahrer && a.beifahrer.is_some() {
                return;
            }
            let sitz = if fahrer { AutoSitz::Fahrer } else { AutoSitz::Beifahrer };
            kx.senden(ClientNachricht::Spielzeug(S::AutoEinsteigen { auto: i as u32, sitz }));
            kx.aktivitaet(Fahren { auto: i, fahrer, stand: s.clone(), seit: 0.0 });
        },
    );

    // Hupen anderer hört man auch.
    reg.tick(Phase::Umgebung, move |kx: &mut Kontext, _| {
        let _ = &stand;
        for (i, a) in kx.zustand.spielzeug.autos.iter().enumerate() {
            if kx.aenderungen.hupen.contains(&(i as u32)) {
                klang(Klang::Hupe { pos: [a.x, kx.strasse() + 0.8, a.z] });
            }
        }
    });
}
