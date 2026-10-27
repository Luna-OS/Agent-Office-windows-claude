//! Was Menschen (und andere Agenten) mit einem laufenden Arbeiter tun: Aufträge tippen, ins
//! Terminal schreiben, zuschauen, die Größe ändern, heimschicken.

use std::time::Duration;

use protokoll::ServerNachricht;
use protokoll::agenten::Server as AgSrv;
use protokoll::arbeiter::{ArbeiterArt, Aufraeumen, LetzteEingabe, Server as ArbSrv};

use crate::arbeiter::{Arbeiter, kuerzen};
use crate::{Agenten, Innen, Ziel, jetzt_ms, speicher, worktree};

/// Tippt einen Text als einen Auftrag in die Sitzung: als Bracketed Paste (mehrzeilig bleibt eine
/// Nachricht), dann Enter.
pub fn tippen(innen: &Innen, a: &mut Arbeiter, text: &str) {
    let sauber = text.replace("\r\n", "\n").replace('\r', "\n");
    let sauber = sauber.trim();
    let Some(pty) = a.pty.as_mut() else { return };
    if pty.schreiben(format!("\x1b[200~{sauber}\x1b[201~").as_bytes()).is_err() {
        return;
    }
    a.letzte_ausgabe = Some(std::time::Instant::now());
    let (etage, id, lauf) = (a.info.etage.clone(), a.info.id.clone(), a.lauf);
    let schwach = innen.selbst();
    innen.laufzeit.spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let Some(innen) = schwach.upgrade() else { return };
        let mut z = innen.zustand();
        if let Some(a) = z.arbeiter_mut(&etage, &id)
            && a.lauf == lauf
            && let Some(pty) = a.pty.as_mut()
        {
            let _ = pty.schreiben(b"\r");
        }
    });
}

/// Ein Auftrag für einen Arbeiter, von einem Menschen oder Agenten.
pub fn prompt(ag: &Agenten, etage: &str, id: &str, text: &str, von: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Leerer Auftrag".into());
    }
    let laeuft = {
        let z = ag.innen.zustand();
        let a = z.etagen.get(etage).and_then(|e| e.arbeiter.get(id)).ok_or("Diesen Arbeiter gibt es nicht")?;
        if a.info.art != ArbeiterArt::Agent {
            return Err(format!("{} ist eine Shell, kein Agent", a.info.name));
        }
        a.laeuft()
    };
    if !laeuft {
        // Gestoppt oder schlafend: er wacht mit diesem Auftrag auf.
        return crate::start::fortsetzen(ag, etage, id, Some(text.to_string()));
    }
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return Err("Diesen Arbeiter gibt es nicht".into()) };
    tippen(&ag.innen, a, text);
    a.info.aktivitaet = Some(kuerzen(text, 80));
    a.info.letzte_eingabe = Some(LetzteEingabe { von: von.to_string(), um: jetzt_ms() });
    ag.innen.arbeiter_melden(&z.etagen[etage], id);
    Ok(())
}

pub fn eingabe(ag: &Agenten, etage: &str, id: &str, daten: &str, von: &str) {
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    let Some(pty) = a.pty.as_mut() else { return };
    let _ = pty.schreiben(daten.as_bytes());
    let jetzt = jetzt_ms();
    // Höchstens einmal pro Sekunde melden, wer tippt.
    if a.info.letzte_eingabe.as_ref().is_none_or(|l| l.von != von || jetzt - l.um > 1000) {
        a.info.letzte_eingabe = Some(LetzteEingabe { von: von.to_string(), um: jetzt });
        ag.innen.arbeiter_melden(&z.etagen[etage], id);
    }
}

pub fn groesse(ag: &Agenten, etage: &str, id: &str, spalten: u16, zeilen: u16) {
    let (spalten, zeilen) = (spalten.clamp(20, 500), zeilen.clamp(5, 200));
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    if (a.info.spalten, a.info.zeilen) == (spalten, zeilen) {
        return;
    }
    a.info.spalten = spalten;
    a.info.zeilen = zeilen;
    a.terminal.groesse_aendern(spalten, zeilen);
    if let Some(pty) = a.pty.as_ref() {
        let _ = pty.groesse(spalten, zeilen);
    }
}

