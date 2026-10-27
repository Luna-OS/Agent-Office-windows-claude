//! Das gemeinsame Aufgabenboard einer Etage. Menschen (über das UI) und Agenten (über
//! `office-agenten`) legen Aufgaben an, übernehmen und erledigen sie.

use protokoll::aufgaben::{Aufgabe, AufgabenStatus, Board};
use serde::{Deserialize, Serialize};

pub const MAX_TITEL: usize = 200;
pub const MAX_NOTIZ: usize = 4000;
pub const MAX_AUFGABEN: usize = 500;

/// Eine neue Aufgabe.
#[derive(Debug, Default, Clone)]
pub struct Neu {
    pub titel: String,
    pub notiz: Option<String>,
    pub prompt: Option<String>,
    pub issue: Option<u64>,
    pub besitzer: Option<String>,
    pub automatisch: bool,
    pub provider: Option<String>,
}

/// Eine Änderung; `None` lässt das Feld, wie es ist.
#[derive(Debug, Default, Clone)]
pub struct Aenderung {
    pub titel: Option<String>,
    pub status: Option<AufgabenStatus>,
    /// `Some("")` nimmt den Besitzer weg.
    pub besitzer: Option<String>,
    pub notiz: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Aufgabenboard {
    pub board: Board,
    naechste: u64,
}

fn kappen(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

fn leer_zu_none(text: Option<String>, max: usize) -> Option<String> {
    text.map(|t| kappen(&t, max)).filter(|t| !t.is_empty())
}

/// Liest einen Status wie ihn Menschen und Agenten tippen.
pub fn status_lesen(text: &str) -> Option<AufgabenStatus> {
    Some(match text.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "offen" | "todo" | "neu" => AufgabenStatus::Offen,
        "in_arbeit" | "doing" | "dran" | "arbeite" => AufgabenStatus::InArbeit,
        "erledigt" | "done" | "fertig" => AufgabenStatus::Erledigt,
        "blockiert" | "blocked" => AufgabenStatus::Blockiert,
        _ => return None,
    })
}

impl Aufgabenboard {
    pub fn neu(max_arbeiter: u32) -> Self {
        Self { board: Board { aufgaben: Vec::new(), max_arbeiter }, naechste: 0 }
    }

    pub fn hinzufuegen(&mut self, neu: Neu, von: &str, um: u64) -> Result<&Aufgabe, String> {
        let titel = kappen(&neu.titel, MAX_TITEL);
        if titel.is_empty() {
            return Err("Die Aufgabe braucht einen Titel".into());
        }
        if self.board.aufgaben.len() >= MAX_AUFGABEN {
            return Err("Das Board ist voll: erst erledigte Aufgaben leeren".into());
        }
        self.naechste += 1;
        let besitzer = leer_zu_none(neu.besitzer, 64);
        self.board.aufgaben.push(Aufgabe {
            id: self.naechste,
            titel,
            notiz: leer_zu_none(neu.notiz, MAX_NOTIZ),
            status: if besitzer.is_some() && !neu.automatisch { AufgabenStatus::InArbeit } else { AufgabenStatus::Offen },
            besitzer,
            angelegt_von: von.to_string(),
            angelegt_um: um,
            geaendert_um: um,
            issue: neu.issue,
            prompt: leer_zu_none(neu.prompt, 20_000),
            automatisch: neu.automatisch,
            provider: neu.provider,
            arbeiter: None,
        });
        Ok(self.board.aufgaben.last().expect("eben angelegt"))
    }

    pub fn holen(&self, id: u64) -> Option<&Aufgabe> {
        self.board.aufgaben.iter().find(|a| a.id == id)
    }

    pub fn holen_mut(&mut self, id: u64) -> Option<&mut Aufgabe> {
        self.board.aufgaben.iter_mut().find(|a| a.id == id)
    }

    pub fn aendern(&mut self, id: u64, a: Aenderung, um: u64) -> Result<&Aufgabe, String> {
        let aufgabe = self.holen_mut(id).ok_or_else(|| format!("Keine Aufgabe #{id}"))?;
        if let Some(t) = a.titel.map(|t| kappen(&t, MAX_TITEL)).filter(|t| !t.is_empty()) {
            aufgabe.titel = t;
        }
        if let Some(b) = a.besitzer {
            aufgabe.besitzer = leer_zu_none(Some(b), 64);
        }
        if let Some(n) = a.notiz {
            aufgabe.notiz = leer_zu_none(Some(n), MAX_NOTIZ);
        }
        if let Some(s) = a.status {
            aufgabe.status = s;
        }
        aufgabe.geaendert_um = um;
        Ok(aufgabe)
    }

