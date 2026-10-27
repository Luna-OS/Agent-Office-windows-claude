//! Die Einrichtung einer Büro-Etage: Tische, Kioske, Tafeln, Lounge mit TV, Küche, Korb, Gong,
//! Whiteboard, Regal, Jukebox, Arcade, Chefbüro, Teleskop, Leiter und Feuerwehrstange.
//! Jedes benutzbare Ding meldet sich als `Nutzbar` an; was es tut, steht in `features/`.

use std::f32::consts::{FRAC_PI_2, PI};

use kiss3d::prelude::*;

use super::einrichtung::{self, pflanze, sofa, sitzsack, stuhl};
use super::{Bau, farbe, toenen};
use crate::farben;
use crate::grundriss::draussen::{KORB_BRETT_X, KORB_Z, RING};
use crate::grundriss::sitze::{self, BESPRECHUNGS_TISCH};
use crate::grundriss::*;

pub fn bauen(b: &mut Bau, fluegel: u32) {
    tische(b, fluegel);
    tafeln(b);
    lounge(b);
    kueche_und_korb(b);
    nordwand(b);
    chefbuero(b);
    for (x, z, s) in pflanzen(fluegel) {
        pflanze(b, x, z, s);
    }
    leiter_und_stange(b);
    sitzplaetze(b);
}

fn tische(b: &mut Bau, fluegel: u32) {
    for (i, t) in alle_tische().iter().enumerate() {
        if t.fluegel > fluegel {
            continue;
        }
        let (sx, sz) = tisch_sitz(t, 0.85);
        match t.art {
            TischArt::Schreibtisch => {
                let bild = einrichtung::schreibtisch(b, t.x, t.z, t.rot_y, farben::STUEHLE[i % 6]);
                b.merken(format!("monitor:{}", t.id), bild);
            }
            TischArt::Kiosk(_) => {
                let mut g = b.gruppe(t.x, 0.0, t.z, t.rot_y);
                g.fester_block(0.0, 0.0, 0.0, 0.8, 0.55, 0.5, farbe("#ffffff"));
                let mut s = g.quader(0.0, 0.62, 0.05, 0.5, 0.3, 0.04, farbe("#2b2d42"));
                s.set_rotation(Quat::from_axis_angle(Vec3::X, -0.5));
            }
            // Sitzsäcke stehen nur, wenn jemand darauf arbeitet (siehe features/arbeiter);
            // die Stühle am Besprechungstisch baut chefbuero().
            TischArt::Sitzsack | TischArt::Besprechung => {}
        }
        b.nutzbar("tisch", t.id.clone(), [sx, 1.0, sz], 1.5);
    }
}

fn tafeln(b: &mut Bau) {
    let kork = farben::KORK;
    for (board, name, zettel) in [
        (Board::Issues, "Issues", &["#ffd166", "#ef476f", "#ffd166", "#06d6a0", "#ffd166", "#ef476f", "#8ecae6"][..]),
        (Board::Warteschlange, "Warteschlange", &["#ffffff", "#ffffff", "#ffd166", "#ffffff"][..]),
        (Board::PullRequests, "Pull Requests", &["#06d6a0", "#118ab2", "#06d6a0", "#b388eb", "#118ab2"][..]),
        (Board::Dienste, "Dienste", &["#caffbf", "#bde0fe", "#caffbf"][..]),
    ] {
        let t = tafel(board);
        let f = einrichtung::tafel(b, t.x, t.y, t.z, t.rot_y, t.breite, t.hoehe, kork, "#8b5e3c");
        b.merken(format!("tafel:{name}"), f);
        einrichtung::zettel(b, t.x, t.y, t.z, t.rot_y, t.breite, t.hoehe, zettel);
        let (vx, vz) = (t.rot_y.sin() * 1.4, t.rot_y.cos() * 1.4);
        b.nutzbar("tafel", name, [t.x + vx, t.y - 0.6, t.z + vz], 2.6);
    }
    // Der Monitor des Rechners an der Westwand.
    let m = RECHNER_MONITOR;
    let mut g = b.gruppe(m.x, m.y, m.z, m.rot_y);
    g.quader(0.0, 0.0, 0.0, m.breite + 0.1, m.hoehe + 0.1, 0.06, farbe("#2b2d42"));
    let mut bild = g.flaeche(0.0, 0.0, 0.035, m.breite, m.hoehe, 0.0, farbe("#0b3d2e"));
    bild.set_emissive(farbe("#0b3d2e"));
    b.merken("rechner-monitor", bild);
}

