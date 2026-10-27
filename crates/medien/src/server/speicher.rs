//! Jukebox und Leinwand überleben einen Neustart: kleine JSON-Dateien im Datenordner.

use std::path::PathBuf;

use protokoll::dach::LeinwandZustand;

use super::jetzt_ms;
use crate::jukebox::Jukebox;
use crate::leinwand::Leinwand;

pub(super) struct Speicher {
    ordner: PathBuf,
}

impl Speicher {
    pub fn neu(ordner: PathBuf) -> Self {
        Self { ordner }
    }

    fn jukebox_datei(&self, etage: &str) -> PathBuf {
        self.ordner.join(format!("jukebox-{}.json", super::pfad_teil(etage)))
    }

    pub fn jukebox_laden(&self, etage: &str) -> Jukebox {
        match std::fs::read_to_string(self.jukebox_datei(etage)) {
            Ok(json) => Jukebox::laden(&json, jetzt_ms()),
            Err(_) => Jukebox::neu(jetzt_ms()),
        }
    }

    pub fn jukebox_speichern(&self, etage: &str, json: &str) {
        self.schreiben(self.jukebox_datei(etage), json);
    }

    pub fn leinwand_laden(&self) -> Leinwand {
        std::fs::read_to_string(self.ordner.join("leinwand.json"))
            .ok()
            .and_then(|j| serde_json::from_str::<LeinwandZustand>(&j).ok())
            .map(Leinwand::aus_zustand)
            .unwrap_or_default()
    }

    pub fn leinwand_speichern(&self, z: &LeinwandZustand) {
        if let Ok(json) = serde_json::to_string_pretty(z) {
            self.schreiben(self.ordner.join("leinwand.json"), &json);
        }
    }

    /// Plattenprobleme sollen das Office nicht umwerfen: Fehler werden nur geloggt.
    fn schreiben(&self, datei: PathBuf, inhalt: &str) {
        let r = std::fs::create_dir_all(&self.ordner).and_then(|_| std::fs::write(&datei, inhalt));
        if let Err(e) = r {
            eprintln!("medien: konnte {} nicht speichern: {e}", datei.display());
        }
    }
}
