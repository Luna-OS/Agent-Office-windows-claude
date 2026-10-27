//! Die Wache: schaut alle paar Sekunden nach Agenten ohne Hooks. Wer arbeitet, aber eine Weile
//! nichts mehr ausgibt, gilt als fertig – so bekommt er auch seine wartenden Nachrichten.

use std::time::Duration;

use protokoll::arbeiter::{ArbeiterArt, ArbeiterStatus};

use crate::{Agenten, kommunikation};

const TAKT: Duration = Duration::from_secs(2);
/// So lange still, und ein Agent ohne Hooks ist fertig.
pub const RUHE: Duration = Duration::from_secs(8);

pub fn starten(ag: &Agenten) {
    let schwach = ag.innen.selbst();
    ag.innen.laufzeit.spawn(async move {
        loop {
            tokio::time::sleep(TAKT).await;
            let Some(innen) = schwach.upgrade() else {
                return;
            };
            runde(&Agenten { innen });
        }
    });
}

fn runde(ag: &Agenten) {
    let mut z = ag.innen.zustand();
    let mut ruhig = Vec::new();
    for e in z.etagen.values_mut() {
        for a in e.arbeiter.values_mut() {
            let still = a.letzte_ausgabe.is_none_or(|t| t.elapsed() >= RUHE);
            if !a.hat_hooks
                && a.laeuft()
                && a.info.art == ArbeiterArt::Agent
                && a.info.status == ArbeiterStatus::Arbeitet
                && still
            {
                a.status_setzen(ArbeiterStatus::Fertig);
                ruhig.push((e.id.clone(), a.info.id.clone()));
            }
        }
    }
    for (etage, id) in ruhig {
        kommunikation::zustellen(ag, &mut z, &etage, &id);
        ag.innen.arbeiter_melden(&z.etagen[&etage], &id);
    }
}
