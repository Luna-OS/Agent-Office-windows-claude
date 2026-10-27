//! Wie ein heimgeschickter Arbeiter geht, mit seinem Karton unter dem Arm:
//! - auf der untersten Etage zum Aufzug,
//! - in der Burg die Treppe hinunter in den Kerker,
//! - auf den Etagen darüber (keine Tür nach draußen) durch die Balkontür aufs Geländer und am
//!   Fallschirm hinunter auf den Hof vor der Garage (wie `features/workers/leaving.ts`).

use std::f32::consts::TAU;

use kiss3d::prelude::*;

use crate::bau::farbe;
use crate::figur::{Figur, Haltung};
use crate::grundriss::{AUFZUG_ANKUNFT, BALKON, BALKON_TUER, BODEN, burg, tisch_sitz};
use crate::welt::Kontext;

const TEMPO: f32 = 1.6;
/// So lange fällt er frei, bevor der Schirm aufgeht, und so schnell sinkt er dann.
const FREIER_FALL: f32 = 0.6;
const SINKEN: f32 = 2.2;
const SCHIRMFARBEN: [&str; 5] = ["#ef476f", "#ffd166", "#06d6a0", "#118ab2", "#f78c6b"];

enum Phase {
    /// In der Halle: der Wächter kommt die Kerkertreppe herauf zum Platz und holt ihn ab.
    Holen,
    /// Den Wegpunkten nach (seit wann, für die Sprüche).
    Gehen,
    /// Am Fallschirm hinunter: seit wann, und wohin.
    Fallen { t: f32, von: Vec3, nach: Vec3 },
    /// Unten gelandet: noch kurz stehen, dann weg.
    Unten(f32),
}

pub struct Abgang {
    figur: Figur,
    karton: SceneNode3d,
    weg: Vec<Vec3>,
    /// Der Fallschirm, wenn es über das Balkongeländer geht (Ziel unten auf dem Hof).
    schirm: Option<(SceneNode3d, Vec3)>,
    phase: Phase,
    /// Der Wächter des Königs (in der Halle mit Kerker) und wo er gerade ist.
    waechter: Option<(Figur, Vec3)>,
    /// Was beide beim Abführen sagen, und seit wann.
    sprueche: Option<(&'static str, &'static str, f32)>,
}

/// Was der Wächter sagt, und was der Arbeiter darauf antwortet (wie im Original).
const WAECHTER_SAGT: [&str; 4] = [
    "Im Namen der Krone, du kommst mit.",
    "Auf die Beine. Der König ist fertig mit dir.",
    "Du bist entlassen. Komm ohne Theater.",
    "Hoch mit dir. Der Kerker wartet.",
];
const ARBEITER_SAGT: [&str; 6] = [
    "Gnade, Majestät!",
    "Aber mein Pull Request …",
    "Ich war fast fertig!",
    "Nur noch ein Commit!",
    "Nicht der Kerker!",
    "…",
];
const WAECHTER_TEMPO: f32 = 2.4;

