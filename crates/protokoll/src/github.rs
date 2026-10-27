//! GitHub: Issues und Pull Requests des Projekts einer Etage (über die `gh`-CLI).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    /// CSS-Farbe, z. B. `#d73a4a`.
    pub farbe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub nummer: u64,
    pub titel: String,
    /// `OPEN` oder `CLOSED`.
    pub zustand: String,
    pub url: String,
    pub autor: String,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub zugewiesen: Vec<String>,
    pub erstellt: String,
    pub geaendert: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub kommentare: u32,
}

/// Stand der Prüfungen (CI) eines Pull Requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pruefungen {
    Bestanden,
    Fehlgeschlagen,
    Laufen,
    #[default]
    Keine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub nummer: u64,
    pub titel: String,
    /// `OPEN`, `MERGED` oder `CLOSED`.
    pub zustand: String,
    pub entwurf: bool,
    pub url: String,
    pub autor: String,
    #[serde(default)]
    pub labels: Vec<Label>,
    /// `APPROVED`, `CHANGES_REQUESTED`, `REVIEW_REQUIRED` oder leer.
    #[serde(default)]
    pub review: String,
    pub branch: String,
    pub ziel: String,
    pub erstellt: String,
    pub geaendert: String,
    pub plus: u64,
    pub minus: u64,
    pub pruefungen: Pruefungen,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeArt {
    #[default]
    Squash,
    Merge,
    Rebase,
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Issues und PRs neu von GitHub holen.
    Aktualisieren,
    IssueAnlegen {
        titel: String,
        #[serde(default)]
        text: String,
    },
    Kommentieren {
        nummer: u64,
        text: String,
    },
    Mergen {
        nummer: u64,
        #[serde(default)]
        art: MergeArt,
        #[serde(default)]
        branch_loeschen: bool,
    },
    Schliessen { nummer: u64 },
    /// Einen Arbeiter für dieses Issue anlegen (am nächsten freien Tisch, wenn keiner genannt ist).
    IssueVergeben {
        nummer: u64,
        #[serde(default)]
        tisch: Option<String>,
        #[serde(default)]
        provider: Option<String>,
    },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    Stand {
        issues: Vec<Issue>,
        prs: Vec<PullRequest>,
        /// Fehlermeldung von `gh`, auf Deutsch.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fehler: Option<String>,
        laedt: bool,
    },
}
