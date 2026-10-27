//! Die Burghalle in 3D: Steinboden mit rotem Teppich, Wände mit bunten Glasfenstern, Säulen,
//! Holzdach, Podest mit Thron aus Eisenklingen, Tafeln mit Bänken und Folianten, Pulte, Ratstisch,
//! Wandtafeln, Feuerschalen, Kamin, Kronleuchter, Gong und ein Fass Bier. Der Kerker: `kerker.rs`.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;

use super::{Bau, farbe, toenen};
use crate::grundriss::burg::*;
use crate::grundriss::{Board, TischArt, tisch_sitz};
use crate::kollision::Kasten;

const STEIN: &str = "#8c847a";
const BODEN: &str = "#6f685f";
const TEPPICH: &str = "#8e1b1b";
const HOLZ: &str = "#6b4526";
const GOLD: &str = "#d9ab2e";

pub fn bauen(b: &mut Bau) {
    huelle(b);
    podest_und_thron(b);
    tafeln(b);
    pulte_und_rat(b);
    feuer(b);
    schmuck(b);
    super::kerker::bauen(b);
}

fn huelle(b: &mut Bau) {
    let stein = farbe(STEIN);
    let h = HALLE;
    // Boden, mit einem Loch über der Kerkertreppe (Kollision: zwei Teile drumherum).
    let (tx0, tx1) = (
        TREPPE_X - TREPPE_BREITE / 2.0,
        TREPPE_X + TREPPE_BREITE / 2.0,
    );
    let treppe_z1 = KERKER.min_z + super::kerker::TREPPE_LAUF;
    for (x0, x1, z0, z1) in [
        (h.min_x, h.max_x, h.min_z, KERKER.min_z),
        (h.min_x, tx0, KERKER.min_z, h.max_z),
        (tx1, h.max_x, KERKER.min_z, h.max_z),
        (tx0, tx1, treppe_z1, h.max_z),
    ] {
        b.block(
            (x0 + x1) / 2.0,
            -0.3,
            (z0 + z1) / 2.0,
            x1 - x0,
            0.3,
            z1 - z0,
            farbe(BODEN),
        );
        b.kasten(Kasten::neu(x0, x1, z0, z1, 0.0).mit_unten(-0.3));
    }
    // Fugen im Steinboden.
    let mut z = h.min_z + 2.0;
    while z < h.max_z {
        b.block(
            0.0,
            0.0,
            z,
            BREITE,
            0.004,
            0.05,
            toenen(farbe(BODEN), -0.25),
        );
        z += 2.0;
    }
    // Teppich vom Tor bis zum Podest.
    b.block(0.0, 0.0, 3.35, 3.6, 0.015, 52.5, farbe(TEPPICH));
    b.block(0.0, 0.0, 3.35, 3.9, 0.012, 52.8, farbe(GOLD));
    // Wände mit Sockel.
    for (x, z, bx, bz) in [
        (0.0, h.min_z - 0.4, BREITE + 1.6, 0.8),
        (0.0, h.max_z + 0.4, BREITE + 1.6, 0.8),
        (h.min_x - 0.4, 0.0, 0.8, LAENGE),
        (h.max_x + 0.4, 0.0, 0.8, LAENGE),
    ] {
        b.fester_block(x, 0.0, z, bx, HOEHE, bz, stein);
    }
    for x in [h.min_x + 0.1, h.max_x - 0.1] {
        b.block(x, 0.0, 0.0, 0.2, 1.0, LAENGE, toenen(stein, -0.15));
    }
    // Das Tor im Süden.
    let mut tor = b.block(0.0, 0.0, h.max_z - 0.02, 4.0, 5.5, 0.1, farbe(HOLZ));
    tor.set_roughness(0.95);
    b.block(0.0, 5.5, h.max_z - 0.03, 4.4, 0.4, 0.12, farbe("#3a3a3a"));
    // Holzdach: Balken quer über der Halle, darüber die Dachfläche.
    let mut z = h.min_z + 3.0;
    while z < h.max_z {
        b.block(0.0, HOEHE - 1.2, z, BREITE, 0.5, 0.45, farbe(HOLZ));
        z += 6.0;
    }
    let mut dach = b.block(
        0.0,
        HOEHE,
        0.0,
        BREITE + 1.6,
        0.4,
        LAENGE + 1.6,
        toenen(farbe(HOLZ), -0.3),
    );
    dach.set_emissive(toenen(farbe(HOLZ), -0.7));
    // Säulen mit Spitzbögen dazwischen (als Querbalken angedeutet).
    for z in SAEULEN_Z {
        for s in [-1.0f32, 1.0] {
            b.fester_block(
                s * SAEULE_X,
                0.0,
                z,
                0.9,
                HOEHE - 1.2,
                0.9,
                toenen(stein, 0.08),
            );
            b.block(s * SAEULE_X, 0.0, z, 1.2, 0.5, 1.2, toenen(stein, -0.1));
        }
    }
    for s in [-1.0f32, 1.0] {
        b.block(
            s * SAEULE_X,
            HOEHE - 2.6,
            0.0,
            0.7,
            0.6,
            LAENGE - 4.0,
            toenen(stein, -0.05),
        );
    }
    // Bunte Glasfenster hoch in jedem Joch, und Rosetten über Thron und Tor.
    let glas = ["#e63946", "#457b9d", "#e9c46a", "#2a9d8f", "#9d4edd"];
    for (i, z) in [-21.0f32, -15.0, -9.0, -3.0, 3.0, 9.0, 15.0, 21.0]
        .into_iter()
        .enumerate()
    {
        for s in [-1.0f32, 1.0] {
            if s < 0.0 && z == KAMIN.1 {
                continue;
            }
            let c = farbe(glas[(i + (s > 0.0) as usize) % glas.len()]);
            let mut f = b.flaeche(
                s * (BREITE / 2.0 - 0.05),
                6.4,
                z,
                2.3,
                5.2,
                -s * FRAC_PI_2,
                c,
            );
            f.set_emissive(toenen(c, -0.35));
        }
    }
    for (z, rot) in [(h.min_z + 0.05, 0.0), (h.max_z - 0.05, PI)] {
        let mut r = b.saeule(0.0, 0.0, z, 2.2, 0.05, farbe("#e9c46a"));
        r.set_position(Vec3::new(0.0, 10.4, z));
        r.set_rotation(
            Quat::from_axis_angle(Vec3::X, FRAC_PI_2) * Quat::from_axis_angle(Vec3::Y, rot),
        );
        r.set_emissive(farbe("#9c6644"));
    }
}

