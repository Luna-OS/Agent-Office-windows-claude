//! Erst-Einrichtung im Terminal: wohin Projekte geklont werden, die GitHub-Anmeldung und die ersten
//! Repositories als Etagen. Ein neues Office fragt das beim ersten Start (nur mit Terminal),
//! `open-agentic-office einrichten` fragt es noch einmal. Enter überspringt jede Frage.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use crate::etagen::{KlonStart, git, klonen, zum_anzeigen};
use crate::office::OfficeRef;
use crate::speicher::jetzt_ms;

const GEZEIGT: usize = 15;

/// Sitzt jemand am Terminal, der antworten kann?
pub fn interaktiv() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal() && std::env::var_os("CI").is_none()
}

/// Eine Frage, beantwortet mit einer Zeile ("" bei Enter oder Strg+D).
async fn fragen(frage: &str) -> String {
    print!("{frage}");
    let _ = std::io::stdout().flush();
    tokio::task::spawn_blocking(|| {
        let mut s = String::new();
        let _ = std::io::stdin().lock().read_line(&mut s);
        s.trim().to_string()
    })
    .await
    .unwrap_or_default()
}

/// Beim Start: fragt, wenn das Office neu ist (keine Etagen, nichts auf der Kommandozeile).
pub async fn beim_start(o: &OfficeRef) {
    let k = &o.konfig;
    let schon = o.einstellungen.d.lock().unwrap().eingerichtet;
    if schon || o.etagen.anzahl() > 0 || k.projekt_ordner.is_some() || k.projekt_repo.is_some() || !interaktiv() {
        return;
    }
    println!(
        "\n  👋 Willkommen bei OpenAgenticOffice!\n\n  Jedes Projekt wird eine Etage im Gebäude. Drei kurze Fragen, dann geht das Office auf.\n  Enter überspringt jede Frage; alles geht später auch im Office."
    );
    fragen_stellen(o, true).await;
}

/// `open-agentic-office einrichten`: die Fragen noch einmal.
pub async fn noch_einmal(o: &OfficeRef) {
    fragen_stellen(o, true).await;
}

async fn fragen_stellen(o: &OfficeRef, ordner_fragen: bool) {
    if ordner_fragen {
        let (vorschlag, _) = projekte(o);
        println!("\n  📁 Wohin sollen Projekte geklont werden? Jedes landet in <ordner>/<owner>/<repo>.");
        let antwort = fragen(&format!("     Ordner [{}]: ", zum_anzeigen(&vorschlag))).await;
        if !antwort.is_empty() {
            let p = PathBuf::from(antwort.replacen('~', &crate::konfig::home().display().to_string(), 1));
            o.einstellungen.aendern(|d| d.projekte_ordner = Some((p.display().to_string(), "Terminal".into(), jetzt_ms())));
        }
        println!("     ✓ {}/<owner>/<repo> (Admins können das in ⚙️ Einstellungen ändern)", zum_anzeigen(&projekte(o).0));
    }
    if github_pruefen().await {
        repos_waehlen(o).await;
    }
    o.einstellungen.aendern(|d| d.eingerichtet = true);
}

fn projekte(o: &OfficeRef) -> (PathBuf, protokoll::etagen::ProjekteOrdner) {
    let eigen = o.einstellungen.d.lock().unwrap().projekte_ordner.clone();
    o.etagen.projekte_ordner(eigen.as_ref())
}

/// Ist gh angemeldet? Bietet sonst die Anmeldung an.
async fn github_pruefen() -> bool {
    let wer = || async {
        let aus = tokio::process::Command::new("gh").args(["api", "user", "--jq", ".login"]).output().await.ok()?;
        aus.status.success().then(|| String::from_utf8_lossy(&aus.stdout).trim().to_string()).filter(|s| !s.is_empty())
    };
    if crate::anmeldungen::ablauf::finden("gh").is_none() {
        println!("\n  🐙 Das Office klont mit der GitHub-CLI (gh), die hier nicht installiert ist.\n     Installiere sie (https://cli.github.com) und führe `gh auth login` aus.");
        return false;
    }
    if let Some(login) = wer().await {
        println!("\n  🐙 Bei GitHub angemeldet als {login}");
        return true;
    }
    println!("\n  🐙 gh ist noch nicht bei GitHub angemeldet.");
    if fragen("     Jetzt anmelden? [J/n] ").await.to_lowercase().starts_with('n') {
        return false;
    }
    let ok = tokio::process::Command::new("gh").args(["auth", "login", "--hostname", "github.com", "--git-protocol", "https", "--web"]).status().await.is_ok_and(|s| s.success());
    ok && wer().await.is_some()
}

