//! Ein Arbeiter über zwei Etagen: ein Arbeitsbereich mit einem Worktree je Repository, in dem er
//! startet; heimschicken räumt beide Worktrees samt Branch und den Arbeitsbereich weg.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use agenten::{Absender, Agenten, AgentenEinstellungen};
use protokoll::arbeiter::{Aufraeumen, Client as ArbCl, Server as ArbSrv};
use protokoll::{ClientNachricht, ServerNachricht};

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success()
}

fn repo(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join(name);
    std::fs::create_dir_all(&p).unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "t@e.de"],
        &["config", "user.name", "T"],
    ] {
        assert!(git(&p, args));
    }
    std::fs::write(p.join("README.md"), name).unwrap();
    assert!(git(&p, &["add", "."]));
    assert!(git(&p, &["commit", "-qm", "start"]));
    (d, p)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn arbeiter_ueber_zwei_etagen() {
    // SAFETY: nur dieser Test setzt die Variablen, bevor irgendetwas sie liest.
    unsafe {
        std::env::set_var("OFFICE_PTY_WIRT", "aus");
        std::env::set_var(
            "OFFICE_AGENT_EIGEN",
            if cfg!(windows) { "cmd.exe" } else { "/bin/sh" },
        );
    }
    let daten = tempfile::tempdir().unwrap();
    let (_w, web) = repo("web");
    let (_a, api) = repo("api");
    let ag = Agenten::starten(AgentenEinstellungen {
        daten_verzeichnis: daten.path().into(),
        hook_port: 0,
        cli_pfad: None,
    })
    .await
    .unwrap();
    ag.etage_anmelden("web", web.clone());
    ag.etage_anmelden("api", api.clone());
    let mut rx = ag.ereignisse();
    let luna = Absender {
        verbindung: "v".into(),
        name: "Luna".into(),
        etage: Some("web".into()),
        konto: None,
    };
    let anlegen = ArbCl::AnlegenUeberRepos {
        tisch: "t1".into(),
        prompt: None,
        repos: vec!["api".into()],
        provider: Some("eigen".into()),
        modell: None,
        aufwand: None,
    };
    ag.nachricht(&luna, ClientNachricht::Arbeiter(anlegen))
        .await;
    let info = loop {
        let e = tokio::time::timeout(Duration::from_secs(20), rx.recv())
            .await
            .expect("kein Arbeiter")
            .unwrap();
        if let ServerNachricht::Fehler { text } = &e.nachricht {
            panic!("{text}");
        }
        if let ServerNachricht::Arbeiter(ArbSrv::Aktualisiert { arbeiter }) = e.nachricht
            && !arbeiter.repos.is_empty()
        {
            break *arbeiter;
        }
    };
    let wt = info.worktree.clone().unwrap();
    let bereich = web.join(&wt.pfad).parent().unwrap().to_path_buf();
    assert!(bereich.join("web/README.md").is_file());
    assert_eq!(
        std::fs::read_to_string(bereich.join("api/README.md")).unwrap(),
        "api"
    );
    assert_eq!(info.repos[0].etage, "api");
    assert_eq!(info.repos[0].branch, wt.branch);

    let stoppen = ArbCl::Stoppen {
        arbeiter: info.id.clone(),
        aufraeumen: Aufraeumen::Alles,
    };
    ag.nachricht(&luna, ClientNachricht::Arbeiter(stoppen))
        .await;
    for _ in 0..40 {
        if !bereich.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(!bereich.exists(), "Arbeitsbereich ist noch da");
    assert!(
        !git(&api, &["rev-parse", "--verify", "-q", &wt.branch]),
        "Branch in api ist noch da"
    );
    ag.alles_beenden().await;
}
