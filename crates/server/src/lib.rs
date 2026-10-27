//! Der Server von OpenAgenticOffice: HTTP (axum), WebSocket fürs Spiel, Anmeldung, Etagen,
//! Spielzeug-Zustände; Agenten (`agenten`) und Medien (`medien`) hängen als Anbindungen daran.

pub mod anbindung;
pub mod anmeldung;
pub mod anmeldungen;
pub mod einrichtung;
pub mod einstellungen;
pub mod etagen;
pub mod http;
pub mod konfig;
pub mod konten_cli;
pub mod office;
pub mod speicher;
pub mod spielzeug;
pub mod tailnet;
pub mod team;
pub mod webhook;
pub mod ws;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::konfig::Konfig;
use crate::office::{Office, OfficeRef};

/// Die Version des Servers (für "das Office wurde aktualisiert").
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ein gebautes, noch nicht lauschendes Office.
pub struct Gebaut {
    pub office: OfficeRef,
    pub router: axum::Router,
    /// Ein frisch erzeugtes Passwort, das angezeigt werden muss.
    pub neues_passwort: Option<String>,
}

/// Baut das Office aus der Konfiguration (lädt alles aus dem Datenordner, bindet agenten und medien an).
pub async fn bauen(konfig: Konfig) -> Result<Gebaut> {
    std::fs::create_dir_all(&konfig.daten).with_context(|| format!("Datenordner {} lässt sich nicht anlegen", konfig.daten.display()))?;
    let (zugang, neues_passwort) = anmeldung::Zugang::starten(&konfig);
    let projekte = konfig.projekte.clone().unwrap_or_else(|| konfig.daten.join("projekte"));
    let einstellungen = einstellungen::Einstellungen::laden(&konfig);
    einstellungen.karten_neu_lesen(&konfig.daten);
    let mut angebunden = anbindung::starten(&konfig).await?;
    let domaenen = std::mem::take(&mut angebunden.domaenen);
    let routen = std::mem::take(&mut angebunden.routen);
    let office: OfficeRef = Arc::new(Office {
        zugang,
        verbindungen: Mutex::default(),
        chat: Mutex::default(),
        etagen: etagen::Etagen::laden(&konfig.daten, projekte),
        spielzeug: spielzeug::Spielzeug::laden(&konfig.daten),
        einstellungen,
        register: ws::Register::standard_mit(domaenen),
        version: VERSION.into(),
        anbindungen: Mutex::default(),
        agenten: Default::default(),
        anmeldungen: anmeldungen::Anmeldungen::neu(&konfig.daten),
        konfig,
    });
    if let Some(ordner) = office.konfig.projekt_ordner.clone() {
        let def = office.etagen.lokale_sicherstellen(&ordner);
        tracing::info!("Etage {} aus {}", def.name, def.ordner.display());
    }
    anbindung::verbinden(&office, &mut angebunden);
    ws::anmeldungen::beobachten(&office);
    for e in office.etagen.liste.lock().unwrap().clone() {
        office.etage_angemeldet(&e);
    }
    let router = http::router(office.clone(), routen);
    Ok(Gebaut { office, router, neues_passwort })
}

/// Die Hintergrund-Aufgaben: Hund, Rechner, Wetter.
pub fn hintergrund(o: &OfficeRef) {
    let a = o.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(1));
        loop {
            t.tick().await;
            ws::spielzeug::hunde_ticken(&a);
        }
    });
    let a = o.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(5));
        let mut n = 0u32;
        loop {
            t.tick().await;
            a.einstellungen.maschine.messen();
            n += 1;
            if n.is_multiple_of(2) && !a.verbindungen.lock().unwrap().is_empty() {
                ws::einstellungen::maschine_an_alle(&a);
            }
        }
    });
    let a = o.clone();
    tokio::spawn(async move { wetter(a).await });
}

/// Das Wetter: mit `--stadt` von open-meteo (bei Fehlern ausgedacht), sonst ausgedacht.
async fn wetter(o: OfficeRef) {
    use chrono::Datelike;
    let mut ort = None;
    if let Some(stadt) = o.einstellungen.himmel.stadt.clone() {
        match einstellungen::vorhersage::ort_suchen(&stadt).await {
            Ok(x) => {
                tracing::info!("Wetter von {}", x.name);
                ort = Some(x);
            }
            Err(e) => tracing::warn!("Kein echtes Wetter: {e}"),
        }
    }
    loop {
        let echt = match &ort {
            Some(ort) => match einstellungen::vorhersage::wetter(ort).await {
                Ok(h) => {
                    o.einstellungen.himmel.setzen(h);
                    true
                }
                Err(e) => {
                    tracing::warn!("Wetter von open-meteo kam nicht: {e}");
                    false
                }
            },
            None => false,
        };
        if !echt {
            o.einstellungen.himmel.wandern(chrono::Local::now().month());
        }
        ws::einstellungen::himmel_an_alle(&o);
        tokio::time::sleep(if echt { einstellungen::vorhersage::ABSTAND } else { Duration::from_secs(12 * 60) }).await;
    }
}

/// Startet das Office und lauscht, bis Strg+C kommt.
pub async fn starten(konfig: Konfig) -> Result<()> {
    let oeffnen = konfig.oeffnen;
    let adresse = SocketAddr::new(konfig.host, konfig.port);
    let einrichten = konfig.einrichten;
    let g = bauen(konfig).await?;
    if einrichten {
        einrichtung::noch_einmal(&g.office).await;
    } else {
        einrichtung::beim_start(&g.office).await;
    }
    let lauscher = tokio::net::TcpListener::bind(adresse).await.with_context(|| format!("Port {} ist belegt oder nicht erlaubt", adresse.port()))?;
    let echt = lauscher.local_addr()?;
    hintergrund(&g.office);
    tailnet::starten(&g.office);
    let o = g.office.clone();
    tokio::spawn(async move { einrichtung::projekt_beim_start(&o).await });

    let host = if echt.ip().is_unspecified() { "localhost".to_string() } else { echt.ip().to_string() };
    let url = format!("http://{host}:{}", echt.port());
    println!();
    println!("  OpenAgenticOffice läuft auf {url}");
    println!("  Daten: {}", g.office.konfig.daten.display());
    if let Some(pw) = &g.neues_passwort {
        println!("  Office-Passwort: {pw}   (bitte notieren – es wird nicht noch einmal gezeigt)");
    }
    let link = if g.office.zugang.offen() { url.clone() } else { format!("{url}/anmelden#schluessel={}", g.office.zugang.sitzungen.link_schluessel(None)) };
    if !g.office.zugang.offen() {
        println!("  Einmal-Anmeldelink: {link}");
    }
    println!();
    if oeffnen && let Err(e) = open::that_detached(&link) {
        tracing::warn!("Konnte den Browser nicht öffnen: {e}");
    }

    let dienst = g.router.into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(lauscher, dienst)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            println!("\n  Office wird beendet …");
        })
        .await?;
    anbindung::beenden(&g.office).await;
    Ok(())
}
