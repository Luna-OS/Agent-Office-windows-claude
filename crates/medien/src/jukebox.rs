//! Die Jukebox in der Lounge jeder Etage: welche Stücke sie hat und was sie spielt.
//!
//! Der Server sagt nur, was läuft und seit wann; jeder Browser spielt selbst ab derselben Stelle
//! (die eingebauten Stücke werden prozedural erzeugt, siehe `client::klang::musik`).

use protokoll::medien::JukeboxZustand;
use serde::{Deserialize, Serialize};

use crate::leinwand::kuerzen;

/// Ein eingebautes Stück.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stueck {
    pub id: &'static str,
    pub titel: &'static str,
    /// Ein paar Worte auf der Karte in der Liste.
    pub stimmung: &'static str,
}

pub const STUECKE: [Stueck; 4] = [
    Stueck { id: "rainy-window", titel: "Regen am Fenster", stimmung: "langsam und verträumt" },
    Stueck { id: "coffee-break", titel: "Kaffeepause", stimmung: "jazzig, lockerer Swing" },
    Stueck { id: "late-commit", titel: "Später Commit", stimmung: "Moll, zwei Uhr nachts" },
    Stueck { id: "green-build", titel: "Grüner Build", stimmung: "hell und hüpfend" },
];

/// Das `stueck` eines eingefügten Streams.
pub const STREAM: &str = "stream";

pub fn stueck(id: &str) -> Option<&'static Stueck> {
    STUECKE.iter().find(|s| s.id == id)
}

/// Was gerade läuft, für Hinweise: ein Titel oder woher der Stream kommt.
pub fn titel(stueck_id: &str, url: Option<&str>) -> String {
    if stueck_id != STREAM {
        return stueck(stueck_id).map_or("Ein Stück".into(), |s| s.titel.into());
    }
    let Some(url) = url else { return "Ein Stream".into() };
    let ohne = url.split_once("://").map_or(url, |(_, r)| r);
    let ohne = ohne.split(['?', '#']).next().unwrap_or("");
    let mut teile = ohne.split('/');
    let wirt = teile.next().unwrap_or("");
    match teile.rfind(|t| !t.is_empty()) {
        Some(datei) => format!("{wirt} · {datei}"),
        None if wirt.is_empty() => "Ein Stream".into(),
        None => wirt.into(),
    }
}

/// Prüft eine Stream-URL (nur http/https, nicht zu lang).
pub fn stream_url_pruefen(roh: &str) -> Result<String, String> {
    let s = roh.trim();
    if s.is_empty() {
        return Err("Bitte einen Link zu einem Stream oder einer Audiodatei einfügen".into());
    }
    if s.len() > 2048 {
        return Err("Der Link ist zu lang".into());
    }
    let Some((schema, rest)) = s.split_once("://") else {
        return Err("Das ist kein Web-Link. Bitte eine Adresse mit https:// einfügen".into());
    };
    if !schema.eq_ignore_ascii_case("https") && !schema.eq_ignore_ascii_case("http") {
        return Err("Auf der Jukebox laufen nur http- und https-Links".into());
    }
    if rest.is_empty() || rest.starts_with('/') || s.chars().any(char::is_whitespace) {
        return Err("Das ist kein Web-Link. Bitte eine Adresse mit https:// einfügen".into());
    }
    Ok(s.to_string())
}

/// Was gespeichert wird (`jukebox.json` im Datenordner der Etage).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gespeichert {
    pub an: bool,
    pub stueck: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub von: Option<String>,
    pub start_ms: u64,
}

/// Die Jukebox einer Etage.
#[derive(Debug, Clone)]
pub struct Jukebox {
    s: Gespeichert,
}

impl Jukebox {
    pub fn neu(jetzt_ms: u64) -> Self {
        Self { s: Gespeichert { an: false, stueck: STUECKE[0].id.into(), url: None, von: None, start_ms: jetzt_ms } }
    }

    /// Aus dem gespeicherten JSON; Kaputtes heißt einfach: eine stille Lounge.
    pub fn laden(json: &str, jetzt_ms: u64) -> Self {
        let Ok(mut s) = serde_json::from_str::<Gespeichert>(json) else { return Self::neu(jetzt_ms) };
        if s.stueck == STREAM {
            match s.url.as_deref().map(stream_url_pruefen) {
                Some(Ok(u)) => s.url = Some(u),
                _ => return Self::neu(jetzt_ms),
            }
        } else if stueck(&s.stueck).is_none() {
            return Self::neu(jetzt_ms);
        } else {
            s.url = None;
        }
        s.von = s.von.map(|v| kuerzen(&v, 24));
        Self { s }
    }

    /// Zum Speichern.
    pub fn als_json(&self) -> String {
        serde_json::to_string_pretty(&self.s).unwrap_or_default()
    }

