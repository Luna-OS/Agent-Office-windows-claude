//! Die Etagen des Gebäudes (etagen.json): ein Projekt je Etage, dazu laufende Klone.

pub mod git;
pub mod klonen;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use protokoll::etagen::{EtageInfo, FLUEGEL_MAX, Grundriss, KlonFortschritt, ProjektInfo, ProjekteOrdner, Schild};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::speicher::{jetzt_ms, laden, speichern};

/// Eine Etage, wie sie gespeichert wird.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtageDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub repo: Option<String>,
    pub ordner: PathBuf,
    pub palette: u32,
    #[serde(default)]
    pub lokal: bool,
    pub hinzugefuegt_von: String,
    pub hinzugefuegt_um: u64,
    #[serde(default)]
    pub grundriss: Grundriss,
}

/// Ein Klon, der noch läuft.
pub struct KlonLauf {
    pub def: EtageDef,
    pub fortschritt: Option<KlonFortschritt>,
    /// Verbindung, die ihn wollte (bekommt `Hinzugefuegt`).
    pub verbindung: String,
    /// Name dessen, der ihn startete (darf abbrechen).
    pub von: String,
    pub abbruch: Option<oneshot::Sender<()>>,
}

/// Was `klon_anmelden` ergab.
pub enum KlonStart {
    /// Klonen: die Etage ist angemeldet, `abbruch` meldet einen Abbruch.
    Klonen(EtageDef, oneshot::Receiver<()>),
    /// Der Ordner ist schon da (früher geklont): übernehmen statt klonen.
    Vorhanden(PathBuf),
}

/// Zahlen je Etage für die Aufzugtafel, die andere liefern (Arbeiter von `agenten`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Zahlen {
    pub arbeiter: u32,
    pub beschaeftigt: u32,
    pub wartend: u32,
}

pub struct Etagen {
    datei: PathBuf,
    pub liste: Mutex<Vec<EtageDef>>,
    pub klone: Mutex<HashMap<String, KlonLauf>>,
    standard_projekte: PathBuf,
}

/// Eine ID aus einem Namen: klein, nur a-z0-9 und Bindestriche.
pub fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|t| !t.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "etage".into() } else { s.chars().take(48).collect() }
}

/// Ein Pfad zum Anzeigen: unter dem Home-Ordner als ~/….
pub fn zum_anzeigen(p: &Path) -> String {
    let home = crate::konfig::home();
    match p.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.display()).replace('\\', "/"),
        Err(_) => p.display().to_string(),
    }
}

/// Ein absoluter Pfad ohne das Windows-Präfix für lange Pfade.
pub fn absolut(p: &Path) -> PathBuf {
    let p = p.canonicalize().unwrap_or_else(|_| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()));
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p,
    }
}

impl Etagen {
    pub fn laden(daten: &Path, standard_projekte: PathBuf) -> Self {
        let datei = daten.join("etagen.json");
        let liste: Vec<EtageDef> = laden(&datei);
        Self { datei, liste: Mutex::new(liste), klone: Mutex::default(), standard_projekte }
    }

    fn sichern(&self, liste: &[EtageDef]) {
        speichern(&self.datei, liste);
    }

    pub fn get(&self, id: &str) -> Option<EtageDef> {
        self.liste.lock().unwrap().iter().find(|e| e.id == id).cloned()
    }

    pub fn gibt_es(&self, id: &str) -> bool {
        self.liste.lock().unwrap().iter().any(|e| e.id == id)
    }

    pub fn erste(&self) -> Option<EtageDef> {
        self.liste.lock().unwrap().first().cloned()
    }

    pub fn anzahl(&self) -> usize {
        self.liste.lock().unwrap().len()
    }

