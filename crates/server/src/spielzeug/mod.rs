//! Die Spielsachen jeder Etage: Bilder, Hund, Basketball, Autos, Arcade, Whiteboard.
//! Gespeichert je Etage unter `<daten>/etagen/<id>/spielzeug.json` (Ball und Autos nur im Speicher).

pub mod ball;
pub mod hund;
pub mod whiteboard;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use protokoll::spielzeug::{Arcade, ArcadeBild, Auto, Ball, Bestenliste, Dekor, Stand};
use serde::{Deserialize, Serialize};

use crate::speicher::{laden, speichern};

pub const MAX_DEKOR: usize = 60;
pub const MAX_BESTE: usize = 10;

/// Die Autos auf ihren Parkplätzen (aus `welt`, Index = Auto-Nummer).
pub fn autos_geparkt() -> Vec<Auto> {
    welt::grundriss::draussen::AUTOS
        .iter()
        .map(|p| Auto {
            x: p.x,
            z: p.z,
            rot_y: p.rot_y,
            tempo: 0.0,
            lenkung: 0.0,
            fahrer: None,
            beifahrer: None,
        })
        .collect()
}

/// Was von einer Etage gespeichert wird.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gespeichert {
    pub dekor: Vec<Dekor>,
    pub hund_name: Option<String>,
    pub whiteboard: Vec<serde_json::Value>,
}

/// Der Zustand einer Etage.
pub struct EtagenSpielzeug {
    pub g: Gespeichert,
    pub ball: Ball,
    /// Wann der letzte Wurf losging (ms seit 1970).
    pub ball_seit: u64,
    pub autos: Vec<Auto>,
    pub hund: hund::Hund,
    pub arcade_spieler: Option<protokoll::spielzeug::ArcadeSpieler>,
    pub arcade_bild: Option<ArcadeBild>,
    pub wb_leute: Vec<String>,
}

pub struct Spielzeug {
    daten: PathBuf,
    pub etagen: Mutex<HashMap<String, EtagenSpielzeug>>,
    /// Die Bestenliste des ganzen Gebäudes.
    pub bestenliste: Mutex<Vec<Bestenliste>>,
}

/// Schlüssel einer Etage (`None` = Gebäude ohne Etagen).
pub fn schluessel(etage: Option<&str>) -> String {
    etage.unwrap_or("").to_string()
}

impl Spielzeug {
    pub fn laden(daten: &std::path::Path) -> Self {
        Self {
            daten: daten.to_path_buf(),
            etagen: Mutex::default(),
            bestenliste: Mutex::new(laden(&daten.join("bestenliste.json"))),
        }
    }

    fn datei(&self, etage: &str) -> PathBuf {
        let name = if etage.is_empty() { "_ohne" } else { etage };
        self.daten
            .join("etagen")
            .join(name.replace(['/', '\\', '.'], "_"))
            .join("spielzeug.json")
    }

    /// Arbeitet mit dem Spielzeug einer Etage (lädt es beim ersten Mal).
    pub fn mit<R>(&self, etage: Option<&str>, f: impl FnOnce(&mut EtagenSpielzeug) -> R) -> R {
        let key = schluessel(etage);
        let mut m = self.etagen.lock().unwrap();
        let e = m.entry(key.clone()).or_insert_with(|| {
            let g: Gespeichert = laden(&self.datei(&key));
            EtagenSpielzeug {
                hund: hund::Hund::neu(&key, g.hund_name.clone()),
                g,
                ball: Ball::default(),
                ball_seit: 0,
                autos: autos_geparkt(),
                arcade_spieler: None,
                arcade_bild: None,
                wb_leute: Vec::new(),
            }
        });
        f(e)
    }

    /// Wie `mit`, speichert danach.
    pub fn aendern<R>(&self, etage: Option<&str>, f: impl FnOnce(&mut EtagenSpielzeug) -> R) -> R {
        let key = schluessel(etage);
        let (r, g) = self.mit(etage, |e| {
            let r = f(e);
            (r, e.g.clone())
        });
        speichern(&self.datei(&key), &g);
        r
    }

