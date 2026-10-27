//! Agent-zu-Agent-Kommunikation: Nachrichten auf den Bus legen und den Empfängern ins Terminal
//! zustellen, den eigenen Status melden, das gemeinsame Board ändern. Browser und CLI nutzen
//! dieselben Funktionen.

use protokoll::ServerNachricht;
use protokoll::agenten::{BusNachricht, Server as AgSrv};
use protokoll::arbeiter::{ArbeiterArt, ArbeiterStatus};
use protokoll::aufgaben::{Aufgabe, AufgabenStatus};

use crate::arbeiter::kuerzen;
use crate::board::{Aenderung, Aufgabenboard, Neu};
use crate::bus::Bus;
use crate::etage::Etage;
use crate::{Agenten, Ziel, Zustand, jetzt_ms, speicher, steuerung};

/// Was beim Senden herauskam.
#[derive(Debug, Clone)]
pub struct Gesendet {
    pub nachricht: BusNachricht,
    /// Namen der Agenten, die sie bekommen.
    pub empfaenger: Vec<String>,
}

/// Legt eine Nachricht auf den Bus der Etage und stellt sie den Agenten zu.
pub fn senden(ag: &Agenten, etage: &str, von: &str, von_arbeiter: Option<&str>, an: Option<&str>, text: &str) -> Result<Gesendet, String> {
    if text.trim().is_empty() {
        return Err("Die Nachricht ist leer".into());
    }
    let mut z = ag.innen.zustand();
    let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let nachricht = e.bus.senden(von, von_arbeiter, an, text, jetzt_ms());
    let empfaenger_ids: Vec<String> = e
        .arbeiter
        .values()
        .filter(|a| a.info.art == ArbeiterArt::Agent && Bus::ist_fuer(&nachricht, &a.info.id, &a.info.name))
        .map(|a| a.info.id.clone())
        .collect();
    let mut empfaenger = Vec::new();
    for id in &empfaenger_ids {
        if let Some(a) = e.arbeiter.get_mut(id) {
            a.ausstehend.push_back(nachricht.clone());
            empfaenger.push(a.info.name.clone());
        }
    }
    ag.innen.senden(Ziel::Etage(etage.to_string()), ServerNachricht::Agenten(AgSrv::Nachricht { nachricht: nachricht.clone() }));
    speicher::bus_speichern(&ag.innen.daten, e);
    for id in &empfaenger_ids {
        zustellen(ag, &mut z, etage, id);
    }
    Ok(Gesendet { nachricht, empfaenger })
}

/// Wie eine Nachricht im Terminal eines Agenten ankommt.
pub fn als_eingabe(nachrichten: &[BusNachricht]) -> String {
    let mut text: Vec<String> = nachrichten
        .iter()
        .map(|n| match &n.an {
            Some(_) => format!("[Nachricht von {}] {}", n.von, n.text),
            None => format!("[Nachricht von {} an alle] {}", n.von, n.text),
        })
        .collect();
    let absender = nachrichten.last().map(|n| n.von.as_str()).unwrap_or("…");
    text.push(format!("(Antworten mit: office-agenten senden {absender} \"…\")"));
    text.join("\n\n")
}

/// Tippt einem Agenten seine wartenden Nachrichten ein, wenn er gerade Zeit hat.
pub fn zustellen(ag: &Agenten, z: &mut Zustand, etage: &str, id: &str) {
    let Some(e) = z.etagen.get_mut(etage) else { return };
    let Some(a) = e.arbeiter.get_mut(id) else { return };
    if a.ausstehend.is_empty() || !a.hat_zeit() || a.prompt_tippen.is_some() {
        return;
    }
    let nachrichten: Vec<BusNachricht> = a.ausstehend.drain(..).collect();
    let bis = nachrichten.iter().map(|n| n.id).max().unwrap_or(0);
    steuerung::tippen(&ag.innen, a, &als_eingabe(&nachrichten));
    a.info.aktivitaet = Some(kuerzen(&format!("Nachricht von {}", nachrichten[nachrichten.len() - 1].von), 80));
    a.status_setzen(ArbeiterStatus::Arbeitet);
    e.bus.gelesen_bis(id, bis);
    ag.innen.arbeiter_melden(e, id);
}

/// Die ungelesenen Nachrichten eines Agenten (über die CLI); sie gelten dann als zugestellt.
pub fn posteingang(ag: &Agenten, etage: &str, id: &str) -> Vec<BusNachricht> {
    let mut z = ag.innen.zustand();
    let Some(e) = z.etagen.get_mut(etage) else { return Vec::new() };
    let Some(name) = e.arbeiter.get(id).map(|a| a.info.name.clone()) else { return Vec::new() };
    let neu = e.bus.posteingang(id, &name);
    if let Some(a) = e.arbeiter.get_mut(id) {
        a.ausstehend.clear();
    }
    speicher::bus_speichern(&ag.innen.daten, e);
    neu
}

