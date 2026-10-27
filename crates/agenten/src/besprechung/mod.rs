//! Der Besprechungsraum: 2–5 Agenten am Tisch arbeiten in Runden nach einem Muster an einer Frage
//! oder Aufgabe (siehe `muster.rs`). Jeder Schritt ist vorbei, wenn jeder Beteiligte seinen Zug mit
//! der genannten Datei beendet hat – die Dateien, nicht das Gerede, bringen die Besprechung weiter
//! (siehe `ablauf.rs`). Sie endet, wenn die Ausgabedatei steht, oder hält an Runden- oder Tokengrenze.

mod ablauf;
pub mod muster;

use protokoll::ServerNachricht;
use protokoll::agenten::{
    Besprechung, BesprechungEintrag, BesprechungStand, BesprechungStatus, BesprechungWunsch, Platz,
    Server as AgSrv, Zug, ZugZustand,
};
use protokoll::arbeiter::{ArbeiterArt, Aufraeumen};

use crate::{Agenten, Ziel, jetzt_ms, speicher, start, steuerung, worktree, zufall_hex};

pub use ablauf::starten as pumpe_starten;

/// Der Ordner der Rundennotizen im Worktree der Besprechung.
pub const NOTIZEN: &str = ".besprechung";
pub const TOKENS_JE_PLATZ: u64 = 1_000_000;
pub const MAX_BUDGET: u64 = 50_000_000;
const MAX_FRUEHER: usize = 20;
/// Die Stühle am Besprechungstisch, wenn der Server keine eigenen nennt.
pub fn standard_stuehle() -> Vec<String> {
    (1..=5).map(|i| format!("meeting-{i}")).collect()
}

