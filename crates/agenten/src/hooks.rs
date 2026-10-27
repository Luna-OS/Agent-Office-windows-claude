//! Hook-Ereignisse der Agenten (über `office-agenten hook <provider> <ereignis>`): ihr Status
//! (startet, arbeitet, wartet, fertig), was sie gerade tun, ihre Sitzung.

use protokoll::arbeiter::{Aktion, ArbeiterStatus};
use serde_json::Value;

use crate::arbeiter::{FEHLSCHLAEGE_BIS_VERZWEIFLUNG, aufgabe_aus_prompt, kuerzen};
use crate::provider::{self, HookEreignis};
use crate::{Agenten, jetzt_ms, kommunikation, speicher, steuerung, worktree};

/// Prüft Arbeiter-ID und Token; gibt die Etage zurück.
pub fn ausweisen(ag: &Agenten, arbeiter: &str, token: &str) -> Option<String> {
    let z = ag.innen.zustand();
    let etage = z.etage_von(arbeiter)?;
    let a = &z.etagen[&etage].arbeiter[arbeiter];
    (!a.token.is_empty() && gleich(a.token.as_bytes(), token.as_bytes())).then_some(etage)
}

/// Vergleich in konstanter Zeit, damit das Token nicht Zeichen für Zeichen erraten werden kann.
fn gleich(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Ein Hook-Ereignis eines Arbeiters. Sagt, ob es angenommen wurde.
pub fn verarbeiten(ag: &Agenten, route: &str, arbeiter: &str, token: &str, ereignis: &str, nutzlast: &Value) -> bool {
    let Some(etage) = ausweisen(ag, arbeiter, token) else { return false };
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(&etage, arbeiter) else { return false };
    if !a.laeuft() {
        return false;
    }
    let Some(p) = provider::finden(route) else { return false };
    let mut speichern = false;
    let mut stopp = false;
    let mut dateien = false;
    for ev in p.hook(ereignis, nutzlast) {
        match ev {
            HookEreignis::Sitzung(s) => {
                if a.info.sitzung.as_deref() != Some(&s) {
                    a.info.sitzung = Some(s);
                    speichern = true;
                }
            }
            HookEreignis::Transkript(t) => a.transkript = Some(t.into()),
            HookEreignis::SitzungStart { geleert } => {
                if geleert {
                    a.info.aufgabe = None;
                    a.fehlschlaege = 0;
                }
                if a.info.status == ArbeiterStatus::Startet || (a.info.status == ArbeiterStatus::Wartet && a.info.hinweis.is_some()) {
                    a.status_setzen(ArbeiterStatus::Bereit);
                }
                if let Some(text) = a.prompt_tippen.take() {
                    steuerung::tippen(&ag.innen, a, &text);
                    a.status_setzen(ArbeiterStatus::Arbeitet);
                }
            }
            HookEreignis::Prompt(text) => {
                a.info.hinweis = None;
                if !text.is_empty() {
                    a.info.aktivitaet = Some(kuerzen(&text, 80));
                }
                if a.info.aufgabe.is_none() {
                    a.info.aufgabe = aufgabe_aus_prompt(&text);
                }
                a.status_setzen(ArbeiterStatus::Arbeitet);
                a.info.aktion = None;
            }
            HookEreignis::WerkzeugVor { beschreibung, aktion } => {
                a.info.aktivitaet = Some(beschreibung);
                a.status_setzen(ArbeiterStatus::Arbeitet);
                if a.info.aktion != Some(Aktion::Scheitert) || aktion != Some(Aktion::Testen) {
                    a.info.aktion = aktion;
                }
            }
            HookEreignis::WerkzeugNach { fehlgeschlagen, test } => {
                if test {
                    if fehlgeschlagen {
                        a.fehlschlaege += 1;
                        if a.fehlschlaege >= FEHLSCHLAEGE_BIS_VERZWEIFLUNG {
                            a.info.aktion = Some(Aktion::Scheitert);
                        }
                    } else {
                        a.fehlschlaege = 0;
                        if a.info.aktion == Some(Aktion::Scheitert) {
                            a.info.aktion = None;
                        }
                    }
                }
                // Eine Erlaubnis wurde gegeben: er arbeitet weiter.
                if a.info.status == ArbeiterStatus::Wartet {
                    a.status_setzen(ArbeiterStatus::Arbeitet);
                }
                dateien = true;
            }
            HookEreignis::BrauchtMenschen(was) => {
                if a.info.status != ArbeiterStatus::Wartet {
                    let text = format!("{} wartet auf einen Menschen: {was}", a.info.name);
                    ag.innen.melden(&etage, &a.info.name, crate::MeldungArt::Wartet, text);
                }
                a.info.aktivitaet = Some(was);
                a.status_setzen(ArbeiterStatus::Wartet);
            }
            HookEreignis::Leerlauf => {}
            HookEreignis::Stopp => {
                if a.info.status != ArbeiterStatus::Fertig {
                    let was = a.info.aufgabe.as_ref().map(|t| format!(": {}", t.name)).unwrap_or_default();
                    ag.innen.melden(&etage, &a.info.name, crate::MeldungArt::Fertig, format!("{} ist fertig{was}", a.info.name));
                }
                a.status_setzen(ArbeiterStatus::Fertig);
                stopp = true;
                dateien = true;
            }
        }
    }
    a.feed_um = jetzt_ms();
    if stopp {
        kommunikation::automatische_erledigt(ag, &mut z, &etage, arbeiter);
        kommunikation::zustellen(ag, &mut z, &etage, arbeiter);
    }
    let e = &z.etagen[&etage];
    ag.innen.arbeiter_melden(e, arbeiter);
    if speichern || stopp {
        speicher::arbeiter_speichern(&ag.innen.daten, e);
    }
    drop(z);
    if dateien {
        dateien_auffrischen(ag, &etage, arbeiter);
    }
    if stopp {
        crate::browser::warteschlange_anstossen(ag, &etage);
        verbrauch_auffrischen(ag, &etage, arbeiter);
    }
    true
}

/// Liest den Verbrauch aus seinem Transkript neu.
pub fn verbrauch_auffrischen(ag: &Agenten, etage: &str, id: &str) {
    let Some(pfad) = ag.innen.zustand().arbeiter_mut(etage, id).and_then(|a| a.transkript.clone()) else { return };
    let (ag, etage, id) = (ag.clone(), etage.to_string(), id.to_string());
    ag.innen.laufzeit.clone().spawn(async move {
        let Ok(Some(v)) = tokio::task::spawn_blocking(move || crate::verbrauch::lesen(&pfad)).await else { return };
        let mut z = ag.innen.zustand();
        let Some(a) = z.arbeiter_mut(&etage, &id) else { return };
        if a.info.verbrauch.as_ref() != Some(&v) {
            let alt = a.info.verbrauch.replace(v.clone());
            ag.innen.arbeiter_melden(&z.etagen[&etage], &id);
            drop(z);
            crate::nutzung::buchen(&ag, alt.as_ref(), &v);
        }
    });
}

/// Liest die geänderten Dateien seines Worktrees neu (für den Status-Feed).
pub fn dateien_auffrischen(ag: &Agenten, etage: &str, id: &str) {
    let pfad = {
        let z = ag.innen.zustand();
        let Some(e) = z.etagen.get(etage) else { return };
        let Some(wt) = e.arbeiter.get(id).and_then(|a| a.info.worktree.clone()) else { return };
        worktree::absolut(&e.verzeichnis, &wt.pfad)
    };
    let (ag, etage, id) = (ag.clone(), etage.to_string(), id.to_string());
    ag.innen.laufzeit.clone().spawn(async move {
        let Ok(dateien) = worktree::geaenderte_dateien(&pfad).await else { return };
        let branch = worktree::aktueller_branch(&pfad).await;
        let mut z = ag.innen.zustand();
        let Some(a) = z.arbeiter_mut(&etage, &id) else { return };
        let dateien: Vec<String> = dateien.into_iter().take(50).collect();
        let mut geaendert = a.dateien != dateien;
        a.dateien = dateien;
        if let (Some(b), Some(wt)) = (branch, a.info.worktree.as_mut())
            && wt.branch != b
        {
            wt.branch = b;
            geaendert = true;
        }
        if geaendert {
            ag.innen.arbeiter_melden(&z.etagen[&etage], &id);
        }
    });
}
