//! Die Dachterrasse: Holzdeck mit Glasgeländer, Bühne mit DJ-Pult und großer Leinwand,
//! LED-Tanzfläche, Bar mit Hockern, Feuerstelle mit Sofas, Liegen, Stehtische, Darts und Axtwurf.
//! Der Aufzug kommt an derselben Stelle an wie in den Etagen.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;

use super::einrichtung::{hocker, pflanze, sofa};
use super::{Bau, farbe, toenen};
use crate::farben;
use crate::grundriss::dach::*;
use crate::grundriss::sitze;
use crate::grundriss::*;
use crate::kollision::Kasten;

pub fn bauen(b: &mut Bau) {
    deck(b);
    aufzughaus(b);
    buehne(b);
    tanzflaeche(b);
    bar(b);
    lounge(b);
    spiele(b);
    for s in sitze::sitze().iter().filter(|s| s.dach) {
        for i in 0..s.plaetze.len() {
            let p = sitze::platz(s, i);
            b.nutzbar("sitz", p.schluessel.clone(), [p.x, p.y + 0.6, p.z], 1.2);
        }
    }
}

fn deck(b: &mut Bau) {
    let (cx, cz) = BODEN.mitte();
    let holz = farbe("#b08968");
    b.block(
        cx,
        -PLATTE,
        cz,
        BODEN.breite() + 2.0 * WANDSTAERKE,
        PLATTE,
        BODEN.tiefe() + 2.0 * WANDSTAERKE,
        holz,
    );
    b.kasten(
        Kasten::neu(
            BODEN.min_x - WANDSTAERKE,
            BODEN.max_x + WANDSTAERKE,
            BODEN.min_z - WANDSTAERKE,
            BODEN.max_z + WANDSTAERKE,
            0.0,
        )
        .mit_unten(-PLATTE),
    );
    let mut z = BODEN.min_z + 0.5;
    while z < BODEN.max_z {
        b.block(cx, 0.0, z, BODEN.breite(), 0.006, 0.04, toenen(holz, -0.2));
        z += 1.0;
    }
    // Glasgeländer ringsum.
    let weiss = farbe("#ffffff");
    let r = BODEN;
    for (x, z, bx, bz) in [
        (cx, r.min_z - 0.1, r.breite(), 0.06),
        (cx, r.max_z + 0.1, r.breite(), 0.06),
        (r.min_x - 0.1, cz, 0.06, r.tiefe()),
        (r.max_x + 0.1, cz, 0.06, r.tiefe()),
    ] {
        b.block(x, 1.05, z, bx, 0.07, bz, weiss);
        let mut g = b.block(
            x,
            0.0,
            z,
            bx,
            1.05,
            bz * 0.4,
            Color::new(0.84, 0.95, 1.0, 0.25),
        );
        g.enable_backface_culling(false);
        b.kasten(
            Kasten::neu(
                x - bx / 2.0 - 0.05,
                x + bx / 2.0 + 0.05,
                z - bz / 2.0 - 0.05,
                z + bz / 2.0 + 0.05,
                1.12,
            )
            .als_zaun(),
        );
    }
    // Lichterketten über dem Deck.
    for i in 0..24 {
        let x = BODEN.min_x + 1.0 + i as f32 * 1.45;
        let c = farben::STUEHLE[i % 6];
        let mut l = b.kugel(x, 4.2 + (i as f32 * 0.7).sin() * 0.25, 0.0, 0.09, farbe(c));
        l.set_emissive(farbe(c));
    }
    for (x, z) in [(BODEN.min_x + 0.5, 0.0), (BODEN.max_x - 0.5, 0.0)] {
        b.fester_block(x, 0.0, z, 0.12, 4.3, 0.12, farbe("#343a40"));
    }
    pflanze(b, BODEN.min_x + 0.8, BODEN.max_z - 0.8, 1.4);
    pflanze(b, BODEN.max_x - 0.8, BODEN.max_z - 0.8, 1.4);
}

