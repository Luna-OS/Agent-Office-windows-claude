//! BLOCKFALL: der Schacht ist 10 breit und 20 tief, sieben Teile, Halten, harter Fall, Landeplatz.
//! Reine Spiellogik ohne Browser, damit sie sich testen lässt.

pub const SPALTEN: usize = 10;
pub const ZEILEN: usize = 20;
/// Punkte für 1–4 Reihen auf einmal, mal Level.
const PUNKTE: [u32; 5] = [0, 100, 300, 500, 800];

/// Die sieben Teile (I, O, T, S, Z, J, L) als Zellen in einem 4×4-Feld, Grunddrehung.
const TEILE: [[(i32, i32); 4]; 7] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lage {
    Laeuft,
    Pause,
    Vorbei,
}

#[derive(Clone)]
pub struct Spiel {
    /// 0 leer, 1–7 Farbe.
    pub schacht: [[u8; SPALTEN]; ZEILEN],
    pub teil: u8,
    pub drehung: u8,
    pub x: i32,
    pub y: i32,
    pub naechstes: u8,
    pub halten: u8,
    pub gehalten: bool,
    pub punkte: u32,
    pub reihen: u32,
    pub teile: u32,
    pub lage: Lage,
    zufall: u64,
}

impl Spiel {
    pub fn neu(saat: u64) -> Self {
        let mut s = Self {
            schacht: [[0; SPALTEN]; ZEILEN],
            teil: 1,
            drehung: 0,
            x: 3,
            y: 0,
            naechstes: 1,
            halten: 0,
            gehalten: false,
            punkte: 0,
            reihen: 0,
            teile: 0,
            lage: Lage::Laeuft,
            zufall: saat | 1,
        };
        s.naechstes = s.wuerfeln();
        s.neues_teil();
        s
    }

    fn wuerfeln(&mut self) -> u8 {
        // xorshift64
        self.zufall ^= self.zufall << 13;
        self.zufall ^= self.zufall >> 7;
        self.zufall ^= self.zufall << 17;
        (self.zufall % 7) as u8 + 1
    }

    pub fn level(&self) -> u32 {
        (1 + self.reihen / 10).min(99)
    }

    /// Millisekunden pro Schritt nach unten.
    pub fn takt_ms(&self) -> u32 {
        (800.0 * 0.85f64.powi(self.level() as i32 - 1)).max(60.0) as u32
    }

    /// Die vier Zellen eines Teils in einer Drehung (gedreht um die Mitte des 4×4-Felds; O dreht nicht).
    pub fn zellen(teil: u8, drehung: u8) -> [(i32, i32); 4] {
        let mut z = TEILE[(teil - 1) as usize];
        if teil == 2 {
            return z;
        }
        let groesse = if teil == 1 { 4 } else { 3 };
        for _ in 0..drehung % 4 {
            for c in z.iter_mut() {
                *c = (groesse - 1 - c.1, c.0);
            }
        }
        z
    }

    fn passt(&self, teil: u8, drehung: u8, x: i32, y: i32) -> bool {
        Self::zellen(teil, drehung).iter().all(|(dx, dy)| {
            let (cx, cy) = (x + dx, y + dy);
            (0..SPALTEN as i32).contains(&cx) && cy < ZEILEN as i32 && (cy < 0 || self.schacht[cy as usize][cx as usize] == 0)
        })
    }

    fn neues_teil(&mut self) {
        self.teil = self.naechstes;
        self.naechstes = self.wuerfeln();
        self.drehung = 0;
        self.x = 3;
        self.y = 0;
        self.gehalten = false;
        if !self.passt(self.teil, 0, self.x, self.y) {
            self.lage = Lage::Vorbei;
        }
    }

    pub fn schieben(&mut self, dx: i32) -> bool {
        if self.lage == Lage::Laeuft && self.passt(self.teil, self.drehung, self.x + dx, self.y) {
            self.x += dx;
            return true;
        }
        false
    }

    /// Drehen, mit kleinen Ausweichschritten an Wänden.
    pub fn drehen(&mut self) -> bool {
        if self.lage != Lage::Laeuft {
            return false;
        }
        let d = (self.drehung + 1) % 4;
        for dx in [0, -1, 1, -2, 2] {
            if self.passt(self.teil, d, self.x + dx, self.y) {
                self.drehung = d;
                self.x += dx;
                return true;
            }
        }
        false
    }