fn podest_und_thron(b: &mut Bau) {
    let (pb, pt, ph) = PODEST;
    let (tx, tz) = THRON;
    let stein = toenen(farbe(STEIN), 0.1);
    for i in 0..3 {
        let h = ph * (3 - i) as f32 / 3.0;
        let tiefe = pt + i as f32 * 0.6;
        b.block(
            tx,
            0.0,
            tz + (tiefe - pt) / 2.0,
            pb + i as f32 * 0.6,
            h,
            tiefe,
            toenen(stein, -0.05 * i as f32),
        );
        b.kasten(
            Kasten::um(tx, tz + (tiefe - pt) / 2.0, pb + i as f32 * 0.6, tiefe, h).mit_unten(0.0),
        );
    }
    // Der Thron: Sitz, hohe Lehne, ein Fächer aus Klingen dahinter.
    let eisen = farbe("#5c5f66");
    let mut g = b.gruppe(tx, ph, tz, 0.0);
    g.fester_block(0.0, 0.0, 0.0, 1.9, 0.55, 1.9, eisen);
    g.block(0.0, 0.55, -0.75, 1.9, 2.6, 0.4, toenen(eisen, -0.1));
    for i in 0..15 {
        let w = (i as f32 - 7.0) * 0.12;
        let mut klinge = g.quader(
            w.sin() * 1.2,
            2.6 + w.cos() * 0.9,
            -0.85,
            0.08,
            1.8 + (i % 3) as f32 * 0.3,
            0.03,
            farbe("#9aa0a8"),
        );
        klinge.set_rotation(Quat::from_axis_angle(Vec3::Z, -w));
        klinge.set_metallic(0.9);
    }
    g.nutzbar("thron", "thron", [0.0, 1.2, 1.2], 1.8);
    // Der große Banner hinter dem Thron, Kerzen daneben, Ritterstatuen.
    b.flaeche(0.0, 7.7, HALLE.min_z + 0.12, 3.4, 6.0, 0.0, farbe(TEPPICH));
    b.flaeche(0.0, 8.6, HALLE.min_z + 0.13, 1.4, 1.4, 0.0, farbe(GOLD));
    for x in [-5.0f32, 5.0] {
        b.fester_block(x, ph, -28.6, 0.9, 0.5, 0.9, stein);
        b.saeule(x, ph + 0.5, -28.6, 0.28, 1.4, toenen(stein, -0.1));
        b.kugel(x, ph + 2.2, -28.6, 0.3, toenen(stein, -0.1));
    }
    // Die Hand des Königs (eine Figur in Rot und Gold, nur angedeutet).
    let (hx, hz, _) = HAND;
    b.saeule(hx, ph, hz, 0.3, 1.2, farbe("#7b2cbf"));
    b.kugel(hx, ph + 1.45, hz, 0.22, farbe("#f1c27d"));
    b.saeule(hx, ph + 1.6, hz, 0.24, 0.12, farbe(GOLD));
    b.nutzbar("hand", "hand", [hx, ph + 1.0, hz + 0.8], 2.0);
}

