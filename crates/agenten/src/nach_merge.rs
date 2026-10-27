//! Pull Requests der Arbeiter verfolgen: einen PR, den ein Agent selbst geöffnet hat, an seinem
//! Branch erkennen; und – wenn eingeschaltet – Arbeiter heimschicken, deren PR gemergt ist.
//! Die Einstellung gilt fürs ganze Office und liegt in `<daten>/nach-merge.json`.

use protokoll::ServerNachricht;
use protokoll::arbeiter::{ArbeiterStatus, Aufraeumen, PrVerweis, Server as ArbSrv};
use serde::{Deserialize, Serialize};

use crate::{Absender, Agenten, Ziel, kommunikation, speicher, steuerung};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NachMerge {
    pub an: bool,
    #[serde(default)]
    pub von: Option<String>,
}

pub fn laden(daten: &std::path::Path) -> NachMerge {
    std::fs::read_to_string(daten.join("nach-merge.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn nachricht(n: &NachMerge) -> ServerNachricht {
    ServerNachricht::Arbeiter(ArbSrv::Einstellungen { nach_merge_gehen: n.an, von: n.von.clone() })
}

pub fn setzen(ag: &Agenten, von: &Absender, an: bool) {
    let n = NachMerge { an, von: Some(von.name.clone()) };
    if let Ok(text) = serde_json::to_string_pretty(&n) {
        let _ = std::fs::write(ag.innen.daten.join("nach-merge.json"), text);
    }
    ag.innen.senden(Ziel::Alle, nachricht(&n));
    let etagen: Vec<String> = {
        let mut z = ag.innen.zustand();
        z.nach_merge = n;
        z.etagen.keys().cloned().collect()
    };
    if an {
        for e in etagen {
            pruefen(ag, &e);
        }
    }
}

/// Nach jedem GitHub-Abgleich: PRs den Arbeitern zuordnen und Gemergte heimschicken.
pub fn pruefen(ag: &Agenten, etage: &str) {
    let mut heim = Vec::new();
    {
        let mut z = ag.innen.zustand();
        let an = z.nach_merge.an;
        let Some(e) = z.etagen.get_mut(etage) else { return };
        let prs = e.github.prs.clone();
        let mut geaendert = Vec::new();
        for a in e.arbeiter.values_mut() {
            let Some(branch) = a.info.worktree.as_ref().map(|w| w.branch.clone()) else { continue };
            let pr = match &a.info.pr {
                Some(p) => prs.iter().find(|x| x.nummer == p.nummer),
                None => prs.iter().find(|x| x.branch == branch && x.zustand == "OPEN"),
            };
            let Some(pr) = pr else { continue };
            if a.info.pr.is_none() {
                a.info.pr = Some(PrVerweis { nummer: pr.nummer, url: pr.url.clone() });
                geaendert.push(a.info.id.clone());
            }
            let beschaeftigt = matches!(a.info.status, ArbeiterStatus::Arbeitet | ArbeiterStatus::Wartet | ArbeiterStatus::Startet);
            if an && pr.zustand == "MERGED" && !beschaeftigt {
                heim.push((a.info.id.clone(), a.info.name.clone(), pr.nummer));
            }
        }
        for id in &geaendert {
            ag.innen.arbeiter_melden(e, id);
        }
        if !geaendert.is_empty() {
            speicher::arbeiter_speichern(&ag.innen.daten, e);
        }
    }
    if heim.is_empty() {
        return;
    }
    let (ag, etage) = (ag.clone(), etage.to_string());
    ag.innen.laufzeit.clone().spawn(async move {
        for (id, name, nummer) in heim {
            if steuerung::stoppen(&ag, &etage, &id, Aufraeumen::Alles).await.is_ok() {
                let text = format!("{name} ist heimgegangen: PR #{nummer} ist gemergt.");
                ag.innen.melden(&etage, &name, crate::MeldungArt::PrGemergt, text.clone());
                let _ = kommunikation::senden(&ag, &etage, "Office", None, None, &text);
            }
        }
    });
}