/// Ein Agent meldet, woran er arbeitet.
pub fn status_melden(ag: &Agenten, etage: &str, id: &str, text: &str) -> Result<(), String> {
    let mut z = ag.innen.zustand();
    let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let a = e.arbeiter.get_mut(id).ok_or("Diesen Arbeiter gibt es nicht")?;
    let t = kuerzen(text, 200);
    a.notiz = (!t.is_empty()).then_some(t);
    a.feed_um = jetzt_ms();
    ag.innen.arbeiter_melden(e, id);
    speicher::arbeiter_speichern(&ag.innen.daten, e);
    Ok(())
}

/// Sagt einem Agenten, dass ihm eine Aufgabe zugewiesen wurde (wenn das nicht er selbst war).
fn zuweisung_melden(ag: &Agenten, z: &mut Zustand, etage: &str, aufgabe: &Aufgabe, von: &str) {
    let Some(besitzer) = aufgabe.besitzer.clone() else { return };
    let Some(e) = z.etagen.get_mut(etage) else { return };
    let Some(ziel) = e.finden(&besitzer).filter(|a| !a.info.name.eq_ignore_ascii_case(von)).map(|a| a.info.id.clone()) else { return };
    let text = format!("Dir wurde Aufgabe #{} „{}“ zugewiesen (von {von}). Details: office-agenten aufgaben", aufgabe.id, aufgabe.titel);
    let n = e.bus.senden("Aufgabenboard", None, Some(&besitzer), &text, jetzt_ms());
    if let Some(a) = e.arbeiter.get_mut(&ziel) {
        a.ausstehend.push_back(n.clone());
    }
    ag.innen.senden(Ziel::Etage(etage.to_string()), ServerNachricht::Agenten(AgSrv::Nachricht { nachricht: n }));
    zustellen(ag, z, etage, &ziel);
}

fn board_fertig(ag: &Agenten, e: &Etage) {
    ag.innen.board_melden(e);
    speicher::board_speichern(&ag.innen.daten, e);
}

pub fn aufgabe_neu(ag: &Agenten, etage: &str, von: &str, neu: Neu) -> Result<Aufgabe, String> {
    let automatisch = neu.automatisch;
    let aufgabe = {
        let mut z = ag.innen.zustand();
        let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
        let a = e.board.hinzufuegen(neu, von, jetzt_ms())?.clone();
        board_fertig(ag, e);
        zuweisung_melden(ag, &mut z, etage, &a, von);
        a
    };
    if automatisch {
        crate::browser::warteschlange_anstossen(ag, etage);
    }
    Ok(aufgabe)
}

pub fn aufgabe_aendern(ag: &Agenten, etage: &str, von: &str, id: u64, aenderung: Aenderung) -> Result<Aufgabe, String> {
    let mut z = ag.innen.zustand();
    let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let vorher = e.board.holen(id).and_then(|a| a.besitzer.clone());
    let a = e.board.aendern(id, aenderung, jetzt_ms())?.clone();
    board_fertig(ag, e);
    if a.besitzer != vorher {
        zuweisung_melden(ag, &mut z, etage, &a, von);
    }
    Ok(a)
}

/// Eine andere Board-Änderung (Entfernen, Verschieben, Leeren, Limit).
pub fn board_aendern<T>(ag: &Agenten, etage: &str, f: impl FnOnce(&mut Aufgabenboard) -> T) -> Result<T, String> {
    let mut z = ag.innen.zustand();
    let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let r = f(&mut e.board);
    board_fertig(ag, e);
    Ok(r)
}

/// Ein Agent mit einer automatischen Aufgabe hat seinen Zug beendet: die Aufgabe ist erledigt.
pub fn automatische_erledigt(ag: &Agenten, z: &mut Zustand, etage: &str, id: &str) -> bool {
    let Some(e) = z.etagen.get_mut(etage) else { return false };
    let jetzt = jetzt_ms();
    let mut geaendert = false;
    for a in e.board.board.aufgaben.iter_mut() {
        if a.automatisch && a.arbeiter.as_deref() == Some(id) && a.status == AufgabenStatus::InArbeit {
            a.status = AufgabenStatus::Erledigt;
            a.geaendert_um = jetzt;
            geaendert = true;
        }
    }
    if geaendert {
        board_fertig(ag, e);
    }
    geaendert
}
