//! Nach einem Neustart des Servers: Terminals, die im PTY-Wirt weitergelaufen sind, wieder an ihre
//! Arbeiter hängen – samt Bildschirm, Hook-Token (der Agent hat ihn in seiner Umgebung) und Status.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use protokoll::arbeiter::{ArbeiterArt, ArbeiterStatus};
use serde::{Deserialize, Serialize};

use crate::terminal::Terminal;
use crate::{Agenten, provider, speicher, start};

/// Was `workers.json` über ein Terminal im Wirt festhält.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WirtStand {
    pub sitzung: String,
    pub token: String,
    pub status: ArbeiterStatus,
}

/// Hängt die gespeicherten Wirt-Sitzungen einer Etage wieder an (im Hintergrund).
pub fn uebernehmen(ag: &Agenten, etage: &str, staende: HashMap<String, WirtStand>) {
    let Some(wirt) = ag.innen.wirt.clone() else {
        return;
    };
    if staende.is_empty() {
        return;
    }
    let (ag, etage) = (ag.clone(), etage.to_string());
    ag.innen.laufzeit.clone().spawn(async move {
        for (id, stand) in staende {
            if !wirt.vorhanden.contains(&stand.sitzung) {
                continue;
            }
            let lauf = {
                let mut z = ag.innen.zustand();
                let Some(a) = z.arbeiter_mut(&etage, &id) else {
                    continue;
                };
                a.lauf += 1;
                a.lauf
            };
            let schwach = Arc::downgrade(&ag.innen);
            let schwach2 = schwach.clone();
            let (e1, i1, e2, i2) = (etage.clone(), id.clone(), etage.clone(), id.clone());
            let Some((pty, u)) = wirt
                .anhaengen(
                    &stand.sitzung,
                    move |daten, antwort| {
                        start::bei_daten(&schwach, &e1, &i1, lauf, daten, antwort)
                    },
                    move |code| start::bei_ende(&schwach2, &e2, &i2, lauf, code),
                )
                .await
            else {
                continue;
            };
            let mut z = ag.innen.zustand();
            let Some(a) = z.arbeiter_mut(&etage, &id) else {
                continue;
            };
            if a.lauf != lauf {
                continue;
            }
            a.terminal = Terminal::neu(u.spalten, u.zeilen);
            a.terminal.einspeisen(u.bildschirm.as_bytes());
            a.info.spalten = u.spalten;
            a.info.zeilen = u.zeilen;
            a.info.titel = u.titel.or(a.info.titel.take());
            a.pty = Some(pty);
            a.token = stand.token;
            a.gestartet = Some(Instant::now() - std::time::Duration::from_secs(3600));
            a.hat_hooks = a.info.art == ArbeiterArt::Agent
                && a.info
                    .provider
                    .as_deref()
                    .and_then(provider::finden)
                    .is_some_and(|p| p.hat_hooks());
            let status = match (stand.status, u.arbeitet) {
                (_, Some(true)) => ArbeiterStatus::Arbeitet,
                (
                    ArbeiterStatus::Offline | ArbeiterStatus::Beendet | ArbeiterStatus::Startet,
                    _,
                ) => ArbeiterStatus::Bereit,
                (s, _) => s,
            };
            a.info.status = ArbeiterStatus::Offline;
            a.status_setzen(status);
            let e = &z.etagen[&etage];
            ag.innen.arbeiter_melden(e, &id);
            speicher::arbeiter_speichern(&ag.innen.daten, e);
        }
    });
}