    pub fn zustand(&self, jetzt_ms: u64) -> JukeboxZustand {
        let s = &self.s;
        JukeboxZustand {
            an: s.an,
            stueck: s.stueck.clone(),
            url: if s.stueck == STREAM { s.url.clone() } else { None },
            von: s.von.clone(),
            start_ms: s.start_ms,
            vergangen_ms: jetzt_ms.saturating_sub(s.start_ms),
        }
    }

    pub fn titel(&self) -> String {
        titel(&self.s.stueck, self.s.url.as_deref())
    }

    /// Legt ein Stück oder einen Stream auf, oder schaltet (mit keinem von beiden) wieder an.
    /// `Ok(true)`, wenn sich etwas geändert hat.
    pub fn spielen(&mut self, stueck_id: Option<&str>, url: Option<&str>, wer: &str, jetzt_ms: u64) -> Result<bool, String> {
        if let Some(u) = url.filter(|u| !u.is_empty()) {
            let u = stream_url_pruefen(u)?;
            self.setzen(STREAM.into(), Some(u), wer, jetzt_ms);
        } else if let Some(id) = stueck_id {
            if stueck(id).is_none() {
                return Err("Das hat die Jukebox nicht".into());
            }
            self.setzen(id.into(), None, wer, jetzt_ms);
        } else {
            if self.s.an {
                return Ok(false);
            }
            let (stueck, url) = (self.s.stueck.clone(), self.s.url.clone());
            self.setzen(stueck, url, wer, jetzt_ms);
        }
        Ok(true)
    }

    /// Weiter zum nächsten Stück; von einem Stream zurück zum ersten.
    pub fn weiter(&mut self, wer: &str, jetzt_ms: u64) {
        let i = STUECKE.iter().position(|s| s.id == self.s.stueck).map_or(0, |i| i + 1);
        self.setzen(STUECKE[i % STUECKE.len()].id.into(), None, wer, jetzt_ms);
    }

    /// Aus. `false`, wenn sie schon aus war.
    pub fn stopp(&mut self, wer: &str) -> bool {
        if !self.s.an {
            return false;
        }
        self.s.an = false;
        self.s.von = Some(kuerzen(wer, 24));
        true
    }

    fn setzen(&mut self, stueck: String, url: Option<String>, wer: &str, jetzt_ms: u64) {
        self.s = Gespeichert { an: true, stueck, url, von: Some(kuerzen(wer, 24)), start_ms: jetzt_ms };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spielen_weiter_stopp() {
        let mut j = Jukebox::neu(0);
        assert!(!j.zustand(0).an);
        assert!(j.spielen(None, None, "Luna", 100).unwrap());
        assert!(!j.spielen(None, None, "Luna", 200).unwrap());
        assert_eq!(j.zustand(1100).vergangen_ms, 1000);
        j.weiter("Vega", 2000);
        assert_eq!(j.zustand(2000).stueck, "coffee-break");
        assert!(j.spielen(Some("green-build"), None, "x", 3000).unwrap());
        j.weiter("x", 4000);
        assert_eq!(j.zustand(4000).stueck, "rainy-window");
        assert!(j.stopp("x"));
        assert!(!j.stopp("x"));
        assert!(j.spielen(Some("gibts-nicht"), None, "x", 0).is_err());
    }

    #[test]
    fn streams() {
        let mut j = Jukebox::neu(0);
        assert!(j.spielen(None, Some("ftp://radio.example/x"), "a", 0).is_err());
        assert!(j.spielen(None, Some("radio.example"), "a", 0).is_err());
        j.spielen(None, Some("https://radio.example.com/live/stream.mp3?x=1"), "a", 0).unwrap();
        let z = j.zustand(0);
        assert_eq!(z.stueck, STREAM);
        assert_eq!(j.titel(), "radio.example.com · stream.mp3");
        j.weiter("a", 0);
        assert_eq!(j.zustand(0).stueck, "rainy-window");
        assert_eq!(j.zustand(0).url, None);
    }

    #[test]
    fn speichern_und_laden() {
        let mut j = Jukebox::neu(0);
        j.spielen(None, Some("https://radio.example.com/"), "Ein sehr sehr sehr langer Name hier", 500).unwrap();
        let k = Jukebox::laden(&j.als_json(), 9_000);
        assert_eq!(k.zustand(1500), j.zustand(1500));
        assert_eq!(k.zustand(0).von.unwrap().chars().count(), 24);
        assert!(!Jukebox::laden("kaputt", 1).zustand(1).an);
        assert!(!Jukebox::laden(r#"{"an":true,"stueck":"weg","start_ms":0}"#, 1).zustand(1).an);
    }
}
