//! Die Hülle einer Etage: Boden, Decke, Außenwände mit Fenstern und Türen, Hinterbüro,
//! Aufzugsschacht, Empore mit Treppe und Besprechungsraum.

use kiss3d::prelude::*;

use super::{Bau, farbe, toenen};
use crate::farben::{self, EtagenPalette};
use crate::grundriss::*;
use crate::kollision::Kasten;

/// Baut die Hülle. `unterste`: nur die unterste Etage hat die Tür nach draußen.
pub fn bauen(b: &mut Bau, pal: &EtagenPalette, fluegel: u32, unterste: bool) {
    boden_und_decke(b, pal, fluegel);
    let oeffnungen = oeffnungen(unterste);
    for seite in [Seite::Nord, Seite::Sued, Seite::Ost, Seite::West] {
        wand(b, pal, seite, &oeffnungen, fluegel);
    }
    if fluegel > 0 {
        hinterbuero(b, pal, fluegel);
    }
    aufzug(b, pal);
    empore(b, pal);
}

fn boden_und_decke(b: &mut Bau, pal: &EtagenPalette, fluegel: u32) {
    let boden = farbe(pal.boden);
    let (cx, cz) = BODEN.mitte();
    // Die Bodenplatte (unten sieht man sie als Garagendecke).
    b.block(cx, -PLATTE, cz, BODEN.breite() + 2.0 * WANDSTAERKE, PLATTE, BODEN.tiefe() + 2.0 * WANDSTAERKE, boden);
    b.kasten(Kasten::neu(BODEN.min_x - WANDSTAERKE, BODEN.max_x + WANDSTAERKE, BODEN.min_z - WANDSTAERKE, BODEN.max_z + WANDSTAERKE, 0.0).mit_unten(-PLATTE));
    // Dielen-Streifen in der zweiten Bodenfarbe.
    let alt = farbe(pal.boden2);
    let mut x = BODEN.min_x + 1.2;
    while x < BODEN.max_x {
        b.block(x, 0.0, cz, 0.6, 0.004, BODEN.tiefe(), alt);
        x += 2.4;
    }
    // Teppiche unter den Tischinseln und in der Lounge.
    for (i, (tx, tz, tb, tt)) in [(-10.5, -4.0, 6.0, 4.2), (-1.5, -4.0, 6.0, 4.2), (-10.5, 4.0, 6.0, 4.2), (-1.5, 4.0, 6.0, 4.2), (13.5, 0.0, 6.5, 7.0)].into_iter().enumerate() {
        b.block(tx, 0.0, tz, tb, 0.012, tt, farbe(farben::TEPPICHE[i % 4]));
    }
    // Decke, mit Loch für die Leiter.
    let decke = toenen(farbe(pal.wand), 0.3);
    let mut d = b.block(cx, WANDHOEHE, cz, BODEN.breite(), PLATTE, BODEN.tiefe(), decke);
    // Von unten bekommt die Decke kein Sonnenlicht: etwas Eigenleuchten, sonst wirkt sie grau.
    d.set_emissive(toenen(decke, -0.55));
    // Lampen unter der Decke.
    for lx in [-12.0, -4.0, 4.0, 12.0] {
        for lz in [-6.0, 0.0, 6.0] {
            let mut l = b.block(lx, WANDHOEHE - 0.06, lz, 1.6, 0.06, 0.5, farbe("#fffbea"));
            l.set_emissive(farbe("#fff3c4"));
        }
    }
    if fluegel > 0 {
        let tiefe = BODEN.min_z - fluegel_min_z(fluegel);
        let fx = (FLUEGEL_MIN_X + BODEN.max_x) / 2.0;
        let fz = BODEN.min_z - tiefe / 2.0;
        b.block(fx, -PLATTE, fz, BODEN.max_x - FLUEGEL_MIN_X, PLATTE, tiefe, boden);
        b.kasten(Kasten::neu(FLUEGEL_MIN_X, BODEN.max_x, fluegel_min_z(fluegel), BODEN.min_z, 0.0).mit_unten(-PLATTE));
        b.block(fx, WANDHOEHE, fz, BODEN.max_x - FLUEGEL_MIN_X, PLATTE, tiefe, decke);
    }
}

