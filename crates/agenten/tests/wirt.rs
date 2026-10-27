//! Der PTY-Wirt: ein Terminal überlebt, dass der Server beendet und neu gestartet wird, und der neue
//! Server bekommt es samt Bildschirm zurück und kann weiter hineinschreiben.

use std::time::Duration;

use agenten::{Absender, Agenten, AgentenEinstellungen, Ereignis};
use protokoll::arbeiter::{
    ArbeiterArt, ArbeiterInfo, ArbeiterStatus, Client as ArbCl, Server as ArbSrv,
};
use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::broadcast::Receiver;

async fn warte<T>(
    rx: &mut Receiver<Ereignis>,
    sekunden: u64,
    mut pruefen: impl FnMut(&ServerNachricht) -> Option<T>,
) -> T {
    let ende = tokio::time::Instant::now() + Duration::from_secs(sekunden);
    loop {
        let rest = ende.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(rest, rx.recv()).await {
            Ok(Ok(e)) => {
                if let Some(t) = pruefen(&e.nachricht) {
                    return t;
                }
            }
            Ok(Err(_)) => {}
            Err(_) => panic!("Ereignis kam nicht innerhalb von {sekunden}s"),
        }
    }
}

fn info(n: &ServerNachricht) -> Option<ArbeiterInfo> {
    match n {
        ServerNachricht::Arbeiter(ArbSrv::Aktualisiert { arbeiter }) => Some((**arbeiter).clone()),
        _ => None,
    }
}

fn eingabe(text: &str) -> String {
    if cfg!(windows) {
        format!("{text}\r\n")
    } else {
        format!("{text}\n")
    }
}

async fn office(daten: &std::path::Path) -> Agenten {
    Agenten::starten(AgentenEinstellungen {
        daten_verzeichnis: daten.into(),
        hook_port: 0,
        cli_pfad: Some(env!("CARGO_BIN_EXE_office-agenten").into()),
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_ueberlebt_neustart() {
    let daten = tempfile::tempdir().unwrap();
    let projekt = tempfile::tempdir().unwrap();
    let luna = Absender {
        verbindung: "v".into(),
        name: "Luna".into(),
        etage: Some("e".into()),
        konto: None,
    };

    let ag = office(daten.path()).await;
    assert!(
        daten.path().join("pty-wirt.json").is_file(),
        "der Wirt läuft"
    );
    let mut rx = ag.ereignisse();
    ag.etage_anmelden("e", projekt.path().into());
    let anlegen = ArbCl::Anlegen {
        tisch: "t1".into(),
        prompt: None,
        worktree: None,
        art: ArbeiterArt::Shell,
        provider: None,
        modell: None,
        aufwand: None,
        issue: None,
    };
    ag.nachricht(&luna, ClientNachricht::Arbeiter(anlegen))
        .await;
    let id = warte(&mut rx, 15, |n| info(n).map(|a| a.id)).await;
    ag.nachricht(
        &luna,
        ClientNachricht::Arbeiter(ArbCl::Anschauen {
            arbeiter: id.clone(),
        }),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    ag.nachricht(
        &luna,
        ClientNachricht::Arbeiter(ArbCl::Eingabe {
            arbeiter: id.clone(),
            daten: eingabe("echo vorher-111"),
        }),
    )
    .await;
    let mut gesehen = String::new();
    warte(&mut rx, 15, |n| match n {
        ServerNachricht::Arbeiter(ArbSrv::TerminalDaten { daten, .. }) => {
            gesehen.push_str(daten);
            gesehen.contains("vorher-111").then_some(())
        }
        _ => None,
    })
    .await;
    // Der Server geht; das Terminal läuft im Wirt weiter.
    ag.beenden().await;
    drop(rx);
    drop(ag);
    tokio::time::sleep(Duration::from_secs(1)).await;

    let ag = office(daten.path()).await;
    // Erst zuhören, dann anmelden: die Übernahme ist schnell.
    let mut rx = ag.ereignisse();
    ag.etage_anmelden("e", projekt.path().into());
    let a = warte(&mut rx, 15, |n| {
        info(n).filter(|a| a.id == id && a.status != ArbeiterStatus::Offline)
    })
    .await;
    assert_eq!(a.status, ArbeiterStatus::Bereit);
    ag.nachricht(
        &luna,
        ClientNachricht::Arbeiter(ArbCl::Anschauen {
            arbeiter: id.clone(),
        }),
    )
    .await;
    let bild = warte(&mut rx, 10, |n| match n {
        ServerNachricht::Arbeiter(ArbSrv::TerminalSnapshot { daten, .. }) => Some(daten.clone()),
        _ => None,
    })
    .await;
    assert!(bild.contains("vorher-111"), "der Bildschirm kam mit zurück");
    ag.nachricht(
        &luna,
        ClientNachricht::Arbeiter(ArbCl::Eingabe {
            arbeiter: id.clone(),
            daten: eingabe("echo nachher-222"),
        }),
    )
    .await;
    let mut gesehen = String::new();
    warte(&mut rx, 15, |n| match n {
        ServerNachricht::Arbeiter(ArbSrv::TerminalDaten { daten, .. }) => {
            gesehen.push_str(daten);
            gesehen.contains("nachher-222").then_some(())
        }
        _ => None,
    })
    .await;
    ag.alles_beenden().await;
    // Der Wirt geht mit, wenn er keine Terminals mehr hat.
    for _ in 0..40 {
        if !daten.path().join("pty-wirt.json").exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("der Wirt läuft nach alles_beenden noch");
}
