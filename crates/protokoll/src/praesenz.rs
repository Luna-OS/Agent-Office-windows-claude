//! Wer ist wo: Positionen, Avatare, Emotes, Chat. (Voice, WebRTC und ICE sind in `medien`.)
//!
//! Beim Verbinden schickt der Server `Willkommen`, danach `etagen::Server::Liste` und
//! `etagen::Server::Betreten` (mit allen Personen der Etage) und die Stände der anderen Domänen.

use serde::{Deserialize, Serialize};

/// Wie jemand aussieht: drei kleine Indizes in die Listen des Clients (Hautton, Haarfarbe, Frisur).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Aussehen {
    pub haut: u8,
    pub haare: u8,
    pub frisur: u8,
}

/// Anzahl der Haut-, Haarfarben und Frisuren, die der Client kennt (wie im Original).
pub const HAUTTOENE: u8 = 8;
pub const HAARFARBEN: u8 = 10;
pub const FRISUREN: u8 = 7;

impl Aussehen {
    /// Klemmt die Indizes in den gültigen Bereich.
    pub fn bereinigt(self) -> Self {
        Self {
            haut: self.haut % HAUTTOENE,
            haare: self.haare % HAARFARBEN,
            frisur: self.frisur % FRISUREN,
        }
    }
}

/// Ein Spiel an der Dachbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Barspiel {
    Darts,
    Axt,
}

/// Ein Getränk von der Dachbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Getraenk {
    Bier,
    Wein,
    Martini,
    MaiTai,
    Kurzer,
    Mojito,
    Wasser,
}

/// Eine kurze Reaktion der Figur, sichtbar für alle auf der Etage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emote {
    Winken,
    Daumen,
    Klatschen,
    Tanzen,
    Zeigen,
    Facepalm,
}

/// Eine Issue-Karte, die jemand vom Board durch die Etage trägt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetragenesIssue {
    pub issue: u64,
    pub titel: String,
}

/// Ort und Blickrichtung (Drehung um y; 0 schaut nach +z).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Ort {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_y: f32,
}

/// Ein Mensch im Office.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    /// Verbindungs-ID, eindeutig solange der Browser verbunden ist.
    pub id: String,
    pub name: String,
    /// Farbe als "#rrggbb".
    pub farbe: String,
    pub aussehen: Aussehen,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_y: f32,
    /// Läuft gerade (für die Lauf-Animation).
    pub laeuft: bool,
    /// Raucht auf dem Balkon.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub raucht: bool,
    /// Steht mit Schläger am Golf-Abschlag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub golft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wirft: Option<Barspiel>,
    /// Sitzplatz wie "couch:1", wenn er sitzt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sitz: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traegt: Option<GetragenesIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub getraenk: Option<Getraenk>,
    /// Mit eigenem Konto angemeldet: der Name gehört ihm.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub konto: bool,
    /// Die Etage, auf der er ist; keine, solange das Gebäude keine Etagen hat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etage: Option<String>,
    /// Was er offen hat, in Worten: "im Terminal von Pixel".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tut: Option<String>,
    /// Liest ein Buch aus dem Regal.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub liest: bool,
    /// In der 2D-Ansicht (Handy): im Office, aber nirgends stehend.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lite: bool,
}

/// Eine Zeile im Chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatZeile {
    /// Verbindungs-ID des Absenders.
    pub von: String,
    pub name: String,
    pub farbe: String,
    pub text: String,
    /// Zeitpunkt in ms seit 1970.
    pub um: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub konto: bool,
}

