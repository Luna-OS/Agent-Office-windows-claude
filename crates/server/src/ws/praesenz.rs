//! Domäne `praesenz`: herumlaufen, etwas in der Hand, sitzen, Karten tragen, Emotes, Profil, Chat, Ping.

use protokoll::praesenz::{ChatZeile, Client, Server};
use protokoll::{ClientNachricht, ServerNachricht};

use super::{Domaene, Zukunft, eingabe, fertig};
use crate::office::{DACH, OfficeRef};
use crate::speicher::jetzt_ms;

pub struct Praesenz;

fn p(s: Server) -> ServerNachricht {
    ServerNachricht::Praesenz(s)
}

impl Domaene for Praesenz {
    fn name(&self) -> &'static str {
        "praesenz"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Praesenz(_))
    }

    fn begruessung(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        let (Some(ich_selbst), Some(s)) = (o.person(von), o.sitzung_von(von)) else {
            return Vec::new();
        };
        vec![p(Server::Willkommen {
            du: von.into(),
            ich_selbst,
            chat: o.chat_letzte(50),
            version: o.version.clone(),
            ich: o.zugang.ich(&s),
            einladungen: crate::team::verfuegbar(&o.konfig) || o.konfig.tailscale_host.is_some(),
        })]
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        if let ClientNachricht::Praesenz(c) = n {
            bearbeiten(o, von, c);
        }
        fertig()
    }
}

/// Gibt es den Sitzplatz ("couch:1") auf dem Dach bzw. auf einer Büro-Etage?
fn sitz_gibt_es(schluessel: &str, auf_dach: bool) -> bool {
    use welt::grundriss::sitze;
    let Some((id, _)) = schluessel.rsplit_once(':') else {
        return false;
    };
    sitze::platz_nach_schluessel(schluessel).is_some()
        && sitze::sitze()
            .iter()
            .any(|s| s.id == id && s.dach == auf_dach)
}

fn geaendert(o: &OfficeRef, von: &str, f: impl FnOnce(&mut protokoll::praesenz::Person) -> bool) {
    let mut anders = false;
    let person = o.person_aendern(von, |p| anders = f(p));
    if anders && let Some(person) = person {
        o.an_alle(&p(Server::PersonGeaendert { person }), Some(von));
    }
}

fn bearbeiten(o: &OfficeRef, von: &str, c: Client) {
    match c {
        Client::Bewegen {
            x,
            y,
            z,
            rot_y,
            laeuft,
        } => {
            let (x, y, z, rot_y) = (
                eingabe::zahl(x, 500.0),
                eingabe::zahl(y, 200.0),
                eingabe::zahl(z, 500.0),
                eingabe::zahl(rot_y, 100.0),
            );
            o.person_aendern(von, |p| {
                (p.x, p.y, p.z, p.rot_y, p.laeuft) = (x, y, z, rot_y, laeuft);
            });
            o.an_nachbarn(
                von,
                &p(Server::PersonBewegt {
                    id: von.into(),
                    x,
                    y,
                    z,
                    rot_y,
                    laeuft,
                }),
            );
        }
        Client::Handlung {
            rauchen,
            golfen,
            getraenk,
            wirft,
        } => {
            let auf_dach = o.etage_von(von).as_deref() == Some(DACH);
            geaendert(o, von, |p| {
                let vorher = (p.raucht, p.golft, p.getraenk, p.wirft);
                if let Some(r) = rauchen {
                    p.raucht = r;
                }
                if let Some(g) = golfen {
                    // Der Abschlag ist auf dem Balkon einer Büro-Etage, nicht auf dem Dach.
                    p.golft = g && !auf_dach;
                }
                if let Some(g) = getraenk {
                    // Getränke gibt es an der Dachbar, und sie bleiben oben.
                    p.getraenk = g.filter(|_| auf_dach);
                }
                if let Some(w) = wirft {
                    p.wirft = w.filter(|_| auf_dach);
                }
                vorher != (p.raucht, p.golft, p.getraenk, p.wirft)
            });
        }
        Client::Sitzen { sitz } => {
            let auf_dach = o.etage_von(von).as_deref() == Some(DACH);
            // Nur Plätze, die es dort gibt, wo man ist.
            let sitz = sitz
                .map(|s| eingabe::text(&s, 40))
                .filter(|s| sitz_gibt_es(s, auf_dach));
            if let Some(s) = &sitz {
                let etage = o.etage_von(von);
                let ich = o.person(von);
                let belegt = o.personen().into_iter().find(|q| {
                    q.id != von
                        && q.etage == etage
                        && q.sitz.as_ref() == Some(s)
                        && ich.as_ref().is_none_or(|i| i.name != q.name)
                });
                if let Some(q) = belegt {
                    o.an(
                        von,
                        &p(Server::SitzBelegt {
                            sitz: s.clone(),
                            von: q.name,
                        }),
                    );
                    return;
                }
            }
            geaendert(o, von, |p| {
                let anders = p.sitz != sitz;
                p.sitz = sitz;
                anders
            });
        }
        Client::Tragen { karte } => {
            let karte = karte.map(|mut k| {
                k.titel = eingabe::text(&k.titel, 200);
                k
            });
            geaendert(o, von, |p| {
                let anders = p.traegt != karte;
                p.traegt = karte;
                anders
            });
        }
        Client::Emote { emote } => {
            let ok = o
                .verbindungen
                .lock()
                .unwrap()
                .get_mut(von)
                .is_some_and(|v| v.emotes.nehmen(jetzt_ms()));
            if ok {
                o.an_nachbarn(
                    von,
                    &p(Server::PersonEmote {
                        id: von.into(),
                        emote,
                    }),
                );
            }
        }
        Client::Profil {
            name,
            farbe,
            aussehen,
        } => {
            let name = crate::anmeldung::konten::name_saeubern(&name);
            let person = o.person_aendern(von, |p| {
                if !name.is_empty() && !p.konto {
                    p.name = name;
                }
                if eingabe::ist_farbe(&farbe) {
                    p.farbe = farbe;
                }
                p.aussehen = aussehen.bereinigt();
            });
            if let Some(person) = person {
                o.an_alle(&p(Server::PersonGeaendert { person }), None);
            }
        }
        Client::Tut { was, liest } => {
            let was = was
                .map(|w| eingabe::text(w.trim(), 60))
                .filter(|w| !w.is_empty());
            geaendert(o, von, |p| {
                let anders = p.tut != was || p.liest != liest;
                (p.tut, p.liest) = (was, liest);
                anders
            });
        }
        Client::Lite { an } => geaendert(o, von, |p| std::mem::replace(&mut p.lite, an) != an),
        Client::Chat { text } => {
            let text = eingabe::text(text.trim(), 500);
            let Some(ich) = o.person(von) else { return };
            if text.is_empty() {
                return;
            }
            let zeile = ChatZeile {
                von: von.into(),
                name: ich.name,
                farbe: ich.farbe,
                text,
                um: jetzt_ms(),
                konto: ich.konto,
            };
            o.chat_merken(zeile.clone());
            o.an_alle(&p(Server::Chat { zeile }), None);
        }
        Client::Ping { um } => o.an(
            von,
            &p(Server::Pong {
                um: if um.is_finite() { um } else { 0.0 },
                jetzt: jetzt_ms(),
            }),
        ),
    }
}
