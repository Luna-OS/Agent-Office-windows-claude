//! Etagen (ein Projekt je Etage), Aufzug, Klonen von Repositories, Grundriss der Etage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::praesenz::{Ort, Person};

/// Eine Etage des Gebäudes: ein Projekt in seinem eigenen Checkout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtageInfo {
    pub id: String,
    /// Name des Repositorys, sonst des Ordners.
    pub name: String,
    /// owner/name auf GitHub.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Checkout auf dem Rechner des Office.
    pub ordner: String,
    /// Branch des Checkouts ("HEAD" wenn losgelöst); keiner, wenn es kein git-Checkout ist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Index in die Etagen-Farbpaletten des Clients.
    pub palette: u32,
    /// Wird gerade geklont: im Aufzug sichtbar, aber noch nicht betretbar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub klont: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub klon: Option<KlonFortschritt>,
    /// Das Projekt, in dem das Office gestartet wurde.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lokal: bool,
    pub hinzugefuegt_von: String,
    /// ms seit 1970.
    pub hinzugefuegt_um: u64,
    /// Für die Aufzugtafel: eingestellte Arbeiter, davon beschäftigt, davon wartend; Menschen dort.
    pub arbeiter: u32,
    pub beschaeftigt: u32,
    pub wartend: u32,
    pub personen: u32,
    /// Wie viele Reihen das Hinterbüro ausgebaut ist.
    pub fluegel: u32,
}

/// Wie weit ein Klon ist, aus der Ausgabe von git.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KlonFortschritt {
    /// "Herunterladen", "Dateien auschecken" …
    pub schritt: String,
    /// 0–100, wenn git es sagt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prozent: Option<u8>,
    /// z. B. "231.4 MiB · 1.5 MiB/s".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Wohin neue Etagen geklont werden: <ordner>/<owner>/<repo>.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjekteOrdner {
    /// Zum Anzeigen (unter dem Home-Ordner als ~/…).
    pub ordner: String,
    /// In den Einstellungen oder mit --projekte gesetzt.
    pub eigen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub um: Option<u64>,
}

/// Ein Repository, das als Etage hinzugefügt werden kann.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoAuswahl {
    /// owner/name
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beschreibung: Option<String>,
    pub privat: bool,
    /// ISO-Zeit des letzten Pushs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gepusht_um: Option<String>,
}

/// Das Projekt einer Etage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjektInfo {
    pub name: String,
    pub ordner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

/// Ein Schild über einem Schreibtisch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schild {
    pub text: String,
    /// Eine der Schildfarben des Clients ("#rrggbb").
    pub farbe: String,
    pub von: String,
    pub um: u64,
}

/// Grundriss einer Etage: Ausbau des Hinterbüros und Schilder je Schreibtisch-ID.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Grundriss {
    pub fluegel: u32,
    #[serde(default)]
    pub schilder: BTreeMap<String, Schild>,
}

/// Die Etagen-ID der Dachterrasse (über allen Projekten): `Gehen { etage: DACH }`, `Person::etage`, `Betreten`.
pub const DACH: &str = "@dach";

/// Höchster Ausbau des Hinterbüros (Reihen).
pub const FLUEGEL_MAX: u32 = 4;

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Auf eine andere Etage fahren; Antwort ist `Betreten`. Ohne `an` kommt man im Aufzug an.
    Gehen {
        etage: String,
        #[serde(default)]
        an: Option<Ort>,
    },
    /// Welche Repositories Etagen werden könnten; Antwort `Repos`.
    Repos {
        #[serde(default)]
        neu_laden: bool,
    },
    /// Ein Repository (owner/name oder URL) klonen und zur Etage machen; Antwort `Hinzugefuegt`.
    Hinzufuegen { repo: String },
    /// Einen laufenden Klon abbrechen (Admins oder wer ihn gestartet hat).
    Abbrechen { etage: String },
    /// Eine Etage entfernen (nur Admins). Der Checkout bleibt auf der Platte.
    Entfernen { etage: String },
    /// Wohin neue Etagen geklont werden (nur Admins); "" = Standard.
    ProjekteOrdner { ordner: String },
    /// Ein Schild über einen Schreibtisch hängen; leerer Text nimmt es ab.
    Schild {
        tisch: String,
        text: String,
        #[serde(default)]
        farbe: Option<String>,
    },
    /// Hinterbüro um eine Reihe erweitern bzw. zurückbauen.
    Erweitern,
    Verkleinern,
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Man ist auf einer (anderen) Etage angekommen: ersetzt alles von der vorigen. Danach folgen die
    /// Etagen-Stände der anderen Domänen (Spielzeug, Arbeiter, Medien …).
    Betreten {
        /// Keine, solange das Gebäude keine Etagen hat.
        etage: Option<String>,
        projekt: Option<ProjektInfo>,
        grundriss: Grundriss,
        /// Alle Menschen im Gebäude (ohne einen selbst), jeweils mit ihrer `etage`.
        personen: Vec<Person>,
        /// Wo man ankommt.
        ort: Ort,
    },
    /// Alle Etagen, für den Aufzug (bei jeder Änderung).
    Liste { etagen: Vec<EtageInfo> },
    /// Antwort auf `Repos`. `eigene`: mit der eigenen GitHub-Anmeldung gelistet.
    Repos {
        repos: Vec<RepoAuswahl>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fehler: Option<String>,
        #[serde(default)]
        eigene: bool,
    },
    /// An den, der die Etage wollte: fertig geklont (mit `etage`) oder nicht (mit `fehler`).
    Hinzugefuegt {
        repo: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        etage: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fehler: Option<String>,
    },
    ProjekteOrdner { stand: ProjekteOrdner },
    /// Schilder oder Ausbau der eigenen Etage haben sich geändert.
    Grundriss { grundriss: Grundriss },
}