impl Abgang {
    pub fn neu(kx: &mut Kontext, figur: Figur, tisch: &str) -> Self {
        let von = kx.tisch(tisch).map(|t| {
            let (x, z) = tisch_sitz(&t, 1.3);
            Vec3::new(x, 0.0, z)
        });
        let von = von.unwrap_or_else(|| figur.wurzel.position());
        let oben = !kx.in_der_burg() && !kx.zustand.auf_dem_dach() && kx.zustand.etagen_index() > 0;
        let mut weg = vec![von];
        let mut schirm = None;
        if let Some(p) = kx.halle() {
            // In den Kerker, wenn es einen gibt; sonst durch das Tor im Süden hinaus.
            let ziel = if p.kerker {
                Vec3::new(
                    burg::TREPPE_X,
                    0.0,
                    burg::KERKER.min_z + p.kerker_versatz() + 0.5,
                )
            } else {
                Vec3::new(0.0, 0.0, p.laenge / 2.0 - 0.8)
            };
            weg.push(ziel);
        } else if oben {
            // Zur Balkontür, hinaus ans Geländer.
            let tuer = BALKON_TUER.u;
            weg.push(Vec3::new(tuer, 0.0, BODEN.max_z - 1.0));
            weg.push(Vec3::new(tuer, 0.0, BALKON.max_z - 0.45));
            let unten = kx.strasse();
            let seite = 0.6 + (tisch.len() % 3) as f32 * 0.5;
            let ziel = Vec3::new(tuer + seite, unten, BALKON.max_z + 1.2 + 2.0);
            let mut s = kx.etage.add_group();
            s.set_visible(false);
            let farbe_ = farbe(SCHIRMFARBEN[tisch.len() % SCHIRMFARBEN.len()]);
            let mut dach = s.add_sphere(1.1);
            dach.set_local_scale(1.0, 0.45, 1.0);
            dach.set_position(Vec3::new(0.0, 2.6, 0.0));
            dach.set_color(farbe_);
            for i in 0..6 {
                let w = i as f32 / 6.0 * TAU;
                let mut leine = s.add_cylinder(0.006, 1.3);
                leine.set_position(Vec3::new(w.sin() * 0.5, 1.95, w.cos() * 0.5));
                leine.set_rotation(Quat::from_axis_angle(
                    Vec3::new(w.cos(), 0.0, -w.sin()),
                    0.6,
                ));
                leine.set_color(farbe("#f8f9fa"));
            }
            schirm = Some((s, ziel));
        } else {
            let (ax, az) = AUFZUG_ANKUNFT;
            weg.push(Vec3::new(ax, 0.0, az));
        }
        let mut karton = kx.etage.add_cube(0.5, 0.35, 0.4);
        karton.set_color(farbe("#c98b5a"));
        // In einer Halle mit Kerker holt ihn der Wächter ab.
        let waechter_start = kx.halle().filter(|p| p.kerker).map(|p| {
            Vec3::new(
                burg::TREPPE_X,
                0.0,
                burg::KERKER.min_z + p.kerker_versatz() + 0.5,
            )
        });
        let waechter = waechter_start.map(|start| {
            let look = crate::figur::Look {
                hemd: "#8e1b1b".into(),
                haut: 1,
                haare: 6,
                frisur: 0,
            };
            (Figur::neu(&mut kx.etage, &look), start)
        });
        let n = tisch.bytes().map(|b| b as usize).sum::<usize>();
        let phase = if waechter.is_some() {
            Phase::Holen
        } else {
            Phase::Gehen
        };
        Self {
            figur,
            karton,
            weg,
            schirm,
            phase,
            waechter,
            sprueche: Some((
                WAECHTER_SAGT[n % WAECHTER_SAGT.len()],
                ARBEITER_SAGT[n % ARBEITER_SAGT.len()],
                -1.0,
            )),
        }
    }

    /// Sprechblasen über den Köpfen (Text, Ort), solange jemand etwas sagt.
    pub fn sprechblasen(&self) -> Vec<(String, [f32; 3])> {
        let mut v = Vec::new();
        if let Some((w, a, seit)) = self.sprueche
            && (0.0..4.0).contains(&seit)
        {
            if let Some((_, p)) = &self.waechter {
                v.push((format!("„{w}“"), [p.x, p.y + 2.3, p.z]));
            }
            if seit > 1.2 {
                let k = self.figur.kopf_welt();
                v.push((format!("„{a}“"), [k.x, k.y + 0.4, k.z]));
            }
        }
        v
    }

    fn zeigen(&mut self, p: Vec3, blick: f32, dt: f32) {
        self.figur.setzen(p.x, p.y, p.z, blick);
        self.figur.animieren(dt);
        self.karton
            .set_position(p + Vec3::new(blick.sin() * 0.35, 1.0, blick.cos() * 0.35));
        self.karton
            .set_rotation(Quat::from_axis_angle(Vec3::Y, blick));
    }

    fn weg_damit(&mut self) {
        if let Some((mut w, _)) = self.waechter.take() {
            w.entfernen();
        }
        self.figur.entfernen();
        self.karton.detach();
        if let Some((mut s, _)) = self.schirm.take() {
            s.detach();
        }
    }

