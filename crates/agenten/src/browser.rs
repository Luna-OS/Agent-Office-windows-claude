//! Nachrichten aus dem Browser (Domänen agenten, arbeiter, aufgaben, github), die Begrüßung beim
//! Betreten einer Etage und die Warteschlange, die automatische Aufgaben an neue Arbeiter vergibt.

use protokoll::agenten::{Client as AgCl, Server as AgSrv};
use protokoll::arbeiter::{Client as ArbCl, Server as ArbSrv};
use protokoll::aufgaben::{AufgabenStatus, Client as AufCl, Server as AufSrv};
use protokoll::github::Client as GhCl;
use protokoll::{ClientNachricht, ServerNachricht};

use crate::board::{Aenderung, Neu};
use crate::{Absender, Agenten, STANDARD_ETAGE, Ziel, github, kommunikation, provider, start, steuerung, worktree};

fn etage_von(von: &Absender) -> String {
    von.etage.clone().unwrap_or_else(|| STANDARD_ETAGE.to_string())
}

/// Was fürs ganze Gebäude gilt (beim Verbinden).
pub fn begruessung(ag: &Agenten) -> Vec<ServerNachricht> {
    let mut z = ag.innen.zustand();
    // Die Limits liest das Office erst, wenn jemand da ist, der sie sieht.
    if !z.limits_laufen {
        z.limits_laufen = true;
        crate::nutzung::starten(ag);
    }
    vec![
        ServerNachricht::Arbeiter(ArbSrv::Provider { provider: provider::uebersicht(), standard: provider::STANDARD.into() }),
        crate::nach_merge::nachricht(&z.nach_merge),
        ServerNachricht::Agenten(AgSrv::Nutzung { stand: z.nutzung.clone() }),
        ServerNachricht::Agenten(AgSrv::Limits { stand: z.limits.clone() }),
        ServerNachricht::Agenten(AgSrv::Prompts { liste: crate::prompts::liste(&z.prompts) }),
    ]
}

/// Der Stand einer Etage (beim Betreten).
pub fn etage_betreten(ag: &Agenten, von: &Absender) -> Vec<ServerNachricht> {
    let etage = etage_von(von);
    let z = ag.innen.zustand();
    let mut aus = Vec::new();
    let Some(e) = z.etagen.get(&etage) else { return aus };
    aus.push(ServerNachricht::Arbeiter(ArbSrv::Liste { arbeiter: e.arbeiter.values().map(|a| a.info.clone()).collect() }));
    aus.push(ServerNachricht::Agenten(AgSrv::Stand {
        nachrichten: e.bus.verlauf(100),
        feed: e.arbeiter.values().map(|a| a.feed_eintrag()).collect(),
    }));
    aus.push(ServerNachricht::Aufgaben(AufSrv::Stand { board: e.board.board.clone() }));
    aus.push(ServerNachricht::Agenten(AgSrv::Dienste { liste: e.dienste.clone() }));
    aus.push(ServerNachricht::Agenten(AgSrv::Kerker { stand: e.kerker.clone() }));
    aus.push(ServerNachricht::Agenten(AgSrv::Besprechung { stand: e.besprechung.clone() }));
    aus.push(e.github.nachricht());
    aus
}

pub async fn verarbeiten(ag: &Agenten, von: &Absender, n: ClientNachricht) {
    let etage = etage_von(von);
    let ergebnis = match n {
        ClientNachricht::Arbeiter(m) => arbeiter(ag, von, &etage, m).await,
        ClientNachricht::Agenten(AgCl::BesprechungStarten { wunsch }) => {
            crate::besprechung::starten(ag, &etage, &von.name, von.konto.clone(), *wunsch).await
        }
        ClientNachricht::Agenten(AgCl::BesprechungStoppen) => {
            crate::besprechung::anhalten(ag, &etage, &format!("Angehalten von {}", von.name));
            Ok(())
        }
        ClientNachricht::Agenten(AgCl::BesprechungAbraeumen) => crate::besprechung::abraeumen(ag, &etage).await,
        ClientNachricht::Agenten(m) => agenten(ag, von, &etage, m),
        ClientNachricht::Aufgaben(m) => aufgaben(ag, von, &etage, m),
        ClientNachricht::Github(m) => github_nachricht(ag, von, &etage, m).await,
        _ => Ok(()),
    };
    if let Err(f) = ergebnis {
        ag.innen.fehler(&von.verbindung, f);
    }
}

/// Die ID eines Arbeiters auf dieser Etage (oder ein Fehler).
fn arbeiter_id(ag: &Agenten, etage: &str, id: &str) -> Result<String, String> {
    let z = ag.innen.zustand();
    z.etagen.get(etage).and_then(|e| e.id_von(id)).ok_or_else(|| "Diesen Arbeiter gibt es auf dieser Etage nicht".into())
}