fn lounge(b: &mut Bau) {
    // Der große Fernseher an der Ostwand.
    let tv = TV;
    let mut g = b.gruppe(tv.x, tv.y, tv.z, tv.rot_y);
    g.quader(0.0, 0.0, 0.0, tv.breite + 0.16, tv.hoehe + 0.16, 0.08, farbe("#1b1b2f"));
    let mut bild = g.flaeche(0.0, 0.0, 0.045, tv.breite, tv.hoehe, 0.0, farbe("#0d1b2a"));
    bild.set_emissive(farbe("#0d1b2a"));
    b.merken("tv", bild);
    b.nutzbar("tv", "tv", [tv.x - 2.0, 1.4, tv.z], 3.2);
    // Lowboard unter dem TV.
    b.fester_block(BODEN.max_x - 0.35, 0.0, 0.0, 0.6, 0.45, 3.4, farbe(farben::HOLZ));
    // Couch mit Rücken zum Raum, Couchtisch.
    sofa(b, 10.5, 0.0, 0.0, FRAC_PI_2, 3.8, "#5bc0eb");
    b.fester_block(12.6, 0.0, 0.0, 1.0, 0.38, 2.0, toenen(farbe(farben::HOLZ), 0.1));
    sitzsack(b, 12.5, 3.5, "#ffb400");
    sitzsack(b, 14.5, -3.4, "#b388eb");
    // Jukebox und Arcade-Automat an der Ostwand.
    let (jx, jz) = JUKEBOX;
    let mut j = b.gruppe(jx, 0.0, jz, -FRAC_PI_2);
    j.fester_block(0.0, 0.0, 0.0, 1.3, 1.6, 0.72, farbe("#9d4edd"));
    j.kugel(0.0, 1.6, 0.0, 0.66, farbe("#ffd166")).set_local_scale(1.0, 0.45, 0.55);
    let mut licht = j.quader(0.0, 1.0, 0.37, 0.9, 0.5, 0.02, farbe("#ff8fab"));
    licht.set_emissive(farbe("#ff4d6d"));
    b.nutzbar("jukebox", "jukebox", [jx - 1.0, 1.0, jz], 1.8);
    let (ax, az) = ARCADE;
    let schirm = {
        let mut a = b.gruppe(ax, 0.0, az, -FRAC_PI_2);
        a.fester_block(0.0, 0.0, 0.0, 0.8, 1.9, 0.8, farbe("#3a0ca3"));
        a.block(0.0, 0.95, 0.5, 0.7, 0.08, 0.3, farbe("#f72585"));
        let mut schirm = a.flaeche(0.0, 1.35, 0.41, 0.6, 0.48, 0.0, farbe("#000000"));
        schirm.set_emissive(farbe("#06d6a0"));
        schirm
    };
    b.merken("arcade-schirm", schirm);
    b.nutzbar("arcade", "arcade", [ax - 1.0, 1.0, az], 1.6);
    // Das Hundekörbchen.
    let (hx, hz) = draussen_korb();
    b.saeule(hx, 0.0, hz, 0.55, 0.12, farbe("#e76f51"));
    b.saeule(hx, 0.02, hz, 0.45, 0.12, farbe("#ffd6a5"));
}

fn draussen_korb() -> (f32, f32) {
    crate::grundriss::draussen::HUND_KORB
}

