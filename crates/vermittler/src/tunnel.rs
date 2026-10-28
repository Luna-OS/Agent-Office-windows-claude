//! Eine Browser-Verbindung zum Gastgeber durchreichen: Kopf umgeschrieben, danach Bytes in beide
//! Richtungen, bis eine Seite aufhört.

use crate::knoten::{ZumKnoten, text};
use crate::kopf::Kopf;
use crate::lobby::{Ausgang, Lobby};
use crate::seiten;
use bytes::Bytes;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// So viele Bytes gehen höchstens in einer Nachricht zum Gastgeber.
const STUECK: usize = 16 * 1024;

pub async fn durchreichen(lobby: Arc<Lobby>, mut tcp: TcpStream, kopf: Kopf, von: String) {
    let Some(gastgeber) = lobby.aktiver() else {
        seiten::geschlossen(&mut tcp, &kopf).await;
        return;
    };
    let id = lobby.neuer_strom();
    let (zum_browser, mut vom_gastgeber) = mpsc::channel::<Option<Bytes>>(64);
    match gastgeber.stroeme.lock() {
        Ok(mut s) => {
            s.insert(id, zum_browser);
        }
        Err(_) => return,
    }
    let auf = text(&ZumKnoten::Auf {
        strom: id,
        von: &von,
    });
    let erster = Bytes::from(kopf.fuer_gastgeber(&von));
    if gastgeber.ausgang.send(Ausgang::Text(auf)).await.is_err()
        || gastgeber
            .ausgang
            .send(Ausgang::Daten(id, erster))
            .await
            .is_err()
    {
        gastgeber.strom_weg(id);
        return;
    }

    let (mut lesen, mut schreiben) = tcp.into_split();
    let ausgang = gastgeber.ausgang.clone();
    let mut hin = tokio::spawn(async move {
        let mut puffer = vec![0u8; STUECK];
        loop {
            match lesen.read(&mut puffer).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let stueck = Bytes::copy_from_slice(&puffer[..n]);
                    if ausgang.send(Ausgang::Daten(id, stueck)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    loop {
        tokio::select! {
            d = vom_gastgeber.recv() => match d {
                Some(Some(b)) => {
                    if schreiben.write_all(&b).await.is_err() {
                        break;
                    }
                }
                // Der Gastgeber hat sie beendet, oder er ist weg.
                _ => break,
            },
            // Der Browser hat sie beendet.
            _ = &mut hin => break,
        }
    }
    hin.abort();
    let _ = schreiben.shutdown().await;
    // Hat der Gastgeber sie nicht selbst beendet, erfährt er es jetzt.
    if gastgeber.strom_weg(id).is_some() {
        let _ = gastgeber
            .ausgang
            .send(Ausgang::Text(text(&ZumKnoten::Zu { strom: id })))
            .await;
    }
}
