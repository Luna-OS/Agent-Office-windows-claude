//! Das gemeinsame Aufgabenboard einer Etage: Menschen und Agenten legen Aufgaben an, übernehmen
//! und erledigen sie. Aufgaben mit `automatisch` setzt das Office selbst einem neuen Arbeiter vor
//! (die Warteschlange), solange weniger als `max_arbeiter` davon laufen.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AufgabenStatus {
    #[default]
    Offen,
    InArbeit,
    Erledigt,
    Blockiert,
}

/// Eine Aufgabe auf dem Board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aufgabe {
    /// Fortlaufend je Etage.
    pub id: u64,
    pub titel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notiz: Option<String>,
    pub status: AufgabenStatus,
    /// Name des Arbeiters oder Menschen, der sie macht.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besitzer: Option<String>,
    pub angelegt_von: String,
    pub angelegt_um: u64,
    pub geaendert_um: u64,
    /// Das GitHub-Issue, aus dem sie kommt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    /// Auftrag für den Agenten, wenn das Office sie selbst vergibt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Das Office setzt dafür selbst einen Arbeiter an (Warteschlange).
    #[serde(default)]
    pub automatisch: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Der Arbeiter, der dafür angesetzt wurde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arbeiter: Option<String>,
}

/// Das ganze Board.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub aufgaben: Vec<Aufgabe>,
    /// Wie viele Arbeiter die Warteschlange gleichzeitig beschäftigen darf; 0 hält sie an.
    pub max_arbeiter: u32,
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    Hinzufuegen {
        titel: String,
        #[serde(default)]
        notiz: Option<String>,
        #[serde(default)]
        prompt: Option<String>,
        #[serde(default)]
        issue: Option<u64>,
        #[serde(default)]
        besitzer: Option<String>,
        #[serde(default)]
        automatisch: bool,
        #[serde(default)]
        provider: Option<String>,
    },
    /// Nur die gesetzten Felder ändern sich; `besitzer: Some("")` nimmt den Besitzer weg.
    Aendern {
        id: u64,
        #[serde(default)]
        titel: Option<String>,
        #[serde(default)]
        status: Option<AufgabenStatus>,
        #[serde(default)]
        besitzer: Option<String>,
        #[serde(default)]
        notiz: Option<String>,
    },
    Entfernen {
        id: u64,
    },
    /// Nach oben (-1) oder unten (+1) schieben.
    Verschieben {
        id: u64,
        delta: i32,
    },
    /// Erledigte Aufgaben vergessen.
    ErledigteLeeren,
    Limit {
        max_arbeiter: u32,
    },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    Stand { board: Board },
}