fn kueche_und_korb(b: &mut Bau) {
    // Theke mit Holzplatte, Kaffeemaschine, Kühlschrank (an der Südwand, Blick in den Raum).
    let k = KUECHE;
    let schrank = farbe("#8ecae6");
    b.fester_block(-14.5, 0.0, (k.min_z + k.max_z) / 2.0, 5.0, 0.98, 1.0, schrank);
    b.block(-14.5, 0.98, (k.min_z + k.max_z) / 2.0, 5.1, 0.05, 1.05, farbe(farben::HOLZ));
    let (mx, _) = KAFFEEMASCHINE;
    let mut m = b.gruppe(mx, 1.03, 12.25, PI);
    m.block(0.0, 0.0, 0.0, 0.5, 0.5, 0.4, farbe("#adb5bd"));
    m.block(0.0, 0.5, 0.0, 0.52, 0.06, 0.42, farbe("#343a40"));
    let mut lampe = m.kugel(0.15, 0.42, 0.21, 0.03, farbe("#ef476f"));
    lampe.set_emissive(farbe("#ef476f"));
    m.saeule(0.0, 0.05, 0.12, 0.05, 0.1, farbe("#ffffff"));
    let (kx, kz) = KAFFEEMASCHINE;
    b.nutzbar("kaffee", "kaffee", [kx, 1.3, kz + 1.2], 1.4);
    b.fester_block(-11.3, 0.0, 12.2, 1.1, 2.2, 1.0, farbe("#f8f9fa"));
    b.block(-11.3, 1.4, 11.68, 0.25, 0.25, 0.02, farbe("#ffd166"));

    // Basketballkorb an der Westwand.
    let weiss = farbe("#ffffff");
    b.fester_block(BODEN.min_x + 0.25, 0.0, KORB_Z, 0.12, 2.9, 0.12, farbe("#6c757d"));
    b.block(KORB_BRETT_X, 2.88, KORB_Z, 0.05, 1.05, 1.4, weiss);
    b.block(KORB_BRETT_X + 0.03, 3.05, KORB_Z, 0.02, 0.45, 0.59, farbe("#ef476f"));
    let (rx, ry, rz, r) = RING;
    let mut ring = b.saeule(rx, ry, rz, r, 0.03, farbe("#ff6b35"));
    ring.set_local_scale(1.0, 1.0, 1.0);
    b.saeule(rx, ry - 0.42, rz, r * 0.7, 0.42, Color::new(1.0, 1.0, 1.0, 0.5));
}

fn nordwand(b: &mut Bau) {
    // Gong mit Gestell.
    let (gx, gz) = GONG;
    let holz = farbe("#7f4f24");
    b.fester_block(gx - 0.9, 0.0, gz, 0.12, 2.45, 0.12, holz);
    b.fester_block(gx + 0.9, 0.0, gz, 0.12, 2.45, 0.12, holz);
    b.block(gx, 2.35, gz, 1.95, 0.12, 0.12, holz);
    let mut gong = b.saeule(gx, 0.0, gz, 0.7, 0.05, farbe("#e9c46a"));
    gong.set_position(Vec3::new(gx, 1.45, gz));
    gong.set_rotation(Quat::from_axis_angle(Vec3::X, FRAC_PI_2));
    gong.set_metallic(0.8);
    b.merken("gong", gong);
    b.nutzbar("gong", "gong", [gx, 1.4, gz + 1.0], 1.8);
    // Bücherregal an der Südwand.
    let (rx, rz) = BUECHERREGAL;
    let mut r = b.gruppe(rx, 0.0, rz, PI);
    r.fester_block(0.0, 0.0, 0.0, 1.7, 2.3, 0.42, farbe(farben::HOLZ));
    for (i, y) in [0.25, 0.8, 1.35, 1.9].into_iter().enumerate() {
        let mut x = -0.75;
        let mut n = i;
        while x < 0.72 {
            let breite = 0.06 + (n % 3) as f32 * 0.02;
            r.block(x + breite / 2.0, y, 0.22, breite, 0.38 - (n % 2) as f32 * 0.06, 0.04, farbe(farben::STUEHLE[n % 6]));
            x += breite + 0.01;
            n += 1;
        }
    }
    b.nutzbar("regal", "regal", [rx, 1.2, rz - 1.0], 1.8);
    // Whiteboard auf Rollen.
    let (wx, wz) = WHITEBOARD;
    let f = einrichtung::tafel(b, wx, 1.6, wz, 0.0, 4.0, 2.2, "#ffffff", "#adb5bd");
    b.merken("whiteboard", f);
    b.fester_block(wx - 1.9, 0.0, wz, 0.08, 0.5, 0.6, farbe("#6c757d"));
    b.fester_block(wx + 1.9, 0.0, wz, 0.08, 0.5, 0.6, farbe("#6c757d"));
    b.nutzbar("whiteboard", "whiteboard", [wx, 1.4, wz + 1.2], 2.4);
}

