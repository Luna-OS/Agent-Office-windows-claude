//! Arbeiter an den Schreibtischen: anlegen, stoppen, Terminal-Ein-/Ausgabe.

use serde::{Deserialize, Serialize};

/// Wie es einem Arbeiter gerade geht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbeiterStatus {
    /// Terminal läuft, der Agent fährt hoch.
    Startet,
    /// Wartet auf seinen ersten Auftrag.
    Bereit,
    /// Der Agent arbeitet.
    Arbeitet,
    /// Eine Rückfrage oder Erlaubnis ist offen: ein Mensch wird gebraucht.
    Wartet,
    /// Hat seinen Zug beendet.
    Fertig,
    /// Der Prozess ist beendet (fortsetzbar, wenn er eine Sitzung hatte).
    Beendet,
    /// Nach einem Neustart des Servers von der Platte geladen, fortsetzbar.
    Offline,
}

/// Was an einem Schreibtisch läuft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbeiterArt {
    /// Ein Agent eines Providers (Claude Code, Codex, ...).
    #[default]
    Agent,
    /// Eine einfache, gemeinsame Shell.
    Shell,
}

/// Was der letzte Werkzeugaufruf eines arbeitenden Agenten ist, zum Vorspielen in der Welt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aktion {
    Lesen,
    Schreiben,
    Testen,
    Web,
    /// Tests oder Build schlagen immer wieder fehl.
    Scheitert,
}

/// Der eigene Git-Worktree eines Arbeiters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    /// Pfad relativ zum Projektordner der Etage.
    pub pfad: String,
    /// Der Branch, auf dem der Worktree gerade steht.
    pub branch: String,
    /// Der Commit, von dem er abgezweigt wurde.
    pub basis: String,
    /// Der Branch, auf dem das Projekt damals stand (Ziel des Pull Requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
}

/// Ein Pull Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrVerweis {
    pub nummer: u64,
    pub url: String,
}

/// Woran ein Arbeiter gerade ist, für das Schild über seinem Kopf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArbeiterAufgabe {
    pub name: String,
    pub zusammenfassung: String,
}

/// Wer zuletzt in sein Terminal getippt hat, und wann (ms).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LetzteEingabe {
    pub von: String,
    pub um: u64,
}

/// Verbrauch einer Sitzung, wenn der Provider ihn meldet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Verbrauch {
    pub tokens_ein: u64,
    pub tokens_aus: u64,
    /// Kosten in US-Dollar, wenn bekannt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kosten: Option<f64>,
}

/// Alles, was der Browser über einen Arbeiter weiß.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbeiterInfo {
    pub id: String,
    pub etage: String,
    pub art: ArbeiterArt,
    /// Provider-ID aus dem Register (`claude`, `codex`, ...), bei Agenten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modell: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aufwand: Option<String>,
    pub tisch: String,
    pub name: String,
    /// Farbe als `#rrggbb`.
    pub farbe: String,
    pub status: ArbeiterStatus,
    /// Jemand hat das Terminal seit dem letzten Fertig/Wartet geöffnet.
    pub gesehen: bool,
    /// Seit wann (ms) er auf einen Menschen wartet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wartet_seit: Option<u64>,
    pub angelegt_von: String,
    pub angelegt_um: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<Worktree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PrVerweis>,
    /// Ein PR wird gerade geöffnet.
    #[serde(default)]
    pub pr_oeffnet: bool,
    /// Terminal-Titel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sitzung: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub spalten: u16,
    pub zeilen: u16,
    /// Namen der Menschen, die gerade ins Terminal schauen.
    #[serde(default)]
    pub zuschauer: Vec<String>,
    /// Letzte sinnvolle Tätigkeit (Prompt oder Werkzeug).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aktivitaet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aktion: Option<Aktion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aufgabe: Option<ArbeiterAufgabe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbrauch: Option<Verbrauch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letzte_eingabe: Option<LetzteEingabe>,
    /// Ein Board-Agent: an welchem Board (`issues`, `pulls`, `aufgaben`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    /// Die Besprechung, zu der er am Besprechungstisch sitzt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub besprechung: Option<String>,
    /// Wie lange er schon gearbeitet hat (ms, abgeschlossene Strecken) und seit wann die laufende geht.
    #[serde(default)]
    pub gearbeitet_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arbeitet_seit: Option<u64>,
    /// Das Konto, das ihn angelegt hat (seine Anmeldungen bei Claude/GitHub nutzt er).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub konto: Option<String>,
    /// Hinweis, warum er hängt (Login, Vertrauensabfrage ...), auf Deutsch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hinweis: Option<String>,
}

/// Was beim Heimschicken mit dem Worktree passiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aufraeumen {
    /// Worktree und Branch bleiben.
    #[default]
    Behalten,
    /// Nur der Worktree-Ordner geht.
    Worktree,
    /// Worktree und Branch gehen.
    Alles,
}

/// Was der Worktree eines Arbeiters enthält, damit man weiß, was Löschen kosten würde.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeStand {
    pub vorhanden: bool,
    /// Dateien mit ungespeicherten Änderungen.
    pub geaendert: u32,
    /// Commits seit dem Abzweigen.
    pub voraus: u32,
    /// Commits, die nirgends sonst sind.
    pub ungepusht: u32,
    /// Geänderte Dateien (höchstens 50), für den Status-Feed.
    #[serde(default)]
    pub dateien: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
}

