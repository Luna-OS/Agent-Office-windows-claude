//! Eine Halle in 3D (die Burg oder eine eigene Karte, `grundriss::halle::Hallenplan`): Steinboden
//! mit rotem Teppich, Wände mit bunten Glasfenstern, Säulen, Holzdach, Podest mit Thron aus
//! Eisenklingen, Tafeln mit Bänken und Folianten, Pulte, Ratstisch, Wandtafeln, Feuerschalen, Kamin,
//! Kronleuchter, Gong und ein Fass Bier. Der Kerker: `kerker.rs`.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;

use super::{Bau, farbe, toenen};
use crate::grundriss::burg::*;
use crate::grundriss::halle::Hallenplan;
use crate::grundriss::{Board, TischArt, tisch_sitz};
use crate::kollision::Kasten;

/// Säulen stehen so weit innen von den Seitenwänden, alle 6 m.
const SAEULE_EIN: f32 = 2.6;
const JOCH: f32 = 6.0;

/// Die Säulen-Reihen entlang der Halle (z), mit Abstand von den Stirnwänden.
fn saeulen_z(p: &Hallenplan) -> Vec<f32> {
    let n = ((p.laenge - 6.0) / JOCH).floor() as i32;
    (0..=n)
        .map(|i| -(n as f32) * JOCH / 2.0 + i as f32 * JOCH)
        .collect()
}

pub fn bauen(b: &mut Bau, p: &Hallenplan) {
    huelle(b, p);
    if p.thron.is_some() {
        podest_und_thron(b, p);
    }
    tafeln(b, p);
    pulte_und_rat(b, p);
    feuer(b, p);
    schmuck(b, p);
    if p.kerker {
        let dz = p.kerker_versatz();
        let mut k = b.gruppe(0.0, 0.0, dz, 0.0);
        super::kerker::bauen(&mut k);
    }
}

