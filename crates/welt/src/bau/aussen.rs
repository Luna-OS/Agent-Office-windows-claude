//! Draußen: die Außenhaut des Gebäudes mit den Etagen darunter und darüber, Garage, Hof, Straße,
//! Gehwege, Laternen, Bäume, der Balkon mit Golf-Abschlag, die Treppe vom Ausgang, das Golfloch
//! und die Stadt drumherum.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;

use super::{Bau, farbe, toenen};
use crate::farben;
use crate::grundriss::draussen::*;
use crate::grundriss::*;
use crate::kollision::Kasten;

/// Baut alles draußen für eine Etage mit Index `index` (0 = unterste) von `etagen` Stück.
/// `strasse` ist die Höhe der Straße relativ zum Boden der Etage.
pub fn bauen(b: &mut Bau, index: usize, etagen: usize, dach: bool) {
    let strasse = if dach { -crate::grundriss::dach::dach_tiefe(etagen) } else { strasse_unter(index) };
    gebaeude(b, index, etagen, strasse, dach);
    let mut s = b.gruppe(0.0, strasse, 0.0, 0.0);
    boden(&mut s);
    garage(&mut s);
    strasse_und_hof(&mut s);
    stadt(&mut s);
    if !dach {
        balkon(b);
        if index == 0 {
            treppe_zum_ausgang(b, strasse);
        }
    }
}

/// Die anderen Stockwerke als Außenhaut mit Fensterbändern, damit das Gebäude von draußen hoch aussieht.
fn gebaeude(b: &mut Bau, index: usize, etagen: usize, strasse: f32, dach: bool) {
    let aussen = farbe(farben::AUSSEN);
    let g = GEBAEUDE;
    let (cx, cz) = g.mitte();
    let unter_mir = if dach { etagen } else { index };
    // Unterhalb: von der Garagendecke bis unter die eigene Platte – jede Etage ein Band.
    for i in 0..unter_mir {
        let y0 = strasse + 3.6 - PLATTE + i as f32 * STOCKWERK;
        huelle_band(b, cx, cz, y0, STOCKWERK, aussen);
    }
    // Darüber: die Etagen über einem, und das Dach-Geländer ganz oben.
    if !dach {
        for i in (index + 1)..etagen.max(index + 1) {
            // Über der eigenen Decke (die liegt bei WANDHOEHE..WANDHOEHE + PLATTE).
            let y0 = (i - index) as f32 * STOCKWERK + 0.01;
            huelle_band(b, cx, cz, y0, STOCKWERK - 0.02, aussen);
        }
        let oben = (etagen.max(index + 1) - index) as f32 * STOCKWERK - PLATTE;
        b.block(cx, oben, cz, g.breite() + 0.4, 0.3, g.tiefe() + 0.4, toenen(aussen, -0.2));
    }
}

fn huelle_band(b: &mut Bau, cx: f32, cz: f32, y0: f32, h: f32, c: Color) {
    let g = GEBAEUDE;
    let mut k = b.block(cx, y0, cz, g.breite(), h, g.tiefe(), c);
    k.set_roughness(0.95);
    // Fensterband ringsum.
    let glas = farbe("#9ec9e8");
    let fh = 2.0;
    let fy = y0 + 1.5;
    b.block(cx, fy, g.max_z + 0.01, g.breite() - 2.0, fh, 0.02, glas);
    b.block(cx, fy, g.min_z - 0.01, g.breite() - 2.0, fh, 0.02, glas);
    b.block(g.min_x - 0.01, fy, cz, 0.02, fh, g.tiefe() - 2.0, glas);
    b.block(g.max_x + 0.01, fy, cz, 0.02, fh, g.tiefe() - 2.0, glas);
    b.block(cx, y0 + h - 0.15, cz, g.breite() + 0.1, 0.15, g.tiefe() + 0.1, toenen(c, -0.15));
}

/// Rasen ringsum (ganz unten, damit man nicht ins Leere fällt).
fn boden(b: &mut Bau) {
    b.block(0.0, -0.2, 20.0, 400.0, 0.2, 400.0, farbe("#8ac926"));
}

