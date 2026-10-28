//! Minesweeper wie im alten Office: 16 × 9 Felder, 22 Minen. Der erste Klick ist nie eine Mine,
//! und auch nichts drumherum. Reine Spiellogik, ohne Browser testbar.

pub const SPALTEN: usize = 16;
pub const ZEILEN: usize = 9;
pub const MINEN: usize = 22;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Feld {
    pub mine: bool,
    pub offen: bool,
    pub fahne: bool,
    /// Minen in den acht Feldern ringsum.
    pub nah: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lage {
    Bereit,
    Laeuft,
    Gewonnen,
    Verloren,
}

#[derive(Clone)]
pub struct Spiel {
    pub felder: Vec<Feld>,
    pub lage: Lage,
    /// Die Mine, die hochging.
    pub knall: Option<usize>,
    zufall: u64,
}

/// Die bis zu acht Nachbarn eines Feldes.
pub fn nachbarn(i: usize) -> impl Iterator<Item = usize> {
    let (s, z) = ((i % SPALTEN) as i32, (i / SPALTEN) as i32);
    (-1..=1)
        .flat_map(move |dz| (-1..=1).map(move |ds| (s + ds, z + dz)))
        .filter(move |&(x, y)| {
            (x, y) != (s, z) && (0..SPALTEN as i32).contains(&x) && (0..ZEILEN as i32).contains(&y)
        })
        .map(|(x, y)| y as usize * SPALTEN + x as usize)
}

impl Spiel {
    pub fn neu(saat: u64) -> Self {
        Self {
            felder: vec![Feld::default(); SPALTEN * ZEILEN],
            lage: Lage::Bereit,
            knall: None,
            zufall: saat | 1,
        }
    }

    fn wuerfeln(&mut self, bis: usize) -> usize {
        self.zufall ^= self.zufall << 13;
        self.zufall ^= self.zufall >> 7;
        self.zufall ^= self.zufall << 17;
        (self.zufall % bis as u64) as usize
    }

    fn vorbei(&self) -> bool {
        matches!(self.lage, Lage::Gewonnen | Lage::Verloren)
    }

    /// Minen legen, überall außer auf `sicher` und drumherum.
    fn legen(&mut self, sicher: usize) {
        let frei: Vec<usize> = std::iter::once(sicher).chain(nachbarn(sicher)).collect();
        let mut plaetze: Vec<usize> = (0..self.felder.len())
            .filter(|i| !frei.contains(i))
            .collect();
        for n in 0..MINEN.min(plaetze.len()) {
            let k = n + self.wuerfeln(plaetze.len() - n);
            plaetze.swap(n, k);
            self.felder[plaetze[n]].mine = true;
        }
        for i in 0..self.felder.len() {
            self.felder[i].nah = nachbarn(i).filter(|&j| self.felder[j].mine).count() as u8;
        }
        self.lage = Lage::Laeuft;
    }

    /// Ein Feld aufdecken; leere Felder decken ihre Nachbarn mit auf.
    pub fn aufdecken(&mut self, i: usize) {
        if self.vorbei() || i >= self.felder.len() || self.felder[i].offen || self.felder[i].fahne {
            return;
        }
        if self.lage == Lage::Bereit {
            self.legen(i);
        }
        if self.felder[i].mine {
            self.knall = Some(i);
            self.lage = Lage::Verloren;
            return;
        }
        let mut offen = vec![i];
        while let Some(j) = offen.pop() {
            let f = &mut self.felder[j];
            if f.offen || f.fahne {
                continue;
            }
            f.offen = true;
            if f.nah == 0 {
                offen.extend(nachbarn(j));
            }
        }
        if self.felder.iter().all(|f| f.offen || f.mine) {
            self.lage = Lage::Gewonnen;
            self.felder
                .iter_mut()
                .filter(|f| f.mine)
                .for_each(|f| f.fahne = true);
        }
    }

    pub fn fahne(&mut self, i: usize) {
        if !self.vorbei() && i < self.felder.len() && !self.felder[i].offen {
            self.felder[i].fahne = !self.felder[i].fahne;
        }
    }

    /// Auf einer Zahl mit so vielen Fahnen ringsum: den Rest ringsum aufdecken.
    pub fn akkord(&mut self, i: usize) {
        let f = self.felder[i];
        if self.vorbei() || !f.offen || f.nah == 0 {
            return;
        }
        if nachbarn(i).filter(|&j| self.felder[j].fahne).count() as u8 == f.nah {
            for j in nachbarn(i).collect::<Vec<_>>() {
                self.aufdecken(j);
            }
        }
    }

    /// Minen minus Fahnen.
    pub fn uebrig(&self) -> i32 {
        MINEN as i32 - self.felder.iter().filter(|f| f.fahne).count() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erster_klick_ist_sicher_und_oeffnet() {
        for saat in 1..20 {
            let mut s = Spiel::neu(saat);
            s.aufdecken(70);
            assert_eq!(s.lage, Lage::Laeuft);
            assert_eq!(s.felder.iter().filter(|f| f.mine).count(), MINEN);
            assert!(s.felder[70].offen && s.felder[70].nah == 0);
        }
    }

    #[test]
    fn mine_verliert_und_alles_frei_gewinnt() {
        let mut s = Spiel::neu(3);
        s.aufdecken(0);
        let mine = s.felder.iter().position(|f| f.mine).unwrap();
        let mut t = s.clone();
        t.aufdecken(mine);
        assert_eq!(t.lage, Lage::Verloren);
        for i in 0..s.felder.len() {
            if !s.felder[i].mine {
                s.aufdecken(i);
            }
        }
        assert_eq!(s.lage, Lage::Gewonnen);
        assert_eq!(s.uebrig(), 0);
    }
}
