//! Die Pumpe des Besprechungsraums: alle paar Sekunden je laufender Besprechung nachsehen, wer
//! seinen Teil geschrieben hat, Säumige einmal erinnern, den nächsten Schritt übergeben, und am Ende
//! die Ausgabe committen (bei einer Review-Runde: als Kommentar an den PR setzen).

use std::path::{Path, PathBuf};
use std::time::Duration;

use protokoll::agenten::{Besprechung, BesprechungStatus, Muster, Zug, ZugZustand};
use protokoll::arbeiter::ArbeiterStatus;

use super::{melden, muster};
use crate::{Agenten, jetzt_ms, steuerung, worktree};

const TAKT: Duration = Duration::from_secs(3);
/// Ein übergebener Teil, mit dem einer so lange nicht anfängt, wird einmal neu übergeben.
const START_GNADE_MS: u64 = 60_000;
const VORSCHAU: usize = 6000;

pub fn starten(ag: &Agenten) {
    let schwach = ag.innen.selbst();
    ag.innen.laufzeit.spawn(async move {
        loop {
            tokio::time::sleep(TAKT).await;
            let Some(innen) = schwach.upgrade() else { return };
            let ag = Agenten { innen };
            let laufend: Vec<String> = {
                let z = ag.innen.zustand();
                z.etagen
                    .values()
                    .filter(|e| e.besprechung.aktuell.as_ref().is_some_and(|b| b.status == BesprechungStatus::Laeuft))
                    .map(|e| e.id.clone())
                    .collect()
            };
            for etage in laufend {
                if let Some(fertig) = pumpen(&ag, &etage) {
                    abschliessen(&ag, &etage, fertig).await;
                }
            }
        }
    });
}

/// Was eine Runde der Pumpe für einen Platz tun will.
enum Tun {
    Tippen(String, String),
    Anhalten(String),
}

fn lesen(dir: &Path, datei: &str) -> String {
    std::fs::read_to_string(dir.join(datei)).unwrap_or_default()
}

fn geschrieben(dir: &Path, datei: &str) -> bool {
    std::fs::metadata(dir.join(datei)).is_ok_and(|m| m.len() > 0)
}

/// Eine Runde für eine Etage. Gibt den Arbeitsordner zurück, wenn die Besprechung gerade fertig wurde.
fn pumpen(ag: &Agenten, etage: &str) -> Option<PathBuf> {
    let mut tun: Vec<Tun> = Vec::new();
    let mut fertig = None;
    {
        let mut z = ag.innen.zustand();
        let e = z.etagen.get_mut(etage)?;
        let projekt = e.verzeichnis.clone();
        let arbeiter = &e.arbeiter;
        let b = e.besprechung.aktuell.as_mut().filter(|b| b.status == BesprechungStatus::Laeuft)?;
        let dir = worktree::absolut(&projekt, &b.worktree.as_ref()?.pfad);
        // Verbrauch: die Summe aller Plätze.
        for p in b.plaetze.iter_mut() {
            if let Some(v) = p.arbeiter.as_ref().and_then(|id| arbeiter.get(id)).and_then(|a| a.info.verbrauch.as_ref()) {
                p.tokens = v.tokens_ein + v.tokens_aus;
            }
        }
        b.tokens = b.plaetze.iter().map(|p| p.tokens).sum();
        if b.tokens > b.budget {
            tun.push(Tun::Anhalten(format!("Das Token-Budget ({}k) ist aufgebraucht", b.budget / 1000)));
        }
        let jetzt = jetzt_ms();
        let aktuelle = muster::teile(b, b.runde, b.schritt, &|d| lesen(&dir, d)).unwrap_or_default();
        for zug in b.zuege.iter_mut().filter(|z| z.zustand != ZugZustand::Fertig) {
            let platz = &b.plaetze[zug.platz];
            let Some(a) = platz.arbeiter.as_ref().and_then(|id| arbeiter.get(id)) else {
                // Beim Hinsetzen ist er vielleicht noch nicht eingetragen.
                if zug.zustand != ZugZustand::Gesendet || jetzt - zug.gesendet_um.unwrap_or(jetzt) > START_GNADE_MS {
                    tun.push(Tun::Anhalten(format!("{} ist nicht mehr am Tisch", platz.rolle)));
                }
                continue;
            };
            let bereit = a.laeuft() && matches!(a.info.status, ArbeiterStatus::Bereit | ArbeiterStatus::Fertig);
            if !a.laeuft() && a.info.status != ArbeiterStatus::Startet {
                tun.push(Tun::Anhalten(format!("{} ({}) läuft nicht mehr", a.info.name, platz.rolle)));
                continue;
            }
            match zug.zustand {
                ZugZustand::Wartet if bereit => {
                    let frage = aktuelle
                        .iter()
                        .find(|t| t.platz == zug.platz)
                        .map(|t| format!("Runde {} von {}: {}", b.runde, b.runden, t.frage));
                    if let Some(f) = frage {
                        tun.push(Tun::Tippen(a.info.id.clone(), f));
                        zug.zustand = ZugZustand::Gesendet;
                        zug.gesendet_um = Some(jetzt);
                    }
                }
                ZugZustand::Gesendet | ZugZustand::Arbeitet if a.info.status == ArbeiterStatus::Arbeitet => zug.zustand = ZugZustand::Arbeitet,
                ZugZustand::Gesendet | ZugZustand::Arbeitet if bereit => {
                    let lange = jetzt.saturating_sub(zug.gesendet_um.unwrap_or(jetzt)) > START_GNADE_MS;
                    if geschrieben(&dir, &zug.datei) {
                        zug.zustand = ZugZustand::Fertig;
                    } else if zug.zustand == ZugZustand::Arbeitet || lange {
                        if zug.erinnert {
                            tun.push(Tun::Anhalten(format!("{} hat {} nicht geschrieben", platz.rolle, zug.datei)));
                        } else {
                            let text = format!("Du hast deinen Zug beendet, ohne {} zu schreiben, auf die die Besprechung wartet. Schreib sie jetzt, dann beende deinen Zug.", zug.datei);
                            tun.push(Tun::Tippen(a.info.id.clone(), text));
                            zug.erinnert = true;
                            zug.zustand = ZugZustand::Gesendet;
                            zug.gesendet_um = Some(jetzt);
                        }
                    }
                }
                _ => {}
            }
        }
        if geschrieben(&dir, &b.ausgabe) {
            let mut text = lesen(&dir, &b.ausgabe);
            if text.len() > VORSCHAU {
                let mut ende = VORSCHAU;
                while !text.is_char_boundary(ende) {
                    ende -= 1;
                }
                text.truncate(ende);
            }
            b.vorschau = Some(text);
        }
        let alle_fertig = b.zuege.iter().all(|z| z.zustand == ZugZustand::Fertig);
        if alle_fertig && tun.iter().all(|t| !matches!(t, Tun::Anhalten(_))) {
            if b.zuege.iter().any(|z| z.datei == b.ausgabe) {
                b.status = BesprechungStatus::Fertig;
                b.beendet_um = Some(jetzt);
                fertig = Some(dir.clone());
            } else {
                weiter(b, &dir);
            }
        }
        // Tippen unter demselben Schloss, damit niemand dazwischen den Arbeiter ändert.
        for t in &tun {
            if let Tun::Tippen(id, text) = t
                && let Some(a) = e.arbeiter.get_mut(id)
            {
                steuerung::tippen(&ag.innen, a, text);
                a.status_setzen(ArbeiterStatus::Arbeitet);
            }
        }
    }
    if let Some(Tun::Anhalten(grund)) = tun.iter().find(|t| matches!(t, Tun::Anhalten(_))) {
        super::anhalten(ag, etage, grund);
        return None;
    }
    melden(ag, etage);
    fertig
}