async fn arbeiter(ag: &Agenten, von: &Absender, etage: &str, m: ArbCl) -> Result<(), String> {
    match m {
        ArbCl::Anlegen { tisch, prompt, worktree, art, provider, modell, aufwand, issue } => {
            let prompt = match (prompt.filter(|p| !p.trim().is_empty()), issue) {
                (p, Some(n)) => Some(issue_auftrag(ag, etage, n, p)),
                (p, None) => p,
            };
            let wunsch = start::Wunsch { tisch: Some(tisch), prompt, worktree, art, provider, modell, aufwand, konto: von.konto.clone(), ..Default::default() };
            start::anlegen(ag, etage, &von.name, wunsch).await?;
            if let Some(n) = issue {
                issue_uebernehmen(ag, etage, n);
            }
        }
        ArbCl::Fortsetzen { arbeiter } => start::fortsetzen(ag, etage, &arbeiter_id(ag, etage, &arbeiter)?, None)?,
        ArbCl::Stoppen { arbeiter, aufraeumen } => {
            let id = arbeiter_id(ag, etage, &arbeiter)?;
            if let Some(f) = steuerung::stoppen(ag, etage, &id, aufraeumen).await? {
                ag.innen.fehler(&von.verbindung, format!("Aufräumen unvollständig: {f}"));
            }
            warteschlange_anstossen(ag, etage);
        }
        ArbCl::WorktreeAbfragen { arbeiter } => {
            let id = arbeiter_id(ag, etage, &arbeiter)?;
            let (dir, wt) = {
                let z = ag.innen.zustand();
                let e = &z.etagen[etage];
                (e.verzeichnis.clone(), e.arbeiter.get(&id).and_then(|a| a.info.worktree.clone()))
            };
            let stand = match wt {
                Some(wt) => worktree::stand(&dir, &wt).await,
                None => Default::default(),
            };
            ag.innen.senden(Ziel::Verbindung(von.verbindung.clone()), ServerNachricht::Arbeiter(ArbSrv::WorktreeStand { arbeiter: id, stand }));
        }
        ArbCl::Anschauen { arbeiter } => steuerung::anschauen(ag, etage, &arbeiter_id(ag, etage, &arbeiter)?, &von.verbindung, &von.name),
        ArbCl::Wegschauen { arbeiter } => steuerung::wegschauen(ag, etage, &arbeiter_id(ag, etage, &arbeiter)?, &von.verbindung),
        ArbCl::Prompt { arbeiter, text } => steuerung::prompt(ag, etage, &arbeiter_id(ag, etage, &arbeiter)?, &text, &von.name)?,
        ArbCl::PrOeffnen { arbeiter } => pr_oeffnen(ag, etage, &arbeiter_id(ag, etage, &arbeiter)?).await?,
        ArbCl::Eingabe { arbeiter, daten } => steuerung::eingabe(ag, etage, &arbeiter, &daten, &von.name),
        ArbCl::Groesse { arbeiter, spalten, zeilen } => steuerung::groesse(ag, etage, &arbeiter, spalten, zeilen),
        ArbCl::StationAuftrag { tisch, text } => crate::stationen::auftrag(ag, etage, &von.name, von.konto.clone(), &tisch, &text).await?,
        ArbCl::NachMergeGehen { an } => crate::nach_merge::setzen(ag, von, an),
        ArbCl::AenderungenAbfragen { arbeiter } => {
            crate::aenderungen::stand_senden(ag, &von.verbindung, etage, &arbeiter_id(ag, etage, &arbeiter)?).await?
        }
        ArbCl::DiffAbfragen { ref arbeiter, .. } | ArbCl::Committen { ref arbeiter, .. } | ArbCl::Verwerfen { ref arbeiter, .. } => {
            let id = arbeiter_id(ag, etage, arbeiter)?;
            crate::aenderungen::verarbeiten(ag, &von.verbindung, etage, &id, m).await?
        }
        ArbCl::ProviderAbfragen => ag.innen.senden(
            Ziel::Verbindung(von.verbindung.clone()),
            ServerNachricht::Arbeiter(ArbSrv::Provider { provider: provider::uebersicht(), standard: provider::STANDARD.into() }),
        ),
    }
    Ok(())
}

