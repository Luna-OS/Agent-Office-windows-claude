//! Die Ereignisschleife: verbindet Fenster, Tray, lokalen Server und Durchleitung.

use crate::einstellungen::Einstellungen;
use crate::fenster::{self, Ereignis, START_URL, Wo};
use crate::lobby::{self, Eingabe};
use crate::passthrough::{self, Durchleitung};
use crate::seiten::{Ansicht, LokalAnsicht};
use crate::server_start::{self, LaufenderServer};
use crate::tray;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

struct Zustand {
    einstellungen: Einstellungen,
    ansicht: Arc<Mutex<Ansicht>>,
    lokal: Option<LaufenderServer>,
    bereit: bool,
    durch: Option<Durchleitung>,
}

impl Zustand {
    fn meldung(&self, text: &str) {
        self.aktualisieren(|a| a.meldung = text.to_string());
    }

    fn aktualisieren(&self, f: impl FnOnce(&mut Ansicht)) {
        if let Ok(mut a) = self.ansicht.lock() {
            f(&mut a);
        }
    }

    /// Spiegelt den Stand (Lobbys, lokaler Server, Durchleitung) in die Ansicht.
    fn abgleichen(&self) {
        let lokal = self.lokal.as_ref().map(|s| LokalAnsicht {
            url: s.url(),
            bereit: self.bereit,
            fuer_andere: s.fuer_andere,
            passwort: s.passwort.clone(),
            adressen: server_start::netz_adresse().map(|ip| vec![format!("{ip}:{}", s.port)]).unwrap_or_default(),
        });
        let (lobbys, durch) = (self.einstellungen.lobbys.clone(), self.durch.as_ref().map(|d| d.ziel.clone()));
        self.aktualisieren(|a| {
            a.lobbys = lobbys;
            a.lokal = lokal;
            a.durchleitung = durch;
        });
    }
}

pub fn browser_oeffnen(url: &str) {
    #[cfg(windows)]
    let _ = Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}

pub fn starten() {
    let schleife = EventLoopBuilder::<Ereignis>::with_user_event().build();
    let proxy = schleife.create_proxy();
    let ansicht = Arc::new(Mutex::new(Ansicht::default()));
    let fenster = fenster::fenster_bauen(&schleife);
    let webview = fenster::webview_bauen(&fenster, ansicht.clone(), proxy.clone());
    let mut z = Zustand { einstellungen: Einstellungen::laden(), ansicht, lokal: None, bereit: false, durch: None };
    z.abgleichen();
    let mut tray_symbol = None;

    schleife.run(move |ereignis, _, ablauf| {
        *ablauf = ControlFlow::Wait;
        match ereignis {
            // Das Tray-Symbol muss auf macOS nach dem Start der Schleife entstehen.
            Event::NewEvents(StartCause::Init) => tray_symbol = tray::erstellen(proxy.clone()),
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if z.lokal.is_some() || z.durch.is_some() {
                    fenster.set_visible(false); // im Infobereich weiterlaufen
                } else {
                    *ablauf = ControlFlow::Exit;
                }
            }
            Event::UserEvent(e) => {
                let zeigen = |url: &str| {
                    let _ = webview.load_url(url);
                    fenster.set_visible(true);
                    fenster.set_focus();
                };
                match e {
                    Ereignis::Anzeigen => zeigen(START_URL),
                    Ereignis::Beenden => {
                        z.lokal = None;
                        z.durch = None;
                        let _ = tray_symbol.take();
                        *ablauf = ControlFlow::Exit;
                    }
                    Ereignis::Beitreten { adresse, wo } => match lobby::deuten(&adresse) {
                        None => {
                            z.meldung("Das ist keine Lobby-Adresse. Beispiel: 100.78.107.162:4600 oder ein Einladungslink.");
                            zeigen(START_URL);
                        }
                        Some(Eingabe { basis, rest }) => {
                            z.meldung(&format!("Verbinde mit {basis} …"));
                            let p = proxy.clone();
                            std::thread::spawn(move || {
                                let erreichbar = lobby::erreichbar(&basis);
                                let _ = p.send_event(Ereignis::LobbyGeprueft { basis, rest, wo, erreichbar });
                            });
                        }
                    },
                    Ereignis::LobbyGeprueft { basis, rest, wo, erreichbar } => {
                        if !erreichbar {
                            z.meldung(&format!("{basis} ist nicht erreichbar. Läuft die Lobby, und ist dein Computer im selben Netz (Heimnetz, Tailscale, NetBird)?"));
                            zeigen(START_URL);
                        } else {
                            z.einstellungen.lobby_merken(&basis);
                            let basis_url = lobby_url(&mut z, &basis);
                            z.meldung(&format!("Verbunden mit {basis}. Anmelden mit eigenem Claude und GitHub: im Office unter ☰ → 🔐 Deine Anmeldungen."));
                            z.abgleichen();
                            oeffnen(&format!("{basis_url}{rest}"), wo, &zeigen);
                        }
                    }
                    Ereignis::Vergessen(adresse) => {
                        z.einstellungen.lobby_vergessen(&adresse);
                        z.abgleichen();
                        zeigen(START_URL);
                    }
                    Ereignis::LokalStart(wo) => lokal_starten(&mut z, false, Some(wo), &proxy, &zeigen),
                    Ereignis::FuerAndere(an) => {
                        z.lokal = None; // neu starten, jetzt mit anderer Erreichbarkeit
                        lokal_starten(&mut z, an, None, &proxy, &zeigen);
                    }
                    Ereignis::LokalStop => {
                        z.lokal = None;
                        z.bereit = false;
                        z.meldung("Das Office auf diesem Computer wurde beendet.");
                        z.abgleichen();
                        zeigen(START_URL);
                    }
                    Ereignis::LokalOeffnen(wo) => {
                        if let Some(s) = &z.lokal {
                            oeffnen(&s.url(), wo, &zeigen);
                        }
                    }
                    Ereignis::ServerBereit { wo } => {
                        z.bereit = true;
                        z.meldung("Das Office auf diesem Computer läuft.");
                        z.abgleichen();
                        match (wo, z.lokal.as_ref().map(|s| s.url())) {
                            (Some(wo), Some(url)) => oeffnen(&url, wo, &zeigen),
                            _ => zeigen(START_URL),
                        }
                    }
                }
            }
            _ => {}
        }
        // Ist der lokale Server von allein beendet worden, den Stand zurücksetzen.
        if let Some(s) = z.lokal.as_mut() {
            if !s.laeuft() {
                z.lokal = None;
                z.bereit = false;
                z.meldung("Das Office auf diesem Computer ist unerwartet beendet worden.");
                z.abgleichen();
            }
        }
    });
}

