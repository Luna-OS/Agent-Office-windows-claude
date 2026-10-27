//! Kommunikation zwischen Agenten: Nachrichten, wer gerade was macht (Status-Feed).
//!
//! Die Agenten einer Etage schreiben sich über den Nachrichten-Bus (direkt oder an alle) und melden
//! ihren Stand in den Status-Feed. Die Menschen sehen beides und können selbst mitschreiben.

use serde::{Deserialize, Serialize};

use crate::arbeiter::ArbeiterStatus;

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
}