    pub fn stand(&self, etage: Option<&str>, jetzt_ms: u64) -> Stand {
        let beste = self.bestenliste.lock().unwrap().clone();
        self.mit(etage, |e| Stand {
            dekor: e.g.dekor.clone(),
            hund: Some(e.hund.zustand(jetzt_ms)),
            ball: ball::jetzt(&e.ball, e.ball_seit, jetzt_ms),
            autos: e.autos.clone(),
            arcade: Arcade {
                spieler: e.arcade_spieler.clone(),
                bestenliste: beste,
                bild: e.arcade_bild.clone(),
            },
            whiteboard: e.g.whiteboard.clone(),
            whiteboard_leute: e.wb_leute.clone(),
        })
    }

    /// Trägt einen Spielstand in die Bestenliste ein; `true`, wenn sie sich änderte.
    pub fn punkte_eintragen(&self, b: Bestenliste) -> bool {
        let mut l = self.bestenliste.lock().unwrap();
        match l.iter_mut().find(|x| x.spiel == b.spiel) {
            Some(x) if x.punkte >= b.punkte => return false,
            Some(x) => *x = b,
            None => {
                if l.len() >= MAX_BESTE && l.last().is_some_and(|x| x.punkte >= b.punkte) {
                    return false;
                }
                l.push(b);
            }
        }
        l.sort_by_key(|b| std::cmp::Reverse(b.punkte));
        l.truncate(MAX_BESTE);
        speichern(&self.daten.join("bestenliste.json"), &*l);
        true
    }

    /// Jemand ist weg (oder auf eine andere Etage): lässt los, was er auf `etage` hatte.
    /// Gibt zurück, was sich änderte, damit die Etage es erfährt.
    pub fn loslassen(&self, etage: Option<&str>, id: &str) -> Losgelassen {
        self.mit(etage, |e| {
            let mut l = Losgelassen::default();
            if e.ball.halter.as_deref() == Some(id) {
                e.ball = Ball::default();
                l.ball = true;
            }
            for a in &mut e.autos {
                if a.fahrer.as_deref() == Some(id) {
                    a.fahrer = None;
                    a.tempo = 0.0;
                    l.autos = true;
                }
                if a.beifahrer.as_deref() == Some(id) {
                    a.beifahrer = None;
                    l.autos = true;
                }
            }
            if e.arcade_spieler.as_ref().is_some_and(|s| s.id == id) {
                e.arcade_spieler = None;
                e.arcade_bild = None;
                l.arcade = true;
            }
            if let Some(i) = e.wb_leute.iter().position(|p| p == id) {
                e.wb_leute.remove(i);
                l.whiteboard = true;
            }
            if e.hund.vergessen(id) {
                l.hund = true;
            }
            l
        })
    }
}

#[derive(Debug, Default)]
pub struct Losgelassen {
    pub ball: bool,
    pub autos: bool,
    pub arcade: bool,
    pub whiteboard: bool,
    pub hund: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eintrag(spiel: &str, punkte: u32) -> Bestenliste {
        Bestenliste {
            spiel: spiel.into(),
            name: "A".into(),
            farbe: "#000000".into(),
            punkte,
            reihen: 0,
            level: 1,
            um: 0,
        }
    }

    #[test]
    fn bestenliste() {
        let dir = tempfile::tempdir().unwrap();
        let s = Spielzeug::laden(dir.path());
        assert!(s.punkte_eintragen(eintrag("a", 10)));
        assert!(!s.punkte_eintragen(eintrag("a", 5)));
        assert!(s.punkte_eintragen(eintrag("a", 20)));
        assert_eq!(s.bestenliste.lock().unwrap().len(), 1);
    }

    #[test]
    fn loslassen() {
        let dir = tempfile::tempdir().unwrap();
        let s = Spielzeug::laden(dir.path());
        s.mit(Some("x"), |e| {
            e.ball.halter = Some("v1".into());
            e.autos[0].fahrer = Some("v1".into());
        });
        let l = s.loslassen(Some("x"), "v1");
        assert!(l.ball && l.autos && !l.arcade);
    }
}