/// Das Häuschen um den Aufzug, mit Türen nach Süden wie in den Etagen.
fn aufzughaus(b: &mut Bau) {
    let wand = farbe(farben::AUSSEN);
    let x0 = AUFZUG_X - AUFZUG_BREITE / 2.0;
    let x1 = AUFZUG_X + AUFZUG_BREITE / 2.0;
    let mz = BODEN.min_z + AUFZUG_TIEFE / 2.0;
    let h = 3.2;
    b.fester_block(x0 + 0.07, 0.0, mz, 0.14, h, AUFZUG_TIEFE, wand);
    b.fester_block(x1 - 0.07, 0.0, mz, 0.14, h, AUFZUG_TIEFE, wand);
    b.fester_block(
        AUFZUG_X,
        0.0,
        BODEN.min_z + 0.07,
        AUFZUG_BREITE,
        h,
        0.14,
        wand,
    );
    let seite = (AUFZUG_BREITE - AUFZUG_TUER) / 2.0;
    let fz = AUFZUG_VORNE - 0.07;
    b.fester_block(x0 + seite / 2.0, 0.0, fz, seite, h, 0.14, wand);
    b.fester_block(x1 - seite / 2.0, 0.0, fz, seite, h, 0.14, wand);
    b.block(
        AUFZUG_X,
        AUFZUG_TUER_HOEHE,
        fz,
        AUFZUG_TUER,
        h - AUFZUG_TUER_HOEHE,
        0.14,
        wand,
    );
    b.block(
        AUFZUG_X,
        h,
        mz,
        AUFZUG_BREITE + 0.3,
        0.15,
        AUFZUG_TIEFE + 0.3,
        toenen(wand, -0.2),
    );
    let stahl = toenen(farbe("#8d99ae"), 0.25);
    let halb = AUFZUG_TUER / 2.0;
    let l = b.block(
        AUFZUG_X - halb / 2.0,
        0.0,
        fz + 0.03,
        halb,
        AUFZUG_TUER_HOEHE,
        0.04,
        stahl,
    );
    let r = b.block(
        AUFZUG_X + halb / 2.0,
        0.0,
        fz + 0.03,
        halb,
        AUFZUG_TUER_HOEHE,
        0.04,
        stahl,
    );
    b.merken("aufzug-tuer-l", l);
    b.merken("aufzug-tuer-r", r);
    b.nutzbar("aufzug", "aufzug", [AUFZUG_X, 1.2, mz], 1.6);
    b.nutzbar(
        "aufzug",
        "aufzug-knopf",
        [x1 + 0.1, 1.2, AUFZUG_VORNE + 0.1],
        1.4,
    );
}

