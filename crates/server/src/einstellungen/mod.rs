//! ⚙️ Einstellungen des Gebäudes (einstellungen.json): Thema, Karte, Webhook, Arbeiter-Limit; dazu Himmel und Rechner.

pub mod himmel;
pub mod maschine;
pub mod update;
pub mod vorhersage;

use std::path::PathBuf;
use std::sync::Mutex;

use protokoll::einstellungen::{Benachrichtigung, Karte, Maschine, Thema, ThemaWahl, Update, WebhookArt, WebhookInfo};
use serde::{Deserialize, Serialize};

use crate::konfig::Konfig;
use crate::speicher::{jetzt_ms, laden, speichern};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gespeichert {
    pub thema: Thema,
    pub karte: Karte,
    /// Die volle Webhook-URL (geht nie an Browser).
    pub webhook_url: Option<String>,
    pub benachrichtigung: Benachrichtigung,
    pub arbeiter_limit: Option<u32>,
    /// Wohin neue Etagen geklont werden (aus den Einstellungen gesetzt).
    pub projekte_ordner: Option<(String, String, u64)>,
    /// Die Erst-Einrichtung im Terminal ist durch (oder übersprungen).
    pub eingerichtet: bool,
}

pub struct Einstellungen {
    datei: PathBuf,
    pub d: Mutex<Gespeichert>,
    pub himmel: himmel::HimmelWetter,
    pub maschine: maschine::Rechner,
    pub update: Mutex<Update>,
    pub obergrenze: Option<u32>,
}

/// Die aktuelle Uhr des Office in Minuten östlich von UTC.
pub fn utc_versatz() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}

/// Welcher Feiertag laut Kalender gerade ist (Oktober: Halloween, Dezember: Weihnachten).
pub fn feiertag_nach_kalender() -> Option<protokoll::einstellungen::Feiertag> {
    use chrono::Datelike;
    use protokoll::einstellungen::Feiertag;
    match chrono::Local::now().month() {
        10 => Some(Feiertag::Halloween),
        12 => Some(Feiertag::Weihnachten),
        _ => None,
    }
}

impl Einstellungen {
    pub fn laden(k: &Konfig) -> Self {
        let datei = k.daten.join("einstellungen.json");
        let d: Gespeichert = laden(&datei);
        Self {
            datei,
            d: Mutex::new(d),
            himmel: himmel::HimmelWetter::neu(k.stadt.clone()),
            maschine: maschine::Rechner::default(),
            update: Mutex::new(update::anfang(k.selbstupdate, k.deploy_skript.as_deref())),
            obergrenze: k.max_arbeiter,
        }
    }

    pub fn aendern<R>(&self, f: impl FnOnce(&mut Gespeichert) -> R) -> R {
        let mut d = self.d.lock().unwrap();
        let r = f(&mut d);
        speichern(&self.datei, &*d);
        r
    }

    pub fn thema(&self) -> Thema {
        let mut t = self.d.lock().unwrap().thema.clone();
        use protokoll::einstellungen::Feiertag;
        t.aktiv = match t.wahl {
            ThemaWahl::Halloween => Some(Feiertag::Halloween),
            ThemaWahl::Weihnachten => Some(Feiertag::Weihnachten),
            ThemaWahl::Aus => None,
            ThemaWahl::Automatisch => feiertag_nach_kalender(),
        };
        t
    }

    pub fn karte(&self) -> Karte {
        self.d.lock().unwrap().karte.clone()
    }

    pub fn benachrichtigung(&self) -> Benachrichtigung {
        self.d.lock().unwrap().benachrichtigung.clone()
    }

    pub fn maschine(&self, arbeiter: u32) -> Maschine {
        let limit = self.d.lock().unwrap().arbeiter_limit.or(self.obergrenze);
        self.maschine.stand(arbeiter, limit, self.obergrenze)
    }

    /// Liest die eigenen Karten aus `<daten>/karten/*.json` neu.
    pub fn karten_neu_lesen(&self, daten: &std::path::Path) {
        let mut eigene = Vec::new();
        if let Ok(dir) = std::fs::read_dir(daten.join("karten")) {
            for f in dir.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let id = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let (daten, fehler) = match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string())) {
                    Ok(v) => (Some(v), None),
                    Err(e) => (None, Some(format!("Lässt sich nicht lesen: {e}"))),
                };
                let name = daten.as_ref().and_then(|d| d.get("name")).and_then(|n| n.as_str()).unwrap_or(&id).to_string();
                eigene.push(protokoll::einstellungen::EigeneKarte { id, name, daten, fehler });
            }
        }
        eigene.sort_by(|a, b| a.id.cmp(&b.id));
        let mut d = self.d.lock().unwrap();
        d.karte.eigene = eigene;
    }

    /// Setzt den Webhook ("" entfernt ihn). Gibt einen Fehler zurück, wenn die URL nicht taugt.
    pub fn webhook_setzen(&self, url: &str, von: &str) -> Result<(), String> {
        let url = url.trim();
        if url.is_empty() {
            self.aendern(|d| {
                d.webhook_url = None;
                d.benachrichtigung = Benachrichtigung::default();
            });
            return Ok(());
        }
        if !url.starts_with("https://") || url.len() > 500 {
            return Err("Der Webhook muss eine https://-Adresse sein".into());
        }
        let ohne = &url["https://".len()..];
        let host = ohne.split('/').next().unwrap_or("");
        let art = if host.ends_with("slack.com") {
            WebhookArt::Slack
        } else if host.ends_with("discord.com") || host.ends_with("discordapp.com") {
            WebhookArt::Discord
        } else {
            WebhookArt::Andere
        };
        let ende: String = url.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        let hinweis = format!("{host}/…{ende}");
        self.aendern(|d| {
            d.webhook_url = Some(url.to_string());
            d.benachrichtigung = Benachrichtigung { webhook: Some(WebhookInfo { art, hinweis, von: von.into(), um: jetzt_ms() }), fehler: None, zuletzt_gesendet: None };
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_wird_versteckt() {
        let dir = tempfile::tempdir().unwrap();
        let e = Einstellungen::laden(&Konfig::fuer_test(dir.path().into()));
        assert!(e.webhook_setzen("http://x", "a").is_err());
        e.webhook_setzen("https://hooks.slack.com/services/T/B/abcd1234", "Luna").unwrap();
        let b = e.benachrichtigung();
        let w = b.webhook.unwrap();
        assert_eq!(w.art, WebhookArt::Slack);
        assert!(!w.hinweis.contains("services"));
        let wieder = Einstellungen::laden(&Konfig::fuer_test(dir.path().into()));
        assert!(wieder.d.lock().unwrap().webhook_url.is_some());
    }
}