fn huelle(b: &mut Bau, p: &Hallenplan) {
    let f = &p.farben;
    let stein = farbe(&f.stein);
    let h = halle(p);
    let (breite, laenge, hoehe) = (p.breite, p.laenge, p.hoehe);
    // Boden, mit einem Loch über der Kerkertreppe.
    let (tx0, tx1) = (
        TREPPE_X - TREPPE_BREITE / 2.0,
        TREPPE_X + TREPPE_BREITE / 2.0,
    );
    let k0 = KERKER.min_z + p.kerker_versatz();
    let stuecke: Vec<(f32, f32, f32, f32)> = if p.kerker {
        let k1 = k0 + super::kerker::TREPPE_LAUF;
        vec![
            (h.min_x, h.max_x, h.min_z, k0),
            (h.min_x, tx0, k0, h.max_z),
            (tx1, h.max_x, k0, h.max_z),
            (tx0, tx1, k1, h.max_z),
        ]
    } else {
        vec![(h.min_x, h.max_x, h.min_z, h.max_z)]
    };
    for (x0, x1, z0, z1) in stuecke {
        b.block(
            (x0 + x1) / 2.0,
            -0.3,
            (z0 + z1) / 2.0,
            x1 - x0,
            0.3,
            z1 - z0,
            farbe(&f.boden),
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
            breite,
            0.004,
            0.05,
            toenen(farbe(&f.boden), -0.25),
        );
        z += 2.0;
    }
    // Teppich vom Tor bis zum Podest (oder zur Nordwand).
    let nord = p
        .thron
        .map(|t| t.z + PODEST.1 / 2.0 + 1.0)
        .unwrap_or(h.min_z + 2.0);
    let sued = h.max_z - 0.6;
    b.block(
        0.0,
        0.0,
        (nord + sued) / 2.0,
        3.6,
        0.015,
        sued - nord,
        farbe(&f.teppich),
    );
    b.block(
        0.0,
        0.0,
        (nord + sued) / 2.0,
        3.9,
        0.012,
        sued - nord + 0.3,
        farbe(&f.gold),
    );
    // Wände mit Sockel.
    for (x, z, bx, bz) in [
        (0.0, h.min_z - 0.4, breite + 1.6, 0.8),
        (0.0, h.max_z + 0.4, breite + 1.6, 0.8),
        (h.min_x - 0.4, 0.0, 0.8, laenge),
        (h.max_x + 0.4, 0.0, 0.8, laenge),
    ] {
        b.fester_block(x, 0.0, z, bx, hoehe, bz, stein);
    }
    for x in [h.min_x + 0.1, h.max_x - 0.1] {
        b.block(x, 0.0, 0.0, 0.2, 1.0, laenge, toenen(stein, -0.15));
    }
    // Das Tor im Süden.
    b.block(0.0, 0.0, h.max_z - 0.02, 4.0, 5.5, 0.1, farbe(&f.holz));
    b.block(0.0, 5.5, h.max_z - 0.03, 4.4, 0.4, 0.12, farbe("#3a3a3a"));
    // Holzdach: Balken quer über der Halle, darüber die Dachfläche.
    let mut z = h.min_z + 3.0;
    while z < h.max_z {
        b.block(0.0, hoehe - 1.2, z, breite, 0.5, 0.45, farbe(&f.holz));
        z += JOCH;
    }
    let mut dach = b.block(
        0.0,
        hoehe,
        0.0,
        breite + 1.6,
        0.4,
        laenge + 1.6,
        toenen(farbe(&f.holz), -0.3),
    );
    dach.set_emissive(toenen(farbe(&f.holz), -0.7));
    // Säulen mit Längsbalken.
    let sx = breite / 2.0 - SAEULE_EIN;
    for z in saeulen_z(p) {
        for s in [-1.0f32, 1.0] {
            b.fester_block(s * sx, 0.0, z, 0.9, hoehe - 1.2, 0.9, toenen(stein, 0.08));
            b.block(s * sx, 0.0, z, 1.2, 0.5, 1.2, toenen(stein, -0.1));
        }
    }
    for s in [-1.0f32, 1.0] {
        b.block(
            s * sx,
            hoehe - 2.6,
            0.0,
            0.7,
            0.6,
            laenge - 4.0,
            toenen(stein, -0.05),
        );
    }
    // Bunte Glasfenster hoch in jedem Joch (nicht über dem Kamin), Rosetten über Thron und Tor.
    let glas = ["#e63946", "#457b9d", "#e9c46a", "#2a9d8f", "#9d4edd"];
    let kz = kamin(p).1;
    let saeulen = saeulen_z(p);
    for (i, w) in saeulen.windows(2).enumerate() {
        let z = (w[0] + w[1]) / 2.0;
        for s in [-1.0f32, 1.0] {
            if s < 0.0 && (z - kz).abs() < 1.0 {
                continue;
            }
            let c = farbe(glas[(i + (s > 0.0) as usize) % glas.len()]);
            let mut fe = b.flaeche(
                s * (breite / 2.0 - 0.05),
                (hoehe * 0.5).max(4.0),
                z,
                2.3,
                (hoehe * 0.4).min(5.2),
                -s * FRAC_PI_2,
                c,
            );
            fe.set_emissive(toenen(c, -0.35));
        }
    }
    for (z, rot) in [(h.min_z + 0.05, 0.0), (h.max_z - 0.05, PI)] {
        let mut r = b.saeule(0.0, 0.0, z, 2.2, 0.05, farbe("#e9c46a"));
        r.set_position(Vec3::new(0.0, hoehe * 0.8, z));
        r.set_rotation(
            Quat::from_axis_angle(Vec3::X, FRAC_PI_2) * Quat::from_axis_angle(Vec3::Y, rot),
        );
        r.set_emissive(farbe("#9c6644"));
    }
}