pub fn laden(ordner: &std::path::Path) -> BesprechungStand {
    std::fs::read_to_string(ordner.join("besprechung.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Speichert den Stand und schickt ihn an die Etage.
pub fn melden(ag: &Agenten, etage: &str) {
    let z = ag.innen.zustand();
    let Some(e) = z.etagen.get(etage) else { return };
    let ordner = speicher::ordner(&ag.innen.daten, etage);
    let _ = std::fs::create_dir_all(&ordner);
    if let Ok(t) = serde_json::to_string_pretty(&e.besprechung) {
        let _ = std::fs::write(ordner.join("besprechung.json"), t);
    }
    ag.innen.senden(
        Ziel::Etage(etage.to_string()),
        ServerNachricht::Agenten(AgSrv::Besprechung {
            stand: e.besprechung.clone(),
        }),
    );
}

/// Die Zeile an der Tür, wenn eine Besprechung vorbei ist.
pub fn zusammenfassung(b: &Besprechung) -> String {
    let name = muster::def(b.muster).name;
    let tokens = format!("{:.1}k Tokens", b.tokens as f64 / 1000.0);
    match b.status {
        BesprechungStatus::Gestoppt => format!(
            "{name} · Runde {} von {} · {tokens} · ⛔ {}",
            b.runde,
            b.runden,
            b.grund.as_deref().unwrap_or("angehalten")
        ),
        BesprechungStatus::Laeuft => {
            format!("{name} · Runde {} von {} · {tokens}", b.runde, b.runden)
        }
        BesprechungStatus::Fertig => {
            let wo = match (&b.review_url, &b.review_fehler, &b.commit, &b.worktree) {
                (Some(_), _, _, _) => " · am PR gepostet".to_string(),
                (_, Some(f), _, _) => format!(" · nicht gepostet: {f}"),
                (_, _, Some(_), Some(wt)) => format!(" auf {}", wt.branch),
                _ => String::new(),
            };
            format!(
                "{name} · {} Runden · {tokens} · ✅ {}{wo}",
                b.runde, b.ausgabe
            )
        }
    }
}

fn eintrag(b: &Besprechung) -> BesprechungEintrag {
    BesprechungEintrag {
        id: b.id.clone(),
        muster: b.muster,
        titel: b.titel.clone(),
        status: b.status,
        zusammenfassung: zusammenfassung(b),
        einberufen_von: b.einberufen_von.clone(),
        beendet_um: b.beendet_um.unwrap_or_else(jetzt_ms),
        branch: b.worktree.as_ref().map(|w| w.branch.clone()),
        ausgabe: b.ausgabe.clone(),
    }
}

/// Die erste Nachricht an einen Platz: wer er ist, worum es geht, wie es läuft.
fn einweisung(b: &Besprechung, platz: usize, verzeichnis: &str) -> String {
    let d = muster::def(b.muster);
    let andere: Vec<String> = b
        .plaetze
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != platz)
        .map(|(_, p)| p.rolle.clone())
        .collect();
    let mut text = format!(
        "{}\n\nDu bist {} in einer Besprechung ({}) im Besprechungsraum des OpenAgenticOffice, am Tisch mit: {}. {}\n\nWorum es geht:\n{}",
        b.titel,
        b.plaetze[platz].rolle,
        d.name,
        andere.join(", "),
        d.wie,
        b.auftrag
    );
    if let Some(pr) = b.pr {
        text.push_str(&format!(
            "\n\nEs geht um Pull Request #{pr}: lies ihn mit gh pr view {pr} und gh pr diff {pr}."
        ));
    }
    if let Some(i) = b.issue {
        text.push_str(&format!(
            "\n\nEs geht um GitHub-Issue #{i}: lies es mit gh issue view {i}."
        ));
    }
    text.push_str(&format!(
        "\n\nSo läuft es: Das Office gibt jedem von euch seinen Teil jeder Runde in einer Nachricht wie dieser. Erledige genau diesen Teil, schreib ihn in die genannte Datei und beende deinen Zug; die nächste Runde beginnt, wenn alle Teile dieser Runde geschrieben sind. Dein Arbeitsordner ist {verzeichnis}; die Notizen liegen in {NOTIZEN}/, dort liest du, was die anderen geschrieben haben. Die Besprechung endet, wenn {} geschrieben ist, und nur der Teil, der das sagt, schreibt sie. Höchstens {} Runden und {}k Tokens für alle zusammen – halte die Notizen kurz, lieber Stichpunkte.\n\nCommitte nicht, pushe nicht und wechsle nicht den Branch: das Office committet die Ausgabe am Ende selbst.",
        b.ausgabe, b.runden, b.budget / 1000
    ));
    text
}

/// Beruft eine Besprechung ein: Worktree, dann ein Agent je Stuhl mit Einweisung und erstem Teil.
pub async fn starten(
    ag: &Agenten,
    etage: &str,
    von: &str,
    konto: Option<String>,
    w: BesprechungWunsch,
) -> Result<(), String> {
    let d = muster::def(w.muster);
    let rollen: Vec<String> = if w.rollen.is_empty() {
        d.rollen
            .iter()
            .take(d.plaetze.2)
            .map(|r| r.to_string())
            .collect()
    } else {
        w.rollen
            .iter()
            .map(|r| r.trim().chars().take(40).collect::<String>())
            .filter(|r| !r.is_empty())
            .collect()
    };
    if rollen.len() < d.plaetze.0 || rollen.len() > d.plaetze.1 {
        return Err(format!(
            "{} braucht {} bis {} Plätze",
            d.name, d.plaetze.0, d.plaetze.1
        ));
    }
    let auftrag = w.auftrag.trim().chars().take(20_000).collect::<String>();
    if auftrag.is_empty() {
        return Err("Sag, worum es in der Besprechung geht".into());
    }
    if d.braucht_pr && w.pr.is_none() {
        return Err("Eine Review-Runde braucht einen Pull Request".into());
    }
    let teile: Vec<String> = w
        .teile
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .take(100)
        .collect();
    if d.braucht_teile && teile.is_empty() {
        return Err("Map-Reduce braucht eine Liste von Teilen (einer pro Zeile)".into());
    }
    let titel = w
        .titel
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| crate::arbeiter::kuerzen(&auftrag, 60));
    let slug = worktree::slug(&titel, "")
        .trim_end_matches('-')
        .chars()
        .take(40)
        .collect::<String>();
    let ausgabe = w
        .ausgabe
        .clone()
        .filter(|a| !a.trim().is_empty())
        .unwrap_or_else(|| muster::standard_ausgabe(w.muster, &slug, w.pr));
    if let Some(p) = muster::ausgabe_problem(&ausgabe) {
        return Err(p.into());
    }
    let id = zufall_hex(6);
    let (projekt, stuehle) = {
        let z = ag.innen.zustand();
        let e = z
            .etagen
            .get(etage)
            .ok_or("Diese Etage gibt es nicht (mehr)")?;
        if let Some(b) = &e.besprechung.aktuell
            && (b.status == BesprechungStatus::Laeuft || !b.abgeraeumt)
        {
            return Err("Am Tisch sitzt noch eine Besprechung: erst anhalten und abräumen".into());
        }
        let stuehle = if e.besprechung_stuehle.is_empty() {
            standard_stuehle()
        } else {
            e.besprechung_stuehle.clone()
        };
        (e.verzeichnis.clone(), stuehle)
    };
    if stuehle.len() < rollen.len() {
        return Err("Am Besprechungstisch sind nicht genug Stühle".into());
    }
    if !worktree::ist_repo(&projekt).await {
        return Err("Besprechungen brauchen ein Git-Repository".into());
    }
    let wt = worktree::erstellen(&projekt, &format!("besprechung-{slug}-{id}"))
        .await
        .map_err(|e| e.to_string())?;
    let runden = w.runden.unwrap_or(d.runden.2).clamp(d.runden.0, d.runden.1);
    let budget = w
        .budget
        .unwrap_or(TOKENS_JE_PLATZ * rollen.len() as u64)
        .clamp(10_000, MAX_BUDGET);
    let plaetze = rollen
        .into_iter()
        .zip(&stuehle)
        .map(|(rolle, tisch)| Platz {
            rolle,
            tisch: tisch.clone(),
            arbeiter: None,
            name: None,
            tokens: 0,
        })
        .collect();
    let mut b = Besprechung {
        id: id.clone(),
        muster: w.muster,
        titel,
        auftrag,
        ausgabe,
        plaetze,
        teile,
        pr: w.pr,
        issue: w.issue,
        provider: w.provider.clone(),
        modell: w.modell.clone(),
        aufwand: w.aufwand.clone(),
        runden,
        runde: 1,
        schritt: 0,
        zuege: Vec::new(),
        budget,
        tokens: 0,
        status: BesprechungStatus::Laeuft,
        grund: None,
        einberufen_von: von.to_string(),
        begonnen_um: jetzt_ms(),
        beendet_um: None,
        worktree: Some(wt.clone()),
        notizen: NOTIZEN.into(),
        commit: None,
        review_url: None,
        review_fehler: None,
        vorschau: None,
        abgeraeumt: false,
    };
    let abs = worktree::absolut(&projekt, &wt.pfad);
    let _ = std::fs::create_dir_all(abs.join(NOTIZEN));
    let lesen = |_: &str| String::new();
    let erste = muster::teile(&b, 1, 0, &lesen).unwrap_or_default();
    b.zuege = erste
        .iter()
        .map(|t| Zug {
            platz: t.platz,
            tut: t.tut.clone(),
            datei: t.datei.clone(),
            zustand: ZugZustand::Gesendet,
            gesendet_um: Some(jetzt_ms()),
            erinnert: false,
        })
        .collect();
    {
        let mut z = ag.innen.zustand();
        if let Some(e) = z.etagen.get_mut(etage) {
            if let Some(alt) = e.besprechung.aktuell.take() {
                e.besprechung.frueher.insert(0, eintrag(&alt));
                e.besprechung.frueher.truncate(MAX_FRUEHER);
            }
            e.besprechung.aktuell = Some(Box::new(b.clone()));
        }
    }
    melden(ag, etage);
    let verzeichnis = abs.to_string_lossy().into_owned();
    for i in 0..b.plaetze.len() {
        let teil = erste.iter().find(|t| t.platz == i).map(|t| format!("Runde 1 von {}: {}", b.runden, t.frage)).unwrap_or_else(|| {
            "Runde 1 hat keinen Teil für dich. Antworte in einer Zeile, dass du bereit bist, und beende deinen Zug; dein Teil kommt in einer späteren Nachricht.".into()
        });
        let wunsch = start::Wunsch {
            tisch: Some(b.plaetze[i].tisch.clone()),
            prompt: Some(format!("{}\n\n{teil}", einweisung(&b, i, &verzeichnis))),
            art: ArbeiterArt::Agent,
            provider: b.provider.clone(),
            modell: b.modell.clone(),
            aufwand: b.aufwand.clone(),
            konto: konto.clone(),
            worktree_vorgegeben: Some(wt.clone()),
            besprechung: Some(id.clone()),
            ..Default::default()
        };
        match start::anlegen(ag, etage, "Besprechungsraum", wunsch).await {
            Ok(aid) => {
                let mut z = ag.innen.zustand();
                let name = z.arbeiter_mut(etage, &aid).map(|a| a.info.name.clone());
                if let Some(b) = z
                    .etagen
                    .get_mut(etage)
                    .and_then(|e| e.besprechung.aktuell.as_mut())
                {
                    b.plaetze[i].arbeiter = Some(aid);
                    b.plaetze[i].name = name;
                }
            }
            Err(f) => {
                anhalten(
                    ag,
                    etage,
                    &format!("{} konnte sich nicht setzen: {f}", b.plaetze[i].rolle),
                );
                return Err(f);
            }
        }
    }
    melden(ag, etage);
    Ok(())
}

/// Hält die laufende Besprechung an (die Arbeiter bleiben sitzen) und sagt warum.
pub fn anhalten(ag: &Agenten, etage: &str, grund: &str) {
    {
        let mut z = ag.innen.zustand();
        let Some(e) = z.etagen.get_mut(etage) else {
            return;
        };
        let Some(b) = e
            .besprechung
            .aktuell
            .as_mut()
            .filter(|b| b.status == BesprechungStatus::Laeuft)
        else {
            return;
        };
        b.status = BesprechungStatus::Gestoppt;
        b.grund = Some(grund.to_string());
        b.beendet_um = Some(jetzt_ms());
    }
    melden(ag, etage);
}

/// Schickt die Arbeiter der letzten Besprechung heim und räumt ihren Worktree weg (der Branch bleibt).
pub async fn abraeumen(ag: &Agenten, etage: &str) -> Result<(), String> {
    let (plaetze, wt, projekt) = {
        let z = ag.innen.zustand();
        let e = z
            .etagen
            .get(etage)
            .ok_or("Diese Etage gibt es nicht (mehr)")?;
        let b = e
            .besprechung
            .aktuell
            .as_ref()
            .ok_or("Am Tisch sitzt niemand")?;
        if b.status == BesprechungStatus::Laeuft {
            return Err("Die Besprechung läuft noch: erst anhalten".into());
        }
        (b.plaetze.clone(), b.worktree.clone(), e.verzeichnis.clone())
    };
    for p in plaetze.iter().filter_map(|p| p.arbeiter.as_ref()) {
        let _ = steuerung::stoppen(ag, etage, p, Aufraeumen::Behalten).await;
    }
    if let Some(wt) = wt {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let _ = worktree::entfernen(&projekt, &wt, Aufraeumen::Worktree).await;
    }
    {
        let mut z = ag.innen.zustand();
        if let Some(e) = z.etagen.get_mut(etage)
            && let Some(mut b) = e.besprechung.aktuell.take()
        {
            b.abgeraeumt = true;
            e.besprechung.frueher.insert(0, eintrag(&b));
            e.besprechung.frueher.truncate(MAX_FRUEHER);
        }
    }
    melden(ag, etage);
    Ok(())
}
