//! Arbeiter anlegen und (wieder) starten: Worktree, Provider-Befehlszeile, PTY und die Rückrufe für
//! Ausgabe und Prozessende.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use protokoll::arbeiter::{ArbeiterArt, ArbeiterInfo, ArbeiterStatus, Server as ArbSrv};
use protokoll::ServerNachricht;

use crate::arbeiter::{Arbeiter, FARBEN, NAMEN};
use crate::provider::{self, StartEingabe};
use crate::pty::{self, Befehl, Pty};
use crate::{Agenten, Innen, Ziel, anleitung, jetzt_ms, speicher, steuerung, worktree, zufall_hex};

/// So lange darf ein Agent mit Hooks hochfahren, bevor seine Karte einen Menschen ruft.
const START_GEDULD: Duration = Duration::from_secs(15);
/// Agenten ohne Hooks bekommen ihren ersten Auftrag nach dieser Zeit getippt.
const TIPP_VERZOEGERUNG: Duration = Duration::from_secs(4);

/// Wie ein Arbeiter angelegt werden soll.
#[derive(Debug, Clone, Default)]
pub struct Wunsch {
    pub tisch: Option<String>,
    pub prompt: Option<String>,
    pub worktree: Option<bool>,
    pub art: ArbeiterArt,
    pub provider: Option<String>,
    pub modell: Option<String>,
    pub aufwand: Option<String>,
}

/// Legt einen Arbeiter an und startet ihn. Gibt seine ID zurück oder sagt auf Deutsch, was fehlt.
pub async fn anlegen(ag: &Agenten, etage: &str, von: &str, w: Wunsch) -> Result<String, String> {
    let provider_id = match w.art {
        ArbeiterArt::Agent => Some(w.provider.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| provider::STANDARD.into())),
        ArbeiterArt::Shell => None,
    };
    if let Some(pid) = &provider_id {
        let p = provider::finden(pid).ok_or_else(|| format!("Unbekannter Provider „{pid}“"))?;
        if provider::programm_von(p.as_ref()).is_none() {
            return Err(format!("{} ist auf diesem Rechner nicht installiert (Programm „{}“ nicht im PATH)", p.name(), p.programm()));
        }
        for wert in [&w.modell, &w.aufwand].into_iter().flatten() {
            if !provider::wert_gueltig(wert) {
                return Err(format!("Ungültiger Wert „{wert}“ für Modell oder Aufwand"));
            }
        }
    }
    let (id, verzeichnis, name) = {
        let mut z = ag.innen.zustand();
        let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
        let tisch = match w.tisch.clone().filter(|t| !t.is_empty()) {
            Some(t) if e.tisch_belegt(&t) => return Err("An diesem Tisch sitzt schon jemand".into()),
            Some(t) => t,
            None => e.freier_tisch().ok_or("Alle Tische sind besetzt: schick erst jemanden heim")?,
        };
        let name = NAMEN
            .iter()
            .find(|n| !e.arbeiter.values().any(|a| a.info.name.eq_ignore_ascii_case(n)))
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("Arbeiter {}", e.arbeiter.len() + 1));
        let mut id = zufall_hex(6);
        while e.arbeiter.contains_key(&id) {
            id = zufall_hex(6);
        }
        let info = ArbeiterInfo {
            id: id.clone(),
            etage: etage.to_string(),
            art: w.art,
            provider: provider_id.clone(),
            modell: w.modell.clone(),
            aufwand: w.aufwand.clone(),
            tisch,
            name: name.clone(),
            farbe: FARBEN[e.arbeiter.len() % FARBEN.len()].to_string(),
            status: ArbeiterStatus::Startet,
            gesehen: true,
            wartet_seit: None,
            angelegt_von: von.to_string(),
            angelegt_um: jetzt_ms(),
            prompt: w.prompt.clone(),
            worktree: None,
            pr: None,
            pr_oeffnet: false,
            titel: None,
            sitzung: None,
            exit_code: None,
            spalten: 120,
            zeilen: 32,
            zuschauer: Vec::new(),
            aktivitaet: w.prompt.as_deref().map(|p| crate::arbeiter::kuerzen(p, 80)),
            aktion: None,
            aufgabe: w.prompt.as_deref().and_then(crate::arbeiter::aufgabe_aus_prompt),
            verbrauch: None,
            letzte_eingabe: None,
            hinweis: None,
        };
        e.arbeiter.insert(id.clone(), Arbeiter::neu(info));
        e.bus.ankunft(&id);
        ag.innen.arbeiter_melden(e, &id);
        (id, e.verzeichnis.clone(), name)
    };
    let will_worktree = w.art == ArbeiterArt::Agent && w.worktree.unwrap_or(true) && worktree::ist_repo(&verzeichnis).await;
    if will_worktree {
        match worktree::erstellen(&verzeichnis, &worktree::slug(&name, &id)).await {
            Ok(wt) => {
                let mut z = ag.innen.zustand();
                if let Some(a) = z.arbeiter_mut(etage, &id) {
                    a.info.worktree = Some(wt);
                }
            }
            Err(fehler) => {
                entfernen_nach_fehler(ag, etage, &id);
                return Err(format!("Worktree konnte nicht angelegt werden: {fehler}"));
            }
        }
    }
    if let Err(fehler) = pty_starten(ag, etage, &id, w.prompt.clone(), None) {
        entfernen_nach_fehler(ag, etage, &id);
        return Err(fehler);
    }
    Ok(id)
}

