//! Die Verdrahtung mit der 3D-Welt (crates/welt): starten, Maus fangen und freigeben, und was man in
//! der Welt benutzt, als Fenster der Oberfläche öffnen. Ohne das Feature `welt3d` bleibt die Welt aus.

#[cfg(feature = "welt3d")]
mod an {
    use oberflaeche::Fenster;
    use protokoll::{ClientNachricht, arbeiter};
    use welt::WeltAktion as A;

    pub fn starten() {
        welt::bei_senden(crate::verbindung::senden);
        welt::bei_aktion(aktion);
        welt::bei_hinweis(oberflaeche::hinweis);
        wasm_bindgen_futures::spawn_local(welt::starten());
    }

    pub fn fenster(offen: bool) {
        welt::eingabe_sperren(offen);
        if offen {
            welt::zeiger_freigeben();
        } else {
            welt::zeiger_fangen();
        }
    }

    pub fn nachricht(n: &protokoll::ServerNachricht) {
        welt::nachricht(n);
    }

    pub fn zur_etage(etage: &str) {
        if oberflaeche::ist_lite() {
            crate::verbindung::senden(ClientNachricht::Etagen(protokoll::etagen::Client::Gehen {
                etage: etage.to_string(),
                an: None,
            }));
        } else {
            welt::zur_etage(etage);
        }
    }

    pub fn perspektive(ich: bool) {
        welt::perspektive_setzen(ich);
    }

    pub fn zur_garage() {
        welt::zur_garage();
    }

    pub fn eigenes_emote(e: protokoll::praesenz::Emote) {
        welt::eigenes_emote(e);
    }

    /// Was man in der Welt benutzt hat, als Fenster (oder direkt als Nachricht).
    fn aktion(a: A) {
        let f = match a {
            A::AufzugOeffnen => Fenster::Aufzug,
            A::TerminalOeffnen { arbeiter } => Fenster::Terminal(arbeiter),
            A::Einstellen { tisch } => Fenster::Anlegen(tisch),
            A::ShellOeffnen { tisch } => {
                crate::verbindung::senden(ClientNachricht::Arbeiter(arbeiter::Client::Anlegen {
                    tisch,
                    prompt: None,
                    worktree: None,
                    art: arbeiter::ArbeiterArt::Shell,
                    provider: None,
                    modell: None,
                    aufwand: None,
                    issue: None,
                }));
                return;
            }
            A::PromptOeffnen { arbeiter } => {
                match oberflaeche::zustand::zustand().arbeiter_mit(&arbeiter) {
                    Some(a) => Fenster::Anlegen(a.tisch),
                    None => return,
                }
            }
            A::AenderungenOeffnen { arbeiter } => Fenster::Aenderungen(arbeiter),
            A::NachHauseSchicken { arbeiter } => Fenster::Heimschicken(arbeiter),
            A::PullRequest { arbeiter } => {
                match oberflaeche::zustand::zustand()
                    .arbeiter_mit(&arbeiter)
                    .and_then(|a| a.pr)
                {
                    Some(pr) => {
                        let _ = web_sys::window()
                            .and_then(|w| w.open_with_url_and_target(&pr.url, "_blank").ok());
                    }
                    None => crate::verbindung::senden(ClientNachricht::Arbeiter(
                        arbeiter::Client::PrOeffnen { arbeiter },
                    )),
                }
                return;
            }
            A::SchildOeffnen { tisch } => Fenster::Schild(tisch),
            A::BoardOeffnen { board } => match board {
                welt::api::Board::Warteschlange => Fenster::Aufgaben,
                welt::api::Board::Issues | welt::api::Board::PullRequests => Fenster::Github,
                welt::api::Board::Dienste => Fenster::Dienste,
            },
            A::TvOeffnen => {
                oberflaeche::tv_oeffnen();
                return;
            }
            A::JukeboxOeffnen => Fenster::Jukebox,
            A::LeinwandOeffnen => Fenster::DachUrl,
            A::HundUmbenennen => Fenster::HundName,
            A::BildAufhaengen { platz } => {
                oberflaeche::fenster::tisch::bild_platz_setzen(Some(platz));
                Fenster::BildAufhaengen
            }
            A::HilfeOeffnen => Fenster::Hilfe,
            A::BarKarte => Fenster::Bar,
            A::StationOeffnen { tisch } => Fenster::Station(tisch),
            A::KerkerOeffnen => Fenster::Kerker,
            A::MinesweeperOeffnen => Fenster::Minen,
            A::BildOeffnen { id } => Fenster::Bild(id),
            A::BuecherregalOeffnen => Fenster::Buecher,
            A::WhiteboardOeffnen => Fenster::Whiteboard,
            A::ArcadeOeffnen => Fenster::Arcade,
            A::Meldung { text } => {
                oberflaeche::zustand::info(text);
                return;
            }
            #[allow(unreachable_patterns)]
            _ => return,
        };
        oberflaeche::fenster_oeffnen(f);
    }
}

#[cfg(not(feature = "welt3d"))]
mod an {
    use protokoll::{ClientNachricht, etagen};

    pub fn starten() {}
    pub fn fenster(_offen: bool) {}
    pub fn nachricht(_n: &protokoll::ServerNachricht) {}
    pub fn zur_etage(etage: &str) {
        crate::verbindung::senden(ClientNachricht::Etagen(etagen::Client::Gehen {
            etage: etage.to_string(),
            an: None,
        }));
    }
    pub fn perspektive(_ich: bool) {}
    pub fn zur_garage() {}
    pub fn eigenes_emote(_e: protokoll::praesenz::Emote) {}
}

/// Startet die Welt (einmal, beim ersten Verbinden).
pub fn starten() {
    an::starten();
}

/// Ein Fenster ging auf (`true`) oder das letzte zu (`false`).
pub fn fenster(offen: bool) {
    an::fenster(offen);
}

/// Server-Frames für die Welt.
pub fn nachricht(n: &protokoll::ServerNachricht) {
    an::nachricht(n);
}

/// Ich-Perspektive oder Schulterblick.
pub fn perspektive(ich: bool) {
    an::perspektive(ich);
}

/// Das eigene Emote an der eigenen Figur.
pub fn eigenes_emote(e: protokoll::praesenz::Emote) {
    an::eigenes_emote(e);
}

/// Mit dem Aufzug in die Garage.
pub fn zur_garage() {
    an::zur_garage();
}

/// Mit dem Aufzug zu einer Etage.
pub fn zur_etage(etage: &str) {
    an::zur_etage(etage);
}