    /// Ein Schritt nach unten; landet das Teil, `true`.
    pub fn fallen(&mut self) -> bool {
        if self.lage != Lage::Laeuft {
            return false;
        }
        if self.passt(self.teil, self.drehung, self.x, self.y + 1) {
            self.y += 1;
            return false;
        }
        self.landen();
        true
    }

    pub fn ganz_runter(&mut self) {
        while self.lage == Lage::Laeuft && !self.fallen() {}
    }

    pub fn halten(&mut self) {
        if self.lage != Lage::Laeuft || self.gehalten {
            return;
        }
        let jetzt = self.teil;
        if self.halten == 0 {
            self.halten = jetzt;
            self.neues_teil();
        } else {
            self.teil = std::mem::replace(&mut self.halten, jetzt);
            self.drehung = 0;
            self.x = 3;
            self.y = 0;
        }
        self.gehalten = true;
    }

    pub fn pause(&mut self) {
        self.lage = match self.lage {
            Lage::Laeuft => Lage::Pause,
            Lage::Pause => Lage::Laeuft,
            Lage::Vorbei => Lage::Vorbei,
        };
    }

    fn landen(&mut self) {
        for (dx, dy) in Self::zellen(self.teil, self.drehung) {
            let (cx, cy) = (self.x + dx, self.y + dy);
            if cy < 0 {
                self.lage = Lage::Vorbei;
                return;
            }
            self.schacht[cy as usize][cx as usize] = self.teil;
        }
        self.teile += 1;
        let voll: Vec<usize> = (0..ZEILEN).filter(|&z| self.schacht[z].iter().all(|&c| c != 0)).collect();
        for &z in &voll {
            for r in (1..=z).rev() {
                self.schacht[r] = self.schacht[r - 1];
            }
            self.schacht[0] = [0; SPALTEN];
        }
        self.punkte = self.punkte.saturating_add(PUNKTE[voll.len().min(4)] * self.level());
        self.reihen += voll.len() as u32;
        self.neues_teil();
    }

    /// Wo das Teil landen würde (y).
    fn landeplatz(&self) -> i32 {
        let mut y = self.y;
        while self.passt(self.teil, self.drehung, self.x, y + 1) {
            y += 1;
        }
        y
    }

    /// Der Schacht als Zeichenkette fürs Protokoll: '0' leer, '1'–'7' Farbe, '8' Landeplatz.
    pub fn zellen_text(&self) -> String {
        let mut b = self.schacht.map(|z| z.map(|c| c + b'0'));
        if self.lage != Lage::Vorbei {
            let zellen = Self::zellen(self.teil, self.drehung);
            let land = self.landeplatz();
            for (dx, dy) in zellen {
                let (cx, cy) = (self.x + dx, land + dy);
                if cy >= 0 && b[cy as usize][cx as usize] == b'0' {
                    b[cy as usize][cx as usize] = b'8';
                }
            }
            for (dx, dy) in zellen {
                let (cx, cy) = (self.x + dx, self.y + dy);
                if cy >= 0 {
                    b[cy as usize][cx as usize] = self.teil + b'0';
                }
            }
        }
        b.iter().flat_map(|z| z.iter().map(|&c| c as char)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reihe_raeumt_und_zaehlt() {
        let mut s = Spiel::neu(42);
        s.schacht[ZEILEN - 1] = [1; SPALTEN];
        s.schacht[ZEILEN - 1][0] = 0;
        // Ein I senkrecht ganz links hinein.
        s.teil = 1;
        s.drehung = 1;
        s.x = -2;
        assert!(s.passt(1, 1, -2, 0));
        s.ganz_runter();
        assert_eq!(s.reihen, 1);
        assert_eq!(s.punkte, 100);
        assert_eq!(s.zellen_text().len(), SPALTEN * ZEILEN);
    }

    #[test]
    fn zu_hoch_ist_vorbei() {
        let mut s = Spiel::neu(7);
        for _ in 0..40 {
            s.ganz_runter();
        }
        assert_eq!(s.lage, Lage::Vorbei);
    }
}
