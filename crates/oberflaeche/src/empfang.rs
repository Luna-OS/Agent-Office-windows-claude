//! Nimmt die Frames vom Server und legt sie in den [`Zustand`](crate::zustand::Zustand).
//! Jede Domäne hat hier ihre eigene kleine Funktion; Terminal-Daten gehen an [`crate::terminal`].

use leptos::prelude::*;
use protokoll::{
    ServerNachricht, agenten, arbeiter, aufgaben, einstellungen, etagen, konten, praesenz,
};

use crate::zustand::{melden, zustand};

/// Ein Frame vom Server.
pub fn empfangen(n: &ServerNachricht) {
    match n {
        ServerNachricht::Praesenz(m) => praesenz(m),
        ServerNachricht::Etagen(m) => etagen(m),
        ServerNachricht::Arbeiter(m) => arbeiter(m),
        ServerNachricht::Agenten(m) => agenten(m),
        ServerNachricht::Aufgaben(m) => aufgaben(m),
        ServerNachricht::Einstellungen(m) => einstellungen(m),
        ServerNachricht::Konten(m) => konten(m),
        ServerNachricht::Github(m) => crate::fenster::github::empfangen(m),
        ServerNachricht::Medien(m) => medien(m),
        ServerNachricht::Spielzeug(m) => {
            crate::fenster::whiteboard::empfangen(m);
            crate::fenster::arcade::empfangen(m);
        }
        ServerNachricht::Dach(m) => dach(m),
        ServerNachricht::Fehler { text } => melden(text.clone(), praesenz::Stufe::Fehler),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

fn praesenz(m: &praesenz::Server) {
    let z = zustand();
    use praesenz::Server as S;
    match m {
        S::Willkommen {
            du,
            ich_selbst,
            chat,
            version,
            ich,
            einladungen,
            ..
        } => {
            let alt = z.version.get_untracked();
            if !alt.is_empty() && &alt != version {
                melden("Das Office wurde aktualisiert.", praesenz::Stufe::Info);
            }
            z.du.set(Some(du.clone()));
            z.ich_selbst.set(Some(ich_selbst.clone()));
            z.chat.set(chat.clone());
            z.version.set(version.clone());
            z.ich.set(ich.clone());
            z.einladungen.set(*einladungen);
        }
        S::PersonDa { person } | S::PersonGeaendert { person } => {
            if z.du
                .with_untracked(|d| d.as_deref() == Some(person.id.as_str()))
            {
                z.ich_selbst.set(Some(person.clone()));
                return;
            }
            z.personen
                .update(|l| match l.iter_mut().find(|p| p.id == person.id) {
                    Some(p) => *p = person.clone(),
                    None => l.push(person.clone()),
                });
        }
        S::PersonWeg { id } => z.personen.update(|l| l.retain(|p| &p.id != id)),
        S::Chat { zeile } => z.chat.update(|l| {
            l.push(zeile.clone());
            let zuviel = l.len().saturating_sub(200);
            l.drain(..zuviel);
        }),
        S::Hinweis { text, stufe } => melden(text.clone(), *stufe),
        S::SitzBelegt { von, .. } => {
            melden(format!("Da sitzt schon {von}."), praesenz::Stufe::Info)
        }
        _ => {}
    }
}

fn etagen(m: &etagen::Server) {
    let z = zustand();
    use etagen::Server as S;
    match m {
        S::Betreten {
            etage,
            projekt,
            grundriss,
            personen,
            ..
        } => {
            z.etage.set(etage.clone());
            z.projekt.set(projekt.clone());
            z.grundriss.set(grundriss.clone());
            z.personen.set(personen.clone());
        }
        S::Liste { etagen } => z.etagen.set(etagen.clone()),
        S::Repos { repos, fehler, .. } => z.repos.set(Some((repos.clone(), fehler.clone()))),
        S::Hinzugefuegt { repo, fehler, .. } => match fehler {
            Some(f) => melden(
                format!("{repo} ließ sich nicht hinzufügen: {f}"),
                praesenz::Stufe::Fehler,
            ),
            None => melden(
                format!("{repo} ist jetzt eine Etage."),
                praesenz::Stufe::Info,
            ),
        },
        S::Grundriss { grundriss } => z.grundriss.set(grundriss.clone()),
        _ => {}
    }
}

fn arbeiter(m: &arbeiter::Server) {
    let z = zustand();
    use arbeiter::Server as S;
    match m {
        S::Liste { arbeiter } => z.arbeiter.set(arbeiter.clone()),
        S::Aktualisiert { arbeiter } => {
            crate::terminal::groesse(&arbeiter.id, arbeiter.spalten, arbeiter.zeilen);
            z.arbeiter
                .update(|l| match l.iter_mut().find(|a| a.id == arbeiter.id) {
                    Some(a) => *a = (**arbeiter).clone(),
                    None => l.push((**arbeiter).clone()),
                })
        }
        S::Entfernt { arbeiter } => z.arbeiter.update(|l| l.retain(|a| &a.id != arbeiter)),
        S::WorktreeStand { arbeiter, stand } => {
            z.worktree.set(Some((arbeiter.clone(), stand.clone())))
        }
        S::TerminalSnapshot {
            arbeiter,
            daten,
            spalten,
            zeilen,
        } => crate::terminal::snapshot(arbeiter, daten, *spalten, *zeilen),
        S::TerminalDaten { arbeiter, daten } => crate::terminal::daten(arbeiter, daten),
        S::Provider { provider, standard } => z.provider.set((provider.clone(), standard.clone())),
        S::Aenderungen { stand } => crate::fenster::aenderungen::stand_erhalten(stand),
        S::Diff {
            arbeiter,
            datei,
            diff,
            gekuerzt,
            fehler,
        } => crate::fenster::aenderungen::diff_erhalten(crate::fenster::aenderungen::Diff {
            arbeiter: arbeiter.clone(),
            datei: datei.clone(),
            diff: diff.clone(),
            gekuerzt: *gekuerzt,
            fehler: fehler.clone(),
        }),
        S::Einstellungen {
            nach_merge_gehen, ..
        } => z.nach_merge_gehen.set(*nach_merge_gehen),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

fn agenten(m: &agenten::Server) {
    let z = zustand();
    use agenten::Server as S;
    match m {
        S::Stand { nachrichten, feed } => {
            z.bus.set(nachrichten.clone());
            z.feed.set(feed.clone());
        }
        S::Nachricht { nachricht } => z.bus.update(|l| {
            l.push(nachricht.clone());
            let zuviel = l.len().saturating_sub(300);
            l.drain(..zuviel);
        }),
        S::Feed { eintrag } => {
            z.feed.update(
                |l| match l.iter_mut().find(|e| e.arbeiter == eintrag.arbeiter) {
                    Some(e) => *e = eintrag.clone(),
                    None => l.push(eintrag.clone()),
                },
            )
        }
        S::FeedEntfernt { arbeiter } => z.feed.update(|l| l.retain(|e| &e.arbeiter != arbeiter)),
        S::Nutzung { stand } => z.nutzung.set(Some(stand.clone())),
        S::Limits { stand } => z.limits.set(Some(stand.clone())),
        S::Prompts { liste } => z.prompts.set(liste.clone()),
        S::Dienste { liste } => z.dienste.set(liste.clone()),
        S::Besprechung { stand } => z.besprechung.set(stand.clone()),
        S::Kerker { stand } => z.kerker.set(stand.clone()),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

fn aufgaben(m: &aufgaben::Server) {
    #[allow(irrefutable_let_patterns)]
    if let aufgaben::Server::Stand { board } = m {
        zustand().board.set(board.clone());
    }
}

fn einstellungen(m: &einstellungen::Server) {
    let e = zustand().einstellungen;
    use einstellungen::Server as S;
    match m {
        S::Stand { stand } => e.set(Some((**stand).clone())),
        S::Benachrichtigung { stand } => e.update(|s| {
            s.iter_mut()
                .for_each(|s| s.benachrichtigung = stand.clone())
        }),
        S::Maschine { stand } => {
            e.update(|s| s.iter_mut().for_each(|s| s.maschine = stand.clone()))
        }
        S::Update { stand } => {
            e.update(|s| s.iter_mut().for_each(|s| s.update = (**stand).clone()))
        }
        S::Himmel { stand } => {
            e.update(|s| s.iter_mut().for_each(|s| s.himmel = Some(stand.clone())))
        }
        S::Thema { stand } => e.update(|s| s.iter_mut().for_each(|s| s.thema = stand.clone())),
        S::Karte { stand } => e.update(|s| s.iter_mut().for_each(|s| s.karte = stand.clone())),
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

fn konten(m: &konten::Server) {
    let z = zustand();
    use konten::Server as S;
    match m {
        S::Konten { stand } => z.konten.set(Some(stand.clone())),
        S::Eingeladen { einladung, fehler } => {
            crate::fenster::team::eingeladen(einladung.clone(), fehler.clone())
        }
        S::Ich { ich } => z.ich.set(ich.clone()),
        S::Anmeldungen { stand } => z.anmeldungen.set(Some(stand.clone())),
        S::AnmeldungNoetig { warum, .. } => {
            melden(warum.clone(), praesenz::Stufe::Warnung);
            crate::modal::oeffnen(crate::modal::Fenster::Konten);
        }
        S::Link { pfad } => crate::fenster::konten::link_erhalten(pfad),
        S::Team { stand } => z.team_ssh.set(Some(stand.clone())),
        S::TeamEingeladen { github, fehler, .. } => match fehler {
            Some(f) => melden(f.clone(), praesenz::Stufe::Fehler),
            None => melden(
                format!("{github} hat jetzt SSH-Zugang."),
                praesenz::Stufe::Info,
            ),
        },
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

fn medien(m: &protokoll::medien::Server) {
    let z = zustand();
    use protokoll::medien::Server as S;
    match m {
        S::Bildschirme { liste } => z.bildschirme.set(liste.clone()),
        S::Jukebox { zustand } => z.jukebox.set(Some(zustand.clone())),
        _ => {}
    }
}

fn dach(m: &protokoll::dach::Server) {
    #[allow(irrefutable_let_patterns)]
    if let protokoll::dach::Server::Leinwand { zustand: l } = m {
        zustand().leinwand.set(l.clone());
    }
}
