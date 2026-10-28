//! Eigene Claude- und GitHub-Anmeldungen je Konto: jede in ihrem eigenen Ordner unter
//! `<daten>/homes/<konto>/` (CLAUDE_CONFIG_DIR, GH_CONFIG_DIR, GIT_CONFIG_GLOBAL), ein eingefügter
//! Token oder die Anmeldung des Office-Rechners (Admins; Mitglieder für Claude, wenn geteilt).

pub mod ablauf;
pub mod claude;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use protokoll::konten::{AnmeldeArt, AnmeldeStatus, AnmeldeWeg, Anmeldung, Anmeldungen as Stand};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::speicher::{laden, speichern};

/// Diese Variablen gehören zu einer Claude- bzw. GitHub-Anmeldung und werden für ein Konto ersetzt.
pub const CLAUDE_VARIABLEN: [&str; 7] = [
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_SECURESTORAGE_CONFIG_DIR",
];
pub const GITHUB_VARIABLEN: [&str; 6] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_CONFIG_DIR",
    "GIT_CONFIG_GLOBAL",
];

/// Wie ein Konto sich angemeldet hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "weg")]
pub enum Wahl {
    Token { token: String },
    Office,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gespeichert {
    pub claude: Option<Wahl>,
    pub github: Option<Wahl>,
    /// Wer zuletzt angemeldet war (für den nächsten Start).
    pub gesehen_claude: Option<String>,
    pub gesehen_github: Option<String>,
}

/// Was gerade läuft (nur im Speicher).
#[derive(Default)]
pub struct Live {
    pub claude: Option<Anmeldung>,
    pub github: Option<Anmeldung>,
    /// Ein laufender Claude-Login: hierhin geht der Code von der Anmeldeseite.
    pub claude_eingabe: Option<std::sync::mpsc::Sender<String>>,
    /// Laufende Abläufe abbrechen.
    pub abbruch_claude: Option<Box<dyn FnOnce() + Send>>,
    pub abbruch_github: Option<Box<dyn FnOnce() + Send>>,
}

pub struct Anmeldungen {
    homes: PathBuf,
    pub live: Mutex<HashMap<String, Live>>,
    /// Meldet, wessen Anmeldungen sich änderten (Konto-ID).
    pub geaendert: broadcast::Sender<String>,
}

pub type AnmeldungenRef = Arc<Anmeldungen>;

/// Ein eingefügter Claude-Token (`claude setup-token`) oder Anthropic-API-Schlüssel?
pub fn ist_claude_token(t: &str) -> bool {
    t.starts_with("sk-ant-")
        && t.len() >= 30
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn ist_api_schluessel(t: &str) -> bool {
    t.starts_with("sk-ant-api")
}

fn sicher(konto: &str) -> String {
    konto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn leer(status: AnmeldeStatus) -> Anmeldung {
    Anmeldung {
        status,
        weg: AnmeldeWeg::Login,
        wer: None,
        offen: None,
        fehler: None,
    }
}

impl Anmeldungen {
    pub fn neu(daten: &Path) -> AnmeldungenRef {
        Arc::new(Self {
            homes: daten.join("homes"),
            live: Mutex::default(),
            geaendert: broadcast::channel(64).0,
        })
    }

    /// Der Ordner eines Kontos (wird angelegt).
    pub fn home(&self, konto: &str) -> PathBuf {
        let p = self.homes.join(sicher(konto));
        let _ = std::fs::create_dir_all(p.join("claude"));
        let _ = std::fs::create_dir_all(p.join("gh"));
        p
    }

    pub fn laden(&self, konto: &str) -> Gespeichert {
        laden(&self.home(konto).join("anmeldungen.json"))
    }

    pub fn sichern(&self, konto: &str, g: &Gespeichert) {
        speichern(&self.home(konto).join("anmeldungen.json"), g);
    }

    pub fn melden(&self, konto: &str) {
        let _ = self.geaendert.send(konto.to_string());
    }

    /// Wie `art` für das Konto läuft. Office nur, wenn erlaubt (sonst wieder der eigene Login).
    pub fn weg(&self, g: &Gespeichert, art: AnmeldeArt, darf_office: bool) -> AnmeldeWeg {
        match if art == AnmeldeArt::Claude {
            &g.claude
        } else {
            &g.github
        } {
            Some(Wahl::Office) if darf_office => AnmeldeWeg::Office,
            Some(Wahl::Token { .. }) => AnmeldeWeg::Token,
            _ => AnmeldeWeg::Login,
        }
    }

    pub fn stand(&self, konto: &str, office_claude: bool, office_github: bool) -> Stand {
        let g = self.laden(konto);
        let live = self.live.lock().unwrap();
        let l = live.get(konto);
        let eine = |art: AnmeldeArt, darf: bool| {
            let (laufend, gesehen) = match art {
                AnmeldeArt::Claude => (l.and_then(|l| l.claude.clone()), g.gesehen_claude.clone()),
                AnmeldeArt::Github => (l.and_then(|l| l.github.clone()), g.gesehen_github.clone()),
            };
            let mut a = laufend.unwrap_or_else(|| match gesehen {
                Some(wer) => Anmeldung {
                    wer: Some(wer),
                    ..leer(AnmeldeStatus::Ok)
                },
                None => leer(AnmeldeStatus::Keine),
            });
            a.weg = self.weg(&g, art, darf);
            a
        };
        Stand {
            claude: eine(AnmeldeArt::Claude, office_claude),
            github: eine(AnmeldeArt::Github, office_github),
            office_claude,
            office_github,
        }
    }

    /// Setzt den laufenden Stand einer Anmeldung.
    pub fn setzen(&self, konto: &str, art: AnmeldeArt, a: Option<Anmeldung>) {
        let mut live = self.live.lock().unwrap();
        let l = live.entry(konto.to_string()).or_default();
        match art {
            AnmeldeArt::Claude => l.claude = a,
            AnmeldeArt::Github => l.github = a,
        }
        drop(live);
        self.melden(konto);
    }

    /// Merkt sich, wer angemeldet ist.
    pub fn gesehen(&self, konto: &str, art: AnmeldeArt, wer: Option<String>) {
        let mut g = self.laden(konto);
        let feld = match art {
            AnmeldeArt::Claude => &mut g.gesehen_claude,
            AnmeldeArt::Github => &mut g.gesehen_github,
        };
        if *feld != wer {
            *feld = wer;
            self.sichern(konto, &g);
        }
    }

    /// Die Umgebung für Programme im Namen des Kontos. Ein leerer Wert heißt: Variable entfernen.
    pub fn umgebung(
        &self,
        konto: &str,
        nur: Option<AnmeldeArt>,
        office_claude: bool,
        office_github: bool,
    ) -> Vec<(String, String)> {
        let g = self.laden(konto);
        let home = self.home(konto);
        let mut env = Vec::new();
        if nur != Some(AnmeldeArt::Github)
            && self.weg(&g, AnmeldeArt::Claude, office_claude) != AnmeldeWeg::Office
        {
            env.extend(
                CLAUDE_VARIABLEN
                    .iter()
                    .map(|k| (k.to_string(), String::new())),
            );
            env.push((
                "CLAUDE_CONFIG_DIR".into(),
                home.join("claude").display().to_string(),
            ));
            if let Some(Wahl::Token { token }) = &g.claude {
                env.push((
                    if ist_api_schluessel(token) {
                        "ANTHROPIC_API_KEY"
                    } else {
                        "CLAUDE_CODE_OAUTH_TOKEN"
                    }
                    .into(),
                    token.clone(),
                ));
            }
        }
        if nur != Some(AnmeldeArt::Claude)
            && self.weg(&g, AnmeldeArt::Github, office_github) != AnmeldeWeg::Office
        {
            env.extend(
                GITHUB_VARIABLEN
                    .iter()
                    .map(|k| (k.to_string(), String::new())),
            );
            env.push((
                "GH_CONFIG_DIR".into(),
                home.join("gh").display().to_string(),
            ));
            env.push((
                "GIT_CONFIG_GLOBAL".into(),
                home.join("gitconfig").display().to_string(),
            ));
            if let Some(Wahl::Token { token }) = &g.github {
                env.push(("GH_TOKEN".into(), token.clone()));
            }
        }
        // Spätere Einträge überschreiben frühere: nur den letzten Wert je Variable behalten.
        let mut gesehen = std::collections::HashSet::new();
        let mut aus: Vec<(String, String)> = env
            .into_iter()
            .rev()
            .filter(|(k, _)| gesehen.insert(k.clone()))
            .collect();
        aus.reverse();
        aus
    }

    /// Ein Konto wurde entzogen: Abläufe stoppen und den Ordner vergessen (bleibt auf der Platte).
    pub fn vergessen(&self, konto: &str) {
        if let Some(mut l) = self.live.lock().unwrap().remove(konto) {
            for f in [l.abbruch_claude.take(), l.abbruch_github.take()]
                .into_iter()
                .flatten()
            {
                f();
            }
        }
    }
}

/// Wendet eine Umgebung aus [`Anmeldungen::umgebung`] auf einen Befehl an.
pub fn anwenden(cmd: &mut tokio::process::Command, env: &[(String, String)]) {
    for (k, v) in env {
        if v.is_empty() {
            cmd.env_remove(k);
        } else {
            cmd.env(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn umgebung_mit_token() {
        let dir = tempfile::tempdir().unwrap();
        let a = Anmeldungen::neu(dir.path());
        let mut g = a.laden("k1");
        g.claude = Some(Wahl::Token {
            token: "sk-ant-oat01-abcdefghijklmnopqrstuvwxyz".into(),
        });
        a.sichern("k1", &g);
        let env = a.umgebung("k1", None, false, false);
        let wert = |k: &str| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        assert!(
            wert("CLAUDE_CODE_OAUTH_TOKEN")
                .unwrap()
                .starts_with("sk-ant-oat01")
        );
        assert_eq!(
            wert("ANTHROPIC_API_KEY").as_deref(),
            Some(""),
            "fremde Schlüssel werden entfernt"
        );
        assert!(wert("GH_CONFIG_DIR").unwrap().contains("gh"));
        assert_eq!(
            env.iter().filter(|(k, _)| k == "CLAUDE_CONFIG_DIR").count(),
            1
        );
    }

    #[test]
    fn office_nur_wenn_erlaubt() {
        let dir = tempfile::tempdir().unwrap();
        let a = Anmeldungen::neu(dir.path());
        let g = Gespeichert {
            claude: Some(Wahl::Office),
            ..Default::default()
        };
        assert_eq!(a.weg(&g, AnmeldeArt::Claude, true), AnmeldeWeg::Office);
        assert_eq!(a.weg(&g, AnmeldeArt::Claude, false), AnmeldeWeg::Login);
        assert!(ist_claude_token("sk-ant-api03-abcdefghijklmnopqrstuvwxyz"));
        assert!(!ist_claude_token("hallo"));
    }
}