    /// Ein Schritt; `false`, wenn er fort ist.
    pub fn schritt(&mut self, dt: f32) -> bool {
        if let Some((_, _, seit)) = self.sprueche.as_mut()
            && *seit >= 0.0
        {
            *seit += dt;
        }
        match &mut self.phase {
            Phase::Holen => {
                // Der Arbeiter packt am Platz, der Wächter kommt zu ihm.
                let am_platz = self.weg[0];
                let blick = self
                    .waechter
                    .as_ref()
                    .map(|(_, p)| (p.x - am_platz.x).atan2(p.z - am_platz.z))
                    .unwrap_or(0.0);
                self.figur.haltung = Haltung::Stehen;
                self.zeigen(am_platz, blick, dt);
                let Some((w, p)) = self.waechter.as_mut() else {
                    self.phase = Phase::Gehen;
                    return true;
                };
                let ziel = am_platz + Vec3::new(0.8, 0.0, 0.0);
                let d = ziel - *p;
                if d.length() <= WAECHTER_TEMPO * dt {
                    *p = ziel;
                    self.phase = Phase::Gehen;
                    if let Some((_, _, seit)) = self.sprueche.as_mut() {
                        *seit = 0.0;
                    }
                } else {
                    *p += d.normalize() * WAECHTER_TEMPO * dt;
                }
                w.haltung = Haltung::Laufen;
                w.setzen(p.x, p.y, p.z, d.x.atan2(d.z));
                w.animieren(dt);
                true
            }
            Phase::Gehen => {
                // Der Wächter geht neben ihm, eine Hand auf seiner Schulter.
                if let Some((w, p)) = self.waechter.as_mut() {
                    let jetzt = self.weg[0];
                    let richtung = self
                        .weg
                        .get(1)
                        .map(|b| (*b - jetzt).normalize_or_zero())
                        .unwrap_or(Vec3::Z);
                    *p = jetzt + Vec3::new(richtung.z, 0.0, -richtung.x) * 0.7;
                    w.haltung = Haltung::Laufen;
                    w.setzen(p.x, p.y, p.z, richtung.x.atan2(richtung.z));
                    w.animieren(dt);
                }
                if self.weg.len() < 2 {
                    // Angekommen: im Aufzug oder Kerker verschwinden, oder springen.
                    if let Some((_, ziel)) = &self.schirm {
                        let von = self.weg[0] + Vec3::new(0.0, 1.1, 0.3);
                        self.phase = Phase::Fallen {
                            t: 0.0,
                            von,
                            nach: *ziel,
                        };
                        return true;
                    }
                    self.weg_damit();
                    return false;
                }
                let (a, b) = (self.weg[0], self.weg[1]);
                let d = b - a;
                let schritt = TEMPO * dt;
                let blick = d.x.atan2(d.z);
                if d.length() <= schritt {
                    self.weg.remove(0);
                    self.figur.haltung = Haltung::Laufen;
                    self.zeigen(b, blick, dt);
                } else {
                    let p = a + d.normalize() * schritt;
                    self.weg[0] = p;
                    self.figur.haltung = Haltung::Laufen;
                    self.zeigen(p, blick, dt);
                }
                true
            }
            Phase::Fallen { t, von, nach } => {
                *t += dt;
                let (t, von, nach) = (*t, *von, *nach);
                let hoehe = von.y - nach.y;
                // Erst frei fallen, dann am offenen Schirm in einer Spirale sinken.
                let p = if t < FREIER_FALL {
                    von + Vec3::new(0.0, -4.9 * t * t, t * 1.2)
                } else {
                    let s = t - FREIER_FALL;
                    let start_y = von.y - 4.9 * FREIER_FALL * FREIER_FALL;
                    let dauer = ((start_y - nach.y) / SINKEN).max(0.1);
                    let k = (s / dauer).min(1.0);
                    let kreis = (1.0 - k) * 1.2;
                    let w = s * 1.4;
                    let ziel = von.lerp(nach, k);
                    Vec3::new(
                        ziel.x + w.sin() * kreis,
                        start_y - (start_y - nach.y) * k,
                        ziel.z + w.cos() * kreis,
                    )
                };
                let blick = (nach.x - von.x).atan2(nach.z - von.z);
                self.figur.haltung = Haltung::Stehen;
                self.zeigen(p, blick, dt);
                if let Some((s, _)) = self.schirm.as_mut() {
                    let auf = ((t - FREIER_FALL) / 0.4).clamp(0.0, 1.0);
                    s.set_visible(auf > 0.0);
                    s.set_position(p);
                    s.set_local_scale(auf.max(0.05), auf.max(0.05), auf.max(0.05));
                }
                if p.y <= nach.y + 0.01 || hoehe <= 0.0 {
                    self.phase = Phase::Unten(0.0);
                }
                true
            }
            Phase::Unten(t) => {
                *t += dt;
                let t = *t;
                // Der Schirm fällt in sich zusammen, dann geht er.
                if let Some((s, _)) = self.schirm.as_mut() {
                    let k = (1.0 - t / 0.8).max(0.05);
                    s.set_local_scale(1.0, k, 1.0);
                }
                if t > 1.5 {
                    self.weg_damit();
                    return false;
                }
                true
            }
        }
    }
}
