//! Durchleitung: reicht `http://127.0.0.1:48600` unverändert (auch WebSockets) an eine Lobby weiter.
//! Für den Browser ist das dann „der eigene Computer“, also gehen Sprache und Bildschirm teilen auch
//! bei einer Lobby über `http://` auf einem anderen Gerät.

use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

pub const PORT: u16 = 48600;

pub struct Durchleitung {
    pub ziel: String,
    aktiv: Arc<AtomicBool>,
}

impl Durchleitung {
    /// Startet die Durchleitung zu `ziel` (`host:port`). Ein Fehler heißt meist: Port ist belegt.
    pub fn starten(ziel: &str) -> io::Result<Self> {
        let hoerer = TcpListener::bind(("127.0.0.1", PORT))?;
        let aktiv = Arc::new(AtomicBool::new(true));
        let (flag, ziel_kopie) = (aktiv.clone(), ziel.to_string());
        thread::spawn(move || {
            for eingang in hoerer.incoming() {
                if !flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(eingang) = eingang else { continue };
                let ziel = ziel_kopie.clone();
                thread::spawn(move || {
                    if let Ok(aus) = TcpStream::connect(&ziel) {
                        verbinden(eingang, aus);
                    }
                });
            }
        });
        Ok(Self {
            ziel: ziel.to_string(),
            aktiv,
        })
    }
}

impl Drop for Durchleitung {
    fn drop(&mut self) {
        self.aktiv.store(false, Ordering::Relaxed);
        // Den wartenden `accept` wecken, damit der Thread endet und der Port frei wird.
        let _ = TcpStream::connect(("127.0.0.1", PORT));
    }
}

fn verbinden(a: TcpStream, b: TcpStream) {
    let (Ok(mut a_lesen), Ok(mut b_lesen)) = (a.try_clone(), b.try_clone()) else {
        return;
    };
    let (mut a_schreiben, mut b_schreiben) = (a, b);
    let hin = thread::spawn(move || {
        let _ = io::copy(&mut a_lesen, &mut b_schreiben);
        let _ = b_schreiben.shutdown(Shutdown::Write);
    });
    let _ = io::copy(&mut b_lesen, &mut a_schreiben);
    let _ = a_schreiben.shutdown(Shutdown::Write);
    let _ = hin.join();
}
