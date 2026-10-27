//! ⚙️ Einstellungen und die Dienste des Gebäudes: Benachrichtigungen, der Rechner, Updates,
//! der Himmel, Feiertage, Karten. (Prompts, Dienste der Arbeiter und "nach Merge gehen" sind in `agenten`.)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookArt {
    Slack,
    Discord,
    Andere,
}

/// Der Slack-/Discord-Webhook des Office. Nie die URL selbst, nur wohin er geht.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Benachrichtigung {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<WebhookInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zuletzt_gesendet: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookInfo {
    pub art: WebhookArt,
    /// z. B. "hooks.slack.com/…/abcd".
    pub hinweis: String,
    pub von: String,
    pub um: u64,
}

/// Der Rechner des Office: Auslastung und das Arbeiter-Limit über alle Etagen.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Maschine {
    /// Prozent über alle Kerne, 0–100.
    pub cpu: f32,
    pub kerne: u32,
    /// Bytes.
    pub speicher_belegt: u64,
    pub speicher_gesamt: u64,
    /// Die letzten Minuten, älteste zuerst: [cpu %, speicher %].
    pub verlauf: Vec<[f32; 2]>,
    /// Was einen weiteren Arbeiter gerade schwer macht, z. B. "Speicher zu 93 % belegt".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub druck: Option<String>,
    /// Arbeiter im Office über alle Etagen.
    pub arbeiter: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// --max-arbeiter: höher geht es aus dem Office nicht.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obergrenze: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub sha: String,
    pub betreff: String,
    /// ISO-Datum des Commits.
    pub datum: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    #[default]
    Ruhe,
    Baut,
    Startet,
    Fehlgeschlagen,
}

/// Selbst-Update eines aus git installierten Office.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Update {
    pub verfuegbar: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aktuell: Option<Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neueste: Option<Version>,
    /// Neue Commits, neueste zuerst (höchstens 15).
    #[serde(default)]
    pub aenderungen: Vec<Version>,
    #[serde(default)]
    pub prueft: bool,
    pub phase: UpdatePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wetter {
    Klar,
    Bewoelkt,
    Regen,
    Gewitter,
    Schnee,
    Nebel,
}

/// Wie es draußen aussieht; der Server entscheidet, damit alle denselben Himmel sehen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Himmel {
    pub breite: f32,
    pub laenge: f32,
    /// Die Uhr des Office in Minuten östlich von UTC.
    pub utc_versatz: i32,
    pub wetter: Wetter,
    /// 0–1.
    pub staerke: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stadt: Option<String>,
    /// °C.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperatur: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feiertag {
    Halloween,
    Weihnachten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemaWahl {
    Halloween,
    Weihnachten,
    #[default]
    Automatisch,
    Aus,
}

/// Die Feiertags-Deko des Gebäudes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Thema {
    pub wahl: ThemaWahl,
    /// Was gerade hängt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aktiv: Option<Feiertag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub um: Option<u64>,
}

/// Eine eigene Karte aus dem Ordner karten/ des Datenordners.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EigeneKarte {
    pub id: String,
    pub name: String,
    /// Die Konfiguration der Karte, so wie sie im Ordner liegt (Format bestimmt `welt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daten: Option<serde_json::Value>,
    /// Warum sie nicht lädt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
}

/// Wie das Gebäude innen aussieht: "office", "burg" oder eine eigene Karte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Karte {
    pub wahl: String,
    #[serde(default)]
    pub eigene: Vec<EigeneKarte>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub um: Option<u64>,
}

impl Default for Karte {
    fn default() -> Self {
        Self { wahl: "office".into(), eigene: Vec::new(), von: None, um: None }
    }
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Webhook setzen; "" entfernt ihn.
    Webhook { url: String },
    /// Testnachricht über den Webhook; das Ergebnis kommt als Hinweis.
    WebhookTesten,
    /// Admins: höchstens so viele Arbeiter; `None` hebt das Limit auf.
    MaschinenLimit { limit: Option<u32> },
    UpdatePruefen,
    UpdateStarten,
    ThemaSetzen { wahl: ThemaWahl },
    /// Karte wechseln; ohne `karte` den Ordner der eigenen Karten neu lesen.
    KarteSetzen {
        #[serde(default)]
        karte: Option<String>,
    },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Alles auf einmal, direkt nach `praesenz::Server::Willkommen`.
    Stand {
        benachrichtigung: Benachrichtigung,
        maschine: Maschine,
        update: Update,
        himmel: Himmel,
        thema: Thema,
        karte: Karte,
    },
    Benachrichtigung { stand: Benachrichtigung },
    Maschine { stand: Maschine },
    Update { stand: Update },
    Himmel { stand: Himmel },
    Thema { stand: Thema },
    Karte { stand: Karte },
}
