//! Domäne `konten`: Konten und Einladungen verwalten (Admins), Rollen, gemeinsames Passwort,
//! Einmal-Links für den Connector. Eigene Claude-/GitHub-Anmeldungen kommen über eine Anbindung.

use protokoll::konten::{Client, Server};
use protokoll::praesenz::Stufe;
use protokoll::{ClientNachricht, ServerNachricht};

use super::{Domaene, Zukunft, fertig};
use crate::office::OfficeRef;

pub struct KontenDomaene;

fn k(s: Server) -> ServerNachricht {
    ServerNachricht::Konten(s)
}

/// Wo andere Rechner das Office erreichen (für Einladungslinks).
pub fn adressen(o: &OfficeRef) -> Vec<String> {
    let mut a = Vec::new();
    if let Some(h) = &o.konfig.oeffentlicher_host {
        a.push(if h.starts_with("http") { h.clone() } else { format!("https://{h}") });
    }
    if let Some(t) = &o.konfig.tailscale_host {
        a.push(format!("https://{t}"));
    }
    if !o.konfig.nur_lokal() && !o.konfig.host.is_unspecified() {
        a.push(format!("http://{}:{}", o.konfig.host, o.konfig.port));
    }
    a
}

/// Der Konten-Stand an alle Admins (bei jeder Änderung, auch wenn jemand kommt oder geht).
pub fn konten_an_admins(o: &OfficeRef) {
    let stand = o.zugang.konten.lock().unwrap().stand(&o.konten_online(), adressen(o));
    o.an_admins(&k(Server::Konten { stand }));
}

/// Allen ihre (vielleicht neue) Rolle schicken und Abgemeldete hinauswerfen.
fn rollen_neu(o: &OfficeRef) {
    let alle: Vec<(String, crate::anmeldung::Sitzung)> = o.verbindungen.lock().unwrap().iter().map(|(id, v)| (id.clone(), v.sitzung.clone())).collect();
    for (id, s) in alle {
        o.an(&id, &k(Server::Ich { ich: o.zugang.ich(&s) }));
    }
    o.ungueltige_abmelden();
}

impl Domaene for KontenDomaene {
    fn name(&self) -> &'static str {
        "konten"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Konten(c) if !matches!(c,
            Client::AnmeldungenHolen | Client::AnmeldungStarten { .. } | Client::AnmeldungCode { .. } |
            Client::AnmeldungAbbrechen { .. } | Client::AnmeldungToken { .. } | Client::AnmeldungOffice { .. } | Client::Abmelden { .. }))
    }

    fn begruessung(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        if !o.ist_admin(von) {
            return Vec::new();
        }
        let stand = o.zugang.konten.lock().unwrap().stand(&o.konten_online(), adressen(o));
        vec![k(Server::Konten { stand })]
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        let ClientNachricht::Konten(c) = n else { return fertig() };
        if let Client::LinkErzeugen = c {
            if let Some(s) = o.sitzung_von(von) {
                let schluessel = o.zugang.sitzungen.link_schluessel(Some(&s));
                o.an(von, &k(Server::Link { pfad: format!("/anmelden#schluessel={schluessel}") }));
            }
            return fertig();
        }
        if !o.ist_admin(von) {
            o.hinweis(von, "Das dürfen nur Admins", Stufe::Warnung);
            return fertig();
        }
        if matches!(c, Client::TeamHolen | Client::TeamEinladen { .. } | Client::TeamEntfernen { .. }) {
            return Box::pin(team(o, von, c));
        }
        let wer = o.person(von).map(|p| p.name).unwrap_or_default();
        let ergebnis: Result<(), String> = {
            let mut konten = o.zugang.konten.lock().unwrap();
            match c {
                Client::KontenHolen => Ok(()),
                Client::Einladen { name, rolle } => {
                    let r = konten.einladen(&wer, rolle, name.as_deref());
                    drop(konten);
                    match r {
                        Ok(einladung) => o.an(von, &k(Server::Eingeladen { einladung: Some(einladung), fehler: None })),
                        Err(f) => o.an(von, &k(Server::Eingeladen { einladung: None, fehler: Some(f) })),
                    }
                    Ok(())
                }
                Client::EinladungZurueckziehen { einladung } => {
                    konten.einladung_zurueckziehen(&einladung);
                    Ok(())
                }
                Client::Entziehen { konto } => match konten.entziehen(&konto) {
                    Some(weg) => {
                        o.anmeldungen.vergessen(&weg.id);
                        tracing::info!("{wer} hat das Konto {} entzogen", weg.name);
                        Ok(())
                    }
                    None => Err("Dieses Konto gibt es nicht mehr".into()),
                },
                Client::RolleSetzen { konto, rolle } => konten.rolle_setzen(&konto, rolle),
                Client::GemeinsamesPasswort { an } => konten.gemeinsames_passwort_setzen(an),
                Client::ClaudeTeilen { an } => {
                    konten.claude_teilen_setzen(an);
                    Ok(())
                }
                _ => Ok(()),
            }
        };
        if let Err(f) = ergebnis {
            o.hinweis(von, f, Stufe::Warnung);
        }
        rollen_neu(o);
        konten_an_admins(o);
        fertig()
    }
}

/// Das SSH-Team (Admins): Stand, Einladen, Entfernen; danach bekommen alle Admins den neuen Stand.
async fn team(o: &OfficeRef, von: &str, c: Client) {
    match c {
        Client::TeamEinladen { github } => {
            let antwort = match crate::team::einladen(&o.konfig, &github).await {
                Ok((name, schluessel)) => Server::TeamEingeladen { github, name: Some(name), schluessel: Some(schluessel), fehler: None },
                Err(f) => Server::TeamEingeladen { github, name: None, schluessel: None, fehler: Some(f) },
            };
            o.an(von, &k(antwort));
        }
        Client::TeamEntfernen { name } => {
            if let Err(f) = crate::team::entfernen(&o.konfig, &name).await {
                o.hinweis(von, f, Stufe::Warnung);
            }
        }
        _ => {}
    }
    let stand = crate::team::stand(&o.konfig).await;
    o.an_admins(&k(Server::Team { stand }));
}