/// Eine Außenwand, ausgespart um ihre Öffnungen. Glas in die Fenster, Rahmen in Leistenfarbe.
fn wand(b: &mut Bau, pal: &EtagenPalette, seite: Seite, alle: &[Oeffnung], fluegel: u32) {
    let w = farbe(pal.wand);
    let t = WANDSTAERKE;
    let h = WANDHOEHE;
    // Länge entlang der Wand und die feste Koordinate quer dazu.
    let (von, bis, quer) = match seite {
        Seite::Nord => (BODEN.min_x - t, BODEN.max_x + t, BODEN.min_z - t / 2.0),
        Seite::Sued => (BODEN.min_x - t, BODEN.max_x + t, BODEN.max_z + t / 2.0),
        Seite::Ost => (BODEN.min_z, BODEN.max_z, BODEN.max_x + t / 2.0),
        Seite::West => (BODEN.min_z, BODEN.max_z, BODEN.min_x - t / 2.0),
    };
    let mut loecher: Vec<Oeffnung> = alle.iter().copied().filter(|o| o.wand == seite).collect();
    if seite == Seite::Nord && fluegel > 0 {
        loecher.push(Oeffnung { wand: seite, u: (FLUEGEL_MIN_X + BODEN.max_x + t) / 2.0, breite: BODEN.max_x + t - FLUEGEL_MIN_X, unten: 0.0, oben: h });
    }
    loecher.sort_by(|a, b| a.u.total_cmp(&b.u));

    let stueck = |b: &mut Bau, u0: f32, u1: f32, y0: f32, y1: f32| {
        if u1 - u0 < 0.01 || y1 - y0 < 0.01 {
            return;
        }
        let u = (u0 + u1) / 2.0;
        let (x, z, bx, bz) = match seite {
            Seite::Nord | Seite::Sued => (u, quer, u1 - u0, t),
            Seite::Ost | Seite::West => (quer, u, t, u1 - u0),
        };
        b.fester_block(x, y0, z, bx, y1 - y0, bz, w);
    };
    let mut u = von;
    for o in &loecher {
        let (u0, u1) = (o.u - o.breite / 2.0, o.u + o.breite / 2.0);
        stueck(b, u, u0, 0.0, h);
        stueck(b, u0, u1, 0.0, o.unten);
        stueck(b, u0, u1, o.oben, h);
        if o.oben < h {
            fenster(b, pal, seite, quer, o);
        }
        u = u1;
    }
    stueck(b, u, bis, 0.0, h);
    // Fußleiste innen.
    let leiste = farbe(pal.leiste);
    let innen = match seite {
        Seite::Nord => quer + t / 2.0 + 0.02,
        Seite::Sued => quer - t / 2.0 - 0.02,
        Seite::Ost => quer - t / 2.0 - 0.02,
        Seite::West => quer + t / 2.0 + 0.02,
    };
    match seite {
        Seite::Nord | Seite::Sued => b.block((von + bis) / 2.0, 0.0, innen, bis - von, 0.12, 0.04, leiste),
        Seite::Ost | Seite::West => b.block(innen, 0.0, (von + bis) / 2.0, 0.04, 0.12, bis - von, leiste),
    };
}

/// Glas und Rahmen in einer Öffnung (Türen ohne Glas: die Balkontür schiebt sich auf).
fn fenster(b: &mut Bau, pal: &EtagenPalette, seite: Seite, quer: f32, o: &Oeffnung) {
    let rot = match seite {
        Seite::Nord | Seite::Sued => 0.0,
        Seite::Ost | Seite::West => std::f32::consts::FRAC_PI_2,
    };
    let (x, z) = match seite {
        Seite::Nord | Seite::Sued => (o.u, quer),
        Seite::Ost | Seite::West => (quer, o.u),
    };
    let hoehe = o.oben - o.unten;
    let mut g = b.gruppe(x, o.unten, z, rot);
    let rahmen = farbe(pal.leiste);
    g.block(0.0, -0.05, 0.0, o.breite + 0.1, 0.05, WANDSTAERKE + 0.12, rahmen);
    g.block(0.0, hoehe, 0.0, o.breite + 0.1, 0.05, WANDSTAERKE + 0.04, rahmen);
    if o.unten > 0.0 {
        let mut glas = g.flaeche(0.0, hoehe / 2.0, 0.0, o.breite, hoehe, 0.0, Color::new(0.84, 0.95, 1.0, 0.18));
        glas.enable_backface_culling(false);
        g.block(0.0, 0.0, 0.0, 0.05, hoehe, 0.06, rahmen);
    }
}