fn garage(b: &mut Bau) {
    let g = GEBAEUDE;
    let (cx, cz) = g.mitte();
    let beton = farbe("#adb5bd");
    let hoehe = 3.6 - PLATTE;
    b.block(cx, 0.0, cz, g.breite(), 0.02, g.tiefe(), toenen(beton, -0.1));
    // Rück- und Westwand, Säulen vorne.
    b.fester_block(cx, 0.0, g.min_z + WANDSTAERKE / 2.0, g.breite(), hoehe, WANDSTAERKE, farbe(farben::AUSSEN));
    b.fester_block(g.min_x + WANDSTAERKE / 2.0, 0.0, cz, WANDSTAERKE, hoehe, g.tiefe(), farbe(farben::AUSSEN));
    for x in [-9.6, 0.0, 9.6] {
        b.fester_block(x, 0.0, g.max_z - 0.3, 0.5, hoehe, 0.5, beton);
        b.fester_block(x, 0.0, cz, 0.5, hoehe, 0.5, beton);
    }
    // Parkplatz-Linien.
    for a in AUTOS.iter().filter(|a| GEBAEUDE.enthaelt(a.x, a.z)) {
        for s in [-1.25, 1.25] {
            let (lx, lz) = (a.x + s * a.rot_y.cos(), a.z);
            b.block(lx, 0.02, lz, 0.08, 0.005, 4.8, farbe("#ffffff"));
        }
    }
}

fn strasse_und_hof(b: &mut Bau) {
    let asphalt = farbe("#495057");
    let gehweg = farbe("#ced4da");
    let (z0, z1) = STRASSE;
    // Hof vor der Garage und an der Ostseite.
    let h = HOF;
    b.block((h.min_x + h.max_x) / 2.0, 0.0, (h.min_z + h.max_z) / 2.0, h.breite(), 0.02, h.tiefe(), toenen(asphalt, 0.2));
    let s = SEITENHOF;
    b.block((s.min_x + s.max_x) / 2.0, 0.0, (s.min_z + s.max_z) / 2.0, s.breite(), 0.02, s.tiefe(), toenen(asphalt, 0.2));
    // Gehwege und Straße mit Mittelstreifen.
    b.block(0.0, 0.0, (21.0 + z0) / 2.0, 2.0 * STRASSE_ENDE, 0.12, z0 - 21.0, gehweg);
    b.block(0.0, 0.0, (z0 + z1) / 2.0, 2.0 * STRASSE_ENDE, 0.03, z1 - z0, asphalt);
    b.block(0.0, 0.0, z1 + 1.5, 2.0 * STRASSE_ENDE, 0.12, 3.0, gehweg);
    let mut x = -STRASSE_ENDE + 2.0;
    while x < STRASSE_ENDE {
        b.block(x, 0.03, (z0 + z1) / 2.0, 2.0, 0.005, 0.15, farbe("#ffffff"));
        x += 5.0;
    }
    // Laternen und Bäume am Gehweg.
    let mut x = -60.0;
    while x <= 60.0 {
        laterne(b, x, z0 - 0.6);
        baum(b, x + 6.0, z1 + 1.8, 1.0 + ((x * 0.37).sin().abs()) * 0.4);
        x += 12.0;
    }
    // Das Golfloch auf der anderen Straßenseite.
    let (gx, gz) = GOLF_LOCH;
    b.block(gx, 0.0, gz, 11.0, 0.03, 11.0, farbe("#52b788"));
    b.block(-5.5, 0.0, (z1 + 3.0 + gz - 5.5) / 2.0, 11.0, 0.02, gz - 5.5 - z1 - 3.0, farbe("#74c69d"));
    b.saeule(gx, 0.0, gz, 0.11, 0.04, farbe("#1b1b1b"));
    b.saeule(gx, 0.0, gz, 0.02, 2.2, farbe("#ffffff"));
    b.block(gx + 0.3, 1.8, gz, 0.6, 0.4, 0.02, farbe("#ef476f"));
}

