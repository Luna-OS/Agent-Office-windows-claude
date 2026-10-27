//! Domäne `einstellungen`: Webhook, Arbeiter-Limit, Updates, Feiertags-Thema, Karte; dazu Himmel und Rechner.

use protokoll::einstellungen::{Client, Gesamt, Server};
use protokoll::praesenz::Stufe;
use protokoll::{ClientNachricht, ServerNachricht};

use super::{Domaene, Zukunft, fertig};
use crate::office::OfficeRef;
use crate::speicher::jetzt_ms;

pub struct EinstellungenDomaene;

fn s(n: Server) -> ServerNachricht {
    ServerNachricht::Einstellungen(n)
}

/// Arbeiter im ganzen Office (von den Anbindungen).
fn arbeiter(o: &OfficeRef) -> u32 {
    let ids: Vec<String> = o
        .etagen
        .liste
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.id.clone())
        .collect();
    ids.iter().map(|id| o.zahlen(id).arbeiter).sum()
}

pub fn maschine_an_alle(o: &OfficeRef) {
    o.an_alle(
        &s(Server::Maschine {
            stand: o.einstellungen.maschine(arbeiter(o)),
        }),
        None,
    );
}

pub fn himmel_an_alle(o: &OfficeRef) {
    o.an_alle(
        &s(Server::Himmel {
            stand: o.einstellungen.himmel.stand(),
        }),
        None,
    );
}

impl Domaene for EinstellungenDomaene {
    fn name(&self) -> &'static str {
        "einstellungen"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Einstellungen(_))
    }

    fn begruessung(&self, o: &OfficeRef, _von: &str) -> Vec<ServerNachricht> {
        let e = &o.einstellungen;
        vec![s(Server::Stand {
            stand: Box::new(Gesamt {
                benachrichtigung: e.benachrichtigung(),
                maschine: e.maschine(arbeiter(o)),
                update: e.update.lock().unwrap().clone(),
                himmel: Some(e.himmel.stand()),
                thema: e.thema(),
                karte: e.karte(),
            }),
        })]
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        let ClientNachricht::Einstellungen(c) = n else {
            return fertig();
        };
        let wer = o.person(von).map(|p| p.name).unwrap_or_default();
        let admin = o.ist_admin(von);
        let e = &o.einstellungen;
        match c {
            Client::Webhook { url } => match e.webhook_setzen(&url, &wer) {
                Ok(()) => o.an_alle(
                    &s(Server::Benachrichtigung {
                        stand: e.benachrichtigung(),
                    }),
                    None,
                ),
                Err(f) => o.hinweis(von, f, Stufe::Warnung),
            },
            Client::WebhookTesten => {
                let o = o.clone();
                let von = von.to_string();
                return Box::pin(async move {
                    let text = format!("👋 Test aus dem Office, geschickt von {wer}");
                    match crate::webhook::senden(&o, &text).await {
                        Ok(()) => o.hinweis(&von, "Testnachricht verschickt", Stufe::Info),
                        Err(f) => o.hinweis(&von, f, Stufe::Fehler),
                    }
                });
            }
            Client::MaschinenLimit { limit } => {
                if !admin {
                    o.hinweis(von, "Nur Admins können das Limit ändern", Stufe::Warnung);
                } else {
                    let limit = match (limit, e.obergrenze) {
                        (Some(l), Some(g)) => Some(l.min(g)),
                        (l, _) => l,
                    };
                    e.aendern(|d| d.arbeiter_limit = limit);
                    maschine_an_alle(o);
                }
            }
            Client::UpdatePruefen | Client::UpdateStarten => {
                let starten = matches!(c, Client::UpdateStarten);
                let skript = o
                    .konfig
                    .deploy_skript
                    .clone()
                    .filter(|_| o.konfig.selbstupdate);
                let Some(skript) = skript else {
                    o.hinweis(
                        von,
                        "Dieses Office kann sich nicht selbst aktualisieren",
                        Stufe::Info,
                    );
                    return fertig();
                };
                if starten && !admin {
                    o.hinweis(
                        von,
                        "Nur Admins können das Office aktualisieren",
                        Stufe::Warnung,
                    );
                    return fertig();
                }
                let o = o.clone();
                return Box::pin(async move {
                    let mut u = o.einstellungen.update.lock().unwrap().clone();
                    if starten {
                        u.phase = protokoll::einstellungen::UpdatePhase::Baut;
                        u.von = Some(wer.clone());
                        tracing::info!("{wer} aktualisiert das Office");
                    } else {
                        u.prueft = true;
                    }
                    *o.einstellungen.update.lock().unwrap() = u.clone();
                    o.an_alle(
                        &s(Server::Update {
                            stand: Box::new(u.clone()),
                        }),
                        None,
                    );
                    let u = if starten {
                        crate::einstellungen::update::starten(&skript, u).await
                    } else {
                        crate::einstellungen::update::pruefen(&skript, u).await
                    };
                    *o.einstellungen.update.lock().unwrap() = u.clone();
                    o.an_alle(&s(Server::Update { stand: Box::new(u) }), None);
                });
            }
            Client::ThemaSetzen { wahl } => {
                e.aendern(|d| {
                    d.thema.wahl = wahl;
                    d.thema.von = Some(wer.clone());
                    d.thema.um = Some(jetzt_ms());
                });
                o.an_alle(&s(Server::Thema { stand: e.thema() }), None);
            }
            Client::KarteSetzen { karte } => {
                if let Some(k) = karte {
                    let k: String = k.chars().take(64).collect();
                    let bekannt =
                        k == "office" || k == "burg" || e.karte().eigene.iter().any(|x| x.id == k);
                    if !bekannt {
                        o.hinweis(von, "Diese Karte gibt es nicht", Stufe::Warnung);
                        return fertig();
                    }
                    e.aendern(|d| {
                        d.karte.wahl = k;
                        d.karte.von = Some(wer.clone());
                        d.karte.um = Some(jetzt_ms());
                    });
                }
                e.karten_neu_lesen(&o.konfig.daten);
                o.an_alle(&s(Server::Karte { stand: e.karte() }), None);
            }
        }
        fertig()
    }
}