/// Die Bucht hinter der Nordwand: Seitenwand im Westen, Rückwand im Norden, Ostwand verlängert.
fn hinterbuero(b: &mut Bau, pal: &EtagenPalette, fluegel: u32) {
    let w = farbe(pal.wand);
    let t = WANDSTAERKE;
    let z0 = fluegel_min_z(fluegel);
    let tiefe = BODEN.min_z - z0;
    b.fester_block(FLUEGEL_MIN_X - t / 2.0, 0.0, BODEN.min_z - tiefe / 2.0, t, WANDHOEHE, tiefe, w);
    b.fester_block((FLUEGEL_MIN_X + BODEN.max_x) / 2.0, 0.0, z0 - t / 2.0, BODEN.max_x - FLUEGEL_MIN_X + 2.0 * t, WANDHOEHE, t, w);
    b.fester_block(BODEN.max_x + t / 2.0, 0.0, BODEN.min_z - tiefe / 2.0, t, WANDHOEHE, tiefe + t, w);
}

/// Der Aufzugsschacht an der Nordwand mit zwei Schiebetüren (benannt "aufzug-tuer-l/r").
fn aufzug(b: &mut Bau, pal: &EtagenPalette) {
    let stahl = farbe("#8d99ae");
    let x0 = AUFZUG_X - AUFZUG_BREITE / 2.0;
    let x1 = AUFZUG_X + AUFZUG_BREITE / 2.0;
    let mitte_z = BODEN.min_z + AUFZUG_TIEFE / 2.0;
    let wd = AUFZUG_WAND;
    // Seitenwände.
    b.fester_block(x0 + wd / 2.0, 0.0, mitte_z, wd, WANDHOEHE, AUFZUG_TIEFE, toenen(farbe(pal.leiste), 0.2));
    b.fester_block(x1 - wd / 2.0, 0.0, mitte_z, wd, WANDHOEHE, AUFZUG_TIEFE, toenen(farbe(pal.leiste), 0.2));
    // Front: links und rechts der Tür, darüber.
    let fz = AUFZUG_VORNE - wd / 2.0;
    let seite = (AUFZUG_BREITE - AUFZUG_TUER) / 2.0;
    b.fester_block(x0 + seite / 2.0, 0.0, fz, seite, WANDHOEHE, wd, stahl);
    b.fester_block(x1 - seite / 2.0, 0.0, fz, seite, WANDHOEHE, wd, stahl);
    b.block(AUFZUG_X, AUFZUG_TUER_HOEHE, fz, AUFZUG_TUER, WANDHOEHE - AUFZUG_TUER_HOEHE, wd, stahl);
    // Kabinenboden und Anzeige über der Tür.
    b.block(AUFZUG_X, 0.0, mitte_z, AUFZUG_BREITE - 2.0 * wd, 0.02, AUFZUG_TIEFE - wd, farbe("#4a4e69"));
    let mut anzeige = b.quader(AUFZUG_X, AUFZUG_TUER_HOEHE + 0.35, AUFZUG_VORNE + 0.01, 0.7, 0.25, 0.02, farbe("#1b1b2f"));
    anzeige.set_emissive(farbe("#ff9f1c"));
    // Die Türen: halb so breit wie die Öffnung, gleiten zur Seite (siehe features/aufzug).
    let halb = AUFZUG_TUER / 2.0;
    let l = b.block(AUFZUG_X - halb / 2.0, 0.0, fz + 0.03, halb, AUFZUG_TUER_HOEHE, 0.04, toenen(stahl, 0.25));
    let r = b.block(AUFZUG_X + halb / 2.0, 0.0, fz + 0.03, halb, AUFZUG_TUER_HOEHE, 0.04, toenen(stahl, 0.25));
    b.merken("aufzug-tuer-l", l);
    b.merken("aufzug-tuer-r", r);
    b.nutzbar("aufzug", "aufzug", [AUFZUG_X, 1.2, mitte_z], 1.6);
    // Der Knopf neben der Tür.
    b.nutzbar("aufzug", "aufzug-knopf", [x1 + 0.1, 1.2, AUFZUG_VORNE + 0.1], 1.4);
}

