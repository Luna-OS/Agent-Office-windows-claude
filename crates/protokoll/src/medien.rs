//! Bildschirm teilen über HTTP, Sprache (Voice), Jukebox.
//!
//! Die Bilder eines geteilten Bildschirms laufen nicht über den WebSocket, sondern über HTTP
//! (`/medien/bildschirm/...`, siehe Crate `medien`). Hier steht nur, wer gerade teilt.

use serde::{Deserialize, Serialize};

/// Nachrichten vom Browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Eigener Voice-Zustand (beigetreten, stumm), für Anzeige und Mundbewegung bei den anderen.
    Sprache { aktiv: bool, stumm: bool },
    /// WebRTC-Signal (Angebot, Antwort, ICE-Kandidat) an einen anderen Spieler weiterreichen.
    Rtc { an: String, daten: RtcSignal },
    /// Jukebox der Etage: ein eingebautes Stück (`stueck`), einen Stream (`url`) oder mit keinem von beiden wieder an.
    JukeboxSpielen {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stueck: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    /// Nächstes Stück.
    JukeboxWeiter,
    /// Jukebox aus.
    JukeboxStopp,
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Wer auf der Etage gerade seinen Bildschirm teilt (vollständige Liste, ersetzt die alte).
    Bildschirme { liste: Vec<Bildschirm> },
    /// ICE-Server (STUN/TURN) für WebRTC, kommt einmal nach der Anmeldung.
    IceServer { server: Vec<IceServer> },
    /// Voice-Zustand eines anderen Spielers.
    Sprache {
        spieler: String,
        aktiv: bool,
        stumm: bool,
    },
    /// Ein WebRTC-Signal von einem anderen Spieler.
    Rtc { von: String, daten: RtcSignal },
    /// Was die Jukebox der Etage spielt.
    Jukebox { zustand: JukeboxZustand },
}

/// Ein geteilter Bildschirm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bildschirm {
    /// Spieler-ID dessen, der teilt.
    pub spieler: String,
    /// Anzeigename.
    pub name: String,
    /// Etage, auf der geteilt wird.
    pub etage: String,
    /// MJPEG-Stream zum Zuschauen (`multipart/x-mixed-replace`).
    pub stream_url: String,
    /// Das jeweils letzte Bild als JPEG (z. B. für eine 3D-Textur).
    pub bild_url: String,
    /// Seit wann geteilt wird (Unix-Zeit in ms).
    pub seit_ms: u64,
}

/// Ein STUN- oder TURN-Server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// Ein Signal der „perfect negotiation“: entweder eine Beschreibung (SDP) oder ein Kandidat.
/// `kandidat: None` zusammen mit `ende: true` heißt „keine weiteren Kandidaten“.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RtcSignal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beschreibung: Option<SdpBeschreibung>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kandidat: Option<IceKandidat>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ende: bool,
}

/// Eine SDP-Beschreibung (`typ` ist "offer", "answer", "pranswer" oder "rollback").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdpBeschreibung {
    pub typ: String,
    pub sdp: String,
}

/// Ein ICE-Kandidat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IceKandidat {
    pub candidate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdp_mid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdp_m_line_index: Option<u16>,
}

/// Was die Jukebox spielt. Jeder Browser spielt selbst, ab derselben Stelle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JukeboxZustand {
    pub an: bool,
    /// Ein eingebautes Stück oder `"stream"` für `url`. Bleibt stehen, während die Jukebox aus ist.
    pub stueck: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Wer zuletzt etwas aufgelegt oder ausgeschaltet hat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    /// Wann das Stück anfing (Server-Uhr, Unix-ms).
    pub start_ms: u64,
    /// Wie weit das Stück beim Senden war, in ms.
    pub vergangen_ms: u64,
}
