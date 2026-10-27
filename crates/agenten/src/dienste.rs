//! Dienste: die Server, die Arbeiter starten (`npm run dev`, `cargo run`, `python -m http.server` …).
//! Alle paar Sekunden: welche Prozesse lauschen auf TCP-Ports, und gehört einer davon (über seine
//! Eltern) zum Terminal eines Arbeiters? Dann ist der Port ein Dienst dieses Arbeiters.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use protokoll::ServerNachricht;
use protokoll::agenten::{Dienst, Server as AgSrv};
use protokoll::arbeiter::ArbeiterArt;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::{Agenten, Ziel, jetzt_ms};

const TAKT: Duration = Duration::from_secs(5);
/// Lauschende Prozesse, die kein Dienst zum Anschauen sind (von Tests gesteuerte Browser u. Ä.).
const RAUSCHEN: &[&str] = &["--remote-debugging-port", "--type=renderer", "--type=gpu-process", "--type=utility", "crashpad"];

/// Ein Arbeiter mit laufendem Terminal: (Etage, Arbeiter-ID, Name, PID, ist Agent).
pub type Wurzel = (String, String, String, u32, bool);

/// Ordnet lauschende Ports den Arbeitern zu: `eltern` bildet PID → Eltern-PID ab, `befehl` PID → Befehlszeile.
pub fn zuordnen(
    lauscher: &[(u32, u16, String)],
    eltern: &HashMap<u32, u32>,
    befehl: &HashMap<u32, String>,
    wurzeln: &[Wurzel],
) -> Vec<(String, Dienst)> {
    let nach_pid: HashMap<u32, &Wurzel> = wurzeln.iter().map(|w| (w.3, w)).collect();
    let mut gesehen = HashSet::new();
    let mut aus = Vec::new();
    for (pid, port, programm) in lauscher {
        if !gesehen.insert(*port) {
            continue;
        }
        if befehl.get(pid).is_some_and(|b| RAUSCHEN.iter().any(|r| b.contains(r))) {
            continue;
        }
        let mut p = *pid;
        for _ in 0..64 {
            if let Some(w) = nach_pid.get(&p) {
                // Die eigenen Ports eines Agenten (IDE-Anbindung, Login-Rückruf) sind keine Dienste.
                if !(w.4 && p == *pid) {
                    let d = Dienst { port: *port, arbeiter: w.1.clone(), name: w.2.clone(), programm: programm.clone(), seit: 0 };
                    aus.push((w.0.clone(), d));
                }
                break;
            }
            match eltern.get(&p) {
                Some(&e) if e != p && e != 0 => p = e,
                _ => break,
            }
        }
    }
    aus.sort_by_key(|(_, d)| d.port);
    aus
}

fn runde(ag: &Agenten, system: &mut System) {
    let wurzeln: Vec<Wurzel> = {
        let z = ag.innen.zustand();
        z.etagen
            .values()
            .flat_map(|e| {
                e.arbeiter.values().filter_map(|a| {
                    let pid = a.pty.as_ref()?.pid()?;
                    Some((e.id.clone(), a.info.id.clone(), a.info.name.clone(), pid, a.info.art == ArbeiterArt::Agent))
                })
            })
            .collect()
    };
    let gefunden = if wurzeln.is_empty() {
        Vec::new()
    } else {
        // Unter Windows meldet `listeners` die Socket-Zustände nicht verlässlich: dort zählt jeder
        // TCP-Socket als Kandidat, und ob er wirklich lauscht, klärt unten ein kurzer Verbindungsversuch.
        let lauscher: Vec<(u32, u16, String)> = listeners::get_all()
            .map(|l| {
                l.into_iter()
                    .filter(|l| l.protocol == listeners::Protocol::TCP)
                    .filter(|l| cfg!(windows) || l.state == listeners::SocketState::Listen)
                    .map(|l| (l.process.pid, l.socket.port(), l.process.name))
                    .collect()
            })
            .unwrap_or_default();
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet));
        let mut eltern = HashMap::new();
        let mut befehl = HashMap::new();
        for (pid, p) in system.processes() {
            if let Some(e) = p.parent() {
                eltern.insert(pid.as_u32(), e.as_u32());
            }
            let cmd: Vec<String> = p.cmd().iter().map(|c| c.to_string_lossy().into_owned()).collect();
            befehl.insert(pid.as_u32(), cmd.join(" "));
        }
        let mut gefunden = zuordnen(&lauscher, &eltern, &befehl, &wurzeln);
        if cfg!(windows) {
            gefunden.retain(|(_, d)| lauscht(d.port));
        }
        gefunden
    };
    let mut z = ag.innen.zustand();
    let jetzt = jetzt_ms();
    let mut geaendert = false;
    for e in z.etagen.values_mut() {
        let mut neu: Vec<Dienst> = gefunden.iter().filter(|(et, _)| *et == e.id).map(|(_, d)| d.clone()).collect();
        for d in neu.iter_mut() {
            d.seit = e.dienste.iter().find(|alt| alt.port == d.port && alt.arbeiter == d.arbeiter).map_or(jetzt, |alt| alt.seit);
        }
        if neu != e.dienste {
            geaendert |= neu.iter().map(|d| d.port).ne(e.dienste.iter().map(|d| d.port));
            e.dienste = neu;
            ag.innen.senden(Ziel::Etage(e.id.clone()), ServerNachricht::Agenten(AgSrv::Dienste { liste: e.dienste.clone() }));
        }
    }
    if geaendert {
        let ports = z.etagen.values().flat_map(|e| e.dienste.iter().map(|d| d.port)).collect();
        drop(z);
        ag.innen.dienste_geaendert(ports);
    }
}

/// Nimmt auf diesem Port jemand Verbindungen an (IPv4 oder IPv6, lokal)?
fn lauscht(port: u16) -> bool {
    let warten = Duration::from_millis(150);
    ["127.0.0.1", "[::1]"].iter().any(|h| {
        format!("{h}:{port}").parse().is_ok_and(|a| std::net::TcpStream::connect_timeout(&a, warten).is_ok())
    })
}

pub fn starten(ag: &Agenten) {
    let schwach = ag.innen.selbst();
    ag.innen.laufzeit.spawn(async move {
        let mut system = System::new();
        loop {
            tokio::time::sleep(TAKT).await;
            let Some(innen) = schwach.upgrade() else { return };
            let ag = Agenten { innen };
            system = tokio::task::spawn_blocking(move || {
                runde(&ag, &mut system);
                system
            })
            .await
            .unwrap_or_default();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_gehoeren_dem_arbeiter_dessen_terminal_sie_startete() {
        // 10 = Terminal von Ada (Shell), 20 = Claude von Bob; 11 ← 10, 21 ← 20, 30 gehört niemandem.
        let wurzeln = vec![("e".into(), "a".into(), "Ada".into(), 10, false), ("e".into(), "b".into(), "Bob".into(), 20, true)];
        let eltern = HashMap::from([(11, 10), (12, 11), (21, 20), (30, 1)]);
        let befehl = HashMap::from([(21, "chrome --remote-debugging-port=9222".to_string())]);
        let lauscher = vec![(12, 5173, "node".into()), (20, 4000, "claude".into()), (21, 9222, "chrome".into()), (30, 80, "nginx".into()), (12, 5173, "node".into())];
        let d = zuordnen(&lauscher, &eltern, &befehl, &wurzeln);
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].1.port, d[0].1.name.as_str()), (5173, "Ada"));
    }
}
