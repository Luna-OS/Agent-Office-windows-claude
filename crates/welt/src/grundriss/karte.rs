//! Welche Karte das Gebäude ist (Büro, Burg oder eine eigene Halle aus JSON), und was davon
//! abhängt: Plätze, Tafeln, Sitze.

use std::sync::Arc;

use super::halle::Hallenplan;
use super::{Board, Tafel, Tisch, burg, sitze, tafel, tisch_nach_id};

/// Wie das Gebäude innen aussieht (`einstellungen::Karte`).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Karte {
    #[default]
    Buero,
    /// Eine Halle: die Burg oder eine eigene Karte.
    Halle(Arc<Hallenplan>),
}

impl Karte {
    /// Aus der Wahl allein: "burg" ist die Burg, alles andere das Büro (eigene Karten brauchen
    /// ihre Daten, siehe `aus_einstellungen`).
    pub fn aus_wahl(wahl: &str) -> Self {
        if wahl == burg::BURG_ID {
            Karte::Halle(Arc::new(Hallenplan::burg()))
        } else {
            Karte::Buero
        }
    }

    /// Aus den Einstellungen, mit den eigenen Karten. Eine eigene Karte, die nicht lädt, zeigt das Büro.
    pub fn aus_einstellungen(k: &protokoll::einstellungen::Karte) -> Self {
        if let Some(e) = k.eigene.iter().find(|e| e.id == k.wahl) {
            return match e.daten.as_ref().map(Hallenplan::aus_json) {
                Some(Ok(p)) => Karte::Halle(Arc::new(p)),
                _ => Karte::Buero,
            };
        }
        Self::aus_wahl(&k.wahl)
    }

    /// Prüft eine eigene Karte (für die Liste in den Einstellungen): `Err` mit dem Grund.
    pub fn pruefen(daten: &serde_json::Value) -> Result<(), String> {
        Hallenplan::aus_json(daten).map(|_| ())
    }

    /// Der Hallenplan, wenn es eine Halle ist.
    pub fn halle(&self) -> Option<&Hallenplan> {
        match self {
            Karte::Halle(p) => Some(p),
            Karte::Buero => None,
        }
    }

    /// Ob Heimgeschickte auf dieser Karte in den Kerker kommen.
    pub fn kerker(&self) -> bool {
        self.halle().is_some_and(|p| p.kerker)
    }

    /// Ein Arbeitsplatz auf dieser Karte.
    pub fn tisch(&self, id: &str) -> Option<Tisch> {
        match self {
            Karte::Buero => tisch_nach_id(id),
            Karte::Halle(p) => burg::tisch_nach_id(p, id),
        }
    }

    /// Ein Sitzplatz für Menschen ("couch:1", "throne:0" …), wenn es ihn auf dieser Karte und dort
    /// gibt, wo man ist (`dach`: auf dem Dach, sonst auf einer Etage).
    pub fn sitz(&self, schluessel: &str, dach: bool) -> Option<sitze::Platz> {
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
            (Karte::Halle(p), false) => (schluessel == burg::THRON_SITZ)
                .then(|| burg::thron_platz(p))
                .flatten(),
        }
    }

    pub fn tafel(&self, b: Board) -> Tafel {
        match self {
            Karte::Buero => tafel(b),
            Karte::Halle(p) => burg::tafel(p, b),
        }
    }
}
