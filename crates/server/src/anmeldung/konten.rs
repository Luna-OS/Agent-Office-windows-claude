//! Eigene Konten mit Rollen, Einladungslinks und dem gemeinsamen Office-Passwort (konten.json).

use std::collections::HashSet;
use std::path::PathBuf;

use protokoll::konten::{Einladung, KontenStand, KontoInfo, Rolle};
use serde::{Deserialize, Serialize};

use super::passwort::{self, PASSWORT_MAX, PASSWORT_MIN};
use crate::speicher::{geheimnis, jetzt_ms, laden, speichern, zufalls_id};

pub const NAME_MAX: usize = 24;
const EINLADUNG_GUELTIG_MS: u64 = 7 * 24 * 60 * 60_000;
const MAX_EINLADUNGEN: usize = 100;

/// Ein Konto, wie es gespeichert wird.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Konto {
    pub id: String,
    pub name: String,
    pub rolle: Rolle,
    pub passwort: String,
    pub erstellt_um: u64,
    pub erstellt_von: String,
    #[serde(default)]
    pub zuletzt_da: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Gespeichert {
    konten: Vec<Konto>,
    einladungen: Vec<Einladung>,
    gemeinsames_passwort: bool,
    claude_teilen: bool,
}

impl Default for Gespeichert {
    fn default() -> Self {
        Self { konten: Vec::new(), einladungen: Vec::new(), gemeinsames_passwort: true, claude_teilen: false }
    }
}

pub struct Konten {
    datei: PathBuf,
    d: Gespeichert,
}

/// Säubert einen Namen: Steuerzeichen raus, Leerraum zusammen, höchstens `NAME_MAX` Zeichen.
pub fn name_saeubern(name: &str) -> String {
    let s: String = name.chars().filter(|c| !c.is_control()).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = s.chars().take(NAME_MAX).collect();
    if s.chars().any(char::is_alphanumeric) { s } else { String::new() }
}