    /// Wohin neue Etagen geklont werden.
    pub fn projekte_ordner(&self, eigen: Option<&(String, String, u64)>) -> (PathBuf, ProjekteOrdner) {
        match eigen {
            Some((ordner, von, um)) => {
                let p = PathBuf::from(ordner);
                let a = ProjekteOrdner { ordner: zum_anzeigen(&p), eigen: true, von: Some(von.clone()), um: Some(*um) };
                (p, a)
            }
            None => (self.standard_projekte.clone(), ProjekteOrdner { ordner: zum_anzeigen(&self.standard_projekte), eigen: false, von: None, um: None }),
        }
    }

    fn freie_id(&self, wunsch: &str) -> String {
        let basis = slug(wunsch);
        let liste = self.liste.lock().unwrap();
        let klone = self.klone.lock().unwrap();
        let belegt = |id: &str| liste.iter().any(|e| e.id == id) || klone.contains_key(id) || id == protokoll::etagen::DACH;
        if !belegt(&basis) {
            return basis;
        }
        (2..).map(|n| format!("{basis}-{n}")).find(|id| !belegt(id)).expect("unendlich viele Kandidaten")
    }

    fn naechste_palette(&self) -> u32 {
        let n = self.liste.lock().unwrap().len() + self.klone.lock().unwrap().len();
        n as u32
    }

    /// Das Projekt, in dem das Office gestartet wurde: wird (falls nicht schon da) die lokale Etage.
    pub fn lokale_sicherstellen(&self, ordner: &Path) -> EtageDef {
        let ordner = absolut(ordner);
        if let Some(e) = self.liste.lock().unwrap().iter().find(|e| e.ordner == ordner) {
            return e.clone();
        }
        let name = ordner.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Projekt".into());
        let repo = git::remote(&ordner).and_then(|r| git::repo_aus(&r));
        let def = EtageDef {
            id: self.freie_id(repo.as_deref().and_then(|r| r.split('/').nth(1)).unwrap_or(&name)),
            name: repo.as_deref().and_then(|r| r.split('/').nth(1)).map(String::from).unwrap_or(name),
            repo,
            ordner,
            palette: self.naechste_palette(),
            lokal: true,
            hinzugefuegt_von: "Terminal".into(),
            hinzugefuegt_um: jetzt_ms(),
            grundriss: Grundriss::default(),
        };
        let mut liste = self.liste.lock().unwrap();
        liste.insert(0, def.clone());
        self.sichern(&liste);
        def
    }

