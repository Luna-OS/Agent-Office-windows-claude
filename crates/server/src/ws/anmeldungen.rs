//! Domäne `konten`, Teil eigene Anmeldungen: Claude und GitHub je Konto (nur mit eigenem Konto).

use protokoll::konten::{AnmeldeArt, Client, Server};
use protokoll::praesenz::Stufe;
use protokoll::{ClientNachricht, ServerNachricht};

use super::{Domaene, Zukunft, fertig};
use crate::anmeldungen::ablauf;
use crate::office::OfficeRef;

pub struct AnmeldungenDomaene;

/// Darf das Konto die Anmeldungen des Office-Rechners nutzen? (Claude, GitHub)
pub fn darf_office(o: &OfficeRef, konto: &str) -> (bool, bool) {
    let k = o.zugang.konten.lock().unwrap();
    let admin = k
        .get(konto)
        .is_some_and(|k| k.rolle == protokoll::konten::Rolle::Admin);
    (admin || k.claude_teilen(), admin)
}

/// Die Umgebung für Programme im Namen eines Kontos (leerer Wert = Variable entfernen).
pub fn umgebung(o: &OfficeRef, konto: &str, nur: Option<AnmeldeArt>) -> Vec<(String, String)> {
    let (c, g) = darf_office(o, konto);
    o.anmeldungen.umgebung(konto, nur, c, g)
}

/// Der Stand an alle Verbindungen des Kontos.
pub fn stand_senden(o: &OfficeRef, konto: &str) {
    let (c, g) = darf_office(o, konto);
    let n = ServerNachricht::Konten(Server::Anmeldungen {
        stand: o.anmeldungen.stand(konto, c, g),
    });
    o.an_wen(&n, None, |v| v.sitzung.konto.as_deref() == Some(konto));
}

/// Hört auf Änderungen der Anmeldungen und schickt sie den Browsern (einmal beim Start aufrufen).
pub fn beobachten(o: &OfficeRef) {
    let mut rx = o.anmeldungen.geaendert.subscribe();
    let o = o.clone();
    tokio::spawn(async move {
        while let Ok(k) = rx.recv().await {
            if let Some(konto) = k.strip_prefix("nachsehen:") {
                let env = umgebung(&o, konto, None);
                let (a, konto) = (o.anmeldungen.clone(), konto.to_string());
                tokio::spawn(async move { ablauf::nachsehen(&a, &konto, env).await });
            } else {
                stand_senden(&o, &k);
            }
        }
    });
}

impl Domaene for AnmeldungenDomaene {
    fn name(&self) -> &'static str {
        "anmeldungen"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(
            n,
            ClientNachricht::Konten(
                Client::AnmeldungenHolen
                    | Client::AnmeldungStarten { .. }
                    | Client::AnmeldungCode { .. }
                    | Client::AnmeldungAbbrechen { .. }
                    | Client::AnmeldungToken { .. }
                    | Client::AnmeldungOffice { .. }
                    | Client::Abmelden { .. }
            )
        )
    }

    fn begruessung(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        let Some(konto) = o.sitzung_von(von).and_then(|s| s.konto) else {
            return Vec::new();
        };
        let _ = o.anmeldungen.geaendert.send(format!("nachsehen:{konto}"));
        let (c, g) = darf_office(o, &konto);
        vec![ServerNachricht::Konten(Server::Anmeldungen {
            stand: o.anmeldungen.stand(&konto, c, g),
        })]
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        let ClientNachricht::Konten(c) = n else {
            return fertig();
        };
        let Some(konto) = o.sitzung_von(von).and_then(|s| s.konto) else {
            o.hinweis(
                von,
                "Eigene Anmeldungen gibt es nur mit eigenem Konto",
                Stufe::Info,
            );
            return fertig();
        };
        let a = o.anmeldungen.clone();
        Box::pin(async move {
            let env = umgebung(o, &konto, None);
            let ergebnis = match c {
                Client::AnmeldungenHolen => {
                    ablauf::nachsehen(&a, &konto, env).await;
                    Ok(())
                }
                Client::AnmeldungStarten {
                    art: AnmeldeArt::Claude,
                } => crate::anmeldungen::claude::claude_starten(&a, &konto, env),
                Client::AnmeldungStarten {
                    art: AnmeldeArt::Github,
                } => ablauf::github_starten(&a, &konto, env),
                Client::AnmeldungCode { code } => {
                    crate::anmeldungen::claude::claude_code(&a, &konto, &code)
                }
                Client::AnmeldungAbbrechen { art } => {
                    ablauf::abbrechen(&a, &konto, art);
                    Ok(())
                }
                Client::AnmeldungToken { art, token } => {
                    ablauf::token(&a, &konto, art, &token, env).await
                }
                Client::AnmeldungOffice { art } => {
                    let (c, g) = darf_office(o, &konto);
                    if (art == AnmeldeArt::Claude && c) || (art == AnmeldeArt::Github && g) {
                        ablauf::office(&a, &konto, art);
                        Ok(())
                    } else {
                        Err("Die Anmeldung des Office-Rechners darfst du nicht nutzen".into())
                    }
                }
                Client::Abmelden { art } => {
                    ablauf::abmelden(&a, &konto, art, env).await;
                    Ok(())
                }
                _ => Ok(()),
            };
            match ergebnis {
                Ok(()) => stand_senden(o, &konto),
                Err(f) => o.hinweis(von, f, Stufe::Warnung),
            }
        })
    }
}
