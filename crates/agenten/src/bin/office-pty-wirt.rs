//! `office-pty-wirt <datenordner>`: hält die Terminals der Arbeiter, damit ein Neustart des Servers
//! die Agenten nicht mitreißt. Der Server startet ihn abgekoppelt (siehe `agenten::wirt`) und spricht
//! mit ihm über TCP auf 127.0.0.1; nur mit dem Token aus `<daten>/pty-wirt.token` kommt man hinein.
//! Er führt eine Kopie jedes Bildschirms, damit der nächste Server ihn zurückbekommt, und beendet
//! alles, wenn eine halbe Stunde lang kein Server zurückgekommen ist.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use agenten::pty::{Pty, fragt_cursor};
use agenten::terminal::Terminal;
use agenten::wirt::{AnWirt, INFO_DATEI, TOKEN_DATEI, VERSION, VomWirt, WirtInfo};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// So lange laufen die Terminals ohne Server weiter.
const OHNE_SERVER: Duration = Duration::from_secs(30 * 60);
/// Ein Wirt, zu dem nie ein Server kommt, hat nichts zu tun.
const UNABGEHOLT: Duration = Duration::from_secs(30);

enum Ereignis {
    Ausgabe(String, Vec<u8>),
    Ende(String, Option<i32>),
    Verbunden(u64, mpsc::UnboundedSender<String>),
    Nachricht(u64, AnWirt),
    Getrennt(u64),
}

struct Sitzung {
    pty: Option<Pty>,
    terminal: Terminal,
    arbeitet: Option<bool>,
    /// Der verbundene Server kennt sie: ihre Ausgabe geht dorthin.
    angehaengt: bool,
    /// Beendet, während kein Server da war: wird beim Anhängen gemeldet.
    ende: Option<Option<i32>>,
}