/// Wie wichtig ein Hinweis ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stufe {
    Info,
    Warnung,
    Fehler,
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    /// Eigene Position (mehrmals pro Sekunde, solange man sich bewegt).
    Bewegen { x: f32, y: f32, z: f32, rot_y: f32, laeuft: bool },
    /// Etwas in der Hand: Zigarette, Golfschläger, Getränk, Dart/Axt. Fehlende Felder bleiben, wie sie sind;
    /// bei `getraenk`/`wirft` heißt `null` "weggelegt" – darum `Option<Option<_>>`.
    Handlung {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rauchen: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        golfen: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "doppel_option")]
        getraenk: Option<Option<Getraenk>>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "doppel_option")]
        wirft: Option<Option<Barspiel>>,
    },
    /// Hinsetzen (`sitz` wie "couch:1") oder aufstehen (ohne).
    Sitzen {
        #[serde(default)]
        sitz: Option<String>,
    },
    /// Eine Issue-Karte aufnehmen oder ablegen (ohne).
    Tragen {
        #[serde(default)]
        karte: Option<GetragenesIssue>,
    },
    /// Ein Emote (begrenzt: 3 auf einmal, dann eins alle 2 s).
    Emote { emote: Emote },
    /// Name, Farbe und Aussehen von der Figurenwahl.
    Profil { name: String, farbe: String, aussehen: Aussehen },
    /// Was man gerade offen hat; ohne `was` ist man zurück im Office.
    Tut {
        #[serde(default)]
        was: Option<String>,
        #[serde(default)]
        liest: bool,
    },
    /// In der 2D-Ansicht (Handy) oder wieder in 3D.
    Lite { an: bool },
    Chat { text: String },
    /// `um` kommt im `Pong` zurück (Uhrzeit des Clients, für die Laufzeit).
    Ping { um: f64 },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Die erste Nachricht nach dem Verbinden.
    Willkommen {
        /// Die eigene Verbindungs-ID.
        du: String,
        /// So sieht man selbst aus (gespeichert oder neu gewürfelt).
        ich_selbst: Person,
        /// Die letzten Chat-Zeilen.
        chat: Vec<ChatZeile>,
        /// Version des laufenden Servers; ändert sie sich nach dem Neuverbinden, wurde das Office aktualisiert.
        version: String,
        /// Als wer dieser Browser angemeldet ist.
        ich: crate::konten::Ich,
        /// Ob man von hier aus Leute einladen kann.
        einladungen: bool,
    },
    /// Jemand kam auf die Etage (oder ins Office).
    PersonDa { person: Person },
    /// Profil, Sitz, Hand, Voice ... von jemandem haben sich geändert.
    PersonGeaendert { person: Person },
    PersonBewegt { id: String, x: f32, y: f32, z: f32, rot_y: f32, laeuft: bool },
    /// Jemand ging (von der Etage oder aus dem Office).
    PersonWeg { id: String },
    PersonEmote { id: String, emote: Emote },
    Chat { zeile: ChatZeile },
    /// Eine kurze Meldung am Bildschirmrand.
    Hinweis { text: String, stufe: Stufe },
    /// Der Platz ist schon besetzt (von `von`, einem Namen).
    SitzBelegt { sitz: String, von: String },
    /// `jetzt` ist die Uhr des Servers in ms, nach der z. B. die Jukebox den Takt hält.
    Pong { um: f64, jetzt: u64 },
}

/// Serde-Hilfe: unterscheidet "Feld fehlt" (`None`) von "Feld ist null" (`Some(None)`).
mod doppel_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer>(w: &Option<Option<T>>, s: S) -> Result<S::Ok, S::Error> {
        match w {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handlung_unterscheidet_null_und_fehlend() {
        let c: Client = serde_json::from_str(r#"{"typ":"handlung","getraenk":null}"#).unwrap();
        match c {
            Client::Handlung { getraenk, wirft, .. } => {
                assert_eq!(getraenk, Some(None));
                assert_eq!(wirft, None);
            }
            _ => panic!("falsche Variante"),
        }
    }

    #[test]
    fn bewegen_rundreise() {
        let json = serde_json::to_string(&Client::Bewegen { x: 1.0, y: 0.0, z: 2.0, rot_y: 0.5, laeuft: true }).unwrap();
        assert!(json.contains(r#""typ":"bewegen""#));
        let _: Client = serde_json::from_str(&json).unwrap();
    }
}