pub fn laterne(b: &mut Bau, x: f32, z: f32) {
    b.saeule(x, 0.0, z, 0.08, 4.2, farbe("#343a40"));
    let mut l = b.kugel(x, 4.3, z, 0.25, farbe("#fff3c4"));
    l.set_emissive(farbe("#ffe08a"));
    b.kasten(Kasten::um(x, z, 0.2, 0.2, 4.2).mit_unten(b.welt(0.0, 0.0, 0.0)[1]));
}

pub fn baum(b: &mut Bau, x: f32, z: f32, s: f32) {
    b.saeule(x, 0.0, z, 0.18 * s, 2.0 * s, farbe("#7f4f24"));
    b.kugel(x, 2.6 * s, z, 1.3 * s, farbe("#52b788"));
    b.kugel(x + 0.5 * s, 2.2 * s, z + 0.3 * s, 0.9 * s, farbe("#40916c"));
}

/// Häuser der Nachbarschaft: Reihen hinter der Straße und hinter dem Gebäude.
fn stadt(b: &mut Bau) {
    let farben_ = ["#f4a261", "#e9c46a", "#2a9d8f", "#bde0fe", "#ffafcc", "#cdb4db", "#a8dadc", "#e76f51"];
    let mut i = 0usize;
    for (z, tief) in [(48.0, 12.0), (-36.0, 14.0)] {
        let mut x = -100.0;
        while x < 100.0 {
            let breite = 10.0 + (i * 7 % 6) as f32;
            let hoch = 8.0 + (i * 13 % 9) as f32 * 3.0;
            // Lücke fürs Golfloch gegenüber.
            if !(z > 0.0 && (-14.0..4.0).contains(&x)) {
                let c = farbe(farben_[i % farben_.len()]);
                b.block(x + breite / 2.0, 0.0, z + if z > 0.0 { tief / 2.0 } else { -tief / 2.0 }, breite - 1.0, hoch, tief, c);
                b.block(x + breite / 2.0, hoch, z + if z > 0.0 { tief / 2.0 } else { -tief / 2.0 }, breite - 0.6, 0.4, tief + 0.4, toenen(c, -0.2));
            }
            x += breite;
            i += 1;
        }
    }
    // Ein paar Hochhäuser in der Ferne.
    for (x, z, h) in [(-140.0, -80.0, 60.0), (-90.0, -120.0, 85.0), (60.0, -110.0, 70.0), (130.0, -70.0, 50.0), (150.0, 90.0, 65.0), (-150.0, 100.0, 55.0)] {
        b.block(x, 0.0, z, 18.0, h, 18.0, farbe("#8d99ae"));
    }
}

