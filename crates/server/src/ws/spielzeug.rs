//! Domäne `spielzeug`: Bilder, Hund, Basketball, Autos, Arcade, Whiteboard, Golf und Dachbar.

use protokoll::praesenz::Stufe;
use protokoll::spielzeug::{Arcade, ArcadeSpieler, AutoSitz, Bestenliste, Client, Dekor, Server};
use protokoll::{ClientNachricht, ServerNachricht};

use super::{Domaene, Zukunft, eingabe, fertig};
use crate::office::OfficeRef;
use crate::speicher::{jetzt_ms, zufalls_id};
use crate::spielzeug::{MAX_DEKOR, ball, whiteboard};

pub struct SpielzeugDomaene;

fn sz(s: Server) -> ServerNachricht {
    ServerNachricht::Spielzeug(s)
}

fn arcade_stand(o: &OfficeRef, etage: Option<&str>) -> Arcade {
    let bestenliste = o.spielzeug.bestenliste.lock().unwrap().clone();
    o.spielzeug.mit(etage, |e| Arcade { spieler: e.arcade_spieler.clone(), bestenliste, bild: None })
}

/// Meldet einer Etage, was jemand losgelassen hat (gegangen oder Etage gewechselt).
fn loslassen(o: &OfficeRef, von: &str, etage: Option<&str>) {
    let l = o.spielzeug.loslassen(etage, von);
    let jetzt = jetzt_ms();
    if l.ball {
        let b = o.spielzeug.mit(etage, |e| e.ball.clone());
        o.an_etage(etage, &sz(Server::Ball { ball: b }), None);
    }
    if l.autos {
        let autos = o.spielzeug.mit(etage, |e| e.autos.clone());
        o.an_etage(etage, &sz(Server::Autos { autos, antwort: None }), None);
    }
    if l.arcade {
        o.an_etage(etage, &sz(Server::Arcade { arcade: arcade_stand(o, etage) }), None);
    }
    if l.whiteboard {
        let leute = o.spielzeug.mit(etage, |e| e.wb_leute.clone());
        o.an_etage(etage, &sz(Server::WbLeute { leute }), None);
    }
    if l.hund {
        let h = o.spielzeug.mit(etage, |e| e.hund.zustand(jetzt));
        o.an_etage(etage, &sz(Server::Hund { hund: h }), None);
    }
}

/// Lässt die Hunde aller Etagen mit Menschen weiterlaufen (jede Sekunde aufrufen).
pub fn hunde_ticken(o: &OfficeRef) {
    let jetzt = jetzt_ms();
    let personen = o.personen();
    let mut etagen: Vec<Option<String>> = personen.iter().map(|p| p.etage.clone()).filter(|e| e.as_deref() != Some(crate::office::DACH)).collect();
    etagen.sort();
    etagen.dedup();
    for etage in etagen {
        let leute: Vec<(String, f32, f32)> = personen.iter().filter(|p| p.etage == etage && !p.lite).map(|p| (p.id.clone(), p.x, p.z)).collect();
        let neu = o.spielzeug.mit(etage.as_deref(), |e| e.hund.ticken(jetzt, &leute).then(|| e.hund.zustand(jetzt)));
        if let Some(h) = neu {
            o.an_etage(etage.as_deref(), &sz(Server::Hund { hund: h }), None);
        }
    }
}

impl Domaene for SpielzeugDomaene {
    fn name(&self) -> &'static str {
        "spielzeug"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Spielzeug(_))
    }

    fn etage_betreten(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        let etage = o.etage_von(von);
        vec![sz(Server::Stand { stand: Box::new(o.spielzeug.stand(etage.as_deref(), jetzt_ms())) })]
    }

    fn etage_verlassen(&self, o: &OfficeRef, von: &str, etage: Option<&str>) {
        loslassen(o, von, etage);
    }

    fn weg(&self, o: &OfficeRef, von: &str, etage: Option<&str>) {
        loslassen(o, von, etage);
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        if let ClientNachricht::Spielzeug(c) = n {
            bearbeiten(o, von, c);
        }
        fertig()
    }
}