fn tafeln(b: &mut Bau) {
    let holz = farbe(HOLZ);
    for t in TAFELN {
        b.fester_block(
            t.x,
            0.0,
            t.z,
            TAFEL_BREITE,
            0.78,
            t.laenge,
            toenen(holz, 0.1),
        );
        for s in [-1.0f32, 1.0] {
            b.fester_block(
                t.x + s * (TAFEL_BREITE / 2.0 + 0.52),
                0.0,
                t.z,
                0.42,
                0.45,
                t.laenge - 0.2,
                holz,
            );
        }
    }
    // Ein aufgeschlagener Foliant an jedem Platz (die "Monitore" der Burg).
    for t in tische()
        .into_iter()
        .filter(|t| t.art == TischArt::Schreibtisch)
    {
        let mut g = b.gruppe(t.x, 0.78, t.z, t.rot_y);
        g.block(0.0, 0.0, 0.0, 0.62, 0.05, 0.42, farbe("#5e3023"));
        let mut seite = g.flaeche(0.0, 0.056, 0.0, 0.56, 0.38, 0.0, farbe("#f4e9cd"));
        seite.set_rotation(Quat::from_axis_angle(Vec3::X, -FRAC_PI_2));
        seite.set_emissive(farbe("#2a2418"));
        b.merken(format!("monitor:{}", t.id), seite);
        let (sx, sz) = tisch_sitz(&t, 0.85);
        b.nutzbar("tisch", t.id.clone(), [sx, 1.0, sz], 1.5);
    }
}

fn pulte_und_rat(b: &mut Bau) {
    let holz = farbe(HOLZ);
    for t in tische() {
        match t.art {
            TischArt::Kiosk(_) => {
                let mut g = b.gruppe(t.x, 0.0, t.z, t.rot_y);
                g.fester_block(0.0, 0.0, 0.0, 0.7, 1.05, 0.5, holz);
                let mut pult = g.quader(0.0, 1.12, 0.0, 0.8, 0.06, 0.6, toenen(holz, 0.15));
                pult.set_rotation(Quat::from_axis_angle(Vec3::X, 0.35));
                let (sx, sz) = tisch_sitz(&t, -1.0);
                b.nutzbar("tisch", t.id.clone(), [sx, 1.0, sz], 1.6);
            }
            TischArt::Besprechung => {
                let (sx, sz) = tisch_sitz(&t, 0.85);
                b.saeule(sx, 0.0, sz, 0.22, 0.48, holz);
            }
            _ => {}
        }
    }
    let (rx, rz, rr) = RAT;
    b.saeule(rx, 0.0, rz, 0.15, 0.78, holz);
    b.saeule(rx, 0.74, rz, rr, 0.06, toenen(holz, 0.15));
    b.kasten(Kasten::um(rx, rz, rr * 2.0, rr * 2.0, 0.8));
    // Wandtafeln mit ihren Namen.
    for (board, name) in [
        (Board::Issues, "Issues"),
        (Board::Warteschlange, "Warteschlange"),
        (Board::PullRequests, "Pull Requests"),
        (Board::Dienste, "Dienste"),
    ] {
        let t = tafel(board);
        let f = super::einrichtung::tafel(
            b, t.x, t.y, t.z, t.rot_y, t.breite, t.hoehe, "#e9d8a6", HOLZ,
        );
        b.merken(format!("tafel:{name}"), f);
        let (vx, vz) = (t.rot_y.sin() * 1.4, t.rot_y.cos() * 1.4);
        b.nutzbar("tafel", name, [t.x + vx, t.y - 1.2, t.z + vz], 2.8);
    }
}

fn feuerschale(b: &mut Bau, x: f32, z: f32) {
    b.saeule(x, 0.0, z, 0.08, 1.0, farbe("#3a3a3a"));
    b.saeule(x, 1.0, z, 0.45, 0.2, farbe("#3a3a3a"));
    for (dx, dz, r) in [(0.0, 0.0, 0.28), (0.12, 0.08, 0.18), (-0.1, -0.06, 0.2)] {
        let mut f = b.kugel(x + dx, 1.35, z + dz, r, farbe("#ff9f1c"));
        f.set_emissive(farbe("#ff6b35"));
    }
    b.kasten(Kasten::um(x, z, 0.9, 0.9, 1.2));
}

