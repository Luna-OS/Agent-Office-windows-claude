//! Kommunikation zwischen Agenten: Nachrichten, wer gerade was macht (Status-Feed).
//!
//! Die Agenten einer Etage schreiben sich über den Nachrichten-Bus (direkt oder an alle) und melden
//! ihren Stand in den Status-Feed. Die Menschen sehen beides und können selbst mitschreiben.

use serde::{Deserialize, Serialize};

use crate::arbeiter::{ArbeiterStatus, Verbrauch, Worktree};

/// Eine Nachricht auf dem Bus einer Etage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusNachricht {
    /// Fortlaufend je Etage.
    pub id: u64,
    /// Name des Absenders (Agent oder Mensch).
    pub von: String,
    /// Arbeiter-ID des Absenders, wenn es ein Agent war.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von_arbeiter: Option<String>,
    /// Name des Empfängers; `None` heißt an alle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub an: Option<String>,
    pub text: String,
    /// Zeitpunkt in ms seit 1970.
    pub um: u64,
}

/// Eine Zeile im Status-Feed: wer macht was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedEintrag {
    pub arbeiter: String,
    pub name: String,
    pub status: ArbeiterStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Was er selbst als seinen Stand gemeldet hat (`office-agenten status ...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notiz: Option<String>,
    /// Letzte Tätigkeit laut Hooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aktivitaet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Geänderte Dateien in seinem Worktree.
    #[serde(default)]
    pub dateien: Vec<String>,
    pub um: u64,
}

/// Verbrauch des ganzen Office (auch heimgeschickter Arbeiter), auf der Platte gemerkt.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Nutzung {
    pub gesamt: Verbrauch,
    /// Seit Mitternacht (UTC).
    pub heute: Verbrauch,
    /// Der Tag von `heute`, `JJJJ-MM-TT`.
    pub tag: String,
}

/// Ein Nutzungsfenster des Claude-Plans: die 5-Stunden-Sitzung, die Woche, die Woche eines Modells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitFenster {
    /// z. B. „5-Stunden-Sitzung“, „Woche“, „Opus-Woche“.
    pub name: String,
    /// Verbraucht in Prozent, 0–100.
    pub prozent: f64,
    /// Wann es neu beginnt (ms seit 1970).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neu_um: Option<u64>,
}

/// Die Limits des Claude-Plans, auf dem die Arbeiter laufen.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// `pro`, `max`, `team` …, wenn bekannt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Leer vor dem ersten Lesen oder ohne Plan (API-Schlüssel).
    pub fenster: Vec<LimitFenster>,
    /// Wann gelesen (ms); 0 vor dem ersten Lesen.
    pub um: u64,
}

/// Ein Text, den das Office selbst an Agenten schickt, im Prompt-Editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptInfo {
    pub id: String,
    pub titel: String,
    /// Wofür das Office ihn nimmt.
    pub verwendet: String,
    /// Platzhalter, die das Office füllt (`{{name}}`).
    pub platzhalter: Vec<String>,
    pub standard: String,
    /// Der umgeschriebene Text, wenn es einen gibt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigener: Option<String>,
}

/// Ein Server, den ein Arbeiter gestartet hat (z. B. `npm run dev`), und auf welchem Port er lauscht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dienst {
    pub port: u16,
    pub arbeiter: String,
    /// Name des Arbeiters.
    pub name: String,
    /// Das lauschende Programm (`node`, `python` …).
    pub programm: String,
    /// Seit wann er lauscht (ms).
    pub seit: u64,
}

/// Wie die Arbeiter am Besprechungstisch zusammenarbeiten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Muster {
    /// Jeder schlägt vor, dann kritisieren sie einander; zuletzt schreibt der Vorsitz die Entscheidung.
    Debatte,
    /// Die Leitung teilt die Aufgabe auf, das Team erledigt je einen Teil, die Leitung führt zusammen.
    Leitung,
    /// Dieselbe Aufgabe über viele Teile parallel; der Kopf des Tisches fasst zusammen.
    MapReduce,
    /// Rot greift an, Blau behebt, Runde um Runde; Blau schreibt es auf.
    RotBlau,
    /// Prüfer lesen einen Pull Request je aus ihrem Blickwinkel; das Ergebnis kommt als Kommentar an den PR.
    Review,
}

/// Ein Platz am Besprechungstisch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platz {
    /// Die Rolle, z. B. „Skeptiker“ oder „Rotes Team“.
    pub rolle: String,
    /// Der Stuhl (Sitz-ID der Welt).
    pub tisch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arbeiter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Verbrauchte Tokens (bleibt, wenn er heimgeht).
    #[serde(default)]
    pub tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZugZustand {
    /// Noch nicht übergeben.
    Wartet,
    /// Übergeben, noch nicht begonnen.
    Gesendet,
    Arbeitet,
    /// Seine Datei ist geschrieben.
    Fertig,
}

/// Der Teil eines Arbeiters in einem Schritt einer Runde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zug {
    /// Index in `Besprechung::plaetze`.
    pub platz: usize,
    /// z. B. „schlägt vor“, „kritisiert“, „schreibt die Entscheidung“.
    pub tut: String,
    /// Die Datei, die er schreibt (relativ zum Arbeitsordner der Besprechung).
    pub datei: String,
    pub zustand: ZugZustand,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gesendet_um: Option<u64>,
    /// Wurde schon einmal erinnert.
    #[serde(default)]
    pub erinnert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BesprechungStatus {
    Laeuft,
    Fertig,
    Gestoppt,
}

/// Eine Besprechung: 2–5 Arbeiter an einer Frage oder Aufgabe, in Runden nach einem Muster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Besprechung {
    pub id: String,
    pub muster: Muster,
    pub titel: String,
    pub auftrag: String,
    /// Die Datei, die die Besprechung schreibt (relativ zu ihrem Worktree).
    pub ausgabe: String,
    /// Der Kopf des Tisches zuerst.
    pub plaetze: Vec<Platz>,
    #[serde(default)]
    pub teile: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modell: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aufwand: Option<String>,
    /// Höchstzahl der Runden.
    pub runden: u32,
    /// Aktuelle Runde (ab 1) und Schritt darin.
    pub runde: u32,
    pub schritt: u32,
    pub zuege: Vec<Zug>,
    /// Tokens für alle zusammen, und wie viele verbraucht sind.
    pub budget: u64,
    pub tokens: u64,
    pub status: BesprechungStatus,
    /// Warum sie vorzeitig endete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grund: Option<String>,
    pub einberufen_von: String,
    pub begonnen_um: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beendet_um: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<Worktree>,
    /// Ordner der Rundennotizen (relativ zum Worktree).
    pub notizen: String,
    /// Commit mit der Ausgabe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Review: Link zum Kommentar am PR, oder warum es nicht ging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_fehler: Option<String>,
    /// Der Anfang der Ausgabedatei, für die Tafel im Raum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vorschau: Option<String>,
    /// Die Arbeiter sind heim, der Worktree ist weg.
    #[serde(default)]
    pub abgeraeumt: bool,
}

/// Eine vergangene Besprechung in einer Zeile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BesprechungEintrag {
    pub id: String,
    pub muster: Muster,
    pub titel: String,
    pub status: BesprechungStatus,
    pub zusammenfassung: String,
    pub einberufen_von: String,
    pub beendet_um: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub ausgabe: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BesprechungStand {
    /// Die laufende oder letzte Besprechung (bis abgeräumt oder eine neue beginnt).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aktuell: Option<Box<Besprechung>>,
    /// Frühere, neueste zuerst.
    #[serde(default)]
    pub frueher: Vec<BesprechungEintrag>,
}

/// Was beim Einberufen gewünscht wird.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BesprechungWunsch {
    pub muster: Muster,
    pub auftrag: String,
    #[serde(default)]
    pub titel: Option<String>,
    /// Ausgabedatei; ohne: die des Musters.
    #[serde(default)]
    pub ausgabe: Option<String>,
    /// Eine Rolle je Arbeiter, der Kopf des Tisches zuerst; leer: die des Musters.
    #[serde(default)]
    pub rollen: Vec<String>,
    #[serde(default)]
    pub teile: Vec<String>,
    #[serde(default)]
    pub pr: Option<u64>,
    #[serde(default)]
    pub issue: Option<u64>,
    #[serde(default)]
    pub runden: Option<u32>,
    #[serde(default)]
    pub budget: Option<u64>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub modell: Option<String>,
    #[serde(default)]
    pub aufwand: Option<String>,
}

/// Ein heimgeschickter Arbeiter im Kerker (auf Karten, die Arbeiter einsperren).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gefangener {
    pub id: String,
    pub name: String,
    pub farbe: String,
    /// Wann eingesperrt (ms).
    pub um: u64,
    /// Wie lange er gearbeitet hatte (ms), für wie verbraucht er aussieht.
    #[serde(default)]
    pub gearbeitet_ms: u64,
}

/// Der Kerker einer Etage: alle Eingesperrten, erste zuerst, und wie viele davor nur noch Knochen sind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kerker {
    pub gefangene: Vec<Gefangener>,
    pub knochen: u32,
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Ein Mensch schreibt auf den Bus; `an: None` an alle Agenten der Etage.
    Senden {
        #[serde(default)]
        an: Option<String>,
        text: String,
    },
    /// Verlauf und Feed erneut anfordern.
    Abfragen,
    /// Die Plan-Limits jetzt neu lesen.
    LimitsAuffrischen,
    /// Eine Besprechung einberufen: Arbeiter setzen sich an den Besprechungstisch.
    BesprechungStarten { wunsch: Box<BesprechungWunsch> },
    /// Die laufende Besprechung anhalten; die Arbeiter bleiben am Tisch.
    BesprechungStoppen,
    /// Die Arbeiter der letzten Besprechung heimschicken und den Tisch abräumen.
    BesprechungAbraeumen,
    /// Einen Prompt umschreiben; `text: None` (oder leer) stellt den Standard wieder her.
    PromptSetzen {
        id: String,
        #[serde(default)]
        text: Option<String>,
    },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Die letzten Nachrichten und der ganze Feed (beim Betreten der Etage).
    Stand { nachrichten: Vec<BusNachricht>, feed: Vec<FeedEintrag> },
    /// Eine neue Nachricht auf dem Bus.
    Nachricht { nachricht: BusNachricht },
    /// Eine Zeile im Feed hat sich geändert.
    Feed { eintrag: FeedEintrag },
    /// Ein Arbeiter ist aus dem Feed verschwunden.
    FeedEntfernt { arbeiter: String },
    Nutzung { stand: Nutzung },
    Limits { stand: Limits },
    /// Die Dienste der Etage (beim Betreten und bei jeder Änderung).
    Dienste { liste: Vec<Dienst> },
    /// Der Besprechungsraum der Etage (beim Betreten und bei jeder Änderung).
    Besprechung { stand: BesprechungStand },
    /// Der Kerker der Etage (beim Betreten und wenn jemand eingesperrt wird).
    Kerker { stand: Kerker },
    /// Alle Prompts (beim Verbinden und nach jeder Änderung).
    Prompts { liste: Vec<PromptInfo> },
}
