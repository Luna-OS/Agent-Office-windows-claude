//! Möbel-Bausteine im Cartoon-Stil: Schreibtisch mit Monitor, Stuhl, Sitzsack, Pflanze, Sofa,
//! Hocker, Tafel. Jeder baut in Koordinaten seiner eigenen Gruppe (Vorderseite = +z).

use std::f32::consts::PI;

use kiss3d::prelude::*;

use super::{Bau, farbe, toenen};
use crate::farben;
use crate::grundriss::{TISCH_BREITE, TISCH_HOEHE, TISCH_TIEFE};

/// Ein Schreibtisch, Mitte (x, z), gedreht wie der Tisch im Grundriss (bei 0 sitzt man auf +z).
/// Gibt den Bildschirm-Knoten zurück (für geteilte Bildschirme und den Status-Schein).
pub fn schreibtisch(b: &mut Bau, x: f32, z: f32, rot_y: f32, stuhlfarbe: &str) -> SceneNode3d {
    let mut g = b.gruppe(x, 0.0, z, rot_y);
    let platte = farbe(farben::TISCH);
    let bein = farbe(farben::TISCHBEIN);
    let breite = TISCH_BREITE - 0.12;
    g.block(
        0.0,
        TISCH_HOEHE - 0.05,
        0.0,
        breite,
        0.05,
        TISCH_TIEFE,
        platte,
    );
    g.fest(0.0, 0.0, 0.0, breite, TISCH_HOEHE, TISCH_TIEFE);
    for (bx, bz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        g.block(
            bx * (breite / 2.0 - 0.06),
            0.0,
            bz * (TISCH_TIEFE / 2.0 - 0.06),
            0.05,
            TISCH_HOEHE - 0.05,
            0.05,
            bein,
        );
    }
    // Monitor zur Sitzseite (+z) hin.
    let fuss = farbe("#343a40");
    g.block(0.0, TISCH_HOEHE, -0.3, 0.2, 0.02, 0.15, fuss);
    g.block(0.0, TISCH_HOEHE, -0.33, 0.04, 0.25, 0.04, fuss);
    g.quader(
        0.0,
        TISCH_HOEHE + 0.42,
        -0.34,
        0.68,
        0.42,
        0.04,
        farbe("#2b2d42"),
    );
    let mut bild = g.flaeche(
        0.0,
        TISCH_HOEHE + 0.42,
        -0.317,
        0.62,
        0.36,
        0.0,
        farbe("#14213d"),
    );
    bild.set_emissive(farbe("#14213d"));
    // Tastatur; dazu Becher, Bücher oder eine Sukkulente aus den Modellen, je nach Tisch.
    g.block(0.0, TISCH_HOEHE, 0.08, 0.42, 0.02, 0.14, farbe("#e9ecef"));
    let welcher = ((x * 7.0 + z * 13.0).abs() as usize) % 5;
    let becherfarbe = farben::STUEHLE[welcher % 6];
    let (datei, stueck, px) = match welcher {
        0 | 3 => (crate::modelle::Datei::Tischkram, "mug", 0.42),
        1 => (crate::modelle::Datei::Tischkram, "books_stack", -0.62),
        2 => (crate::modelle::Datei::Tischkram, "books_upright", -0.65),
        _ => (crate::modelle::Datei::Pflanzen, "succulent", 0.6),
    };
    let mut kram = crate::modelle::stueck(&mut g.knoten, datei, stueck, false, |m| match m {
        "Mug" => farbe(becherfarbe),
        "Coffee" => farbe("#6f4518"),
        "CoverRed" => farbe("#e63946"),
        "CoverBlue" => farbe("#457b9d"),
        "CoverOrange" => farbe("#f4a261"),
        "Pages" => farbe("#f3ead8"),
        andere => pflanzenfarbe(andere),
    });
    kram.set_position(Vec3::new(px, TISCH_HOEHE, -0.1));
    stuhl(&mut g, 0.0, 0.85, PI, stuhlfarbe);
    bild
}

/// Ein Bürostuhl, Sitzfläche um (x, z), Rückenlehne hinten (Blick `rot_y`).
pub fn stuhl(b: &mut Bau, x: f32, z: f32, rot_y: f32, c: &str) {
    let mut g = b.gruppe(x, 0.0, z, rot_y);
    let farbe_ = farbe(c);
    g.saeule(0.0, 0.0, 0.0, 0.25, 0.04, farbe("#343a40"));
    g.saeule(0.0, 0.04, 0.0, 0.03, 0.4, farbe("#6c757d"));
    g.block(0.0, 0.44, 0.0, 0.48, 0.08, 0.46, farbe_);
    g.block(0.0, 0.52, -0.22, 0.46, 0.5, 0.07, toenen(farbe_, -0.1));
}

/// Ein Sitzsack (rund und weich).
pub fn sitzsack(b: &mut Bau, x: f32, z: f32, c: &str) {
    let mut n = b.kugel(x, 0.22, z, 0.42, farbe(c));
    n.set_local_scale(1.15, 0.6, 1.15);
    b.kasten(crate::kollision::Kasten::um(x, z, 0.8, 0.8, 0.4));
}