fn agenten(ag: &Agenten, von: &Absender, etage: &str, m: AgCl) -> Result<(), String> {
    match m {
        AgCl::Senden { an, text } => {
            kommunikation::senden(ag, etage, &von.name, None, an.as_deref(), &text)?;
        }
        AgCl::PromptSetzen { id, text } => {
            let mut z = ag.innen.zustand();
            crate::prompts::setzen(&mut z.prompts, &id, text)?;
            crate::prompts::speichern(&ag.innen.daten, &z.prompts);
            ag.innen.senden(Ziel::Alle, ServerNachricht::Agenten(AgSrv::Prompts { liste: crate::prompts::liste(&z.prompts) }));
        }
        AgCl::BesprechungStarten { .. } | AgCl::BesprechungStoppen | AgCl::BesprechungAbraeumen => {}
        AgCl::LimitsAuffrischen => {
            let ag = ag.clone();
            ag.innen.laufzeit.clone().spawn(async move { crate::nutzung::limits_auffrischen(&ag).await });
        }
        AgCl::Abfragen => {
            for n in etage_betreten(ag, von).into_iter().filter(|n| matches!(n, ServerNachricht::Agenten(_))) {
                ag.innen.senden(Ziel::Verbindung(von.verbindung.clone()), n);
            }
        }
    }
    Ok(())
}

fn aufgaben(ag: &Agenten, von: &Absender, etage: &str, m: AufCl) -> Result<(), String> {
    match m {
        AufCl::Hinzufuegen { titel, notiz, prompt, issue, besitzer, automatisch, provider } => {
            let neu = Neu { titel, notiz, prompt, issue, besitzer, automatisch, provider };
            kommunikation::aufgabe_neu(ag, etage, &von.name, neu)?;
        }
        AufCl::Aendern { id, titel, status, besitzer, notiz } => {
            kommunikation::aufgabe_aendern(ag, etage, &von.name, id, Aenderung { titel, status, besitzer, notiz })?;
        }
        AufCl::Entfernen { id } => {
            kommunikation::board_aendern(ag, etage, |b| b.entfernen(id))?;
        }
        AufCl::Verschieben { id, delta } => {
            kommunikation::board_aendern(ag, etage, |b| b.verschieben(id, delta))?;
        }
        AufCl::ErledigteLeeren => kommunikation::board_aendern(ag, etage, |b| b.erledigte_leeren())?,
        AufCl::Limit { max_arbeiter } => {
            kommunikation::board_aendern(ag, etage, |b| b.board.max_arbeiter = max_arbeiter.min(20))?;
            warteschlange_anstossen(ag, etage);
        }
    }
    Ok(())
}

async fn github_nachricht(ag: &Agenten, von: &Absender, etage: &str, m: GhCl) -> Result<(), String> {
    let dir = ag.innen.zustand().etagen.get(etage).map(|e| e.verzeichnis.clone()).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let f = |e: anyhow::Error| e.to_string();
    match m {
        GhCl::Aktualisieren => {}
        GhCl::IssueAnlegen { titel, text } => {
            github::issue_anlegen(&dir, &titel, &text).await.map_err(f)?;
        }
        GhCl::Kommentieren { nummer, text } => {
            github::kommentieren(&dir, nummer, &text).await.map_err(f)?;
        }
        GhCl::Mergen { nummer, art, branch_loeschen } => {
            github::mergen(&dir, nummer, art, branch_loeschen).await.map_err(f)?;
        }
        GhCl::Schliessen { nummer } => {
            github::schliessen(&dir, nummer).await.map_err(f)?;
        }
        GhCl::IssueVergeben { nummer, tisch, provider } => {
            let prompt = Some(issue_auftrag(ag, etage, nummer, None));
            let wunsch = start::Wunsch { tisch, prompt, provider, konto: von.konto.clone(), ..Default::default() };
            start::anlegen(ag, etage, &von.name, wunsch).await?;
            issue_uebernehmen(ag, etage, nummer);
        }
    }
    github::aktualisieren(ag, etage).await;
    Ok(())
}

/// Der Auftrag für einen Arbeiter, der ein GitHub-Issue bearbeiten soll.
fn issue_auftrag(ag: &Agenten, etage: &str, nummer: u64, zusatz: Option<String>) -> String {
    let z = ag.innen.zustand();
    let issue = z.etagen.get(etage).and_then(|e| e.github.issues.iter().find(|i| i.nummer == nummer).cloned());
    let (titel, inhalt) = match issue {
        Some(i) => (i.titel, i.text),
        None => (String::new(), format!("Details: gh issue view {nummer}")),
    };
    let n = nummer.to_string();
    let mut text = crate::prompts::text(&z.prompts, "issue", &[("nummer", &n), ("titel", &titel), ("text", &inhalt)]);
    if let Some(z) = zusatz {
        text.push_str(&format!("\n\n{z}"));
    }
    text
}

