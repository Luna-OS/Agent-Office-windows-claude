//! Die Leinwand auf dem Dach und die Bildschirme im Büro.
//!
//! Leinwand: jeden Frame, solange man auf dem Dach ist, ihre Ecken auf dem Bildschirm an lyra
//! (`bei_leinwand`), damit das YouTube-iframe genau darüber liegt; beim Verlassen einmal unsichtbar.
//! Bildschirme: Bilder aus `bildschirm_setzen` werden Texturen auf dem TV oder einem Tischmonitor.

use std::collections::HashMap;
use std::sync::Arc;

use kiss3d::context::Context;
use kiss3d::prelude::*;
use kiss3d::resource::Texture;

use crate::api::{self, Bildschirm, LeinwandBlick};
use crate::bau::farbe;
use crate::grundriss::dach::LEINWAND;
use crate::register::{Phase, Register};
use crate::welt::Kontext;

/// Berechnet, wo die Leinwand auf dem Bildschirm ist.
pub fn leinwand_blick(kx: &Kontext) -> LeinwandBlick {
    let css = kx.css;
    let mut ecken = [[0.0f32; 2]; 4];
    let mut vorn = true;
    for (i, e) in LEINWAND.ecken().iter().enumerate() {
        let (p, v) = kx.kamera.projizieren(*e, css);
        ecken[i] = p;
        vorn &= v;
    }
    let min_x = ecken.iter().map(|e| e[0]).fold(f32::INFINITY, f32::min);
    let max_x = ecken.iter().map(|e| e[0]).fold(f32::NEG_INFINITY, f32::max);
    let min_y = ecken.iter().map(|e| e[1]).fold(f32::INFINITY, f32::min);
    let max_y = ecken.iter().map(|e| e[1]).fold(f32::NEG_INFINITY, f32::max);
    let im_bild = max_x > 0.0 && min_x < css.x && max_y > 0.0 && min_y < css.y;
    // Von hinten (nördlich der Leinwand) sieht man sie nicht.
    let auge = kx.kamera.auge();
    let davor = auge.z > LEINWAND.z;
    let mitte = Vec3::new(LEINWAND.x, LEINWAND.y, LEINWAND.z);
    LeinwandBlick {
        x: min_x,
        y: min_y,
        breite: max_x - min_x,
        hoehe: max_y - min_y,
        ecken,
        sichtbar: kx.zustand.auf_dem_dach() && vorn && davor && im_bild,
        abstand: (Vec3::from(kx.spieler.pos()) + Vec3::new(0.0, 1.4, 0.0) - mitte).length(),
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let mut war_sichtbar = false;
    reg.tick(Phase::Anzeige, move |kx: &mut Kontext, _| {
        let b = leinwand_blick(kx);
        if kx.zustand.auf_dem_dach() || war_sichtbar {
            api::leinwand_melden(b);
        }
        war_sichtbar = b.sichtbar;
        // Ohne Video bleibt die Fläche dunkel; läuft eins, leuchtet sie leicht (das iframe liegt darüber).
        let an = kx.zustand.leinwand.video_id.is_some();
        if let Some(l) = kx.teil("leinwand") {
            l.set_emissive(farbe(if an { "#1d2d44" } else { "#05070d" }));
        }
    });

    let mut texturen: HashMap<Bildschirm, Arc<Texture>> = HashMap::new();
    reg.tick(Phase::Welt, move |kx: &mut Kontext, _| {
        if kx.neu_gebaut {
            // Die Knoten sind neu: Texturen neu anlegen, sobald das nächste Bild kommt.
            texturen.clear();
        }
        for (ziel, bild) in std::mem::take(&mut kx.bilder) {
            let name = match &ziel {
                Bildschirm::Tv => "tv".to_string(),
                Bildschirm::Schreibtisch(t) => format!("monitor:{t}"),
            };
            let Some(knoten) = kx.gebaut.teile.get_mut(&name) else { continue };
            match bild {
                Some((b, h, rgba)) => {
                    let tex = match texturen.get(&ziel) {
                        Some(t) if t.size == (b, h) => {
                            schreiben(t, b, h, &rgba);
                            t.clone()
                        }
                        _ => {
                            let t = Texture::new(b, h, &rgba, wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::AddressMode::ClampToEdge, wgpu::FilterMode::Linear, false, 1);
                            texturen.insert(ziel.clone(), t.clone());
                            t
                        }
                    };
                    knoten.set_texture(tex);
                    knoten.set_color(Color::new(1.0, 1.0, 1.0, 1.0));
                    knoten.set_emissive(Color::new(0.9, 0.9, 0.9, 1.0));
                }
                None => {
                    texturen.remove(&ziel);
                    knoten.set_texture(kiss3d::resource::TextureManager::get_global_manager(|tm| tm.get_default()));
                    knoten.set_color(farbe("#0d1b2a"));
                    knoten.set_emissive(farbe("#0d1b2a"));
                }
            }
        }
    });
}

/// Neue Pixel in eine vorhandene Textur gleicher Größe.
fn schreiben(t: &Texture, b: u32, h: u32, rgba: &[u8]) {
    let ctx = Context::get();
    ctx.queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &t.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * b), rows_per_image: Some(h) },
        wgpu::Extent3d { width: b, height: h, depth_or_array_layers: 1 },
    );
}