    /// Bereitet einen Klon vor: die Etage steht im Aufzug, ist aber noch nicht betretbar.
    pub fn klon_anmelden(&self, repo: &str, projekte: &Path, von: &str, verbindung: &str) -> Result<KlonStart, String> {
        let (owner, name) = repo.split_once('/').ok_or("Ein Repository heißt owner/name")?;
        if self.liste.lock().unwrap().iter().any(|e| e.repo.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(repo))) {
            return Err(format!("{repo} ist schon eine Etage"));
        }
        if self.klone.lock().unwrap().values().any(|k| k.def.repo.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(repo))) {
            return Err(format!("{repo} wird schon geklont"));
        }
        let ordner = projekte.join(owner).join(name);
        if ordner.exists() && std::fs::read_dir(&ordner).map(|mut d| d.next().is_some()).unwrap_or(true) {
            // Schon ausgecheckt: direkt als Etage übernehmen, ohne zu klonen.
            return Ok(KlonStart::Vorhanden(ordner));
        }
        let def = EtageDef {
            id: self.freie_id(name),
            name: name.to_string(),
            repo: Some(repo.to_string()),
            ordner,
            palette: self.naechste_palette(),
            lokal: false,
            hinzugefuegt_von: von.to_string(),
            hinzugefuegt_um: jetzt_ms(),
            grundriss: Grundriss::default(),
        };
        let (tx, rx) = oneshot::channel();
        self.klone.lock().unwrap().insert(
            def.id.clone(),
            KlonLauf { def: def.clone(), fortschritt: None, verbindung: verbindung.into(), von: von.into(), abbruch: Some(tx) },
        );
        Ok(KlonStart::Klonen(def, rx))
    }

    /// Eine Etage aus einem vorhandenen Checkout anlegen (ohne Klon).
    pub fn uebernehmen(&self, repo: &str, ordner: PathBuf, von: &str) -> EtageDef {
        let name = repo.split('/').nth(1).unwrap_or(repo).to_string();
        let def = EtageDef {
            id: self.freie_id(&name),
            name,
            repo: Some(repo.into()),
            ordner,
            palette: self.naechste_palette(),
            lokal: false,
            hinzugefuegt_von: von.into(),
            hinzugefuegt_um: jetzt_ms(),
            grundriss: Grundriss::default(),
        };
        let mut liste = self.liste.lock().unwrap();
        liste.push(def.clone());
        self.sichern(&liste);
        def
    }

    pub fn klon_fortschritt(&self, id: &str, f: KlonFortschritt) {
        if let Some(k) = self.klone.lock().unwrap().get_mut(id) {
            k.fortschritt = Some(f);
        }
    }

    /// Ein Klon ist zu Ende: bei Erfolg wird er eine richtige Etage.
    pub fn klon_beenden(&self, id: &str, erfolg: bool) -> Option<KlonLauf> {
        let k = self.klone.lock().unwrap().remove(id)?;
        if erfolg {
            let mut liste = self.liste.lock().unwrap();
            liste.push(k.def.clone());
            self.sichern(&liste);
        }
        Some(k)
    }

    /// Bricht einen Klon ab, wenn `darf(von)` es erlaubt.
    pub fn klon_abbrechen(&self, id: &str, darf: impl Fn(&str) -> bool) -> Result<(), String> {
        let mut klone = self.klone.lock().unwrap();
        let k = klone.get_mut(id).ok_or("Dieser Klon läuft nicht mehr")?;
        if !darf(&k.von) {
            return Err("Nur Admins oder wer ihn gestartet hat, können den Klon abbrechen".into());
        }
        if let Some(tx) = k.abbruch.take() {
            let _ = tx.send(());
        }
        Ok(())
    }

    /// Nimmt eine Etage aus dem Gebäude; der Checkout bleibt auf der Platte.
    pub fn entfernen(&self, id: &str) -> Option<EtageDef> {
        let mut liste = self.liste.lock().unwrap();
        let i = liste.iter().position(|e| e.id == id)?;
        let e = liste.remove(i);
        self.sichern(&liste);
        Some(e)
    }

    /// Ändert den Grundriss einer Etage.
    pub fn grundriss_aendern(&self, id: &str, f: impl FnOnce(&mut Grundriss) -> bool) -> Option<Grundriss> {
        let mut liste = self.liste.lock().unwrap();
        let e = liste.iter_mut().find(|e| e.id == id)?;
        if !f(&mut e.grundriss) {
            return None;
        }
        let g = e.grundriss.clone();
        self.sichern(&liste);
        Some(g)
    }

    pub fn projekt(&self, id: &str) -> Option<ProjektInfo> {
        let e = self.get(id)?;
        Some(ProjektInfo { name: e.name.clone(), branch: git::branch(&e.ordner), remote: git::remote(&e.ordner), ordner: e.ordner.display().to_string() })
    }

    /// Alle Etagen für den Aufzug, laufende Klone am Ende.
    pub fn infos(&self, personen: &HashMap<String, u32>, zahlen: &dyn Fn(&str) -> Zahlen) -> Vec<EtageInfo> {
        let info = |e: &EtageDef, klon: Option<&KlonLauf>| {
            let z = if klon.is_some() { Zahlen::default() } else { zahlen(&e.id) };
            EtageInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                repo: e.repo.clone(),
                ordner: zum_anzeigen(&e.ordner),
                branch: if klon.is_some() { None } else { git::branch(&e.ordner) },
                palette: e.palette,
                klont: klon.is_some(),
                klon: klon.and_then(|k| k.fortschritt.clone()),
                lokal: e.lokal,
                hinzugefuegt_von: e.hinzugefuegt_von.clone(),
                hinzugefuegt_um: e.hinzugefuegt_um,
                arbeiter: z.arbeiter,
                beschaeftigt: z.beschaeftigt,
                wartend: z.wartend,
                personen: personen.get(&e.id).copied().unwrap_or(0),
                fluegel: e.grundriss.fluegel,
            }
        };
        let mut aus: Vec<EtageInfo> = self.liste.lock().unwrap().iter().map(|e| info(e, None)).collect();
        let klone = self.klone.lock().unwrap();
        let mut laufend: Vec<&KlonLauf> = klone.values().collect();
        laufend.sort_by_key(|k| k.def.hinzugefuegt_um);
        aus.extend(laufend.into_iter().map(|k| info(&k.def, Some(k))));
        aus
    }
}