fn gleich(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn senden(server: &Option<(u64, mpsc::UnboundedSender<String>)>, n: &VomWirt) {
    if let (Some((_, tx)), Ok(text)) = (server, serde_json::to_string(n)) {
        let _ = tx.send(text);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let Some(daten) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("So: office-pty-wirt <datenordner>");
        std::process::exit(2);
    };
    let Ok(token) = std::fs::read_to_string(daten.join(TOKEN_DATEI)).map(|t| t.trim().to_string())
    else {
        eprintln!("office-pty-wirt: kein Token in {}", daten.display());
        std::process::exit(2);
    };
    let Ok(lauscher) = TcpListener::bind(("127.0.0.1", 0)).await else {
        std::process::exit(1)
    };
    let port = lauscher.local_addr().map(|a| a.port()).unwrap_or(0);
    let info = WirtInfo {
        port,
        pid: std::process::id(),
        version: VERSION,
    };
    if std::fs::write(
        daten.join(INFO_DATEI),
        serde_json::to_string(&info).unwrap_or_default(),
    )
    .is_err()
    {
        std::process::exit(1);
    }
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<Ereignis>();
    annehmen(lauscher, token, ev_tx.clone());

    let mut sitzungen: HashMap<String, Sitzung> = HashMap::new();
    let mut server: Option<(u64, mpsc::UnboundedSender<String>)> = None;
    let gestartet = Instant::now();
    let mut je_verbunden = false;
    let mut allein_seit: Option<Instant> = Some(Instant::now());
    let mut takt = tokio::time::interval(Duration::from_secs(5));
    loop {
        let ereignis = tokio::select! {
            e = ev_rx.recv() => e,
            _ = takt.tick() => {
                let lebend = sitzungen.values().any(|s| s.pty.is_some());
                if server.is_none() {
                    let zu_lange = allein_seit.is_some_and(|t| t.elapsed() > OHNE_SERVER);
                    if (je_verbunden && !lebend) || (!je_verbunden && gestartet.elapsed() > UNABGEHOLT && !lebend) || zu_lange {
                        break;
                    }
                }
                continue;
            }
        };
        let Some(ereignis) = ereignis else { break };
        match ereignis {
            Ereignis::Verbunden(nr, tx) => {
                je_verbunden = true;
                allein_seit = None;
                for s in sitzungen.values_mut() {
                    s.angehaengt = false;
                }
                server = Some((nr, tx));
                senden(
                    &server,
                    &VomWirt::Bereit {
                        version: VERSION,
                        sitzungen: sitzungen.keys().cloned().collect(),
                    },
                );
            }
            Ereignis::Getrennt(nr) => {
                if server.as_ref().is_some_and(|(n, _)| *n == nr) {
                    server = None;
                    allein_seit = Some(Instant::now());
                }
            }
            Ereignis::Ausgabe(id, bytes) => {
                let Some(s) = sitzungen.get_mut(&id) else {
                    continue;
                };
                let ae = s.terminal.einspeisen(&bytes);
                if let Some(f) = ae.neuer_fortschritt {
                    s.arbeitet = Some(f);
                }
                if fragt_cursor(&bytes)
                    && let Some(pty) = s.pty.as_mut()
                {
                    let _ = pty.schreiben(s.terminal.cursor_antwort().as_bytes());
                }
                if s.angehaengt && !ae.text.is_empty() {
                    senden(&server, &VomWirt::Daten { id, daten: ae.text });
                }
            }
            Ereignis::Ende(id, code) => {
                let Some(s) = sitzungen.get_mut(&id) else {
                    continue;
                };
                s.pty = None;
                if s.angehaengt && server.is_some() {
                    senden(
                        &server,
                        &VomWirt::Ende {
                            id: id.clone(),
                            code,
                            fehler: None,
                        },
                    );
                    sitzungen.remove(&id);
                } else {
                    s.ende = Some(code);
                }
            }
            Ereignis::Nachricht(nr, n) => {
                if server.as_ref().is_none_or(|(n2, _)| *n2 != nr) {
                    continue;
                }
                if let AnWirt::Stopp = n {
                    break;
                }
                nachricht(n, &mut sitzungen, &server, &ev_tx);
            }
        }
    }
    for s in sitzungen.values_mut() {
        if let Some(mut pty) = s.pty.take() {
            pty.beenden();
        }
    }
    let _ = std::fs::remove_file(daten.join(INFO_DATEI));
    // Sofort gehen: das Schließen eines ConPTY kann hängen, solange seine Ausgabe nicht gelesen ist.
    std::process::exit(0);
}

fn nachricht(
    n: AnWirt,
    sitzungen: &mut HashMap<String, Sitzung>,
    server: &Option<(u64, mpsc::UnboundedSender<String>)>,
    ev_tx: &mpsc::UnboundedSender<Ereignis>,
) {
    match n {
        AnWirt::Starten {
            id,
            befehl,
            spalten,
            zeilen,
        } => {
            let (t1, t2, i1, i2) = (ev_tx.clone(), ev_tx.clone(), id.clone(), id.clone());
            let ergebnis = Pty::starten(
                &befehl,
                spalten,
                zeilen,
                move |d, _| {
                    let _ = t1.send(Ereignis::Ausgabe(i1.clone(), d.to_vec()));
                },
                move |code| {
                    let _ = t2.send(Ereignis::Ende(i2, code));
                },
            );
            match ergebnis {
                Ok(pty) => {
                    senden(
                        server,
                        &VomWirt::Gestartet {
                            id: id.clone(),
                            pid: pty.pid(),
                        },
                    );
                    let s = Sitzung {
                        pty: Some(pty),
                        terminal: Terminal::neu(spalten, zeilen),
                        arbeitet: None,
                        angehaengt: true,
                        ende: None,
                    };
                    sitzungen.insert(id, s);
                }
                Err(f) => senden(
                    server,
                    &VomWirt::Ende {
                        id,
                        code: None,
                        fehler: Some(format!("{f:#}")),
                    },
                ),
            }
        }
        AnWirt::Anhaengen { id } => {
            let Some(s) = sitzungen.get_mut(&id) else {
                senden(server, &VomWirt::Weg { id });
                return;
            };
            let (spalten, zeilen) = s.terminal.groesse();
            let pid = s.pty.as_ref().and_then(Pty::pid);
            let titel = s.terminal.titel().map(str::to_string);
            senden(
                server,
                &VomWirt::Angehaengt {
                    id: id.clone(),
                    pid,
                    spalten,
                    zeilen,
                    bildschirm: s.terminal.snapshot(),
                    titel,
                    arbeitet: s.arbeitet,
                },
            );
            s.angehaengt = true;
            if let Some(code) = s.ende {
                senden(
                    server,
                    &VomWirt::Ende {
                        id: id.clone(),
                        code,
                        fehler: None,
                    },
                );
                sitzungen.remove(&id);
            }
        }
        AnWirt::Schreiben { id, daten } => {
            if let Some(pty) = sitzungen.get_mut(&id).and_then(|s| s.pty.as_mut()) {
                let _ = pty.schreiben(daten.as_bytes());
            }
        }
        AnWirt::Groesse {
            id,
            spalten,
            zeilen,
        } => {
            if let Some(s) = sitzungen.get_mut(&id) {
                s.terminal.groesse_aendern(spalten, zeilen);
                if let Some(pty) = s.pty.as_ref() {
                    let _ = pty.groesse(spalten, zeilen);
                }
            }
        }
        AnWirt::Beenden { id } => {
            if let Some(pty) = sitzungen.get_mut(&id).and_then(|s| s.pty.as_mut()) {
                pty.beenden();
            }
        }
        AnWirt::Hallo { .. } | AnWirt::Stopp => {}
    }
}

/// Nimmt Verbindungen an; nur eine mit dem richtigen Token im ersten `Hallo` gilt.
fn annehmen(lauscher: TcpListener, token: String, ev_tx: mpsc::UnboundedSender<Ereignis>) {
    tokio::spawn(async move {
        let mut nr = 0u64;
        while let Ok((strom, _)) = lauscher.accept().await {
            nr += 1;
            let (token, ev_tx) = (token.clone(), ev_tx.clone());
            tokio::spawn(async move {
                let (lesen, mut schreiben) = strom.into_split();
                let mut zeilen = BufReader::new(lesen).lines();
                let Ok(Ok(Some(erste))) =
                    tokio::time::timeout(Duration::from_secs(5), zeilen.next_line()).await
                else {
                    return;
                };
                let Ok(AnWirt::Hallo { token: t, .. }) = serde_json::from_str(&erste) else {
                    return;
                };
                if !gleich(t.as_bytes(), token.as_bytes()) {
                    return;
                }
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                tokio::spawn(async move {
                    while let Some(text) = rx.recv().await {
                        if schreiben
                            .write_all(format!("{text}\n").as_bytes())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                let _ = ev_tx.send(Ereignis::Verbunden(nr, tx));
                while let Ok(Some(z)) = zeilen.next_line().await {
                    if let Ok(n) = serde_json::from_str::<AnWirt>(&z) {
                        let _ = ev_tx.send(Ereignis::Nachricht(nr, n));
                    }
                }
                let _ = ev_tx.send(Ereignis::Getrennt(nr));
            });
        }
    });
}
