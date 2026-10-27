//! Den Pull Request eines Arbeiters öffnen: Branch pushen, PR anlegen (oder den vorhandenen finden),
//! bei einem Arbeiter über mehrere Repositories je Repository einen.

use crate::{Agenten, github, worktree};

pub async fn oeffnen(ag: &Agenten, etage: &str, id: &str) -> Result<(), String> {
    let (pfad, wt, titel, konto) = {
        let mut z = ag.innen.zustand();
        let e = z
            .etagen
            .get_mut(etage)
            .ok_or("Diese Etage gibt es nicht (mehr)")?;
        let dir = e.verzeichnis.clone();
        let a = e
            .arbeiter
            .get_mut(id)
            .ok_or("Diesen Arbeiter gibt es nicht")?;
        let wt = a
            .info
            .worktree
            .clone()
            .ok_or("Dieser Arbeiter hat keinen eigenen Worktree")?;
        if a.info.pr_oeffnet {
            return Err("Der Pull Request wird schon geöffnet".into());
        }
        a.info.pr_oeffnet = true;
        let titel = a.info.aufgabe.as_ref().map(|t| t.name.clone());
        let konto = a.info.konto.clone();
        ag.innen.arbeiter_melden(&z.etagen[etage], id);
        (worktree::absolut(&dir, &wt.pfad), wt, titel, konto)
    };
    let umgebung = ag.innen.umgebung_von(konto.as_deref());
    let ergebnis = github::pr_oeffnen(
        &pfad,
        &wt.branch,
        wt.von.as_deref(),
        titel.as_deref(),
        &umgebung,
    )
    .await;
    {
        let mut z = ag.innen.zustand();
        if let Some(a) = z.arbeiter_mut(etage, id) {
            a.info.pr_oeffnet = false;
            if let Ok((nummer, url)) = &ergebnis {
                a.info.pr = Some(protokoll::arbeiter::PrVerweis {
                    nummer: *nummer,
                    url: url.clone(),
                });
            }
            ag.innen.arbeiter_melden(&z.etagen[etage], id);
            crate::speicher::arbeiter_speichern(&ag.innen.daten, &z.etagen[etage]);
        }
    }
    // Ein Arbeiter über mehrere Repositories: je Repository ein eigener Pull Request.
    let fehler = crate::mehrrepo::prs_oeffnen(ag, etage, id, titel.as_deref(), &umgebung).await;
    if let Some(e) = ag.innen.zustand().etagen.get(etage) {
        ag.innen.arbeiter_melden(e, id);
        crate::speicher::arbeiter_speichern(&ag.innen.daten, e);
    }
    let mut meldung: Vec<String> = ergebnis.err().map(|e| e.to_string()).into_iter().collect();
    meldung.extend(fehler);
    if meldung.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Pull Request konnte nicht geöffnet werden: {}",
            meldung.join("; ")
        ))
    }
}