/// Die Arten der Bodenpflanzen, reihum (keine zwei Nachbarn gleich).
const BODENPFLANZEN: [&str; 3] = ["monstera", "snake_plant", "ficus"];

/// Die Farben der Pflanzen-Modelle nach Materialnamen.
pub fn pflanzenfarbe(m: &str) -> Color {
    farbe(match m {
        "Pot" => farben::TOPF,
        "Glaze" => "#8ecae6",
        "Soil" => "#6b4226",
        "Bark" => "#8a5a3b",
        "Leaf" => farben::PFLANZE,
        "LeafDark" => farben::PFLANZE_DUNKEL,
        _ => "#ff00ff",
    })
}

/// Eine Topfpflanze aus dem Modell (Art reihum nach `i`); `s` skaliert sie.
pub fn pflanze_modell(b: &mut Bau, x: f32, z: f32, s: f32, i: usize) {
    let art = BODENPFLANZEN[i % BODENPFLANZEN.len()];
    let ort = b.welt(x, 0.0, z);
    let mut p = crate::modelle::stueck(
        &mut b.knoten,
        crate::modelle::Datei::Pflanzen,
        art,
        false,
        pflanzenfarbe,
    );
    p.set_position(Vec3::new(x, 0.0, z));
    p.set_local_scale(s, s, s);
    b.kasten(crate::kollision::Kasten::um(
        ort[0],
        ort[2],
        0.6 * s,
        0.6 * s,
        ort[1] + 1.3 * s,
    ));
}

/// Ein Sofa mit `plaetze` Sitzen nebeneinander, Blick `rot_y`.
pub fn sofa(b: &mut Bau, x: f32, y: f32, z: f32, rot_y: f32, breite: f32, c: &str) {
    let mut g = b.gruppe(x, y, z, rot_y);
    let f = farbe(c);
    g.fester_block(0.0, 0.0, 0.0, breite, 0.42, 0.9, f);
    g.block(0.0, 0.42, -0.35, breite, 0.45, 0.2, toenen(f, -0.12));
    g.block(
        -breite / 2.0 + 0.1,
        0.42,
        0.0,
        0.2,
        0.2,
        0.9,
        toenen(f, -0.12),
    );
    g.block(
        breite / 2.0 - 0.1,
        0.42,
        0.0,
        0.2,
        0.2,
        0.9,
        toenen(f, -0.12),
    );
    // Kissen.
    let n = (breite / 1.2).round().max(1.0) as usize;
    for i in 0..n {
        let px = -breite / 2.0 + breite * (i as f32 + 0.5) / n as f32;
        g.block(
            px,
            0.42,
            0.05,
            breite / n as f32 - 0.08,
            0.08,
            0.7,
            toenen(f, 0.15),
        );
    }
}

/// Ein Barhocker.
pub fn hocker(b: &mut Bau, x: f32, y: f32, z: f32, c: &str) {
    b.saeule(x, y, z, 0.2, 0.03, farbe("#343a40"));
    b.saeule(x, y, z, 0.03, 0.72, farbe(farben::ALU));
    b.saeule(x, y + 0.72, z, 0.2, 0.06, farbe(c));
}

/// Eine Wandtafel (Rahmen, Fläche) mit Blick `rot_y`; gibt die Fläche zurück (zum Bekleben).
pub fn tafel(
    b: &mut Bau,
    x: f32,
    y: f32,
    z: f32,
    rot_y: f32,
    breite: f32,
    hoehe: f32,
    flaeche: &str,
    rahmen: &str,
) -> SceneNode3d {
    let mut g = b.gruppe(x, y, z, rot_y);
    g.quader(
        0.0,
        0.0,
        0.0,
        breite + 0.12,
        hoehe + 0.12,
        0.05,
        farbe(rahmen),
    );
    let f = g.flaeche(0.0, 0.0, 0.03, breite, hoehe, 0.0, farbe(flaeche));
    // Ablage unten.
    g.quader(
        0.0,
        -hoehe / 2.0 - 0.06,
        0.08,
        breite * 0.8,
        0.04,
        0.1,
        farbe(rahmen),
    );
    f
}

/// Bunte Zettel auf einer Tafel (Issues, Warteschlange, PRs): `n` Stück, je nach Farbe.
pub fn zettel(
    b: &mut Bau,
    x: f32,
    y: f32,
    z: f32,
    rot_y: f32,
    breite: f32,
    hoehe: f32,
    farben_: &[&str],
) {
    let mut g = b.gruppe(x, y, z, rot_y);
    let spalten = 6;
    for (i, c) in farben_.iter().enumerate().take(24) {
        let sx = -breite / 2.0 + 0.5 + (i % spalten) as f32 * (breite - 1.0) / (spalten - 1) as f32;
        let sy = hoehe / 2.0 - 0.45 - (i / spalten) as f32 * 0.62;
        g.flaeche(sx, sy, 0.035 + i as f32 * 0.0005, 0.62, 0.48, 0.0, farbe(c));
    }
}
