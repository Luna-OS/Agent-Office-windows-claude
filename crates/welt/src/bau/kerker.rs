//! Der Kerker unter dem Südende der Burghalle: Treppe an der Ostwand hinunter, ein Gang mit Säulen
//! und Fackeln, neun Zellen mit Gittern und Stroh, am Westende der Knochenhaufen. Die Gefangenen
//! selbst setzt `features/kerker.rs` hinein.

use std::f32::consts::FRAC_PI_2;

use kiss3d::prelude::*;

use super::{Bau, farbe, toenen};
use crate::grundriss::burg::*;
use crate::kollision::Kasten;

/// Die Treppe hinunter: 15 Stufen nach Süden.
pub const TREPPE_STUFEN: usize = 15;
pub const TREPPE_LAUF: f32 = 0.34 * TREPPE_STUFEN as f32;

pub fn bauen(b: &mut Bau) {
    let k = KERKER;
    let (cx, cz) = k.mitte();
    let y = -KERKER_TIEFE;
    let stein = farbe("#5a544c");
    // Boden, Decke (= Hallenboden), Wände.
    b.block(
        cx,
        y - 0.3,
        cz,
        k.breite(),
        0.3,
        k.tiefe(),
        toenen(stein, -0.2),
    );
    b.kasten(Kasten::neu(k.min_x, k.max_x, k.min_z, k.max_z, y).mit_unten(y - 0.3));
    for (x, z, bx, bz) in [
        (cx, k.min_z - 0.3, k.breite() + 0.6, 0.6),
        (cx, k.max_z + 0.3, k.breite() + 0.6, 0.6),
        (k.min_x - 0.3, cz, 0.6, k.tiefe()),
        (k.max_x + 0.3, cz, 0.6, k.tiefe()),
    ] {
        b.fester_block(x, y, z, bx, KERKER_TIEFE - 0.3, bz, stein);
    }
    // Treppe.
    let (tx0, tx1) = (
        TREPPE_X - TREPPE_BREITE / 2.0,
        TREPPE_X + TREPPE_BREITE / 2.0,
    );
    for i in 0..TREPPE_STUFEN {
        let oben = -(i as f32 + 1.0) * KERKER_TIEFE / TREPPE_STUFEN as f32;
        let z = k.min_z + (i as f32 + 0.5) * 0.34;
        b.block(
            TREPPE_X,
            oben - 0.15,
            z,
            TREPPE_BREITE,
            0.15,
            0.34,
            toenen(stein, if i % 2 == 0 { 0.1 } else { 0.0 }),
        );
        b.kasten(Kasten::neu(tx0, tx1, z - 0.17, z + 0.17, oben).mit_unten(oben - 0.15));
    }
    // Geländer oben in der Halle um das Treppenloch, mit Lücke an der Nordseite.
    let holz = farbe("#6b4526");
    b.fester_block(
        tx0 - 0.05,
        0.0,
        k.min_z + TREPPE_LAUF / 2.0,
        0.08,
        1.0,
        TREPPE_LAUF,
        holz,
    );
    b.fester_block(
        TREPPE_X,
        0.0,
        k.min_z + TREPPE_LAUF + 0.05,
        TREPPE_BREITE,
        1.0,
        0.08,
        holz,
    );
    b.nutzbar("kerker", "kerker", [TREPPE_X, 1.0, k.min_z - 0.4], 2.0);
    // Zellen: Zwischenwände, Gitterfront mit Tür, Stroh.
    let eisen = farbe("#2b2d42");
    for (i, z) in zellen().into_iter().enumerate() {
        let nach_innen = if z.rot_y == 0.0 { -1.0 } else { 1.0 };
        let tief_mitte = z.z + nach_innen * ZELLEN_TIEFE / 2.0;
        for s in [-1.0f32, 1.0] {
            b.fester_block(
                z.x + s * z.breite / 2.0,
                y,
                tief_mitte,
                0.25,
                KERKER_TIEFE - 0.3,
                ZELLEN_TIEFE,
                stein,
            );
        }
        let mut stab = 0.0;
        while stab < z.breite - 0.1 {
            let sx = z.x - z.breite / 2.0 + 0.15 + stab;
            b.saeule(sx, y, z.z, 0.03, KERKER_TIEFE - 0.3, eisen);
            stab += 0.22;
        }
        b.block(z.x, y + 2.4, z.z, z.breite, 0.08, 0.08, eisen);
        b.kasten(Kasten::neu(
            z.x - z.breite / 2.0,
            z.x + z.breite / 2.0,
            z.z - 0.06,
            z.z + 0.06,
            y + KERKER_TIEFE,
        ));
        b.block(
            z.x,
            y,
            tief_mitte,
            z.breite - 0.4,
            0.06,
            ZELLEN_TIEFE - 0.3,
            farbe(if i % 2 == 0 { "#d4a373" } else { "#c9a227" }),
        );
    }
    // Fackellicht: warme Punktlichter im Gang, damit man in die Zellen sieht.
    for x in [-7.2f32, -2.7, 1.8, 6.5] {
        let mut l = b.knoten.add_light(Light::point(9.0));
        l.set_position(Vec3::new(x, y + 2.6, GANG_Z));
        l.modify_light(|l| {
            l.color = farbe("#ffb26b");
            l.intensity = 6.0;
            l.casts_shadows = false;
        });
    }
    // Säulen und Fackeln im Gang.
    for x in [-7.2f32, -2.7, 1.8] {
        b.fester_block(
            x,
            y,
            GANG_Z,
            0.6,
            KERKER_TIEFE - 0.3,
            0.6,
            toenen(stein, 0.1),
        );
        for s in [-1.0f32, 1.0] {
            let mut f = b.kugel(x, y + 2.3, GANG_Z + s * 0.45, 0.1, farbe("#ff9f1c"));
            f.set_emissive(farbe("#ff6b35"));
        }
    }
    // Der Wächter am Fuß der Treppe (angedeutet), der Knochenhaufen im Westen.
    let (wx, wz) = WAECHTER;
    b.saeule(wx, y, wz, 0.3, 1.3, farbe("#8e1b1b"));
    b.kugel(wx, y + 1.55, wz, 0.22, farbe("#adb5bd"))
        .set_metallic(0.8);
    let mut speer = b.saeule(wx + 0.35, y, wz, 0.025, 2.4, farbe("#6c757d"));
    speer.set_rotation(Quat::from_axis_angle(Vec3::Z, 0.05));
    let (kx, kz) = KNOCHEN;
    for i in 0..12 {
        let w = i as f32 * 1.7;
        let mut knochen = b.saeule(
            kx + w.sin() * 0.5,
            y + 0.05 + (i % 3) as f32 * 0.07,
            kz + w.cos() * 0.4,
            0.035,
            0.45,
            farbe("#f1e9d2"),
        );
        knochen.set_rotation(Quat::from_axis_angle(
            Vec3::new(w.cos(), 0.0, w.sin()),
            FRAC_PI_2,
        ));
    }
    b.kugel(kx, y + 0.25, kz, 0.14, farbe("#f1e9d2"));
}