/// Der Balkon vor der Südwand, mit Geländer, Aschenbecher, Bank, Bistrotisch und Golf-Abschlag.
fn balkon(b: &mut Bau) {
    let k = BALKON;
    let (cx, cz) = k.mitte();
    let holz = farbe(farben::HOLZ);
    b.block(cx, -0.2, cz, k.breite(), 0.2, k.tiefe(), holz);
    b.kasten(Kasten::neu(k.min_x, k.max_x, k.min_z - WANDSTAERKE, k.max_z, 0.0).mit_unten(-0.2));
    // Geländer: Glas mit Handlauf, an drei Seiten.
    let rand = farbe("#ffffff");
    for (x, z, bx, bz) in [(cx, k.max_z, k.breite(), 0.06), (k.min_x, cz, 0.06, k.tiefe()), (k.max_x, cz, 0.06, k.tiefe())] {
        b.block(x, 1.02, z, bx, 0.06, bz, rand);
        let mut glas = b.block(x, 0.0, z, bx * 0.98, 1.0, bz * 0.5, Color::new(0.84, 0.95, 1.0, 0.25));
        glas.enable_backface_culling(false);
        b.kasten(Kasten::neu(x - bx / 2.0, x + bx / 2.0, z - bz / 2.0 - 0.05, z + bz / 2.0 + 0.05, 1.08).als_zaun());
    }
    // Aschenbecher (Raucherpause).
    let (ax, az) = ASCHENBECHER;
    b.saeule(ax, 0.0, az, 0.15, 0.9, farbe("#6c757d"));
    b.nutzbar("aschenbecher", "aschenbecher", [ax, 1.0, az - 0.6], 1.4);
    // Bank unter dem Fenster und Bistrotisch mit zwei Hockern.
    b.fester_block(-9.0, 0.0, k.min_z + 0.3, 1.6, 0.45, 0.5, holz);
    b.saeule(0.2, 0.0, cz + 0.2, 0.35, 0.75, farbe("#343a40"));
    super::einrichtung::hocker(b, -0.6, 0.0, cz + 0.2, "#e76f51");
    super::einrichtung::hocker(b, 1.0, 0.0, cz + 0.2, "#e76f51");
    // Golf-Abschlag: Rasen-Quadrat, Ball, Tasche an der Wand.
    let (gx, gz) = GOLF_ABSCHLAG;
    b.block(gx, 0.0, gz, 1.5, 0.02, 1.5, farbe("#52b788"));
    b.kugel(gx - 0.2, 0.07, gz, 0.025, farbe("#ffffff"));
    let mut tasche = b.saeule(-5.8, 0.0, k.min_z + 0.28, 0.16, 0.9, farbe("#3a86ff"));
    tasche.set_rotation(Quat::from_axis_angle(Vec3::X, -0.15));
    b.nutzbar("golf", "golf", [gx, 1.0, gz], 1.4);
    // Lampe neben den Balkontüren.
    let (lx, lz) = (BALKON_TUER.u - BALKON_TUER.breite / 2.0 - 0.4, BODEN.max_z + WANDSTAERKE + 0.08);
    let mut l = b.kugel(lx, 2.6, lz, 0.12, farbe("#fff7d6"));
    l.set_emissive(farbe("#ffe08a"));
}

/// Vom Ausgang in der Westwand: Podest und Treppe hinunter zur Straße.
fn treppe_zum_ausgang(b: &mut Bau, strasse: f32) {
    let x0 = BODEN.min_x - WANDSTAERKE - 1.6;
    let x1 = BODEN.min_x - WANDSTAERKE;
    let xm = (x0 + x1) / 2.0;
    let beton = farbe("#adb5bd");
    let (lz0, lz1) = (5.6, 7.5);
    b.block(xm, -0.2, (lz0 + lz1) / 2.0, 1.6, 0.2, lz1 - lz0, beton);
    b.kasten(Kasten::neu(x0, x1 + WANDSTAERKE, lz0, lz1, 0.0).mit_unten(-0.2));
    let stufen = 15;
    let lauf = 0.34;
    let steigung = -strasse / stufen as f32;
    for i in 0..stufen {
        let oben = -(i as f32 + 1.0) * steigung;
        let z = lz1 + (i as f32 + 0.5) * lauf;
        b.block(xm, oben - 0.15, z, 1.6, 0.15, lauf, toenen(beton, if i % 2 == 0 { 0.0 } else { -0.06 }));
        b.kasten(Kasten::neu(x0, x1, z - lauf / 2.0, z + lauf / 2.0, oben).mit_unten(oben - 0.15));
    }
    // Geländer außen.
    b.block(x0 - 0.03, 0.0, (lz0 + lz1 + stufen as f32 * lauf) / 2.0, 0.05, 1.0, lz1 - lz0 + stufen as f32 * lauf, farbe("#495057"));
    b.kasten(Kasten::neu(x0 - 0.1, x0, lz0, lz1 + stufen as f32 * lauf, 1.1).als_zaun());
    // Das EXIT-Schild über der Tür.
    let mut schild = b.quader(BODEN.min_x - 0.02, AUSGANG.oben + 0.3, AUSGANG.u, 0.04, 0.3, 0.7, farbe("#2a9d4b"));
    schild.set_emissive(farbe("#1f7a38"));
    let _ = (FRAC_PI_2, PI);
}
