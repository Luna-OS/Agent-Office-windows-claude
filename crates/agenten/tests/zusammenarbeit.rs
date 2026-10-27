//! Ende-zu-Ende: zwei Arbeiter auf einer Etage, einer schreibt dem anderen über die echte CLI
//! `office-agenten` aus seinem Terminal; die Nachricht landet auf dem Bus, im Feed, im Terminal des
//! Empfängers. Dazu Status, Board, Worktree und Heimschicken.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use agenten::{Absender, Agenten, AgentenEinstellungen, Ereignis};
use protokoll::agenten::Server as AgSrv;
use protokoll::arbeiter::{ArbeiterArt, ArbeiterInfo, Aufraeumen, Client as ArbCl, Server as ArbSrv};
use protokoll::aufgaben::Server as AufSrv;
use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::broadcast::Receiver;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git").args(args).current_dir(dir).status().unwrap().success();
    assert!(ok, "git {args:?}");
}

/// Wartet auf ein Ereignis, auf das `pruefen` passt.
async fn warte<T>(rx: &mut Receiver<Ereignis>, sekunden: u64, mut pruefen: impl FnMut(&Ereignis) -> Option<T>) -> T {
    let ende = tokio::time::Instant::now() + Duration::from_secs(sekunden);
    loop {
        let rest = ende.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(rest, rx.recv()).await {
            Ok(Ok(e)) => {
                if let ServerNachricht::Fehler { text } = &e.nachricht {
                    eprintln!("Fehler vom Office: {text}");
                }
                if let ServerNachricht::Agenten(AgSrv::Dienste { liste }) = &e.nachricht {
                    eprintln!("Dienste: {liste:?}");
                }
                if std::env::var("TEST_TERMINAL").is_ok()
                    && let ServerNachricht::Arbeiter(ArbSrv::TerminalDaten { daten, .. }) = &e.nachricht
                {
                    eprint!("{daten}");
                }
                if let Some(a) = aktualisiert(&e) {
                    eprintln!("{}: {:?} {:?}", a.name, a.status, a.aktivitaet);
                }
                if let Some(t) = pruefen(&e) {
                    return t;
                }
            }
            Ok(Err(_)) => continue,
            Err(_) => panic!("Ereignis kam nicht innerhalb von {sekunden}s"),
        }
    }
}

fn aktualisiert(e: &Ereignis) -> Option<ArbeiterInfo> {
    match &e.nachricht {
        ServerNachricht::Arbeiter(ArbSrv::Aktualisiert { arbeiter }) => Some((**arbeiter).clone()),
        _ => None,
    }
}

