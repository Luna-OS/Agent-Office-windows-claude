//! Domäne `etagen`: Aufzug (Etage wechseln), Repositories auflisten und klonen, Etagen entfernen,
//! Projekte-Ordner, Schilder und Ausbau des Hinterbüros.

use std::path::PathBuf;
use std::sync::Arc;

use protokoll::etagen::{Client, Server};
use protokoll::praesenz::{self, Ort, Stufe};
use protokoll::{ClientNachricht, ServerNachricht};

use super::verbindung::AUFZUG;
use super::{Domaene, Zukunft, eingabe, fertig};
use crate::etagen::{KlonStart, Zahlen, fluegel_aendern, git, klonen, schild_setzen};
use crate::office::{DACH, OfficeRef};
use crate::speicher::jetzt_ms;

pub struct EtagenDomaene;

fn e(s: Server) -> ServerNachricht {
    ServerNachricht::Etagen(s)
}

/// Die Aufzugtafel an alle.
pub fn liste_an_alle(o: &OfficeRef) {
    o.an_alle(&e(Server::Liste { etagen: liste(o) }), None);
}

fn liste(o: &OfficeRef) -> Vec<protokoll::etagen::EtageInfo> {
    let personen = o.personen_je_etage();
    let zahlen = |id: &str| o.zahlen(id);
    o.etagen
        .infos(&personen, &zahlen as &dyn Fn(&str) -> Zahlen)
}

fn projekte_ordner(o: &OfficeRef) -> (PathBuf, protokoll::etagen::ProjekteOrdner) {
    let eigen = o.einstellungen.d.lock().unwrap().projekte_ordner.clone();
    o.etagen.projekte_ordner(eigen.as_ref())
}

/// Bringt `von` auf `ziel` (`None` = Gebäude ohne Etagen) an `ort` und schickt ihm alles von dort.
pub fn wechseln(o: &OfficeRef, von: &str, ziel: Option<String>, ort: Ort) {
    let alt = o.etage_von(von);
    o.register.etage_verlassen(o, von, alt.as_deref());
    let Some(person) = o.person_aendern(von, |p| {
        p.etage = ziel;
        (p.x, p.y, p.z, p.rot_y) = (ort.x, ort.y, ort.z, ort.rot_y);
        p.laeuft = false;
        p.sitz = None;
        p.traegt = None;
        p.raucht = false;
        p.golft = false;
        p.getraenk = None;
        p.wirft = None;
    }) else {
        return;
    };
    o.register.etage_betreten(o, von);
    o.an_alle(
        &ServerNachricht::Praesenz(praesenz::Server::PersonGeaendert { person }),
        Some(von),
    );
    liste_an_alle(o);
}

