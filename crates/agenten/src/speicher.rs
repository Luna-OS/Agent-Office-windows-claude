//! Speichern und Laden einer Etage: `<daten>/etagen/<etage>/workers.json` (Arbeiter, ihre Notizen und
//! ihr letzter Bildschirm), `bus.json` und `board.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use protokoll::arbeiter::ArbeiterInfo;
use serde::{Deserialize, Serialize};

use crate::board::Aufgabenboard;
use crate::bus::Bus;
use crate::etage::Etage;

/// Standard-Grenze der Warteschlange.
pub const STANDARD_MAX_ARBEITER: u32 = 2;

#[derive(Default, Serialize, Deserialize)]
struct ArbeiterDatei {
    arbeiter: Vec<ArbeiterInfo>,
    #[serde(default)]
    notizen: HashMap<String, String>,
    /// Arbeiter-ID → letzter Bildschirm (Escape-Sequenzen).
    #[serde(default)]
    puffer: HashMap<String, String>,
    /// Arbeiter-ID → sein Terminal im PTY-Wirt, zum Übernehmen nach einem Neustart.
    #[serde(default)]
    wirt: HashMap<String, crate::uebernahme::WirtStand>,
}

pub struct Geladen {
    pub arbeiter: Vec<ArbeiterInfo>,
    pub notizen: HashMap<String, String>,
    pub puffer: HashMap<String, String>,
    pub wirt: HashMap<String, crate::uebernahme::WirtStand>,
    pub bus: Bus,
    pub board: Aufgabenboard,
}

/// Der Ordner einer Etage; die Etagen-ID wird für den Dateinamen entschärft.
pub fn ordner(daten: &Path, etage: &str) -> PathBuf {
    let sicher: String = etage
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    daten.join("etagen").join(if sicher.is_empty() {
        "_".into()
    } else {
        sicher
    })
}

fn lesen<T: for<'de> Deserialize<'de>>(pfad: &Path) -> Option<T> {
    let text = std::fs::read_to_string(pfad).ok()?;
    match serde_json::from_str(&text) {
        Ok(wert) => Some(wert),
        Err(e) => {
            tracing::warn!("{} ist beschädigt und wird ignoriert: {e}", pfad.display());
            None
        }
    }
}

fn schreiben<T: Serialize>(pfad: &Path, wert: &T) {
    let ergebnis = (|| -> std::io::Result<()> {
        if let Some(d) = pfad.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = pfad.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(wert)?)?;
        std::fs::rename(&tmp, pfad)
    })();
    if let Err(e) = ergebnis {
        tracing::warn!("{} konnte nicht gespeichert werden: {e}", pfad.display());
    }
}

pub fn laden(daten: &Path, etage: &str) -> Geladen {
    let dir = ordner(daten, etage);
    let datei: ArbeiterDatei = lesen(&dir.join("workers.json")).unwrap_or_default();
    Geladen {
        arbeiter: datei.arbeiter,
        notizen: datei.notizen,
        puffer: datei.puffer,
        wirt: datei.wirt,
        bus: lesen(&dir.join("bus.json")).unwrap_or_default(),
        board: lesen(&dir.join("board.json"))
            .unwrap_or_else(|| Aufgabenboard::neu(STANDARD_MAX_ARBEITER)),
    }
}

/// Speichert die Arbeiter einer Etage.
pub fn arbeiter_speichern(daten: &Path, e: &Etage) {
    let datei = ArbeiterDatei {
        arbeiter: e.arbeiter.values().map(|a| a.info.clone()).collect(),
        notizen: e
            .arbeiter
            .values()
            .filter_map(|a| a.notiz.clone().map(|n| (a.info.id.clone(), n)))
            .collect(),
        puffer: e
            .arbeiter
            .values()
            .map(|a| (a.info.id.clone(), a.terminal.snapshot()))
            .collect(),
        wirt: e
            .arbeiter
            .values()
            .filter_map(|a| {
                let sitzung = a.pty.as_ref()?.wirt_sitzung()?.to_string();
                Some((
                    a.info.id.clone(),
                    crate::uebernahme::WirtStand {
                        sitzung,
                        token: a.token.clone(),
                        status: a.info.status,
                    },
                ))
            })
            .collect(),
    };
    schreiben(&ordner(daten, &e.id).join("workers.json"), &datei);
}

pub fn bus_speichern(daten: &Path, e: &Etage) {
    schreiben(&ordner(daten, &e.id).join("bus.json"), &e.bus);
}

pub fn board_speichern(daten: &Path, e: &Etage) {
    schreiben(&ordner(daten, &e.id).join("board.json"), &e.board);
}

/// Alles einer Etage.
pub fn speichern(daten: &Path, e: &Etage) {
    arbeiter_speichern(daten, e);
    bus_speichern(daten, e);
    board_speichern(daten, e);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordnernamen_sind_sicher() {
        let d = Path::new("/d");
        assert_eq!(ordner(d, "../x"), Path::new("/d/etagen/___x"));
        assert_eq!(ordner(d, "projekt-1"), Path::new("/d/etagen/projekt-1"));
    }

    #[test]
    fn leere_etage_laedt_standard() {
        let dir = tempfile::tempdir().unwrap();
        let g = laden(dir.path(), "neu");
        assert!(g.arbeiter.is_empty());
        assert_eq!(g.board.board.max_arbeiter, STANDARD_MAX_ARBEITER);
    }
}
