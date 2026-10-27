//! Anstoßen: wo man stehen kann, was im Weg ist und was über dem Kopf ist (wie `player/collide.ts`).
//!
//! Alles ist ein Quader auf dem Boden (`Kasten`) mit Oberkante und, für Dinge, unter denen man
//! durchgeht (die Empore), Unterkante. Der Spieler ist ein Kreis mit `RADIUS`.

/// Der Spieler von oben gesehen.
pub const RADIUS: f32 = 0.32;
/// Kopfhöhe über den Füßen.
pub const GROESSE: f32 = 1.7;
/// Die höchste Kante, die man ohne Sprung hinauf- oder hinuntergeht (eine Stufe).
pub const STUFE: f32 = 0.3;

/// Ein Hindernis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kasten {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub oben: f32,
    /// Unterkante; 0 = steht auf dem Boden.
    pub unten: f32,
    /// Nur zum Fernhalten da (Geländer): darauf landet man nicht.
    pub zaun: bool,
}

impl Kasten {
    pub const fn neu(min_x: f32, max_x: f32, min_z: f32, max_z: f32, oben: f32) -> Self {
        Self { min_x, max_x, min_z, max_z, oben, unten: f32::NEG_INFINITY, zaun: false }
    }
    /// Ein Quader um die Mitte (x, z) mit Breite b (x) und Tiefe t (z).
    pub fn um(x: f32, z: f32, b: f32, t: f32, oben: f32) -> Self {
        Self::neu(x - b / 2.0, x + b / 2.0, z - t / 2.0, z + t / 2.0, oben)
    }
    pub fn mit_unten(mut self, unten: f32) -> Self {
        self.unten = unten;
        self
    }
    pub fn als_zaun(mut self) -> Self {
        self.zaun = true;
        self
    }
    /// Ob ein Kreis mit Radius `r` bei (x, z) die Grundfläche berührt.
    pub fn beruehrt(&self, x: f32, z: f32, r: f32) -> bool {
        let nx = x.clamp(self.min_x, self.max_x);
        let nz = z.clamp(self.min_z, self.max_z);
        (x - nx).powi(2) + (z - nz).powi(2) < r * r
    }
    /// Wie tief der Spielerkreis eindringt (auch wenn die Mitte drinnen ist).
    fn eindringen(&self, x: f32, z: f32) -> f32 {
        let dx = (self.min_x - x).max(0.0).max(x - self.max_x);
        let dz = (self.min_z - z).max(0.0).max(z - self.max_z);
        if dx > 0.0 || dz > 0.0 {
            return RADIUS - dx.hypot(dz);
        }
        RADIUS + (x - self.min_x).min(self.max_x - x).min(z - self.min_z).min(self.max_z - z)
    }
    /// Ob ein Schritt von (vx, vz) nach (x, z) aus einer Überlappung herausführt.
    fn entkommt(&self, vx: f32, vz: f32, x: f32, z: f32) -> bool {
        if self.eindringen(x, z) >= self.eindringen(vx, vz) - 1e-6 {
            return false;
        }
        let nx = vx - vx.clamp(self.min_x, self.max_x);
        let nz = vz - vz.clamp(self.min_z, self.max_z);
        if nx != 0.0 || nz != 0.0 {
            return nx * (x - vx) + nz * (z - vz) >= 0.0;
        }
        let naechste = (vx - self.min_x).min(self.max_x - vx).min(vz - self.min_z).min(self.max_z - vz);
        (naechste == vx - self.min_x && x < vx)
            || (naechste == self.max_x - vx && x > vx)
            || (naechste == vz - self.min_z && z < vz)
            || (naechste == self.max_z - vz && z > vz)
    }
}

/// Ein Körper: Füße bei `pos`, ob er auf dem Boden steht, und ein weich ausgeglichener Stufenversatz.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Koerper {
    pub pos: [f32; 3],
    pub am_boden: bool,
    /// Nach einer Stufe hoch/runter: wird über ein paar Frames auf 0 gebracht (für die Kamera).
    pub stufen_versatz: f32,
}

/// Was bei (x, z) mit Füßen auf Höhe `y` im Weg steht.
pub fn blockiert<'a>(kaesten: &'a [Kasten], von: [f32; 3], x: f32, z: f32, y: f32, entkommen: bool) -> Option<&'a Kasten> {
    let mut treffer: Option<&Kasten> = None;
    for k in kaesten {
        let unten = if k.unten.is_finite() { k.unten } else { 0.0 };
        if y >= k.oben - 0.05 || (k.unten.is_finite() && y + GROESSE <= unten) {
            continue;
        }
        if entkommen && k.beruehrt(von[0], von[2], RADIUS) {
            if k.entkommt(von[0], von[2], x, z) {
                continue;
            }
        } else if !k.beruehrt(x, z, RADIUS) {
            continue;
        }
        if treffer.is_none_or(|t| k.oben > t.oben) {
            treffer = Some(k);
        }
    }
    treffer
}

/// Der Boden unter (x, z) für Füße auf Höhe `y`: die höchste Oberkante, auf oder über der man ist.
pub fn boden(kaesten: &[Kasten], x: f32, z: f32, y: f32) -> f32 {
    let mut g = f32::NEG_INFINITY;
    for k in kaesten {
        if k.oben > 50.0 || y < k.oben - 0.1 || k.oben <= g || k.zaun {
            continue;
        }
        if k.beruehrt(x, z, RADIUS) {
            g = k.oben;
        }
    }
    g
}