fn eingabe(text: &str) -> String {
    if cfg!(windows) { format!("{text}\r\n") } else { format!("{text}\n") }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn agenten_reden_miteinander() {
    let shell = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
    // SAFETY: nur dieser Test setzt die Variable, bevor irgendetwas sie liest.
    unsafe { std::env::set_var("OFFICE_AGENT_EIGEN", shell) };

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
    ag.tische_setzen("e", vec!["t1".into(), "t2".into(), "t3".into()]);
    let mut rx = ag.ereignisse();
    let luna = Absender { verbindung: "v1".into(), name: "Luna".into(), etage: Some("e".into()), konto: None };

    // Ada: ein Agent (eigener Befehl = Shell) mit Worktree. Bruno: eine Shell.
    let anlegen = |tisch: &str, art, provider: Option<&str>| {
        ClientNachricht::Arbeiter(ArbCl::Anlegen {
            tisch: tisch.into(),
            prompt: None,
            worktree: Some(true),
            art,
            provider: provider.map(str::to_string),
            modell: None,
            aufwand: None,
            issue: None,
        })
    };
    ag.nachricht(&luna, anlegen("t1", ArbeiterArt::Agent, Some("eigen"))).await;
    let ada = warte(&mut rx, 20, |e| aktualisiert(e).filter(|a| a.tisch == "t1" && a.worktree.is_some())).await;
    assert_eq!(ada.name, "Ada");
    let wt = ada.worktree.clone().unwrap();
    assert!(repo.path().join(&wt.pfad).is_dir(), "Worktree fehlt");
    ag.nachricht(&luna, anlegen("t2", ArbeiterArt::Shell, None)).await;
    let bruno = warte(&mut rx, 20, |e| aktualisiert(e).filter(|a| a.tisch == "t2")).await;
    assert!(bruno.worktree.is_none(), "Shells bekommen keinen Worktree");

    // Luna schaut Ada ins Terminal.
    ag.nachricht(&luna, ClientNachricht::Arbeiter(ArbCl::Anschauen { arbeiter: ada.id.clone() })).await;
    warte(&mut rx, 10, |e| matches!(&e.nachricht, ServerNachricht::Arbeiter(ArbSrv::TerminalSnapshot { .. })).then_some(())).await;

    ag.nachricht(&luna, ClientNachricht::Arbeiter(ArbCl::Anschauen { arbeiter: bruno.id.clone() })).await;

    // Bruno schreibt Ada über die CLI.
    let tippe = |text: &str| ClientNachricht::Arbeiter(ArbCl::Eingabe { arbeiter: bruno.id.clone(), daten: eingabe(text) });
    tokio::time::sleep(Duration::from_secs(2)).await;
    ag.nachricht(&luna, tippe("office-agenten senden Ada Hallo von Bruno")).await;
    let n = warte(&mut rx, 30, |e| match &e.nachricht {
        ServerNachricht::Agenten(AgSrv::Nachricht { nachricht }) if nachricht.von == "Bruno" => Some(nachricht.clone()),
        _ => None,
    })
    .await;
    assert_eq!(n.an.as_deref(), Some("Ada"));
    assert_eq!(n.text, "Hallo von Bruno");

    // Status und Board über die CLI.
    ag.nachricht(&luna, tippe("office-agenten status Teste den Bus")).await;
    warte(&mut rx, 30, |e| match &e.nachricht {
        ServerNachricht::Agenten(AgSrv::Feed { eintrag }) if eintrag.notiz.as_deref() == Some("Teste den Bus") => Some(()),
        _ => None,
    })
    .await;
    ag.nachricht(&luna, tippe("office-agenten aufgabe-neu Board-testen")).await;
    warte(&mut rx, 30, |e| match &e.nachricht {
        ServerNachricht::Aufgaben(AufSrv::Stand { board }) => board.aufgaben.iter().any(|a| a.titel == "Board-testen").then_some(()),
        _ => None,
    })
    .await;

    // Bruno startet einen Server: er erscheint als sein Dienst.
    let server = if cfg!(windows) {
        // Brunos Shell ist hier PowerShell: einfache Anführungszeichen, damit `$l` erst drinnen gilt.
        "powershell -NoProfile -Command '$l=[System.Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$l.Start();Start-Sleep 60'"
    } else {
        "python3 -c 'import socket,time;s=socket.socket();s.bind((\"127.0.0.1\",0));s.listen();time.sleep(60)'"
    };
    ag.nachricht(&luna, tippe(server)).await;
    warte(&mut rx, 40, |e| match &e.nachricht {
        ServerNachricht::Agenten(AgSrv::Dienste { liste }) => liste.iter().any(|d| d.name == "Bruno" && d.port > 0).then_some(()),
        _ => None,
    })
    .await;

    // Ada bekommt die Nachricht in ihr Terminal getippt, sobald sie Zeit hat.
    let mut gesehen = String::new();
    warte(&mut rx, 60, |e| match &e.nachricht {
        ServerNachricht::Arbeiter(ArbSrv::TerminalDaten { arbeiter, daten }) if *arbeiter == ada.id => {
            gesehen.push_str(daten);
            gesehen.contains("Hallo von Bruno").then_some(())
        }
        _ => None,
    })
    .await;

    // Heimschicken räumt den Worktree samt Branch weg.
    ag.nachricht(&luna, ClientNachricht::Arbeiter(ArbCl::Stoppen { arbeiter: ada.id.clone(), aufraeumen: Aufraeumen::Alles })).await;
    warte(&mut rx, 20, |e| matches!(&e.nachricht, ServerNachricht::Arbeiter(ArbSrv::Entfernt { arbeiter }) if *arbeiter == ada.id).then_some(())).await;
    assert!(!repo.path().join(&wt.pfad).exists(), "Worktree ist noch da");

    // Gespeichert: Bruno steht in workers.json.
    ag.beenden().await;
    let datei = std::fs::read_to_string(daten.path().join("etagen/e/workers.json")).unwrap();
    assert!(datei.contains("Bruno") && !datei.contains("\"Ada\""));
}