/// Die Bühne mit DJ-Pult und darüber die große Leinwand (benannt "leinwand").
fn buehne(b: &mut Bau) {
    let s = BUEHNE;
    let (cx, cz) = s.mitte();
    b.fester_block(
        cx,
        0.0,
        cz,
        s.breite(),
        BUEHNE_HOEHE,
        s.tiefe(),
        farbe("#2b2d42"),
    );
    // Stufen an der Vorderkante.
    b.fester_block(
        cx,
        0.0,
        s.max_z + 0.2,
        3.0,
        BUEHNE_HOEHE / 2.0,
        0.4,
        farbe("#3d405b"),
    );
    // DJ-Pult mit Plattenspielern.
    let (dx, dz) = DJ;
    b.fester_block(dx, BUEHNE_HOEHE, dz + 0.7, 2.2, 1.0, 0.7, farbe("#14213d"));
    for sx in [-0.55, 0.55] {
        let mut teller = b.saeule(
            dx + sx,
            BUEHNE_HOEHE + 1.0,
            dz + 0.7,
            0.25,
            0.03,
            farbe("#1b1b1b"),
        );
        teller.set_metallic(0.6);
    }
    let mut leiste = b.quader(
        dx,
        BUEHNE_HOEHE + 0.6,
        dz + 1.06,
        2.0,
        0.08,
        0.02,
        farbe("#f72585"),
    );
    leiste.set_emissive(farbe("#f72585"));
    b.nutzbar("dj", "dj", [dx, BUEHNE_HOEHE + 1.0, dz + 1.4], 1.8);
    // Lautsprecher-Türme.
    for sx in [s.min_x + 0.6, s.max_x - 0.6] {
        b.fester_block(
            sx,
            BUEHNE_HOEHE,
            s.min_z + 0.8,
            1.0,
            2.2,
            0.9,
            farbe("#1b1b1b"),
        );
        for y in [0.6, 1.5] {
            b.saeule(
                sx,
                BUEHNE_HOEHE + y,
                s.min_z + 1.26,
                0.3,
                0.02,
                farbe("#495057"),
            )
            .set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
        }
    }
    // Die Leinwand: Gerüst, schwarzer Rahmen, Bildfläche (lyras YouTube-iframe liegt darüber).
    let l = LEINWAND;
    for sx in [l.x - l.breite / 2.0 - 0.25, l.x + l.breite / 2.0 + 0.25] {
        b.fester_block(
            sx,
            BUEHNE_HOEHE,
            l.z - 0.1,
            0.18,
            l.y + l.hoehe / 2.0 + 0.3 - BUEHNE_HOEHE,
            0.18,
            farbe("#495057"),
        );
    }
    b.quader(
        l.x,
        l.y,
        l.z - 0.08,
        l.breite + 0.3,
        l.hoehe + 0.3,
        0.1,
        farbe("#111111"),
    );
    let mut bild = b.flaeche(l.x, l.y, l.z, l.breite, l.hoehe, 0.0, farbe("#05070d"));
    bild.set_emissive(farbe("#05070d"));
    b.merken("leinwand", bild);
    b.nutzbar("leinwand", "leinwand", [l.x, 1.6, l.z + 6.0], 5.0);
}

/// LED-Kacheln, die zur Musik leuchten (benannt "tanz:i").
fn tanzflaeche(b: &mut Bau) {
    let t = TANZFLAECHE;
    let mut i = 0;
    let mut z = t.min_z + 0.5;
    while z < t.max_z {
        let mut x = t.min_x + 0.5;
        while x < t.max_x {
            let c = farben::STUEHLE[(i * 7 + 3) % 6];
            let mut k = b.block(x, 0.0, z, 0.96, 0.03, 0.96, farbe(c));
            k.set_emissive(toenen(farbe(c), -0.4));
            b.merken(format!("tanz:{i}"), k);
            i += 1;
            x += 1.0;
        }
        z += 1.0;
    }
}

fn bar(b: &mut Bau) {
    let k = BAR;
    let mz = (k.min_z + k.max_z) / 2.0;
    let laenge = k.max_z - k.min_z;
    b.fester_block(k.x, 0.0, mz, k.tiefe, k.hoehe, laenge, farbe("#5e3023"));
    b.block(
        k.x,
        k.hoehe,
        mz,
        k.tiefe + 0.2,
        0.06,
        laenge + 0.2,
        farbe("#f7f3ea"),
    );
    // Regal mit Flaschen hinter der Theke.
    let rx = BODEN.max_x - 0.4;
    b.fester_block(rx, 0.0, mz, 0.5, 2.2, laenge, farbe("#7f4f24"));
    for (i, z) in (0..20).map(|i| (i, k.min_z + 0.3 + i as f32 * 0.5)) {
        let c = ["#2a9d8f", "#e9c46a", "#e76f51", "#8ecae6", "#b5179e"][i % 5];
        let mut f = b.saeule(rx - 0.3, 1.3 + (i % 2) as f32 * 0.5, z, 0.05, 0.3, farbe(c));
        f.set_alpha_mode(AlphaMode::Blend);
    }
    for s in sitze::sitze().iter().filter(|s| s.bar) {
        hocker(b, s.x, 0.0, s.z, "#e63946");
    }
    b.nutzbar("bar", "bar", [k.x - 1.0, 1.1, mz], 2.0);
}

