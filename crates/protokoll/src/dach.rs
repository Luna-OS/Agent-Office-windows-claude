//! Die Dachterrasse: YouTube-Leinwand (URL setzen, Play/Pause, Zeit synchron).

use serde::{Deserialize, Serialize};

/// Nachrichten vom Browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Eine YouTube-URL (oder nackte Video-ID) auf die Leinwand legen. Startzeit aus `t=`/`start=` wird übernommen.
    VideoSetzen { url: String },
    /// Abspielen ab `position_s` (oder ab der aktuellen Stelle, wenn `None`).
    Abspielen {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position_s: Option<f64>,
    },
    /// Anhalten bei `position_s` (oder an der aktuellen Stelle).
    Pausieren {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position_s: Option<f64>,
    },
    /// Zu einer Stelle springen, ohne Play/Pause zu ändern.
    Springen { position_s: f64 },
    /// Die Leinwand leeren.
    Leeren,
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Der ganze Zustand der Leinwand (nach jeder Änderung und beim Betreten des Dachs).
    Leinwand { zustand: LeinwandZustand },
}

/// Was auf der Leinwand läuft.
///
/// Die aktuelle Stelle ist `position_s + (jetzt_ms - stand_ms) / 1000`, solange `laeuft`;
/// sonst genau `position_s`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LeinwandZustand {
    /// Die YouTube-Video-ID (11 Zeichen), oder `None` für eine leere Leinwand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_id: Option<String>,
    pub laeuft: bool,
    /// Stelle im Video in Sekunden zum Zeitpunkt `stand_ms`.
    pub position_s: f64,
    /// Server-Uhr (Unix-ms), zu der `position_s` galt.
    pub stand_ms: u64,
    /// Wer zuletzt etwas geändert hat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
}

impl LeinwandZustand {
    /// Die Stelle im Video zum Zeitpunkt `jetzt_ms` (Server-Uhr).
    pub fn position_um(&self, jetzt_ms: u64) -> f64 {
        if self.laeuft {
            self.position_s + jetzt_ms.saturating_sub(self.stand_ms) as f64 / 1000.0
        } else {
            self.position_s
        }
    }
}