fn issue_uebernehmen(ag: &Agenten, etage: &str, nummer: u64) {
    let Some(dir) = ag.innen.zustand().etagen.get(etage).map(|e| e.verzeichnis.clone()) else { return };
    ag.innen.laufzeit.spawn(async move {
        if let Err(f) = github::issue_uebernehmen(&dir, nummer).await {
            tracing::warn!("Issue #{nummer} konnte nicht zugewiesen werden: {f}");
        }
    });
}

async fn pr_oeffnen(ag: &Agenten, etage: &str, id: &str) -> Result<(), String> {
    let (pfad, wt, titel, konto) = {
        let mut z = ag.innen.zustand();
        let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
        let dir = e.verzeichnis.clone();
        let a = e.arbeiter.get_mut(id).ok_or("Diesen Arbeiter gibt es nicht")?;
        let wt = a.info.worktree.clone().ok_or("Dieser Arbeiter hat keinen eigenen Worktree")?;
        if a.info.pr_oeffnet {
            return Err("Der Pull Request wird schon geöffnet".into());
        }
        a.info.pr_oeffnet = true;
        let titel = a.info.aufgabe.as_ref().map(|t| t.name.clone());
        let konto = a.info.konto.clone();
        ag.innen.arbeiter_melden(&z.etagen[etage], id);
        (worktree::absolut(&dir, &wt.pfad), wt, titel, konto)
    };
    let umgebung = ag.innen.umgebung_von(konto.as_deref());
    let ergebnis = github::pr_oeffnen(&pfad, &wt.branch, wt.von.as_deref(), titel.as_deref(), &umgebung).await;
    let mut z = ag.innen.zustand();
    if let Some(a) = z.arbeiter_mut(etage, id) {
        a.info.pr_oeffnet = false;
        if let Ok((nummer, url)) = &ergebnis {
            a.info.pr = Some(protokoll::arbeiter::PrVerweis { nummer: *nummer, url: url.clone() });
        }
        ag.innen.arbeiter_melden(&z.etagen[etage], id);
        crate::speicher::arbeiter_speichern(&ag.innen.daten, &z.etagen[etage]);
    }
    drop(z);
    ergebnis.map(|_| ()).map_err(|e| format!("Pull Request konnte nicht geöffnet werden: {e}"))
}

/// Vergibt offene automatische Aufgaben an neue Arbeiter, solange die Grenze es erlaubt.
pub fn warteschlange_anstossen(ag: &Agenten, etage: &str) {
    let (ag, etage) = (ag.clone(), etage.to_string());
    ag.innen.laufzeit.clone().spawn(async move {
        loop {
            let aufgabe = {
                let mut z = ag.innen.zustand();
                let Some(e) = z.etagen.get_mut(&etage) else { return };
                let laufend = e.board.automatisch_laufend();
                let Some(a) = e.board.naechste_automatische(laufend).cloned() else { return };
                if e.freier_tisch().is_none() {
                    return;
                }
                // Vorab reservieren, damit ein zweiter Anstoß sie nicht noch einmal vergibt.
                if let Some(t) = e.board.holen_mut(a.id) {
                    t.status = AufgabenStatus::InArbeit;
                    t.arbeiter = Some(String::new());
                }
                a
            };
            let prompt = aufgabe.prompt.clone().unwrap_or_else(|| {
                let notiz = aufgabe.notiz.clone().unwrap_or_default();
                crate::prompts::text(&ag.innen.zustand().prompts, "warteschlange", &[("titel", &aufgabe.titel), ("notiz", &notiz)])
            });
            let wunsch = start::Wunsch { prompt: Some(prompt), provider: aufgabe.provider.clone(), ..Default::default() };
            let ergebnis = start::anlegen(&ag, &etage, "Warteschlange", wunsch).await;
            let ok = ergebnis.is_ok();
            let _ = kommunikation::board_aendern(&ag, &etage, |b| {
                let name = ergebnis.as_ref().ok().cloned();
                if let Some(t) = b.holen_mut(aufgabe.id) {
                    match &name {
                        Some(id) => t.arbeiter = Some(id.clone()),
                        None => {
                            t.arbeiter = None;
                            t.status = AufgabenStatus::Blockiert;
                            t.notiz = ergebnis.as_ref().err().cloned();
                        }
                    }
                }
            });
            if let Ok(id) = &ergebnis {
                let mut z = ag.innen.zustand();
                let name = z.arbeiter_mut(&etage, id).map(|a| a.info.name.clone());
                if let Some(e) = z.etagen.get_mut(&etage)
                    && let Some(t) = e.board.holen_mut(aufgabe.id)
                {
                    t.besitzer = name;
                }
            }
            if !ok {
                return;
            }
        }
    });
}