fn lounge(b: &mut Bau) {
    let (fx, fz, r) = FEUERSTELLE;
    b.fester_block(fx, 0.0, fz, 2.0 * r, 0.4, 2.0 * r, farbe("#6c757d"));
    for (i, w) in (0..6).map(|i| (i, i as f32 / 6.0 * std::f32::consts::TAU)) {
        let mut f = b.kugel(
            fx + w.sin() * 0.25,
            0.55,
            fz + w.cos() * 0.25,
            0.18 + (i % 2) as f32 * 0.05,
            farbe("#ff9f1c"),
        );
        f.set_emissive(farbe("#ff6b35"));
    }
    for s in sitze::sitze()
        .iter()
        .filter(|s| s.dach && s.id.starts_with("roof-sofa"))
    {
        let breite = if s.plaetze.len() == 3 { 3.4 } else { 2.2 };
        sofa(b, s.x, 0.0, s.z, s.rot_y, breite, "#e9c46a");
    }
    for s in sitze::sitze()
        .iter()
        .filter(|s| s.id.starts_with("roof-lounger"))
    {
        let mut g = b.gruppe(s.x, 0.0, s.z, s.rot_y);
        g.fester_block(0.0, 0.0, 0.0, 0.7, 0.35, 1.9, farbe("#ffffff"));
        let mut lehne = g.block(0.0, 0.35, -0.75, 0.7, 0.06, 0.6, farbe("#48cae4"));
        lehne.set_rotation(Quat::from_axis_angle(Vec3::X, 0.6));
    }
    for (x, z) in STEHTISCHE {
        b.saeule(x, 0.0, z, 0.05, 1.05, farbe("#343a40"));
        b.saeule(x, 1.05, z, 0.4, 0.04, farbe("#f7f3ea"));
        b.kasten(Kasten::um(x, z, 0.8, 0.8, 1.09));
    }
}

/// Axtwurfbahn mit Zielscheibe, Dartscheibe außen an ihrer Ostwand.
fn spiele(b: &mut Bau) {
    let a = AXTBAHN;
    let holz = farbe(farben::HOLZ);
    b.fester_block(
        a.min_x - 0.075,
        0.0,
        (a.min_z + a.max_z) / 2.0,
        0.15,
        2.6,
        a.tiefe(),
        holz,
    );
    b.fester_block(
        a.max_x + 0.075,
        0.0,
        (a.min_z + a.max_z) / 2.0,
        0.15,
        2.6,
        a.tiefe(),
        holz,
    );
    let (zx, zy, zz) = AXT_ZIEL;
    for (r, c) in [
        (0.6, "#ffffff"),
        (0.45, "#e63946"),
        (0.3, "#ffffff"),
        (0.12, "#e63946"),
    ] {
        b.saeule(zx, 0.0, zz, r, 0.04 + (0.6 - r) * 0.02, farbe(c))
            .set_position(Vec3::new(zx, zy, zz + (0.6 - r) * 0.05))
            .set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
    }
    b.nutzbar("axt", "axt", [zx, 1.2, zz + AXT_LINIE + 0.3], 1.4);
    let (dx, dy, dz) = DARTSCHEIBE;
    b.saeule(dx, 0.0, dz, 0.23, 0.04, farbe("#1b1b1b"))
        .set_position(Vec3::new(dx, dy, dz))
        .set_rotation(Quat::from_axis_angle(Vec3::Z, FRAC_PI_2));
    b.saeule(dx + 0.01, 0.0, dz, 0.17, 0.03, farbe("#e9c46a"))
        .set_position(Vec3::new(dx + 0.01, dy, dz))
        .set_rotation(Quat::from_axis_angle(Vec3::Z, FRAC_PI_2));
    b.nutzbar("darts", "darts", [dx + DART_ABSTAND + 0.28, 1.2, dz], 1.4);
    let _ = PI;
}
