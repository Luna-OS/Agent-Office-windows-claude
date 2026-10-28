//! App-Einstellungen: gemerkte Lobbys, Daten-Ordner, Abo-Modus. Liegen als JSON im Konfigurationsordner.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Einstellungen {
    /// Adressen der Lobbys, in der Reihenfolge, wie sie zuletzt benutzt wurden.
    pub lobbys: Vec<String>,
    /// Daten-Ordner des Office auf diesem Computer (`--daten`).
    pub daten: Option<String>,
    /// Ordner, in den Projekte geklont werden (`--projekte`).
    pub projekte: Option<String>,
    /// API-Schlüssel-Variablen für die Worker entfernen, damit das Claude-Abo gilt.
    pub nur_abo: bool,
    /// Gemerktes Lobby-Passwort, wenn die Lobby auf diesem Computer für andere offen ist.
    pub lobby_passwort: Option<String>,
}

impl Default for Einstellungen {
    fn default() -> Self {
        Self {
            lobbys: Vec::new(),
            daten: None,
            projekte: None,
            nur_abo: true,
            lobby_passwort: None,
        }
    }
}

pub fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Ordner für die Einstellungen der App.
pub fn konfig_ordner() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(home)
            .join("open-agentic-office-desktop")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support/OpenAgenticOffice")
    } else {
        home().join(".config/open-agentic-office-desktop")
    }
}

fn datei() -> PathBuf {
    konfig_ordner().join("einstellungen.json")
}

impl Einstellungen {
    pub fn laden() -> Self {
        std::fs::read_to_string(datei())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn speichern(&self) {
        let _ = std::fs::create_dir_all(konfig_ordner());
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(datei(), t);
        }
    }

    /// Daten-Ordner des lokalen Office (Standard wie beim Server: `~/open-agentic-office`).
    pub fn daten_ordner(&self) -> PathBuf {
        self.daten
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("open-agentic-office"))
    }

    /// Lobby nach vorn holen (oder neu aufnehmen).
    pub fn lobby_merken(&mut self, adresse: &str) {
        self.lobbys.retain(|l| l != adresse);
        self.lobbys.insert(0, adresse.to_string());
        self.lobbys.truncate(12);
        self.speichern();
    }

    pub fn lobby_vergessen(&mut self, adresse: &str) {
        self.lobbys.retain(|l| l != adresse);
        self.speichern();
    }
}