async fn repos_waehlen(o: &OfficeRef) {
    let repos = match git::repos_auflisten(&[]).await {
        Ok(r) => r,
        Err(f) => return println!("     ✗ Die Repositories ließen sich nicht auflisten: {f}"),
    };
    let zeigen = |liste: &[&protokoll::etagen::RepoAuswahl]| {
        for (i, r) in liste.iter().take(GEZEIGT).enumerate() {
            let b = r.beschreibung.as_deref().map(|b| format!(" – {}", b.chars().take(60).collect::<String>())).unwrap_or_default();
            println!("     {:>2}. {}{}{b}", i + 1, r.name, if r.privat { " 🔒" } else { "" });
        }
    };
    let alle: Vec<&protokoll::etagen::RepoAuswahl> = repos.iter().collect();
    println!("\n  🛗 Deine Repositories, zuletzt gepusht zuerst{}:\n", if repos.len() > GEZEIGT { " (Suchwort tippen zum Suchen)" } else { "" });
    zeigen(&alle);
    let mut liste = alle.clone();
    loop {
        let antwort = fragen("\n     Nummer, owner/name oder Suchwort (Enter = fertig): ").await;
        if antwort.is_empty() {
            return;
        }
        let repo = if let Ok(n) = antwort.parse::<usize>() {
            match liste.get(n.wrapping_sub(1)) {
                Some(r) => r.name.clone(),
                None => {
                    println!("     ✗ Die {n} gibt es in der Liste nicht");
                    continue;
                }
            }
        } else if let Some(r) = git::repo_aus(&antwort) {
            r
        } else {
            let w = antwort.to_lowercase();
            liste = alle.iter().copied().filter(|r| r.name.to_lowercase().contains(&w) || r.beschreibung.as_deref().is_some_and(|b| b.to_lowercase().contains(&w))).collect();
            if liste.is_empty() {
                println!("     Nichts passt zu „{antwort}“. owner/name klont jedes Repository.");
                liste = alle.clone();
            } else {
                println!();
                zeigen(&liste);
            }
            continue;
        };
        klonen_und_melden(o, &repo).await;
    }
}

/// Klont `repo` als Etage, mit Fortschritt im Terminal.
pub async fn klonen_und_melden(o: &OfficeRef, repo: &str) {
    let (projekte, _) = projekte(o);
    match o.etagen.klon_anmelden(repo, &projekte, "Terminal", "") {
        Err(f) => println!("     ✗ {f}"),
        Ok(KlonStart::Vorhanden(ordner)) => {
            let def = o.etagen.uebernehmen(repo, ordner, "Terminal");
            o.etage_angemeldet(&def);
            println!("     ✓ {repo} war schon da und ist Etage {}", o.etagen.anzahl());
        }
        Ok(KlonStart::Klonen(def, abbruch)) => {
            println!("     ⏳ Klone {repo} nach {} …", zum_anzeigen(&def.ordner));
            let ende = klonen::klonen(repo, &def.ordner, &[], |f| println!("        {}{}", f.schritt, f.prozent.map(|p| format!(" {p} %")).unwrap_or_default()), abbruch).await;
            let ok = matches!(ende, klonen::Ende::Fertig);
            o.etagen.klon_beenden(&def.id, ok);
            match ende {
                klonen::Ende::Fertig => {
                    o.etage_angemeldet(&def);
                    println!("     ✓ {repo} ist Etage {}", o.etagen.anzahl());
                }
                klonen::Ende::Abgebrochen => println!("     ✗ abgebrochen"),
                klonen::Ende::Fehler(f) => println!("     ✗ {f}"),
            }
        }
    }
}

/// `--projekt owner/name`: klont es beim Start, wenn es noch keine Etage gibt.
pub async fn projekt_beim_start(o: &OfficeRef) {
    let Some(repo) = o.konfig.projekt_repo.clone() else { return };
    if o.etagen.anzahl() > 0 {
        return;
    }
    let Some(repo) = git::repo_aus(&repo) else { return tracing::warn!("--projekt {repo} ist kein GitHub-Repository (owner/name)") };
    klonen_und_melden(o, &repo).await;
    crate::ws::etagen::liste_an_alle(o);
}