fn entfernen_nach_fehler(ag: &Agenten, etage: &str, id: &str) {
    let mut z = ag.innen.zustand();
    if let Some(e) = z.etagen.get_mut(etage) {
        e.arbeiter.remove(id);
        e.bus.vergessen(id);
        let ziel = Ziel::Etage(etage.to_string());
        ag.innen.senden(ziel.clone(), ServerNachricht::Arbeiter(ArbSrv::Entfernt { arbeiter: id.to_string() }));
        ag.innen.senden(ziel, ServerNachricht::Agenten(protokoll::agenten::Server::FeedEntfernt { arbeiter: id.to_string() }));
    }
}

/// Startet einen Arbeiter, der gerade nicht läuft, wieder; mit seiner Sitzung, wenn er eine hatte.
pub fn fortsetzen(ag: &Agenten, etage: &str, id: &str, prompt: Option<String>) -> Result<(), String> {
    let sitzung = {
        let z = ag.innen.zustand();
        let a = z.etagen.get(etage).and_then(|e| e.arbeiter.get(id)).ok_or("Diesen Arbeiter gibt es nicht")?;
        if a.laeuft() {
            return Err(format!("{} läuft schon", a.info.name));
        }
        if let Some(wt) = &a.info.worktree
            && !worktree::absolut(&z.etagen[etage].verzeichnis, &wt.pfad).is_dir()
        {
            return Err(format!("Der Worktree von {} fehlt – heimschicken und neu anlegen", a.info.name));
        }
        a.info.sitzung.clone()
    };
    pty_starten(ag, etage, id, prompt, sitzung)
}