impl Domaene for EtagenDomaene {
    fn name(&self) -> &'static str {
        "etagen"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Etagen(_))
    }

    fn begruessung(&self, o: &OfficeRef, _von: &str) -> Vec<ServerNachricht> {
        vec![
            e(Server::Liste { etagen: liste(o) }),
            e(Server::ProjekteOrdner {
                stand: projekte_ordner(o).1,
            }),
        ]
    }

    fn etage_betreten(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        let Some(ich) = o.person(von) else {
            return Vec::new();
        };
        let etage = ich.etage.clone();
        let def = etage.as_deref().and_then(|id| o.etagen.get(id));
        let personen = o.personen().into_iter().filter(|p| p.id != von).collect();
        vec![e(Server::Betreten {
            projekt: etage.as_deref().and_then(|id| o.etagen.projekt(id)),
            grundriss: def.map(|d| d.grundriss).unwrap_or_default(),
            etage,
            personen,
            ort: Ort {
                x: ich.x,
                y: ich.y,
                z: ich.z,
                rot_y: ich.rot_y,
            },
        })]
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        let ClientNachricht::Etagen(c) = n else {
            return fertig();
        };
        match c {
            Client::Gehen { etage, an } => gehen(o, von, etage, an),
            Client::Repos { neu_laden: _ } => {
                let o = Arc::clone(o);
                let von = von.to_string();
                tokio::spawn(async move {
                    let umgebung = o.github_umgebung(&von);
                    // Eigene Repositories, wenn mit der eigenen GitHub-Anmeldung gelistet wird.
                    let eigene = umgebung
                        .iter()
                        .any(|(k, v)| k == "GH_CONFIG_DIR" && !v.is_empty());
                    let antwort = match git::repos_auflisten(&umgebung).await {
                        Ok(repos) => Server::Repos {
                            repos,
                            fehler: None,
                            eigene,
                        },
                        Err(f) => Server::Repos {
                            repos: Vec::new(),
                            fehler: Some(f),
                            eigene,
                        },
                    };
                    o.an(&von, &e(antwort));
                });
            }
            Client::Hinzufuegen { repo } => hinzufuegen(o, von, &repo),
            Client::Abbrechen { etage } => {
                let admin = o.ist_admin(von);
                let name = o.person(von).map(|p| p.name).unwrap_or_default();
                if let Err(f) = o.etagen.klon_abbrechen(&etage, |wer| admin || wer == name) {
                    o.hinweis(von, f, Stufe::Warnung);
                }
            }
            Client::Entfernen { etage } => entfernen(o, von, &etage),
            Client::ProjekteOrdner { ordner } => {
                if !o.ist_admin(von) {
                    return Box::pin(async move {
                        o.hinweis(
                            von,
                            "Nur Admins können den Projekte-Ordner ändern",
                            Stufe::Warnung,
                        )
                    });
                }
                let ordner = ordner.trim().to_string();
                let name = o.person(von).map(|p| p.name).unwrap_or_default();
                o.einstellungen.aendern(|d| {
                    d.projekte_ordner = (!ordner.is_empty()).then(|| (ordner, name, jetzt_ms()))
                });
                o.an_alle(
                    &e(Server::ProjekteOrdner {
                        stand: projekte_ordner(o).1,
                    }),
                    None,
                );
            }
            Client::Schild { tisch, text, farbe } => grundriss(o, von, |g, wer| {
                schild_setzen(g, &tisch, &text, farbe.as_deref(), wer)
            }),
            Client::Erweitern => grundriss(o, von, |g, _| fluegel_aendern(g, 1)),
            Client::Verkleinern => grundriss(o, von, |g, _| fluegel_aendern(g, -1)),
        }
        fertig()
    }
}

fn gehen(o: &OfficeRef, von: &str, etage: String, an: Option<Ort>) {
    let ziel = if etage == DACH {
        if o.etagen.anzahl() == 0 {
            return o.hinweis(von, "Ohne Etagen gibt es noch kein Dach", Stufe::Info);
        }
        DACH.to_string()
    } else if o.etagen.gibt_es(&etage) {
        etage
    } else if o.etagen.klone.lock().unwrap().contains_key(&etage) {
        return o.hinweis(von, "Diese Etage wird noch geklont", Stufe::Info);
    } else {
        return o.hinweis(von, "Diese Etage gibt es nicht mehr", Stufe::Warnung);
    };
    let ort = an
        .map(|a| Ort {
            x: eingabe::zahl(a.x, 200.0),
            y: eingabe::zahl(a.y, 100.0),
            z: eingabe::zahl(a.z, 200.0),
            rot_y: eingabe::zahl(a.rot_y, 100.0),
        })
        .unwrap_or(AUFZUG);
    wechseln(o, von, Some(ziel), ort);
}

fn grundriss(
    o: &OfficeRef,
    von: &str,
    f: impl FnOnce(&mut protokoll::etagen::Grundriss, &str) -> bool,
) {
    let Some(p) = o.person(von) else { return };
    let Some(etage) = p.etage.filter(|e| e != DACH) else {
        return;
    };
    if let Some(g) = o.etagen.grundriss_aendern(&etage, |g| f(g, &p.name)) {
        o.fluegel_geaendert(&etage, g.fluegel);
        o.an_etage(Some(&etage), &e(Server::Grundriss { grundriss: g }), None);
        liste_an_alle(o);
    }
}