/// Ein Mensch öffnet das Terminal: er bekommt den Bildschirm und danach die laufende Ausgabe.
pub fn anschauen(ag: &Agenten, etage: &str, id: &str, verbindung: &str, name: &str) {
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    a.zuschauer.insert(verbindung.to_string(), name.to_string());
    a.zuschauer_aktualisieren();
    a.info.gesehen = true;
    let (spalten, zeilen) = a.terminal.groesse();
    let snapshot = ArbSrv::TerminalSnapshot { arbeiter: id.to_string(), daten: a.terminal.snapshot(), spalten, zeilen };
    ag.innen.senden(Ziel::Verbindung(verbindung.to_string()), ServerNachricht::Arbeiter(snapshot));
    ag.innen.arbeiter_melden(&z.etagen[etage], id);
}

pub fn wegschauen(ag: &Agenten, etage: &str, id: &str, verbindung: &str) {
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    if a.zuschauer.remove(verbindung).is_some() {
        a.zuschauer_aktualisieren();
        ag.innen.arbeiter_melden(&z.etagen[etage], id);
    }
}

pub fn verbindung_weg(ag: &Agenten, verbindung: &str) {
    let mut z = ag.innen.zustand();
    for e in z.etagen.values_mut() {
        let geaendert: Vec<String> = e
            .arbeiter
            .values_mut()
            .filter_map(|a| {
                a.zuschauer.remove(verbindung)?;
                a.zuschauer_aktualisieren();
                Some(a.info.id.clone())
            })
            .collect();
        for id in geaendert {
            ag.innen.arbeiter_melden(e, &id);
        }
    }
}

/// Schickt einen Arbeiter heim: Terminal beenden, Worktree nach Wunsch aufräumen, vergessen.
pub async fn stoppen(ag: &Agenten, etage: &str, id: &str, aufraeumen: Aufraeumen) -> Result<Option<String>, String> {
    let (projekt, wt) = {
        let mut z = ag.innen.zustand();
        let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
        let mut a = e.arbeiter.remove(id).ok_or("Diesen Arbeiter gibt es nicht")?;
        a.lauf += 1;
        if let Some(mut pty) = a.pty.take() {
            pty.beenden();
        }
        e.bus.vergessen(id);
        if e.kerker_aktiv {
            crate::kerker::einsperren(&mut e.kerker, &a.info, jetzt_ms());
            crate::kerker::speichern(&speicher::ordner(&ag.innen.daten, etage), &e.kerker);
            let stand = e.kerker.clone();
            ag.innen.senden(Ziel::Etage(etage.to_string()), ServerNachricht::Agenten(AgSrv::Kerker { stand }));
        }
        // Seine automatischen Aufgaben gehen zurück aufs Board.
        for auf in e.board.board.aufgaben.iter_mut().filter(|t| t.arbeiter.as_deref() == Some(id)) {
            auf.arbeiter = None;
            if auf.status == protokoll::aufgaben::AufgabenStatus::InArbeit {
                auf.status = protokoll::aufgaben::AufgabenStatus::Offen;
            }
        }
        let ziel = Ziel::Etage(etage.to_string());
        ag.innen.senden(ziel.clone(), ServerNachricht::Arbeiter(ArbSrv::Entfernt { arbeiter: id.to_string() }));
        ag.innen.senden(ziel, ServerNachricht::Agenten(AgSrv::FeedEntfernt { arbeiter: id.to_string() }));
        ag.innen.board_melden(e);
        speicher::speichern(&ag.innen.daten, e);
        (e.verzeichnis.clone(), a.info.worktree.clone())
    };
    let Some(wt) = wt else { return Ok(None) };
    // Windows gibt Dateien erst frei, wenn der Prozess wirklich weg ist.
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(worktree::entfernen(&projekt, &wt, aufraeumen).await)
}