/// Startet das Terminal eines Arbeiters mit seinem Provider (oder einer Shell).
pub fn pty_starten(ag: &Agenten, etage: &str, id: &str, prompt: Option<String>, fortsetzen: Option<String>) -> Result<(), String> {
    let mut z = ag.innen.zustand();
    let e = z.etagen.get_mut(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let anleitung = anleitung::fuer(e, id);
    let projekt = e.verzeichnis.clone();
    let a = e.arbeiter.get_mut(id).ok_or("Diesen Arbeiter gibt es nicht")?;
    let verzeichnis = a.info.worktree.as_ref().map(|wt| worktree::absolut(&projekt, &wt.pfad)).unwrap_or(projekt);
    let cli = ag.innen.cli.to_string_lossy().replace('\\', "/");
    let token = zufall_hex(32);
    let mut befehl = Befehl { verzeichnis, ..Default::default() };
    let (mut prompt_tippen, mut meldet_start, mut hat_hooks) = (None, false, false);
    match a.info.provider.as_deref().and_then(provider::finden) {
        Some(p) if a.info.art == ArbeiterArt::Agent => {
            let programm = provider::programm_von(p.as_ref())
                .ok_or_else(|| format!("{} ist auf diesem Rechner nicht installiert", p.name()))?;
            let eingabe = StartEingabe {
                info: &a.info,
                prompt: prompt.as_deref(),
                fortsetzen: fortsetzen.as_deref(),
                anleitung: &anleitung,
                daten: &ag.innen.daten,
                cli: &cli,
                skript: pty::ist_skript(&programm),
            };
            let plan = p.starten(&eingabe);
            befehl.programm = programm;
            befehl.argumente = plan.argumente;
            befehl.umgebung = plan.umgebung;
            befehl.entfernen = p.entfernen().iter().map(|s| s.to_string()).collect();
            prompt_tippen = plan.prompt_tippen;
            meldet_start = p.meldet_start();
            hat_hooks = p.hat_hooks();
        }
        _ => befehl.programm = pty::standard_shell(),
    }
    umgebung_setzen(&mut befehl, &ag.innen, etage, a, &token);
    a.lauf += 1;
    let lauf = a.lauf;
    let schwach = Arc::downgrade(&ag.innen);
    let (e1, i1, e2, i2) = (etage.to_string(), id.to_string(), etage.to_string(), id.to_string());
    let schwach2 = schwach.clone();
    let neu = Pty::starten(
        &befehl,
        a.info.spalten,
        a.info.zeilen,
        move |daten, antwort| bei_daten(&schwach, &e1, &i1, lauf, daten, antwort),
        move |code| bei_ende(&schwach2, &e2, &i2, lauf, code),
    )
    .map_err(|f| format!("{f:#}"))?;
    a.pty = Some(neu);
    a.token = token;
    a.hat_hooks = hat_hooks;
    a.gestartet = Some(Instant::now());
    a.prompt_tippen = prompt_tippen;
    a.fehlschlaege = 0;
    a.info.exit_code = None;
    a.info.hinweis = None;
    if prompt.is_some() {
        a.info.prompt = prompt;
    }
    a.status_setzen(if meldet_start { ArbeiterStatus::Startet } else { ArbeiterStatus::Bereit });
    // Ohne Hooks gibt es kein Signal „bereit“: der Auftrag kommt nach einer kurzen Weile.
    let ag2 = ag.clone();
    let (etage2, id2) = (etage.to_string(), id.to_string());
    ag.innen.laufzeit.spawn(async move {
        tokio::time::sleep(if meldet_start { START_GEDULD } else { TIPP_VERZOEGERUNG }).await;
        nach_dem_start(&ag2, &etage2, &id2, lauf, meldet_start);
    });
    let e = &z.etagen[etage];
    ag.innen.arbeiter_melden(e, id);
    speicher::arbeiter_speichern(&ag.innen.daten, e);
    Ok(())
}

fn umgebung_setzen(befehl: &mut Befehl, innen: &Innen, etage: &str, a: &Arbeiter, token: &str) {
    let u = &mut befehl.umgebung;
    u.insert("OFFICE_ARBEITER_ID".into(), a.info.id.clone());
    u.insert("OFFICE_ARBEITER_NAME".into(), a.info.name.clone());
    u.insert("OFFICE_ETAGE".into(), etage.to_string());
    u.insert("OFFICE_HOOK_URL".into(), innen.hook_url.clone());
    u.insert("OFFICE_HOOK_TOKEN".into(), token.to_string());
    // Die CLI liegt vorne im PATH.
    if let Some(ordner) = innen.cli.parent() {
        let mut pfade = vec![ordner.to_path_buf()];
        if let Some(alt) = std::env::var_os("PATH") {
            pfade.extend(std::env::split_paths(&alt));
        }
        if let Ok(neu) = std::env::join_paths(pfade) {
            u.insert("PATH".into(), neu.to_string_lossy().into_owned());
        }
    }
}

/// Kurz nach dem Start: einem Agenten ohne Hooks den Auftrag tippen; einer mit Hooks, der sich noch
/// nicht gemeldet hat, hängt an einer Einrichtungsfrage.
fn nach_dem_start(ag: &Agenten, etage: &str, id: &str, lauf: u64, meldet_start: bool) {
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    if a.lauf != lauf || !a.laeuft() {
        return;
    }
    if meldet_start {
        if a.info.status == ArbeiterStatus::Startet {
            a.status_setzen(ArbeiterStatus::Wartet);
            a.info.hinweis = Some("Wartet auf eine Einrichtungsfrage (Vertrauen/Login) – Terminal öffnen".into());
        }
    } else if let Some(text) = a.prompt_tippen.take() {
        steuerung::tippen(&ag.innen, a, &text);
        a.status_setzen(ArbeiterStatus::Arbeitet);
    }
    let e = &z.etagen[etage];
    ag.innen.arbeiter_melden(e, id);
}

fn bei_daten(schwach: &Weak<Innen>, etage: &str, id: &str, lauf: u64, daten: &[u8], antwort: &pty::Antworter) {
    let Some(innen) = schwach.upgrade() else { return };
    let ag = Agenten { innen };
    let mut z = ag.innen.zustand();
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    if a.lauf != lauf {
        return;
    }
    let ae = a.terminal.einspeisen(daten);
    a.letzte_ausgabe = Some(Instant::now());
    // Cursor-Abfragen beantwortet der Server (ConPTY wartet beim Start darauf); Browser tun es nicht.
    if pty::fragt_cursor(daten) {
        let _ = antwort.schreiben(a.terminal.cursor_antwort().as_bytes());
    }
    if !a.zuschauer.is_empty() && !ae.text.is_empty() {
        let ziel = Ziel::Verbindungen(a.zuschauer.keys().cloned().collect());
        ag.innen.senden(ziel, ServerNachricht::Arbeiter(ArbSrv::TerminalDaten { arbeiter: id.to_string(), daten: ae.text }));
    }
    let mut melden = false;
    if let Some(t) = ae.neuer_titel {
        a.info.titel = Some(t).filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("claude") && !t.eq_ignore_ascii_case("claude code"));
        melden = true;
    }
    if !a.hat_hooks && a.info.art == ArbeiterArt::Agent {
        match ae.neuer_fortschritt {
            Some(true) if a.info.status != ArbeiterStatus::Arbeitet => {
                a.status_setzen(ArbeiterStatus::Arbeitet);
                melden = true;
            }
            Some(false) if a.info.status == ArbeiterStatus::Arbeitet => {
                a.status_setzen(ArbeiterStatus::Fertig);
                melden = true;
            }
            _ => {}
        }
    }
    // Die ersten Sekunden: hängt er an einem Login- oder Vertrauensbildschirm?
    let frisch = a.gestartet.is_some_and(|g| g.elapsed() < Duration::from_secs(90));
    if frisch
        && matches!(a.info.status, ArbeiterStatus::Startet | ArbeiterStatus::Wartet | ArbeiterStatus::Bereit)
        && let Some(p) = a.info.provider.as_deref().and_then(provider::finden)
    {
        let frueh = a.info.status == ArbeiterStatus::Startet || a.info.hinweis.is_some();
        let hinweis = p.blockiert(&a.terminal.text(), frueh);
        if hinweis.is_some() && hinweis != a.info.hinweis {
            a.status_setzen(ArbeiterStatus::Wartet);
            a.info.hinweis = hinweis;
            melden = true;
        }
    }
    if melden {
        crate::kommunikation::zustellen(&ag, &mut z, etage, id);
        ag.innen.arbeiter_melden(&z.etagen[etage], id);
    }
}

fn bei_ende(schwach: &Weak<Innen>, etage: &str, id: &str, lauf: u64, code: Option<i32>) {
    let Some(innen) = schwach.upgrade() else { return };
    let mut z = innen.zustand();
    let schliesst = z.schliesst;
    let Some(a) = z.arbeiter_mut(etage, id) else { return };
    if a.lauf != lauf {
        return;
    }
    a.pty = None;
    a.info.exit_code = code;
    a.status_setzen(if schliesst { ArbeiterStatus::Offline } else { ArbeiterStatus::Beendet });
    let e = &z.etagen[etage];
    innen.arbeiter_melden(e, id);
    speicher::arbeiter_speichern(&innen.daten, e);
}