fn gleicher_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl Konten {
    pub fn laden(daten: &std::path::Path) -> Self {
        let datei = daten.join("konten.json");
        let d = laden(&datei);
        Self { datei, d }
    }

    fn sichern(&self) {
        speichern(&self.datei, &self.d);
    }

    pub fn gibt_es_welche(&self) -> bool {
        !self.d.konten.is_empty()
    }

    pub fn gemeinsames_passwort(&self) -> bool {
        self.d.gemeinsames_passwort || self.d.konten.iter().all(|k| k.rolle != Rolle::Admin)
    }

    pub fn claude_teilen(&self) -> bool {
        self.d.claude_teilen
    }

    pub fn get(&self, id: &str) -> Option<&Konto> {
        self.d.konten.iter().find(|k| k.id == id)
    }

    pub fn nach_name(&self, name: &str) -> Option<&Konto> {
        let n = name_saeubern(name);
        self.d.konten.iter().find(|k| gleicher_name(&k.name, &n))
    }

    pub fn liste(&self) -> &[Konto] {
        &self.d.konten
    }

    pub fn stand(&self, online: &HashSet<String>, adressen: Vec<String>) -> KontenStand {
        KontenStand {
            konten: self
                .d
                .konten
                .iter()
                .map(|k| KontoInfo {
                    id: k.id.clone(),
                    name: k.name.clone(),
                    rolle: k.rolle,
                    erstellt_um: k.erstellt_um,
                    erstellt_von: k.erstellt_von.clone(),
                    zuletzt_da: k.zuletzt_da,
                    online: online.contains(&k.id),
                })
                .collect(),
            einladungen: self.offene_einladungen(),
            gemeinsames_passwort: self.gemeinsames_passwort(),
            claude_teilen: self.d.claude_teilen,
            adressen,
        }
    }

    pub fn offene_einladungen(&self) -> Vec<Einladung> {
        let jetzt = jetzt_ms();
        self.d.einladungen.iter().filter(|e| e.gueltig_bis > jetzt).cloned().collect()
    }

    /// Prüft Name und Passwort eines Kontos.
    pub fn pruefen(&self, name: &str, passwort: &str) -> Option<&Konto> {
        match self.nach_name(name) {
            Some(k) if passwort::pruefen(passwort, &k.passwort) => Some(k),
            Some(_) => None,
            None => {
                passwort::blind_pruefen(passwort);
                None
            }
        }
    }

    fn name_vergeben(&self, name: &str, ausser_einladung: Option<&str>) -> bool {
        self.d.konten.iter().any(|k| gleicher_name(&k.name, name))
            || self.offene_einladungen().iter().any(|e| Some(e.id.as_str()) != ausser_einladung && e.name.as_deref().is_some_and(|n| gleicher_name(n, name)))
    }

    pub fn einladen(&mut self, von: &str, rolle: Rolle, name: Option<&str>) -> Result<Einladung, String> {
        let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => {
                let s = name_saeubern(n);
                if s.is_empty() {
                    return Err("Der Name hat keine Buchstaben".into());
                }
                if self.name_vergeben(&s, None) {
                    return Err(format!("Den Namen {s} gibt es schon"));
                }
                Some(s)
            }
            None => None,
        };
        self.aufraeumen();
        if self.d.einladungen.len() >= MAX_EINLADUNGEN {
            return Err("Zu viele offene Einladungen – zieh erst welche zurück".into());
        }
        let jetzt = jetzt_ms();
        let e = Einladung {
            id: zufalls_id(10),
            token: geheimnis(24),
            name,
            rolle,
            erstellt_von: von.to_string(),
            erstellt_um: jetzt,
            gueltig_bis: jetzt + EINLADUNG_GUELTIG_MS,
        };
        self.d.einladungen.push(e.clone());
        self.sichern();
        Ok(e)
    }

    pub fn einladung_finden(&self, token: &str) -> Option<&Einladung> {
        if token.is_empty() {
            return None;
        }
        let jetzt = jetzt_ms();
        self.d.einladungen.iter().find(|e| e.gueltig_bis > jetzt && passwort::gleich(e.token.as_bytes(), token.as_bytes()))
    }

    pub fn einladung_zurueckziehen(&mut self, id: &str) -> bool {
        let vorher = self.d.einladungen.len();
        self.d.einladungen.retain(|e| e.id != id);
        let weg = self.d.einladungen.len() != vorher;
        if weg {
            self.sichern();
        }
        weg
    }

    /// Löst eine Einladung ein: legt das Konto an.
    pub fn beitreten(&mut self, token: &str, name: &str, passwort_neu: &str) -> Result<Konto, String> {
        let e = self.einladung_finden(token).cloned().ok_or("Dieser Einladungslink ist abgelaufen oder wurde schon benutzt. Bitte um einen neuen.")?;
        let name = e.name.clone().unwrap_or_else(|| name_saeubern(name));
        if name.is_empty() {
            return Err("Wähl einen Namen".into());
        }
        if passwort_neu.chars().count() < PASSWORT_MIN {
            return Err(format!("Wähl ein Passwort mit mindestens {PASSWORT_MIN} Zeichen"));
        }
        if passwort_neu.len() > PASSWORT_MAX {
            return Err("Das Passwort ist zu lang".into());
        }
        if self.name_vergeben(&name, Some(&e.id)) {
            return Err(format!("Den Namen {name} gibt es schon"));
        }
        self.d.einladungen.retain(|v| v.id != e.id);
        let k = Konto {
            id: zufalls_id(12),
            name,
            rolle: e.rolle,
            passwort: passwort::hashen(passwort_neu),
            erstellt_um: jetzt_ms(),
            erstellt_von: e.erstellt_von,
            zuletzt_da: None,
        };
        self.d.konten.push(k.clone());
        self.sichern();
        Ok(k)
    }

    pub fn entziehen(&mut self, id: &str) -> Option<Konto> {
        let i = self.d.konten.iter().position(|k| k.id == id)?;
        let k = self.d.konten.remove(i);
        self.sichern();
        Some(k)
    }

    pub fn rolle_setzen(&mut self, id: &str, rolle: Rolle) -> Result<(), String> {
        let admins = self.d.konten.iter().filter(|k| k.rolle == Rolle::Admin).count();
        let k = self.d.konten.iter_mut().find(|k| k.id == id).ok_or("Dieses Konto gibt es nicht mehr")?;
        if k.rolle == Rolle::Admin && rolle != Rolle::Admin && admins == 1 && !self.d.gemeinsames_passwort {
            return Err("Der letzte Admin kann nicht Mitglied werden, solange das gemeinsame Passwort aus ist".into());
        }
        k.rolle = rolle;
        self.sichern();
        Ok(())
    }

    pub fn gemeinsames_passwort_setzen(&mut self, an: bool) -> Result<(), String> {
        if !an && !self.d.konten.iter().any(|k| k.rolle == Rolle::Admin) {
            return Err("Ohne ein Admin-Konto bleibt das gemeinsame Passwort an – sonst käme niemand mehr herein".into());
        }
        self.d.gemeinsames_passwort = an;
        self.sichern();
        Ok(())
    }

    pub fn claude_teilen_setzen(&mut self, an: bool) {
        self.d.claude_teilen = an;
        self.sichern();
    }

    pub fn gesehen(&mut self, id: &str) {
        if let Some(k) = self.d.konten.iter_mut().find(|k| k.id == id) {
            k.zuletzt_da = Some(jetzt_ms());
            self.sichern();
        }
    }

    fn aufraeumen(&mut self) {
        let jetzt = jetzt_ms();
        self.d.einladungen.retain(|e| e.gueltig_bis > jetzt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn einladen_beitreten_anmelden() {
        let dir = tempfile::tempdir().unwrap();
        let mut k = Konten::laden(dir.path());
        let e = k.einladen("Terminal", Rolle::Admin, Some("Luna")).unwrap();
        assert!(k.einladen("Terminal", Rolle::Mitglied, Some("luna")).is_err(), "Name doppelt");
        assert!(k.beitreten(&e.token, "egal", "kurz").is_err());
        let konto = k.beitreten(&e.token, "egal", "langesPasswort").unwrap();
        assert_eq!(konto.name, "Luna");
        assert!(k.beitreten(&e.token, "x", "langesPasswort").is_err(), "nur einmal");
        assert!(k.pruefen("LUNA", "langesPasswort").is_some());
        assert!(k.pruefen("Luna", "falsch").is_none());
        let wieder = Konten::laden(dir.path());
        assert_eq!(wieder.liste().len(), 1);
        k.gemeinsames_passwort_setzen(false).unwrap();
        assert!(!k.gemeinsames_passwort());
    }

    #[test]
    fn namen_saeubern() {
        assert_eq!(name_saeubern("  a\u{7}  b "), "a b");
        assert_eq!(name_saeubern("!!!"), "");
    }
}