fn podest_und_thron(b: &mut Bau, p: &Hallenplan) {
    let Some(t) = p.thron else { return };
    let (pb, pt, ph) = PODEST;
    let (tx, tz) = (t.x, t.z);
    let stein = toenen(farbe(&p.farben.stein), 0.1);
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
    // Der große Banner hinter dem Thron, Ritterstatuen.
    let nordwand = -p.laenge / 2.0;
    b.flaeche(
        tx,
        p.hoehe * 0.6,
        nordwand + 0.12,
        3.4,
        (p.hoehe * 0.45).min(6.0),
        0.0,
        farbe(&p.farben.teppich),
    );
    b.flaeche(
        tx,
        p.hoehe * 0.66,
        nordwand + 0.13,
        1.4,
        1.4,
        0.0,
        farbe(&p.farben.gold),
    );
    for x in [tx - 5.0, tx + 5.0] {
        b.fester_block(x, ph, tz - 1.2, 0.9, 0.5, 0.9, stein);
        b.saeule(x, ph + 0.5, tz - 1.2, 0.28, 1.4, toenen(stein, -0.1));
        b.kugel(x, ph + 2.2, tz - 1.2, 0.3, toenen(stein, -0.1));
    }
    // Die Hand des Königs (in Rot und Gold, nur angedeutet).
    if let Some((hx, hz, _)) = hand(p) {
        b.saeule(hx, ph, hz, 0.3, 1.2, farbe("#7b2cbf"));
        b.kugel(hx, ph + 1.45, hz, 0.22, farbe("#f1c27d"));
        b.saeule(hx, ph + 1.6, hz, 0.24, 0.12, farbe(&p.farben.gold));
        b.nutzbar("hand", "hand", [hx, ph + 1.0, hz + 0.8], 2.0);
    }
}