fn oeffnen(url: &str, wo: Wo, zeigen: &dyn Fn(&str)) {
    match wo {
        Wo::Browser => browser_oeffnen(url),
        Wo::Fenster => zeigen(url),
    }
}

/// Adresse, unter der der Browser die Lobby erreicht (bei fremdem `http://` über die Durchleitung).
fn lobby_url(z: &mut Zustand, basis: &str) -> String {
    if !lobby::braucht_durchleitung(basis) {
        z.durch = None;
        return basis.to_string();
    }
    let ziel = lobby::host_port(basis).unwrap_or_default();
    if z.durch.as_ref().is_none_or(|d| d.ziel != ziel) {
        z.durch = None; // alten Port freigeben
        match Durchleitung::starten(&ziel) {
            Ok(d) => z.durch = Some(d),
            Err(f) => {
                z.meldung(&format!("Die Durchleitung ließ sich nicht starten ({f}). Sprache und Bildschirm teilen gehen dann nur über https."));
                return basis.to_string();
            }
        }
    }
    format!("http://127.0.0.1:{}", passthrough::PORT)
}

fn lokal_starten(z: &mut Zustand, fuer_andere: bool, danach: Option<Wo>, proxy: &tao::event_loop::EventLoopProxy<Ereignis>, zeigen: &dyn Fn(&str)) {
    if z.lokal.is_some() {
        if let (Some(wo), true) = (danach, z.bereit) {
            let url = z.lokal.as_ref().map(|s| s.url()).unwrap_or_default();
            oeffnen(&url, wo, zeigen);
        }
        return;
    }
    if fuer_andere && z.einstellungen.lobby_passwort.is_none() {
        z.einstellungen.lobby_passwort = Some(server_start::zufalls_passwort());
    }
    match server_start::starten(&z.einstellungen, fuer_andere) {
        Ok(server) => {
            if fuer_andere {
                z.einstellungen.speichern();
            }
            let port = server.port;
            z.bereit = false;
            z.lokal = Some(server);
            z.meldung("Das Office auf diesem Computer startet …");
            let p = proxy.clone();
            std::thread::spawn(move || {
                if server_start::warten(port, Duration::from_secs(60)) {
                    let _ = p.send_event(Ereignis::ServerBereit { wo: danach });
                }
            });
        }
        Err(f) => z.meldung(&f),
    }
    z.abgleichen();
    zeigen(START_URL);
}