    pub fn entfernen(&mut self, id: u64) -> bool {
        let vorher = self.board.aufgaben.len();
        self.board.aufgaben.retain(|a| a.id != id);
        self.board.aufgaben.len() != vorher
    }

    pub fn verschieben(&mut self, id: u64, delta: i32) -> bool {
        let Some(pos) = self.board.aufgaben.iter().position(|a| a.id == id) else { return false };
        let ziel = (pos as i64 + delta as i64).clamp(0, self.board.aufgaben.len() as i64 - 1) as usize;
        let a = self.board.aufgaben.remove(pos);
        self.board.aufgaben.insert(ziel, a);
        ziel != pos
    }

    pub fn erledigte_leeren(&mut self) {
        self.board.aufgaben.retain(|a| a.status != AufgabenStatus::Erledigt);
    }

    /// Die nächste Aufgabe, die das Office selbst vergeben soll, wenn noch Platz ist.
    /// `laufend`: wie viele automatische Aufgaben gerade einen Arbeiter haben.
    pub fn naechste_automatische(&self, laufend: u32) -> Option<&Aufgabe> {
        if laufend >= self.board.max_arbeiter {
            return None;
        }
        self.board.aufgaben.iter().find(|a| a.automatisch && a.status == AufgabenStatus::Offen && a.arbeiter.is_none())
    }

    /// Wie viele automatische Aufgaben gerade einen Arbeiter beschäftigen.
    pub fn automatisch_laufend(&self) -> u32 {
        self.board.aufgaben.iter().filter(|a| a.automatisch && a.status == AufgabenStatus::InArbeit && a.arbeiter.is_some()).count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neu(titel: &str) -> Neu {
        Neu { titel: titel.into(), ..Default::default() }
    }

    #[test]
    fn anlegen_aendern_entfernen() {
        let mut b = Aufgabenboard::neu(2);
        assert!(b.hinzufuegen(neu("  "), "x", 0).is_err());
        let id = b.hinzufuegen(neu("Login reparieren"), "Luna", 1).unwrap().id;
        let a = b.aendern(id, Aenderung { status: Some(AufgabenStatus::InArbeit), besitzer: Some("Ada".into()), ..Default::default() }, 2).unwrap();
        assert_eq!(a.besitzer.as_deref(), Some("Ada"));
        assert_eq!(a.status, AufgabenStatus::InArbeit);
        let a = b.aendern(id, Aenderung { besitzer: Some(String::new()), ..Default::default() }, 3).unwrap();
        assert_eq!(a.besitzer, None);
        assert!(b.aendern(99, Aenderung::default(), 0).is_err());
        assert!(b.entfernen(id));
        assert!(!b.entfernen(id));
    }

    #[test]
    fn reihenfolge_und_leeren() {
        let mut b = Aufgabenboard::neu(1);
        let a = b.hinzufuegen(neu("a"), "x", 0).unwrap().id;
        let c = b.hinzufuegen(neu("c"), "x", 0).unwrap().id;
        assert!(b.verschieben(c, -1));
        assert_eq!(b.board.aufgaben[0].id, c);
        assert!(!b.verschieben(c, -5));
        b.aendern(a, Aenderung { status: Some(AufgabenStatus::Erledigt), ..Default::default() }, 1).unwrap();
        b.erledigte_leeren();
        assert_eq!(b.board.aufgaben.len(), 1);
    }

    #[test]
    fn warteschlange() {
        let mut b = Aufgabenboard::neu(1);
        b.hinzufuegen(neu("manuell"), "x", 0).unwrap();
        let id = b.hinzufuegen(Neu { automatisch: true, ..neu("auto") }, "x", 0).unwrap().id;
        assert_eq!(b.naechste_automatische(0).unwrap().id, id);
        assert!(b.naechste_automatische(1).is_none());
        b.board.max_arbeiter = 0;
        assert!(b.naechste_automatische(0).is_none());
        assert_eq!(status_lesen("in arbeit"), Some(AufgabenStatus::InArbeit));
        assert_eq!(status_lesen("done"), Some(AufgabenStatus::Erledigt));
        assert_eq!(status_lesen("quatsch"), None);
    }
}