fn tafeln(b: &mut Bau, p: &Hallenplan) {
    let holz = farbe(&p.farben.holz);
    for t in &p.tafeln {
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
    // Ein aufgeschlagener Foliant an jedem Platz (die "Monitore" der Halle).
    for t in tische(p)
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

fn pulte_und_rat(b: &mut Bau, p: &Hallenplan) {
    let holz = farbe(&p.farben.holz);
    for t in tische(p) {
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
    let r = p.rat;
    b.saeule(r.x, 0.0, r.z, 0.15, 0.78, holz);
    b.saeule(r.x, 0.74, r.z, RAT_RADIUS, 0.06, toenen(holz, 0.15));
    b.kasten(Kasten::um(
        r.x,
        r.z,
        RAT_RADIUS * 2.0,
        RAT_RADIUS * 2.0,
        0.8,
    ));
    // Wandtafeln mit ihren Namen.
    for (board, name) in [
        (Board::Issues, "Issues"),
        (Board::Warteschlange, "Warteschlange"),
        (Board::PullRequests, "Pull Requests"),
        (Board::Dienste, "Dienste"),
    ] {
        let t = tafel(p, board);
        let fl = super::einrichtung::tafel(
            b,
            t.x,
            t.y,
            t.z,
            t.rot_y,
            t.breite,
            t.hoehe,
            "#e9d8a6",
            &p.farben.holz,
        );
        b.merken(format!("tafel:{name}"), fl);
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

fn feuer(b: &mut Bau, p: &Hallenplan) {
    let nord = p.thron.map(|t| t.z + 6.0).unwrap_or(-p.laenge / 2.0 + 8.6);
    let sued = p.laenge / 2.0 - 13.0;
    for (x, z) in [(-3.7, nord), (3.7, nord), (-3.7, sued), (3.7, sued)] {
        feuerschale(b, x, z);
    }
    // Der Kamin an der Westwand.
    let (kx, kz) = kamin(p);
    let stein = farbe(&p.farben.stein);
    b.fester_block(kx, 0.0, kz, 1.1, 2.4, 3.2, toenen(stein, -0.2));
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
        p.hoehe - 2.4,
        1.6,
        toenen(stein, -0.1),
    );
    // Kronleuchter in der Mitte, jeder mit warmem Licht.
    let y = p.hoehe * 0.7;
    let saeulen = saeulen_z(p);
    for w in saeulen.windows(2).step_by(2) {
        let z = (w[0] + w[1]) / 2.0;
        let mut l = b.knoten.add_light(Light::point(22.0));
        l.set_position(Vec3::new(0.0, y - 0.4, z));
        l.modify_light(|l| {
            l.color = farbe("#ffd29b");
            l.intensity = 1.2;
            l.casts_shadows = false;
        });
        b.saeule(0.0, y, z, 0.03, p.hoehe - y, farbe("#3a3a3a"));
        b.saeule(0.0, y, z, 1.2, 0.08, farbe("#3a3a3a"));
        for i in 0..8 {
            let a = i as f32 / 8.0 * std::f32::consts::TAU;
            let mut k = b.kugel(
                a.sin() * 1.15,
                y + 0.25,
                z + a.cos() * 1.15,
                0.07,
                farbe("#fff3c4"),
            );
            k.set_emissive(farbe("#ffd166"));
        }
    }
    // Fackeln an jeder zweiten Säule.
    let sx = p.breite / 2.0 - SAEULE_EIN;
    for z in saeulen.iter().step_by(2) {
        for s in [-1.0f32, 1.0] {
            let mut f = b.kugel(s * (sx - 0.6), 2.9, *z, 0.12, farbe("#ff9f1c"));
            f.set_emissive(farbe("#ff6b35"));
        }
    }
}

fn schmuck(b: &mut Bau, p: &Hallenplan) {
    let sx = p.breite / 2.0 - SAEULE_EIN;
    let saeulen = saeulen_z(p);
    // Banner an jeder zweiten Säule (versetzt zu den Fackeln).
    let farben_ = ["#8e1b1b", "#1d3557", "#2a9d8f"];
    for (i, z) in saeulen.iter().skip(1).step_by(2).enumerate() {
        for s in [-1.0f32, 1.0] {
            b.flaeche(
                s * (sx - 0.47),
                p.hoehe * 0.67,
                *z,
                1.25,
                (p.hoehe * 0.38).min(5.0),
                -s * FRAC_PI_2,
                farbe(farben_[i % 3]),
            );
        }
    }
    // Rüstungen am Tor.
    let sued = p.laenge / 2.0;
    for (x, z) in [
        (-4.2f32, sued - 1.6),
        (4.2, sued - 1.6),
        (-(p.breite / 2.0 - 0.9), sued - 5.5),
        (p.breite / 2.0 - 0.9, sued - 5.5),
    ] {
        b.fester_block(x, 0.0, z, 0.5, 1.2, 0.4, farbe("#adb5bd"));
        b.kugel(x, 1.45, z, 0.22, farbe("#adb5bd"))
            .set_metallic(0.9);
    }
    // Gong westlich vom Podest, Fass Bier an der Ostwand.
    let (gx, gz) = gong(p);
    for dz in [-0.9f32, 0.9] {
        b.fester_block(gx, 0.0, gz + dz, 0.12, 2.45, 0.12, farbe(&p.farben.holz));
    }
    b.block(gx, 2.35, gz, 0.12, 0.12, 1.95, farbe(&p.farben.holz));
    let mut gong = b.saeule(gx, 0.0, gz, 0.7, 0.05, farbe("#e9c46a"));
    gong.set_position(Vec3::new(gx, 1.45, gz));
    gong.set_rotation(Quat::from_axis_angle(Vec3::Z, FRAC_PI_2));
    gong.set_metallic(0.8);
    b.merken("gong", gong);
    b.nutzbar("gong", "gong", [gx + 1.0, 1.4, gz], 1.8);
    let (fx, fz) = fass(p);
    let mut fass = b.saeule(fx, 0.0, fz, 0.45, 1.0, farbe("#7f4f24"));
    fass.set_position(Vec3::new(fx, 0.75, fz));
    fass.set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
    b.kasten(Kasten::um(fx, fz, 1.0, 1.2, 1.2));
    b.nutzbar("kaffee", "bier", [fx - 1.0, 1.0, fz], 1.6);
}