/// Eine Datei, die ein Arbeiter gegenüber dem Abzweig seines Branches geändert hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeaenderteDatei {
    pub pfad: String,
    /// Der alte Pfad, wenn die Datei umbenannt wurde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    /// `M` geändert, `A` neu, `D` gelöscht, `R` umbenannt, `T` Typ geändert, `?` neu und nie committet.
    pub status: String,
    pub plus: u32,
    pub minus: u32,
    pub binaer: bool,
    /// Noch nicht committet.
    pub offen: bool,
}

/// Was ein Arbeiter geändert hat (Änderungen-Fenster).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aenderungen {
    pub arbeiter: String,
    /// Aktueller Branch (`HEAD`, wenn losgelöst).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Wogegen verglichen wird (Abzweig-Commit oder `HEAD`).
    pub basis: String,
    /// Commits seit dem Abzweig.
    pub voraus: u32,
    /// Betreff des neuesten Commits, wenn `voraus > 0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub betreff: Option<String>,
    pub dateien: Vec<GeaenderteDatei>,
    /// So viele Dateien wurden weggelassen (mehr als 300).
    pub weitere: u32,
    /// Ziel-Branch eines Pull Requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_ziel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
}

/// Ein Provider, wie ihn das Anlege-Fenster zeigt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    /// Das Programm ist auf diesem Rechner gefunden worden.
    pub verfuegbar: bool,
    /// Modelle zur Auswahl (leer: freie Eingabe oder keine).
    #[serde(default)]
    pub modelle: Vec<String>,
    /// Nimmt einen Denk-Aufwand (`low` ... `max`).
    pub aufwand: bool,
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Einen Arbeiter an einen Schreibtisch setzen.
    Anlegen {
        tisch: String,
        #[serde(default)]
        prompt: Option<String>,
        /// Eigener Git-Worktree (Standard: ja, wenn das Projekt ein Git-Repo ist).
        #[serde(default)]
        worktree: Option<bool>,
        #[serde(default)]
        art: ArbeiterArt,
        #[serde(default)]
        provider: Option<String>,
        #[serde(default)]
        modell: Option<String>,
        #[serde(default)]
        aufwand: Option<String>,
        /// Für dieses GitHub-Issue.
        #[serde(default)]
        issue: Option<u64>,
    },
    Fortsetzen { arbeiter: String },
    /// Heimschicken.
    Stoppen {
        arbeiter: String,
        #[serde(default)]
        aufraeumen: Aufraeumen,
    },
    /// Fragt nach dem Stand des Worktrees; Antwort `Server::WorktreeStand`.
    WorktreeAbfragen { arbeiter: String },
    /// Terminal öffnen: Antwort ist ein Snapshot, danach laufende Daten.
    Anschauen { arbeiter: String },
    Wegschauen { arbeiter: String },
    /// Einen Auftrag in seine Sitzung tippen.
    Prompt { arbeiter: String, text: String },
    /// Branch pushen und Pull Request öffnen.
    PrOeffnen { arbeiter: String },
    /// Tastatureingaben ins Terminal.
    Eingabe { arbeiter: String, daten: String },
    Groesse { arbeiter: String, spalten: u16, zeilen: u16 },
    /// Liste der Provider anfordern; Antwort `Server::Provider`.
    ProviderAbfragen,
    /// Eine Bitte an den Board-Agenten am Pult `tisch` (`station-issues`, `station-pulls`,
    /// `station-queue`): getippt, geweckt oder – ist keiner da – dafür angelegt.
    StationAuftrag { tisch: String, text: String },
    /// Arbeiter, deren Pull Request gemergt ist, gehen von selbst heim (für das ganze Office).
    NachMergeGehen { an: bool },
    /// Was hat er geändert? Antwort `Server::Aenderungen`.
    AenderungenAbfragen { arbeiter: String },
    /// Diff einer Datei; Antwort `Server::Diff`.
    DiffAbfragen { arbeiter: String, datei: String },
    /// Alles committen (`git add -A`).
    Committen { arbeiter: String, nachricht: String },
    /// Nicht committete Änderungen verwerfen: eine Datei oder alle (nur im eigenen Worktree).
    Verwerfen {
        arbeiter: String,
        #[serde(default)]
        datei: Option<String>,
    },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Alle Arbeiter der Etage (beim Betreten).
    Liste { arbeiter: Vec<ArbeiterInfo> },
    /// Ein Arbeiter ist neu oder hat sich geändert.
    Aktualisiert { arbeiter: Box<ArbeiterInfo> },
    Entfernt { arbeiter: String },
    WorktreeStand { arbeiter: String, stand: WorktreeStand },
    /// Der ganze Bildschirm (mit Escape-Sequenzen), für ein frisch geöffnetes Terminal.
    TerminalSnapshot { arbeiter: String, daten: String, spalten: u16, zeilen: u16 },
    /// Neue Ausgabe; geht nur an die Zuschauer.
    TerminalDaten { arbeiter: String, daten: String },
    Provider { provider: Vec<ProviderInfo>, standard: String },
    Aenderungen { stand: Aenderungen },
    Diff {
        arbeiter: String,
        datei: String,
        diff: String,
        /// Zu groß, abgeschnitten.
        gekuerzt: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fehler: Option<String>,
    },
    /// Einstellungen der Arbeiter (beim Betreten und nach jeder Änderung).
    Einstellungen {
        nach_merge_gehen: bool,
        /// Wer es zuletzt umgestellt hat.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        von: Option<String>,
    },
}
