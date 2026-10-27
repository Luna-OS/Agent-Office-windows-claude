//! Das Wire-Protokoll von OpenAgenticOffice.
//!
//! Jede Domäne hat ihre eigene Datei mit einem `Client`- und einem `Server`-Enum
//! (Nachrichten vom Browser bzw. an den Browser). Hier werden sie nur zusammengesetzt.
//! Eine neue Domäne: eigene Datei, ein `pub mod`, je eine Variante unten.

pub mod agenten;
pub mod arbeiter;
pub mod aufgaben;
pub mod dach;
pub mod einstellungen;
pub mod etagen;
pub mod github;
pub mod konten;
pub mod medien;
pub mod praesenz;
pub mod spielzeug;

use serde::{Deserialize, Serialize};

/// Ein Frame vom Browser an den Server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "domaene", content = "nachricht", rename_all = "snake_case")]
pub enum ClientNachricht {
    Agenten(agenten::Client),
    Arbeiter(arbeiter::Client),
    Aufgaben(aufgaben::Client),
    Dach(dach::Client),
    Einstellungen(einstellungen::Client),
    Etagen(etagen::Client),
    Github(github::Client),
    Konten(konten::Client),
    Medien(medien::Client),
    Praesenz(praesenz::Client),
    Spielzeug(spielzeug::Client),
}

/// Ein Frame vom Server an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "domaene", content = "nachricht", rename_all = "snake_case")]
pub enum ServerNachricht {
    Agenten(agenten::Server),
    Arbeiter(arbeiter::Server),
    Aufgaben(aufgaben::Server),
    Dach(dach::Server),
    Einstellungen(einstellungen::Server),
    Etagen(etagen::Server),
    Github(github::Server),
    Konten(konten::Server),
    Medien(medien::Server),
    Praesenz(praesenz::Server),
    Spielzeug(spielzeug::Server),
    /// Eine Fehlermeldung für den Menschen, auf Deutsch.
    Fehler { text: String },
}
