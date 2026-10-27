//! Bilder an der Wand (wie `features/hanging`): jedes `Dekor` der Etage hängt gerahmt an seiner
//! Wand, das Bild kommt über den Bild-Proxy (`bilder.rs`). Bis es da ist, ist die Fläche hell grau;
//! geht es nicht, bleibt sie dunkel. E an einem Bild öffnet es in der Oberfläche (ändern/abnehmen).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

use kiss3d::prelude::*;
use kiss3d::resource::Texture;
use protokoll::spielzeug::{Dekor, Wand};

use crate::api::{WeltAktion, melden};
use crate::bau::farbe;
use crate::bilder::{self, Stand};
use crate::grundriss::BODEN;
use crate::register::{Nutzbar, Phase, Register};
use crate::welt::Kontext;

/// Die Rahmenfarben (Index `DekorPlatz::rahmen`), wie im Original.
pub const RAHMEN: [&str; 6] = [
    "#c98b5a", "#2b2d42", "#fffaf3", "#e9b949", "#ff8a5b", "#2a9d8f",
];
const RAHMEN_BREITE: f32 = 0.06;

/// Mitte des Bildes vor der Wand und seine Blickrichtung.
pub fn wandort(w: Wand, u: f32, y: f32) -> ([f32; 3], f32) {
    let ab = 0.04;
    match w {
        Wand::Nord => ([u, y, BODEN.min_z + ab], 0.0),
        Wand::Sued => ([u, y, BODEN.max_z - ab], PI),
        Wand::Ost => ([BODEN.max_x - ab, y, u], -FRAC_PI_2),
        Wand::West => ([BODEN.min_x + ab, y, u], FRAC_PI_2),
    }
}

struct Haengend {
    dekor: Dekor,
    gruppe: SceneNode3d,
    flaeche: SceneNode3d,
    /// Das Bild ist schon aufgezogen (oder gescheitert).
    fertig: bool,
}

fn haengen(eltern: &mut SceneNode3d, d: &Dekor) -> Haengend {
    let p = &d.platz;
    let (ort, rot) = wandort(p.wand, p.u, p.y);
    let mut g = eltern.add_group();
    g.set_position(Vec3::from(ort));
    g.set_rotation(Quat::from_axis_angle(Vec3::Y, rot));
    let rahmen = farbe(RAHMEN[p.rahmen as usize % RAHMEN.len()]);
    let mut r = g.add_cube(p.b + 2.0 * RAHMEN_BREITE, p.h + 2.0 * RAHMEN_BREITE, 0.05);
    r.set_color(rahmen);
    let mut f = g.add_quad(p.b, p.h, 1, 1);
    f.set_position(Vec3::new(0.0, 0.0, 0.03));
    f.set_color(farbe("#dee2e6"));
    Haengend {
        dekor: d.clone(),
        gruppe: g,
        flaeche: f,
        fertig: false,
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let bilder: Rc<RefCell<BTreeMap<String, Haengend>>> = Rc::default();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, _| {
        let mut h = bilder.borrow_mut();
        if kx.neu_gebaut {
            h.clear();
        }
        // Auf dem Dach und in der Burg hängen keine Bilder.
        let liste: Vec<Dekor> = if kx.zustand.auf_dem_dach() || kx.in_der_burg() {
            Vec::new()
        } else {
            kx.zustand.spielzeug.dekor.clone()
        };
        h.retain(|id, b| {
            let bleibt = liste
                .iter()
                .any(|d| &d.id == id && d.platz == b.dekor.platz);
            if !bleibt {
                b.gruppe.detach();
            }
            bleibt
        });
        for d in &liste {
            let b = h
                .entry(d.id.clone())
                .or_insert_with(|| haengen(&mut kx.etage, d));
            if !b.fertig {
                match bilder::holen(&d.platz.url) {
                    Stand::Laedt => {}
                    Stand::Fertig(bild) => {
                        let (w, hh, px) = &*bild;
                        let t = Texture::new(
                            *w,
                            *hh,
                            px,
                            wgpu::TextureFormat::Rgba8UnormSrgb,
                            wgpu::AddressMode::ClampToEdge,
                            wgpu::FilterMode::Linear,
                            false,
                            1,
                        );
                        b.flaeche.set_texture(t);
                        b.flaeche.set_color(Color::new(1.0, 1.0, 1.0, 1.0));
                        b.flaeche.set_emissive(Color::new(0.25, 0.25, 0.25, 1.0));
                        b.fertig = true;
                    }
                    Stand::Fehler(e) => {
                        log::warn!("Bild {}: {e}", d.platz.url);
                        b.flaeche.set_color(farbe("#495057"));
                        b.fertig = true;
                    }
                }
            }
            let (ort, rot) = wandort(d.platz.wand, d.platz.u, d.platz.y);
            let vor = [
                ort[0] + rot.sin() * 1.0,
                ort[1] - 0.4,
                ort[2] + rot.cos() * 1.0,
            ];
            kx.dinge.push(Nutzbar {
                art: "bild",
                id: d.id.clone(),
                pos: vor,
                reichweite: 2.2,
            });
        }
    });

    reg.interaktion(
        "bild",
        |kx: &Kontext, n| {
            let d = kx.zustand.spielzeug.dekor.iter().find(|d| d.id == n.id)?;
            Some(format!(
                "E: Bild „{}“ ändern oder abnehmen",
                d.platz.titel.clone().unwrap_or_else(|| "ohne Titel".into())
            ))
        },
        |_, n| melden(WeltAktion::BildOeffnen { id: n.id.clone() }),
    );
}
