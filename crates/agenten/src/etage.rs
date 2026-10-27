//! Eine Etage: ein Projekt mit seinen Arbeitern, seinem Bus, seinem Board und seinem GitHub-Stand.

use std::collections::BTreeMap;
use std::path::PathBuf;

use protokoll::ServerNachricht;
use protokoll::agenten::Server as AgSrv;
use protokoll::arbeiter::{ArbeiterStatus, Server as ArbSrv};
use protokoll::aufgaben::Server as AufSrv;

use crate::arbeiter::Arbeiter;
use crate::board::Aufgabenboard;
use crate::bus::Bus;
use crate::github::GithubStand;
use crate::{Agenten, Innen, Ziel, Zustand, speicher};

pub struct Etage {
    pub id: String,
    pub verzeichnis: PathBuf,
    pub arbeiter: BTreeMap<String, Arbeiter>,
    pub bus: Bus,
    pub board: Aufgabenboard,
    /// Die Sitzplätze, wie sie der Server kennt (für „nächster freier Tisch“).
    pub tische: Vec<String>,
    pub github: GithubStand,
}

impl Etage {
    pub fn tisch_belegt(&self, tisch: &str) -> bool {
        self.arbeiter.values().any(|a| a.info.tisch == tisch)
    }

    pub fn freier_tisch(&self) -> Option<String> {
        self.tische.iter().find(|t| !self.tisch_belegt(t)).cloned()
    }

    /// Ein Arbeiter nach ID oder Name (Groß-/Kleinschreibung egal).
    pub fn finden(&self, schluessel: &str) -> Option<&Arbeiter> {
        let s = schluessel.trim().trim_start_matches('@');
        self.arbeiter.get(s).or_else(|| self.arbeiter.values().find(|a| a.info.name.eq_ignore_ascii_case(s)))
    }

    pub fn id_von(&self, schluessel: &str) -> Option<String> {
        self.finden(schluessel).map(|a| a.info.id.clone())
    }
}

impl Zustand {
    /// Die Etage, auf der ein Arbeiter sitzt.
    pub fn etage_von(&self, arbeiter: &str) -> Option<String> {
        self.etagen.values().find(|e| e.arbeiter.contains_key(arbeiter)).map(|e| e.id.clone())
    }

    pub fn arbeiter_mut(&mut self, etage: &str, arbeiter: &str) -> Option<&mut Arbeiter> {
        self.etagen.get_mut(etage)?.arbeiter.get_mut(arbeiter)
    }
}

impl Innen {
    /// Sagt der Etage, wie es einem Arbeiter jetzt geht (Karte und Feed).
    pub(crate) fn arbeiter_melden(&self, e: &Etage, id: &str) {
        let Some(a) = e.arbeiter.get(id) else { return };
        let ziel = Ziel::Etage(e.id.clone());
        self.senden(ziel.clone(), ServerNachricht::Arbeiter(ArbSrv::Aktualisiert { arbeiter: Box::new(a.info.clone()) }));
        self.senden(ziel, ServerNachricht::Agenten(AgSrv::Feed { eintrag: a.feed_eintrag() }));
    }

    pub(crate) fn board_melden(&self, e: &Etage) {
        self.senden(Ziel::Etage(e.id.clone()), ServerNachricht::Aufgaben(AufSrv::Stand { board: e.board.board.clone() }));
    }
}

/// Lädt eine Etage samt ihrer gespeicherten Arbeiter (die dann offline sind, bis jemand sie fortsetzt).
pub fn anmelden(ag: &Agenten, id: &str, verzeichnis: PathBuf) {
    let mut z = ag.innen.zustand();
    if let Some(e) = z.etagen.get_mut(id) {
        e.verzeichnis = verzeichnis;
        return;
    }
    let geladen = speicher::laden(&ag.innen.daten, id);
    let mut arbeiter = BTreeMap::new();
    for mut info in geladen.arbeiter {
        info.etage = id.to_string();
        info.zuschauer.clear();
        if !matches!(info.status, ArbeiterStatus::Beendet) {
            info.status = ArbeiterStatus::Offline;
        }
        arbeiter.insert(info.id.clone(), Arbeiter::neu(info));
    }
    for (aid, a) in arbeiter.iter_mut() {
        if let Some(n) = geladen.notizen.get(aid) {
            a.notiz = Some(n.clone());
        }
        if let Some(s) = geladen.puffer.get(aid) {
            a.terminal.einspeisen(s.as_bytes());
        }
    }
    let etage = Etage {
        id: id.to_string(),
        verzeichnis,
        arbeiter,
        bus: geladen.bus,
        board: geladen.board,
        tische: Vec::new(),
        github: GithubStand::default(),
    };
    z.etagen.insert(id.to_string(), etage);
    drop(z);
    crate::github::aktualisieren_spaeter(ag.clone(), id.to_string());
}

/// Stoppt die Terminals einer Etage und speichert sie.
pub fn abmelden(ag: &Agenten, id: &str) {
    let mut z = ag.innen.zustand();
    let Some(mut e) = z.etagen.remove(id) else { return };
    drop(z);
    speicher::speichern(&ag.innen.daten, &e);
    for a in e.arbeiter.values_mut() {
        a.lauf += 1;
        if let Some(mut pty) = a.pty.take() {
            pty.beenden();
        }
    }
}