/// Ein Schild setzen oder (leerer Text) abnehmen. `false`, wenn sich nichts ändert.
pub fn schild_setzen(g: &mut Grundriss, tisch: &str, text: &str, farbe: Option<&str>, von: &str) -> bool {
    let tisch: String = tisch.chars().take(40).collect();
    let text: String = text.trim().chars().take(32).collect();
    if welt::grundriss::tisch_nach_id(&tisch).is_none() {
        return false;
    }
    if text.is_empty() {
        return g.schilder.remove(&tisch).is_some();
    }
    let farbe = farbe.filter(|f| crate::ws::eingabe::ist_farbe(f)).unwrap_or("#f4d35e").to_string();
    g.schilder.insert(tisch, Schild { text, farbe, von: von.into(), um: jetzt_ms() });
    true
}

/// Die Arbeitsplätze (Schreibtische, Sitzsäcke) einer Etage mit diesem Ausbau, für `agenten`.
pub fn arbeitsplaetze(fluegel: u32) -> Vec<String> {
    use welt::grundriss::TischArt;
    welt::grundriss::alle_tische().into_iter().filter(|t| t.fluegel <= fluegel && matches!(t.art, TischArt::Schreibtisch | TischArt::Sitzsack)).map(|t| t.id).collect()
}

/// Hinterbüro um eine Reihe erweitern (`+1`) oder zurückbauen (`-1`).
pub fn fluegel_aendern(g: &mut Grundriss, um: i32) -> bool {
    let neu = (g.fluegel as i32 + um).clamp(0, FLUEGEL_MAX as i32) as u32;
    let anders = neu != g.fluegel;
    g.fluegel = neu;
    anders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klon_ablauf() {
        let dir = tempfile::tempdir().unwrap();
        let e = Etagen::laden(dir.path(), dir.path().join("projekte"));
        let Ok(KlonStart::Klonen(def, _rx)) = e.klon_anmelden("Luna-OS/Office", &dir.path().join("projekte"), "Luna", "v1") else { panic!() };
        assert_eq!(def.id, "office");
        assert!(e.klon_anmelden("luna-os/office", &dir.path().join("projekte"), "Luna", "v1").is_err());
        let infos = e.infos(&HashMap::new(), &|_| Zahlen::default());
        assert!(infos[0].klont);
        assert!(e.klon_abbrechen("office", |von| von == "Jemand").is_err());
        e.klon_beenden("office", true);
        assert!(e.gibt_es("office"));
        let wieder = Etagen::laden(dir.path(), dir.path().join("projekte"));
        assert_eq!(wieder.anzahl(), 1);
        assert_eq!(slug("Mein Projekt!"), "mein-projekt");
    }

    #[test]
    fn grundriss() {
        let mut g = Grundriss::default();
        let tisch = welt::grundriss::alle_tische()[0].id.clone();
        assert!(schild_setzen(&mut g, &tisch, "Backend", Some("#ff0000"), "Luna"));
        assert!(!schild_setzen(&mut g, "gibt-es-nicht", "Backend", None, "Luna"));
        assert!(schild_setzen(&mut g, &tisch, "", None, "Luna"));
        assert!(fluegel_aendern(&mut g, 1));
        assert!(!fluegel_aendern(&mut g, -5) || g.fluegel == 0);
    }
}