fn feuer(b: &mut Bau) {
    for (x, z) in [(-3.7, -21.4), (3.7, -21.4), (-3.7, 17.0), (3.7, 17.0)] {
        feuerschale(b, x, z);
    }
    // Der Kamin an der Westwand.
    let (kx, kz) = KAMIN;
    b.fester_block(kx, 0.0, kz, 1.1, 2.4, 3.2, toenen(farbe(STEIN), -0.2));
    b.block(kx + 0.2, 0.3, kz, 0.9, 1.4, 2.0, farbe("#1b1b1b"));
    for dz in [-0.4f32, 0.0, 0.4] {
        let mut f = b.kugel(kx + 0.5, 0.5, kz + dz, 0.25, farbe("#ff9f1c"));
        f.set_emissive(farbe("#ff6b35"));
    }
    b.block(
        kx - 0.1,
        2.4,
        kz,
        0.9,
        HOEHE - 2.4,
        1.6,
        toenen(farbe(STEIN), -0.1),
    );
    // Kronleuchter in der Mitte, jeder mit warmem Licht.
    for z in [-15.0f32, -3.0, 9.0, 21.0] {
        let mut l = b.knoten.add_light(Light::point(22.0));
        l.set_position(Vec3::new(0.0, 8.8, z));
        l.modify_light(|l| {
            l.color = farbe("#ffd29b");
            l.intensity = 1.2;
            l.casts_shadows = false;
        });
        b.saeule(0.0, 9.2, z, 0.03, HOEHE - 9.2, farbe("#3a3a3a"));
        b.saeule(0.0, 9.2, z, 1.2, 0.08, farbe("#3a3a3a"));
        for i in 0..8 {
            let w = i as f32 / 8.0 * std::f32::consts::TAU;
            let mut k = b.kugel(
                w.sin() * 1.15,
                9.45,
                z + w.cos() * 1.15,
                0.07,
                farbe("#fff3c4"),
            );
            k.set_emissive(farbe("#ffd166"));
        }
    }
    // Fackeln an den Säulen.
    for z in [-24.0f32, -12.0, 0.0, 12.0, 24.0] {
        for s in [-1.0f32, 1.0] {
            let mut f = b.kugel(s * (SAEULE_X - 0.6), 2.9, z, 0.12, farbe("#ff9f1c"));
            f.set_emissive(farbe("#ff6b35"));
        }
    }
}

fn schmuck(b: &mut Bau) {
    // Banner an den Säulen, Schilde in den Jochen ohne Tafel.
    let farben_ = ["#8e1b1b", "#1d3557", "#2a9d8f"];
    for (i, z) in [-18.0f32, -6.0, 6.0, 18.0].into_iter().enumerate() {
        for s in [-1.0f32, 1.0] {
            b.flaeche(
                s * (SAEULE_X - 0.47),
                8.7,
                z,
                1.25,
                5.0,
                -s * FRAC_PI_2,
                farbe(farben_[i % 3]),
            );
        }
    }
    for z in [-21.0f32, -9.0, 9.0, 21.0] {
        for s in [-1.0f32, 1.0] {
            let mut sch = b.saeule(s * (BREITE / 2.0 - 0.1), 0.0, z, 0.55, 0.06, farbe(GOLD));
            sch.set_position(Vec3::new(s * (BREITE / 2.0 - 0.1), 3.6, z));
            sch.set_rotation(Quat::from_axis_angle(Vec3::Z, FRAC_PI_2));
        }
    }
    // Rüstungen am Tor.
    for (x, z) in [(-4.2f32, 28.4f32), (4.2, 28.4), (-12.1, 24.5), (12.1, 24.5)] {
        b.fester_block(x, 0.0, z, 0.5, 1.2, 0.4, farbe("#adb5bd"));
        b.kugel(x, 1.45, z, 0.22, farbe("#adb5bd"))
            .set_metallic(0.9);
    }
    // Gong westlich vom Podest, Fass Bier an der Ostwand.
    let (gx, gz) = GONG;
    for dz in [-0.9f32, 0.9] {
        b.fester_block(gx, 0.0, gz + dz, 0.12, 2.45, 0.12, farbe(HOLZ));
    }
    b.block(gx, 2.35, gz, 0.12, 0.12, 1.95, farbe(HOLZ));
    let mut gong = b.saeule(gx, 0.0, gz, 0.7, 0.05, farbe("#e9c46a"));
    gong.set_position(Vec3::new(gx, 1.45, gz));
    gong.set_rotation(Quat::from_axis_angle(Vec3::Z, FRAC_PI_2));
    gong.set_metallic(0.8);
    b.merken("gong", gong);
    b.nutzbar("gong", "gong", [gx + 1.0, 1.4, gz], 1.8);
    let (fx, fz) = FASS;
    let mut fass = b.saeule(fx, 0.0, fz, 0.45, 1.0, farbe("#7f4f24"));
    fass.set_position(Vec3::new(fx, 0.75, fz));
    fass.set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
    b.kasten(Kasten::um(fx, fz, 1.0, 1.2, 1.2));
    b.nutzbar("kaffee", "bier", [fx - 1.0, 1.0, fz], 1.6);
}