fn hinzufuegen(o: &OfficeRef, von: &str, eingabe_repo: &str) {
    let Some(repo) = git::repo_aus(eingabe_repo) else {
        return o.an(
            von,
            &e(Server::Hinzugefuegt {
                repo: eingabe_repo.into(),
                etage: None,
                fehler: Some("Das ist kein GitHub-Repository (owner/name oder URL)".into()),
            }),
        );
    };
    let name = o.person(von).map(|p| p.name).unwrap_or_default();
    let (projekte, _) = projekte_ordner(o);
    match o.etagen.klon_anmelden(&repo, &projekte, &name, von) {
        Err(f) => o.an(
            von,
            &e(Server::Hinzugefuegt {
                repo,
                etage: None,
                fehler: Some(f),
            }),
        ),
        Ok(KlonStart::Vorhanden(ordner)) => {
            let def = o.etagen.uebernehmen(&repo, ordner, &name);
            o.etage_angemeldet(&def);
            o.an(
                von,
                &e(Server::Hinzugefuegt {
                    repo,
                    etage: Some(def.id),
                    fehler: None,
                }),
            );
            liste_an_alle(o);
        }
        Ok(KlonStart::Klonen(def, abbruch)) => {
            tracing::info!("{name} klont {repo} nach {}", def.ordner.display());
            liste_an_alle(o);
            let o = Arc::clone(o);
            let umgebung = o.github_umgebung(von);
            tokio::spawn(async move {
                let id = def.id.clone();
                let melden = {
                    let o = Arc::clone(&o);
                    let id = id.clone();
                    move |f| {
                        o.etagen.klon_fortschritt(&id, f);
                        liste_an_alle(&o);
                    }
                };
                let ende = klonen::klonen(&repo, &def.ordner, &umgebung, melden, abbruch).await;
                let erfolg = matches!(ende, klonen::Ende::Fertig);
                let lauf = o.etagen.klon_beenden(&id, erfolg);
                let an = lauf.map(|l| l.verbindung).unwrap_or_default();
                let antwort = match ende {
                    klonen::Ende::Fertig => {
                        o.etage_angemeldet(&def);
                        tracing::info!("{repo} ist geklont: Etage {id}");
                        Server::Hinzugefuegt {
                            repo,
                            etage: Some(id),
                            fehler: None,
                        }
                    }
                    klonen::Ende::Abgebrochen => Server::Hinzugefuegt {
                        repo,
                        etage: None,
                        fehler: Some("Der Klon wurde abgebrochen".into()),
                    },
                    klonen::Ende::Fehler(f) => {
                        tracing::warn!("Klon von {repo} fehlgeschlagen: {f}");
                        Server::Hinzugefuegt {
                            repo,
                            etage: None,
                            fehler: Some(f),
                        }
                    }
                };
                o.an(&an, &e(antwort));
                liste_an_alle(&o);
            });
        }
    }
}

fn entfernen(o: &OfficeRef, von: &str, etage: &str) {
    if !o.ist_admin(von) {
        return o.hinweis(von, "Nur Admins können Etagen entfernen", Stufe::Warnung);
    }
    let Some(def) = o.etagen.entfernen(etage) else {
        return o.hinweis(von, "Diese Etage gibt es nicht mehr", Stufe::Warnung);
    };
    o.etage_abgemeldet(&def.id);
    let wer = o.person(von).map(|p| p.name).unwrap_or_default();
    tracing::info!(
        "{wer} hat die Etage {} entfernt (Checkout bleibt in {})",
        def.name,
        def.ordner.display()
    );
    // Alle, die dort waren, fahren auf die erste Etage (oder ins Gebäude ohne Etagen).
    let ziel = o.etagen.erste().map(|e| e.id);
    let dort: Vec<String> = o
        .personen()
        .into_iter()
        .filter(|p| p.etage.as_deref() == Some(etage))
        .map(|p| p.id)
        .collect();
    for id in dort {
        o.hinweis(
            &id,
            format!("{wer} hat die Etage {} entfernt", def.name),
            Stufe::Info,
        );
        wechseln(o, &id, ziel.clone(), AUFZUG);
    }
    if o.etagen.anzahl() == 0 {
        // Kein Projekt mehr: wer auf dem Dach war, kommt herunter.
        let oben: Vec<String> = o
            .personen()
            .into_iter()
            .filter(|p| p.etage.as_deref() == Some(DACH))
            .map(|p| p.id)
            .collect();
        for id in oben {
            wechseln(o, &id, None, AUFZUG);
        }
    }
    liste_an_alle(o);
}