/// Die Empore (Chefbüro) auf Pfosten, mit Glasbrüstung, Treppe und dem Besprechungsraum darunter.
fn empore(b: &mut Bau, pal: &EtagenPalette) {
    let holz = farbe(farben::HOLZ);
    let weiss = farbe("#ffffff");
    let e = EMPORE;
    let (cx, cz) = e.mitte();
    // Boden der Empore: Kollision als Deckel über dem Besprechungsraum.
    b.block(cx, EMPORE_Y - 0.2, cz, e.breite(), 0.2, e.tiefe(), holz);
    b.kasten(Kasten::neu(e.min_x, e.max_x, e.min_z, e.max_z, EMPORE_Y).mit_unten(EMPORE_Y - 0.2));
    // Pfosten an den offenen Ecken.
    for (px, pz) in [(e.min_x + 0.1, e.min_z + 0.1), (e.max_x - 0.1, e.min_z + 0.1)] {
        b.saeule(px, 0.0, pz, 0.1, EMPORE_Y - 0.2, weiss);
        b.kasten(Kasten::um(px, pz, 0.2, 0.2, EMPORE_Y - 0.2));
    }
    // Glasbrüstung an Nord- und Westkante (Lücke für die Treppe).
    let glas = |b: &mut Bau, x: f32, z: f32, breite: f32, rot: f32| {
        let mut g = b.flaeche(x, EMPORE_Y + 0.5, z, breite, 1.0, rot, Color::new(0.84, 0.95, 1.0, 0.22));
        g.enable_backface_culling(false);
    };
    glas(b, cx, e.min_z, e.breite(), 0.0);
    b.block(cx, EMPORE_Y + 1.0, e.min_z, e.breite(), 0.05, 0.08, holz);
    b.kasten(Kasten::neu(e.min_x, e.max_x, e.min_z - 0.05, e.min_z + 0.05, EMPORE_Y + 1.05).mit_unten(EMPORE_Y).als_zaun());
    let treppe_bis = TREPPE.min_z;
    glas(b, e.min_x, (e.min_z + treppe_bis) / 2.0, treppe_bis - e.min_z, std::f32::consts::FRAC_PI_2);
    b.kasten(Kasten::neu(e.min_x - 0.05, e.min_x + 0.05, e.min_z, treppe_bis, EMPORE_Y + 1.05).mit_unten(EMPORE_Y).als_zaun());

    // Die Treppe: Stufen an der Südwand hinauf nach Osten.
    let n = TREPPE_STUFEN as usize;
    let lauf = TREPPE.breite() / n as f32;
    for i in 0..n {
        let oben = (i + 1) as f32 / n as f32 * EMPORE_Y;
        let x = TREPPE.min_x + (i as f32 + 0.5) * lauf;
        b.block(x, oben - 0.12, (TREPPE.min_z + TREPPE.max_z) / 2.0, lauf, 0.12, TREPPE.tiefe(), toenen(holz, if i % 2 == 0 { 0.0 } else { -0.08 }));
        b.kasten(Kasten::neu(x - lauf / 2.0, x + lauf / 2.0, TREPPE.min_z, TREPPE.max_z, oben).mit_unten(oben - 0.12));
    }
    b.block(TREPPE.min_x + TREPPE.breite() / 2.0, EMPORE_Y / 2.0, TREPPE.min_z - 0.03, TREPPE.breite(), 0.04, 0.04, holz);

    // Besprechungsraum: Glas von den Pfosten bis zur Außenwand, Tür in der Nordwand (x 10..11.4).
    let (tx0, tx1) = (10.0, 11.4);
    let gh = EMPORE_Y - 0.25;
    let scheibe = |b: &mut Bau, x0: f32, x1: f32| {
        let mut g = b.flaeche((x0 + x1) / 2.0, gh / 2.0, e.min_z + 0.15, x1 - x0, gh, 0.0, Color::new(0.84, 0.95, 1.0, 0.2));
        g.enable_backface_culling(false);
        b.kasten(Kasten::neu(x0, x1, e.min_z + 0.1, e.min_z + 0.2, gh));
    };
    scheibe(b, e.min_x + 0.15, tx0);
    scheibe(b, tx1, e.max_x);
    let mut w = b.flaeche(e.min_x + 0.15, gh / 2.0, (e.min_z + 0.15 + treppe_bis) / 2.0, treppe_bis - e.min_z - 0.15, gh, std::f32::consts::FRAC_PI_2, Color::new(0.84, 0.95, 1.0, 0.2));
    w.enable_backface_culling(false);
    b.kasten(Kasten::neu(e.min_x + 0.1, e.min_x + 0.2, e.min_z + 0.15, e.max_z, gh));
    let _ = farbe(pal.wand);
}