fn bearbeiten(o: &OfficeRef, von: &str, c: Client) {
    let Some(ich) = o.person(von) else { return };
    let etage = ich.etage.clone();
    let et = etage.as_deref();
    let jetzt = jetzt_ms();
    let an_etage = |n: Server| o.an_etage(et, &sz(n), None);
    let an_andere = |n: Server| o.an_etage(et, &sz(n), Some(von));
    match c {
        Client::DekorHinzufuegen { mut platz } => {
            if !eingabe::ist_url(&platz.url) {
                return o.hinweis(von, "Das Bild braucht eine http(s)-Adresse", Stufe::Warnung);
            }
            platz.titel = platz.titel.map(|t| eingabe::text(&t, 80));
            let d = Dekor { id: zufalls_id(8), platz, von: ich.name.clone(), um: jetzt };
            let liste = o.spielzeug.aendern(et, |e| {
                if e.g.dekor.len() >= MAX_DEKOR {
                    return None;
                }
                e.g.dekor.push(d);
                Some(e.g.dekor.clone())
            });
            match liste {
                Some(liste) => an_etage(Server::Dekor { liste }),
                None => o.hinweis(von, "An den Wänden ist kein Platz mehr", Stufe::Warnung),
            }
        }
        Client::DekorAendern { id, aenderung: a } => {
            if a.url.as_deref().is_some_and(|u| !eingabe::ist_url(u)) {
                return o.hinweis(von, "Das Bild braucht eine http(s)-Adresse", Stufe::Warnung);
            }
            let liste = o.spielzeug.aendern(et, |e| {
                let d = e.g.dekor.iter_mut().find(|d| d.id == id)?;
                let p = &mut d.platz;
                if let Some(v) = a.url { p.url = v }
                if let Some(v) = a.titel { p.titel = Some(eingabe::text(&v, 80)) }
                if let Some(v) = a.wand { p.wand = v }
                if let Some(v) = a.u { p.u = eingabe::zahl(v, 50.0) }
                if let Some(v) = a.y { p.y = eingabe::zahl(v, 20.0) }
                if let Some(v) = a.b { p.b = eingabe::zahl(v, 10.0).abs() }
                if let Some(v) = a.h { p.h = eingabe::zahl(v, 10.0).abs() }
                if let Some(v) = a.rahmen { p.rahmen = v.min(32) }
                Some(e.g.dekor.clone())
            });
            if let Some(liste) = liste {
                an_etage(Server::Dekor { liste });
            }
        }
        Client::DekorEntfernen { id } => {
            let liste = o.spielzeug.aendern(et, |e| {
                e.g.dekor.retain(|d| d.id != id);
                e.g.dekor.clone()
            });
            an_etage(Server::Dekor { liste });
        }
        Client::HundStreicheln => {
            let h = o.spielzeug.mit(et, |e| e.hund.streicheln(jetzt, &ich.name, ich.x, ich.z).then(|| e.hund.zustand(jetzt)));
            if let Some(hund) = h {
                an_etage(Server::Hund { hund });
            }
        }
        Client::HundName { name } => {
            let hund = o.spielzeug.aendern(et, |e| {
                e.hund.umbenennen(&name);
                e.g.hund_name = e.hund.eigener_name.clone();
                e.hund.zustand(jetzt)
            });
            an_etage(Server::Hund { hund });
        }
        Client::BallNehmen => {
            if let Some(b) = o.spielzeug.mit(et, |e| ball::nehmen(&mut e.ball, von).then(|| e.ball.clone())) {
                an_etage(Server::Ball { ball: b });
            }
        }
        Client::BallWerfen { x, y, z, vx, vy, vz } => {
            let b = o.spielzeug.mit(et, |e| {
                ball::werfen(&mut e.ball, von, [x, y, z], [vx, vy, vz]).then(|| {
                    e.ball_seit = jetzt;
                    e.ball.clone()
                })
            });
            if let Some(b) = b {
                an_etage(Server::Ball { ball: b });
            }
        }
        Client::AutoEinsteigen { auto, sitz } => {
            let (ok, autos) = o.spielzeug.mit(et, |e| {
                let frei = e.autos.iter().all(|a| a.fahrer.as_deref() != Some(von) && a.beifahrer.as_deref() != Some(von));
                let ok = frei
                    && e.autos.get_mut(auto as usize).is_some_and(|a| {
                        let platz = match sitz {
                            AutoSitz::Fahrer => &mut a.fahrer,
                            AutoSitz::Beifahrer => &mut a.beifahrer,
                        };
                        platz.is_none() && platz.replace(von.into()).is_none()
                    });
                (ok, e.autos.clone())
            });
            o.an(von, &sz(Server::Autos { autos: autos.clone(), antwort: Some(ok) }));
            if ok {
                an_andere(Server::Autos { autos, antwort: None });
            }
        }
        Client::AutoAussteigen => {
            let autos = o.spielzeug.mit(et, |e| {
                for a in &mut e.autos {
                    if a.fahrer.as_deref() == Some(von) {
                        a.fahrer = None;
                        a.tempo = 0.0;
                        a.lenkung = 0.0;
                    }
                    if a.beifahrer.as_deref() == Some(von) {
                        a.beifahrer = None;
                    }
                }
                e.autos.clone()
            });
            o.an(von, &sz(Server::Autos { autos: autos.clone(), antwort: Some(true) }));
            an_andere(Server::Autos { autos, antwort: None });
        }
        Client::AutoFahren { auto, x, z, rot_y, tempo, lenkung } => {
            let (x, z, rot_y, tempo, lenkung) = (eingabe::zahl(x, 200.0), eingabe::zahl(z, 200.0), eingabe::zahl(rot_y, 100.0), eingabe::zahl(tempo, 100.0), eingabe::zahl(lenkung, 2.0));
            let lage = welt::grundriss::draussen::AutoLage { x, z, rot_y, tempo, lenkung };
            let ok = o.spielzeug.mit(et, |e| match e.autos.get_mut(auto as usize) {
                // Nur wer fährt, und nur auf befahrbarem Grund (sonst schummelt der Browser).
                Some(a) if a.fahrer.as_deref() == Some(von) && welt::grundriss::draussen::auf_dem_asphalt(&lage) => {
                    (a.x, a.z, a.rot_y, a.tempo, a.lenkung) = (x, z, rot_y, tempo, lenkung);
                    true
                }
                _ => false,
            });
            if ok {
                an_andere(Server::AutoBewegt { auto, x, z, rot_y, tempo, lenkung });
            }
        }
        Client::Hupen => {
            let auto = o.spielzeug.mit(et, |e| e.autos.iter().position(|a| a.fahrer.as_deref() == Some(von) || a.beifahrer.as_deref() == Some(von)));
            if let Some(auto) = auto {
                an_etage(Server::Hupe { auto: auto as u32 });
            }
        }
        Client::ArcadeSpielen { spiel } => {
            let ok = o.spielzeug.mit(et, |e| {
                if e.arcade_spieler.as_ref().is_some_and(|s| s.id != von) {
                    return false;
                }
                let spiel = spiel.map(|s| eingabe::text(&s, 32)).filter(|s| !s.is_empty()).unwrap_or_else(|| zufalls_id(8));
                e.arcade_spieler = Some(ArcadeSpieler { id: von.into(), name: ich.name.clone(), spiel });
                e.arcade_bild = None;
                true
            });
            if ok {
                an_etage(Server::Arcade { arcade: arcade_stand(o, et) });
            } else {
                o.hinweis(von, "Am Automaten steht schon jemand", Stufe::Info);
            }
        }
        Client::ArcadeVerlassen => {
            if o.spielzeug.mit(et, |e| e.arcade_spieler.take_if(|s| s.id == von).is_some()) {
                o.spielzeug.mit(et, |e| e.arcade_bild = None);
                an_etage(Server::Arcade { arcade: arcade_stand(o, et) });
            }
        }
        Client::ArcadeBild { bild } => {
            if bild.zellen.len() > 400 {
                return;
            }
            let spiel = o.spielzeug.mit(et, |e| {
                let s = e.arcade_spieler.as_ref().filter(|s| s.id == von)?.spiel.clone();
                e.arcade_bild = Some(bild.clone());
                Some(s)
            });
            let Some(spiel) = spiel else { return };
            let neu = Bestenliste { spiel, name: ich.name.clone(), farbe: ich.farbe.clone(), punkte: bild.punkte, reihen: bild.reihen, level: bild.level, um: jetzt };
            let rekord = bild.punkte > 0 && o.spielzeug.punkte_eintragen(neu);
            an_andere(Server::ArcadeBild { bild });
            if rekord {
                o.an_alle(&sz(Server::Arcade { arcade: Arcade { spieler: None, bestenliste: o.spielzeug.bestenliste.lock().unwrap().clone(), bild: None } }), None);
            }
        }
        Client::WbOeffnen | Client::WbSchliessen => {
            let auf = matches!(c, Client::WbOeffnen);
            let leute = o.spielzeug.mit(et, |e| {
                e.wb_leute.retain(|p| p != von);
                if auf {
                    e.wb_leute.push(von.into());
                }
                e.wb_leute.clone()
            });
            an_etage(Server::WbLeute { leute });
        }
        Client::WbAendern { elemente } => {
            let neu = o.spielzeug.aendern(et, |e| whiteboard::zusammenfuehren(&mut e.g.whiteboard, elemente));
            if !neu.is_empty() {
                an_andere(Server::WbAenderung { elemente: neu });
            }
        }
        Client::WbZeiger { zeiger } => {
            let leute = o.spielzeug.mit(et, |e| e.wb_leute.clone());
            let andere: Vec<String> = leute.into_iter().filter(|p| p != von).collect();
            o.an_viele(&andere, &sz(Server::WbZeiger { id: von.into(), zeiger }));
        }
        Client::Golf { gier, neigung, kraft } => {
            an_andere(Server::Golf { id: von.into(), gier: eingabe::zahl(gier, 10.0), neigung: eingabe::zahl(neigung, 10.0), kraft: eingabe::zahl(kraft, 1.0) });
        }
        Client::Barwurf { spiel, u, v, steckt, n } => {
            an_andere(Server::Barwurf { id: von.into(), spiel, u: eingabe::zahl(u, 2.0), v: eingabe::zahl(v, 2.0), steckt, n: n.min(10) });
        }
    }
}
