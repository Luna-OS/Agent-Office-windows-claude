//! Kommunikation zwischen Agenten: Nachrichten, wer gerade was macht (Status-Feed).
//!
//! Die Agenten einer Etage schreiben sich über den Nachrichten-Bus (direkt oder an alle) und melden
//! ihren Stand in den Status-Feed. Die Menschen sehen beides und können selbst mitschreiben.

use serde::{Deserialize, Serialize};

use crate::arbeiter::{ArbeiterStatus, Verbrauch};

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
    /// Alle Prompts (beim Verbinden und nach jeder Änderung).
    Prompts { liste: Vec<PromptInfo> },
}
