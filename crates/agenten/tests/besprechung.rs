//! Ende-zu-Ende: eine Debatte mit zwei Agenten (Shells als eigener Befehl). Die Notizen schreibt der
//! Test an ihrer Stelle; die Besprechung muss Runde um Runde weitergehen und die Ausgabe committen.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use agenten::{Absender, Agenten, AgentenEinstellungen, Ereignis};
use protokoll::agenten::{BesprechungStand, BesprechungStatus, BesprechungWunsch, Client as AgCl, Muster, Server as AgSrv, ZugZustand};
use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::broadcast::Receiver;

fn git(dir: &Path, args: &[&str]) -> String {
    let aus = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(aus.status.success(), "git {args:?}");
    String::from_utf8_lossy(&aus.stdout).trim().to_string()
}

async fn warte(rx: &mut Receiver<Ereignis>, sekunden: u64, mut pruefen: impl FnMut(&BesprechungStand) -> bool) -> BesprechungStand {
    let ende = tokio::time::Instant::now() + Duration::from_secs(sekunden);
    loop {
        let rest = ende.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(rest, rx.recv()).await {
            Ok(Ok(Ereignis { nachricht: ServerNachricht::Agenten(AgSrv::Besprechung { stand }), .. })) => {
                if let Some(b) = &stand.aktuell {
                    eprintln!("Runde {} Schritt {} {:?} {:?} {:?}", b.runde, b.schritt, b.status, b.grund, b.zuege.iter().map(|z| z.zustand).collect::<Vec<_>>());
                }
                if pruefen(&stand) {
                    return stand;
                }
            }
            Ok(Ok(Ereignis { nachricht: ServerNachricht::Fehler { text }, .. })) => panic!("Fehler vom Office: {text}"),
            Ok(_) => {}
            Err(_) => panic!("Besprechung kam nicht innerhalb von {sekunden}s weiter"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn debatte_laeuft_durch() {
    // SAFETY: nur dieser Test setzt die Variable, bevor irgendetwas sie liest.
    unsafe { std::env::set_var("OFFICE_AGENT_EIGEN", if cfg!(windows) { "cmd.exe" } else { "/bin/sh" }) };
    let daten = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q", "-b", "main"]);
    git(repo.path(), &["config", "user.email", "t@example.com"]);
    git(repo.path(), &["config", "user.name", "T"]);
    std::fs::write(repo.path().join("README.md"), "x").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "start"]);

    let ag = Agenten::starten(AgentenEinstellungen {
        daten_verzeichnis: daten.path().to_path_buf(),
        hook_port: 0,
        cli_pfad: Some(env!("CARGO_BIN_EXE_office-agenten").into()),
    })
    .await
    .unwrap();
    ag.etage_anmelden("e", repo.path().to_path_buf());
    let mut rx = ag.ereignisse();
    let luna = Absender { verbindung: "v1".into(), name: "Luna".into(), etage: Some("e".into()), konto: None };
    let wunsch = BesprechungWunsch {
        muster: Muster::Debatte,
        auftrag: "Welche Farbe bekommt der Knopf?".into(),
        titel: Some("Knopffarbe".into()),
        ausgabe: Some("docs/knopf.md".into()),
        rollen: vec!["Vorsitz".into(), "Skeptiker".into()],
        teile: vec![],
        pr: None,
        issue: None,
        runden: Some(2),
        budget: None,
        provider: Some("eigen".into()),
        modell: None,
        aufwand: None,
    };
    ag.nachricht(&luna, ClientNachricht::Agenten(AgCl::BesprechungStarten { wunsch: Box::new(wunsch) })).await;
    let stand = warte(&mut rx, 30, |s| s.aktuell.as_ref().is_some_and(|b| b.plaetze.iter().all(|p| p.arbeiter.is_some()))).await;
    let b = stand.aktuell.unwrap();
    let dir = repo.path().join(&b.worktree.as_ref().unwrap().pfad);
    assert_eq!(b.zuege.len(), 2, "Runde 1: beide schlagen vor");
    for z in &b.zuege {
        let datei = dir.join(&z.datei);
        std::fs::create_dir_all(datei.parent().unwrap()).unwrap();
        std::fs::write(datei, format!("Vorschlag von Platz {}: Blau", z.platz)).unwrap();
    }
    // Runde 2: nur der Vorsitz schreibt die Entscheidung, sobald er wieder Zeit hat.
    let stand = warte(&mut rx, 90, |s| s.aktuell.as_ref().is_some_and(|b| b.runde == 2 && b.zuege.iter().any(|z| z.zustand != ZugZustand::Wartet))).await;
    let b = stand.aktuell.unwrap();
    assert_eq!(b.zuege.len(), 1);
    assert_eq!(b.zuege[0].datei, "docs/knopf.md");
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    std::fs::write(dir.join("docs/knopf.md"), "# Entscheidung\n\nBlau.\n").unwrap();
    let stand = warte(&mut rx, 90, |s| s.aktuell.as_ref().is_some_and(|b| b.status == BesprechungStatus::Fertig && b.commit.is_some())).await;
    let b = stand.aktuell.unwrap();
    assert!(b.vorschau.unwrap().contains("Blau"));
    assert_eq!(git(&dir, &["log", "-1", "--format=%s"]), "Besprechung: Knopffarbe");

    // Abräumen: Arbeiter heim, Worktree weg, Eintrag bleibt.
    ag.nachricht(&luna, ClientNachricht::Agenten(AgCl::BesprechungAbraeumen)).await;
    let stand = warte(&mut rx, 30, |s| s.aktuell.is_none()).await;
    assert_eq!(stand.frueher.len(), 1);
    assert!(stand.frueher[0].zusammenfassung.contains("docs/knopf.md"));
    assert!(!dir.exists());
    ag.beenden().await;
}