fn chefbuero(b: &mut Bau) {
    let e = EMPORE;
    let (cx, cz) = e.mitte();
    // Großer Schreibtisch mit Blick über den Raum, Chefsessel, Couch.
    b.fester_block(cx + 0.5, EMPORE_Y, cz + 0.2, 2.2, 0.76, 1.0, farbe("#5e3023"));
    b.quader(cx + 0.5, EMPORE_Y + 1.05, cz - 0.05, 0.9, 0.5, 0.04, farbe("#2b2d42"));
    stuhl(b, cx + 0.5, cz + 0.7, PI, "#2b2d42");
    sofa(b, BODEN.max_x - 0.65, EMPORE_Y, cz, -FRAC_PI_2, 1.8, "#e76f51");
    pflanze_auf(b, e.max_x - 0.5, EMPORE_Y, e.min_z + 0.5);
    // Das Teleskop an der Brüstung.
    let (tx, tz) = TELESKOP;
    b.saeule(tx, EMPORE_Y, tz, 0.03, 1.1, farbe("#343a40"));
    let mut rohr = b.saeule(tx, EMPORE_Y + 1.1, tz, 0.07, 0.7, farbe("#4361ee"));
    rohr.set_position(Vec3::new(tx - 0.15, EMPORE_Y + 1.2, tz - 0.1));
    rohr.set_rotation(Quat::from_axis_angle(Vec3::new(1.0, 0.0, -1.0).normalize(), 1.2));
    b.nutzbar("teleskop", "teleskop", [tx, EMPORE_Y + 1.2, tz], 1.4);

    // Besprechungstisch mit Stühlen.
    let (mx, mz, mb, mt) = BESPRECHUNGS_TISCH;
    b.fester_block(mx, 0.0, mz, mb, 0.76, mt, farbe("#f7f3ea"));
    for t in sitze::besprechungs_tische() {
        let (sx, sz) = tisch_sitz(&t, 0.55);
        stuhl(b, sx, sz, t.rot_y + PI, "#4a4e69");
    }
}

fn pflanze_auf(b: &mut Bau, x: f32, y: f32, z: f32) {
    let mut g = b.gruppe(0.0, y, 0.0, 0.0);
    pflanze(&mut g, x, z, 0.9);
}

fn leiter_und_stange(b: &mut Bau) {
    // Leiter an der Westwand, durch Luken in Decke und Boden.
    let (lx, lz) = LEITER;
    let alu = farbe(farben::ALU);
    for s in [-0.3, 0.3] {
        b.block(lx - 0.35, -PLATTE, lz + s, 0.06, WANDHOEHE + PLATTE, 0.06, alu);
    }
    let mut y = 0.3;
    while y < WANDHOEHE {
        b.quader(lx - 0.35, y, lz, 0.05, 0.05, 0.6, alu);
        y += 0.32;
    }
    b.nutzbar("leiter", "leiter", [lx, 1.2, lz], 1.3);
    // Die Feuerwehrstange mit Geländer um ihr Loch.
    let (sx, sz) = STANGE;
    let mut st = b.saeule(sx, -PLATTE, sz, 0.055, WANDHOEHE + PLATTE, farbe("#dee2e6"));
    st.set_metallic(0.9);
    let gelb = farbe("#ffd166");
    for i in 0..8 {
        let w = i as f32 / 8.0 * std::f32::consts::TAU;
        if (w - PI).abs() < 0.5 {
            continue;
        }
        b.saeule(sx + w.sin() * 0.9, 0.0, sz + w.cos() * 0.9, 0.03, 1.0, gelb);
    }
    b.nutzbar("stange", "stange", [sx, 1.2, sz], 1.4);
}

/// Jeder Platz zum Sitzen im Büro ist ein benutzbares Ding (E setzt sich hin).
fn sitzplaetze(b: &mut Bau) {
    for s in sitze::sitze().iter().filter(|s| !s.dach) {
        for i in 0..s.plaetze.len() {
            let p = sitze::platz(s, i);
            b.nutzbar("sitz", p.schluessel.clone(), [p.x, p.y + 0.6, p.z], 1.2);
        }
    }
}