/// Der nächste Schritt: im selben Durchgang (Rot/Blau) oder die nächste Runde.
fn weiter(b: &mut Besprechung, dir: &Path) {
    let lesen = |d: &str| lesen(dir, d);
    let mut kandidaten = vec![(b.runde, b.schritt + 1), (b.runde + 1, 0)];
    if b.muster != Muster::RotBlau {
        kandidaten.remove(0);
    }
    for (runde, schritt) in kandidaten {
        if let Some(teile) = muster::teile(b, runde, schritt, &lesen).filter(|t| !t.is_empty()) {
            b.runde = runde;
            b.schritt = schritt;
            b.zuege = teile
                .into_iter()
                .map(|t| Zug { platz: t.platz, tut: t.tut, datei: t.datei, zustand: ZugZustand::Wartet, gesendet_um: None, erinnert: false })
                .collect();
            return;
        }
    }
    b.status = BesprechungStatus::Gestoppt;
    b.grund = Some(format!("Nach {} Runden ist keine Ausgabe entstanden", b.runden));
    b.beendet_um = Some(jetzt_ms());
}

/// Am Ende: die Ausgabe auf dem Branch der Besprechung committen, ein Review an den PR setzen.
async fn abschliessen(ag: &Agenten, etage: &str, dir: PathBuf) {
    let (ausgabe, titel, pr, konto) = {
        let z = ag.innen.zustand();
        let Some(b) = z.etagen.get(etage).and_then(|e| e.besprechung.aktuell.as_ref()) else { return };
        let konto = b.plaetze.iter().filter_map(|p| p.arbeiter.as_ref()).find_map(|id| z.etagen[etage].arbeiter.get(id)?.info.konto.clone());
        (b.ausgabe.clone(), b.titel.clone(), if b.muster == Muster::Review { b.pr } else { None }, konto)
    };
    let umgebung = ag.innen.umgebung_von(konto.as_deref());
    let commit = async {
        worktree::git_mit(&dir, &["add", "--", &ausgabe], &umgebung).await?;
        worktree::git_mit(&dir, &["commit", "-m", &format!("Besprechung: {titel}"), "--", &ausgabe], &umgebung).await?;
        worktree::git(&dir, &["rev-parse", "HEAD"]).await
    }
    .await;
    let review = match pr {
        Some(n) => {
            let datei = dir.join(&ausgabe).to_string_lossy().into_owned();
            Some(crate::github::gh_mit(&dir, &["pr", "comment", &n.to_string(), "--body-file", &datei], &umgebung).await)
        }
        None => None,
    };
    {
        let mut z = ag.innen.zustand();
        if let Some(b) = z.etagen.get_mut(etage).and_then(|e| e.besprechung.aktuell.as_mut()) {
            b.commit = commit.ok();
            match review {
                Some(Ok(url)) => b.review_url = Some(url.lines().last().unwrap_or("").to_string()),
                Some(Err(f)) => b.review_fehler = Some(f.to_string()),
                None => {}
            }
        }
    }
    melden(ag, etage);
}