/// Die Unterseite dessen, was bei (x, z) über dem Kopf ist, oder unendlich.
pub fn decke(kaesten: &[Kasten], x: f32, z: f32, y: f32) -> f32 {
    let mut d = f32::INFINITY;
    for k in kaesten {
        if !k.unten.is_finite() || k.unten <= y + 0.1 || k.unten >= d {
            continue;
        }
        if k.beruehrt(x, z, RADIUS) {
            d = k.unten;
        }
    }
    d
}

/// Ein Schritt entlang einer Achse nach (x, z): eine Stufe hinauf, wenn oben Platz ist, sonst so weit es geht.
pub fn schritt(k: &mut Koerper, kaesten: &[Kasten], x: f32, z: f32) {
    let Some(hindernis) = blockiert(kaesten, k.pos, x, z, k.pos[1], true) else {
        k.pos[0] = x;
        k.pos[2] = z;
        return;
    };
    let hoch = hindernis.oben - k.pos[1];
    if k.am_boden
        && hoch <= STUFE
        && blockiert(kaesten, k.pos, x, z, hindernis.oben, false).is_none()
        && k.pos[1] + GROESSE + hoch <= decke(kaesten, x, z, k.pos[1])
    {
        k.pos = [x, hindernis.oben, z];
        k.stufen_versatz -= hoch;
        return;
    }
    // Den freien Teil des Schritts nutzen, damit man an Wänden entlanggleitet.
    let (dx, dz) = (x - k.pos[0], z - k.pos[2]);
    let (mut frei, mut zu) = (0.0f32, 1.0f32);
    for _ in 0..12 {
        let f = (frei + zu) / 2.0;
        if blockiert(kaesten, k.pos, k.pos[0] + dx * f, k.pos[2] + dz * f, k.pos[1], true).is_some() {
            zu = f;
        } else {
            frei = f;
        }
    }
    k.pos[0] += dx * frei;
    k.pos[2] += dz * frei;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wand() -> Vec<Kasten> {
        vec![Kasten::neu(1.0, 2.0, -5.0, 5.0, 3.0)]
    }

    #[test]
    fn wand_haelt_auf() {
        let mut k = Koerper { pos: [0.0, 0.0, 0.0], am_boden: true, ..Default::default() };
        schritt(&mut k, &wand(), 1.5, 0.0);
        assert!(k.pos[0] <= 1.0 - RADIUS + 0.01, "steckt in der Wand: {:?}", k.pos);
        assert!(k.pos[0] > 0.5, "sollte bis an die Wand gehen");
    }

    #[test]
    fn stufe_wird_erklommen() {
        let stufe = vec![Kasten::neu(1.0, 3.0, -1.0, 1.0, 0.25)];
        let mut k = Koerper { pos: [0.5, 0.0, 0.0], am_boden: true, ..Default::default() };
        schritt(&mut k, &stufe, 0.8, 0.0);
        assert_eq!(k.pos[1], 0.25);
        assert!(k.stufen_versatz < 0.0);
    }

    #[test]
    fn tisch_ist_zu_hoch_ohne_sprung() {
        let tisch = vec![Kasten::neu(1.0, 3.0, -1.0, 1.0, 0.78)];
        let mut k = Koerper { pos: [0.5, 0.0, 0.0], am_boden: true, ..Default::default() };
        schritt(&mut k, &tisch, 0.8, 0.0);
        assert_eq!(k.pos[1], 0.0);
        assert!(k.pos[0] < 0.7);
        // Im Sprung über der Kante geht es.
        let mut k = Koerper { pos: [0.5, 0.9, 0.0], am_boden: false, ..Default::default() };
        schritt(&mut k, &tisch, 0.8, 0.0);
        assert_eq!(k.pos[0], 0.8);
        assert_eq!(boden(&tisch, 0.8, 0.0, 0.9), 0.78);
    }

    #[test]
    fn unter_der_empore_durch() {
        let empore = vec![Kasten::neu(0.0, 10.0, 0.0, 10.0, 3.2).mit_unten(3.0)];
        let mut k = Koerper { pos: [-1.0, 0.0, 5.0], am_boden: true, ..Default::default() };
        schritt(&mut k, &empore, 1.0, 5.0);
        assert_eq!(k.pos[0], 1.0);
        assert_eq!(decke(&empore, 1.0, 5.0, 0.0), 3.0);
        assert_eq!(boden(&empore, 1.0, 5.0, 0.0), f32::NEG_INFINITY);
    }

    #[test]
    fn aus_ueberlappung_heraus_aber_nicht_hindurch() {
        let w = wand();
        let mut k = Koerper { pos: [0.9, 0.0, 0.0], am_boden: true, ..Default::default() };
        schritt(&mut k, &w, 0.7, 0.0);
        assert_eq!(k.pos[0], 0.7, "heraus geht");
        let mut k = Koerper { pos: [0.9, 0.0, 0.0], am_boden: true, ..Default::default() };
        schritt(&mut k, &w, 1.1, 0.0);
        assert!(k.pos[0] <= 0.9 + 1e-4, "tiefer hinein nicht");
    }

    #[test]
    fn zaun_ist_kein_boden() {
        let z = vec![Kasten::neu(0.0, 1.0, 0.0, 1.0, 1.0).als_zaun()];
        assert_eq!(boden(&z, 0.5, 0.5, 1.0), f32::NEG_INFINITY);
    }
}
