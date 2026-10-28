//! Der Bildschirm des Arcade-Automaten: zeigt das laufende BLOCKFALL-Spiel, wie es der Spieler am
//! Automaten meldet (`spielzeug::ArcadeBild`: der Schacht Zeile für Zeile, '0' leer, '1'–'7' Farben,
//! '8' Landeplatz), für alle auf der Etage. Ohne Spiel ein ruhiges Titelbild.

use std::sync::Arc;

use kiss3d::prelude::*;
use kiss3d::resource::Texture;
use protokoll::spielzeug::ArcadeBild;

use crate::register::{Phase, Register};
use crate::welt::Kontext;

/// Breite des Schachts in Zellen (wie BLOCKFALL).
pub const SPALTEN: usize = 10;
/// Pixel je Zelle in der Textur.
const ZELLE: usize = 6;

/// Farben der Zellen '1'–'7' (RGB), '8' ist der Landeplatz (blass), '0' der Hintergrund.
const FARBEN: [[u8; 3]; 9] = [
    [11, 14, 30],
    [0, 229, 255],
    [255, 214, 0],
    [190, 80, 255],
    [60, 220, 90],
    [255, 70, 90],
    [70, 120, 255],
    [255, 150, 40],
    [70, 76, 110],
];

/// Das Bild als RGBA (Breite, Höhe, Pixel), aus dem Zellen-Text. Zeilen mit weniger Zeichen sind leer.
pub fn rendern(bild: &ArcadeBild) -> (u32, u32, Vec<u8>) {
    let zellen: Vec<u8> = bild.zellen.bytes().filter(|b| b.is_ascii_digit()).collect();
    let zeilen = zellen.len().div_ceil(SPALTEN).max(1);
    let (b, h) = (SPALTEN * ZELLE, zeilen * ZELLE);
    let mut px = vec![0u8; b * h * 4];
    for (i, z) in zellen.iter().enumerate() {
        let c = FARBEN[(*z - b'0').min(8) as usize];
        let (zx, zy) = (i % SPALTEN, i / SPALTEN);
        for y in 0..ZELLE {
            for x in 0..ZELLE {
                // Ein dunkler Rand um jede gefüllte Zelle, damit die Steine Kanten haben.
                let rand = (x == 0 || y == 0 || x == ZELLE - 1 || y == ZELLE - 1) && *z != b'0';
                let o = ((zy * ZELLE + y) * b + zx * ZELLE + x) * 4;
                let f = if rand { 0.6 } else { 1.0 };
                px[o..o + 4].copy_from_slice(&[
                    (c[0] as f32 * f) as u8,
                    (c[1] as f32 * f) as u8,
                    (c[2] as f32 * f) as u8,
                    255,
                ]);
            }
        }
    }
    (b as u32, h as u32, px)
}

/// Das Titelbild ohne Spiel: ein paar Steine, als läge eine Runde in der Luft.
fn titelbild() -> ArcadeBild {
    let mut z = "0".repeat(SPALTEN * 20);
    for (i, c) in [
        (54, '1'),
        (55, '1'),
        (56, '1'),
        (57, '1'),
        (93, '3'),
        (103, '3'),
        (104, '3'),
        (113, '3'),
        (180, '5'),
        (181, '5'),
        (190, '6'),
        (191, '2'),
        (192, '2'),
        (198, '7'),
        (199, '7'),
    ] {
        z.replace_range(i..i + 1, &c.to_string());
    }
    ArcadeBild {
        zellen: z,
        naechstes: 0,
        halten: 0,
        punkte: 0,
        reihen: 0,
        level: 1,
        teile: 0,
        zustand: "pause".into(),
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let mut zuletzt: Option<ArcadeBild> = None;
    let mut textur: Option<Arc<Texture>> = None;
    reg.tick(Phase::Welt, move |kx: &mut Kontext, _| {
        if kx.neu_gebaut {
            zuletzt = None;
            textur = None;
        }
        let bild = kx
            .zustand
            .spielzeug
            .arcade
            .bild
            .clone()
            .unwrap_or_else(titelbild);
        // Wer spielt, und wie viele Punkte: über dem Automaten.
        if let Some(s) = &kx.zustand.spielzeug.arcade.spieler {
            let (ax, az) = crate::grundriss::ARCADE;
            kx.beschriften(
                format!("{} · {} Punkte", s.name, bild.punkte),
                [ax - 0.3, 2.3, az],
                Color::new(0.4, 1.0, 0.8, 1.0),
            );
        }
        if zuletzt.as_ref() == Some(&bild) {
            return;
        }
        let Some(schirm) = kx.gebaut.teile.get_mut("arcade-schirm") else {
            return;
        };
        let (b, h, px) = rendern(&bild);
        let t = match &textur {
            Some(t) if t.size == (b, h) => {
                crate::features::leinwand::schreiben(t, b, h, &px);
                t.clone()
            }
            _ => Texture::new(
                b,
                h,
                &px,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::AddressMode::ClampToEdge,
                wgpu::FilterMode::Nearest,
                false,
                1,
            ),
        };
        schirm.set_texture(t.clone());
        schirm.set_color(Color::new(1.0, 1.0, 1.0, 1.0));
        schirm.set_emissive(Color::new(0.0, 0.0, 0.0, 1.0));
        textur = Some(t);
        zuletzt = Some(bild);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zellen_werden_pixel() {
        let mut b = titelbild();
        b.zellen = format!("1{}", "0".repeat(SPALTEN * 2 - 1));
        let (w, h, px) = rendern(&b);
        assert_eq!((w as usize, h as usize), (SPALTEN * ZELLE, 2 * ZELLE));
        // Mitte der ersten Zelle hat die Farbe '1'.
        let o = ((ZELLE / 2) * w as usize + ZELLE / 2) * 4;
        assert_eq!(&px[o..o + 3], &FARBEN[1]);
        // Die zweite Zelle ist Hintergrund.
        let o2 = ((ZELLE / 2) * w as usize + ZELLE + ZELLE / 2) * 4;
        assert_eq!(&px[o2..o2 + 3], &FARBEN[0]);
    }
}
