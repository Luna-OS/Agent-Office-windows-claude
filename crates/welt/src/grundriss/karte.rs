//! Welche Karte das Gebäude ist (Büro oder Burg), und was davon abhängt: Plätze, Tafeln, Sitze.

use super::{Board, Tafel, Tisch, burg, sitze, tafel, tisch_nach_id};

/// Wie das Gebäude innen aussieht (`einstellungen::Karte::wahl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Karte {
    #[default]
    Buero,
    Burg,
}

impl Karte {
    /// Aus der Wahl in den Einstellungen; unbekannte (eigene) Karten zeigen vorerst das Büro.
    pub fn aus_wahl(wahl: &str) -> Self {
        if wahl == burg::BURG_ID {
            Karte::Burg
        } else {
            Karte::Buero
        }
    }

    /// Ein Arbeitsplatz auf dieser Karte.
    pub fn tisch(self, id: &str) -> Option<Tisch> {
        match self {
            Karte::Buero => tisch_nach_id(id),
            Karte::Burg => burg::tisch_nach_id(id),
        }
    }

    /// Ein Sitzplatz für Menschen ("couch:1", "throne:0" …), wenn es ihn auf dieser Karte und dort
    /// gibt, wo man ist (`dach`: auf dem Dach, sonst auf einer Etage).
    pub fn sitz(self, schluessel: &str, dach: bool) -> Option<sitze::Platz> {
        let ist_dach = |p: &sitze::Platz| {
            let id = p.schluessel.split(':').next().unwrap_or_default();
            sitze::sitze().iter().any(|s| s.id == id && s.dach)
        };
        match (self, dach) {
            // Das Dach ist auf jeder Karte dasselbe.
            (_, true) => sitze::platz_nach_schluessel(schluessel).filter(ist_dach),
            (Karte::Buero, false) => {
                sitze::platz_nach_schluessel(schluessel).filter(|p| !ist_dach(p))
            }
            (Karte::Burg, false) => (schluessel == burg::THRON_SITZ).then(burg::thron_platz),
        }
    }

    pub fn tafel(self, b: Board) -> Tafel {
        match self {
            Karte::Buero => tafel(b),
            Karte::Burg => burg::tafel(b),
        }
    }
}
