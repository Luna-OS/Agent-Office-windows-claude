//! Die YouTube-Leinwand auf dem Dach: der Server hält Video, Play/Pause und Stelle für alle synchron.
//!
//! Reine Logik ohne Uhr: wer die Leinwand ändert, gibt die Server-Zeit (`jetzt_ms`) mit.

use protokoll::dach::{Client, LeinwandZustand};

use crate::youtube::video_aus_url;

/// Wie weit ein Browser von der Server-Stelle abweichen darf, bevor er nachspringt (Sekunden).
pub const ERLAUBTE_ABWEICHUNG_S: f64 = 1.5;

/// Die Leinwand eines Dachs.
#[derive(Debug, Clone, Default)]
pub struct Leinwand {
    zustand: LeinwandZustand,
}

impl Leinwand {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Aus einem gespeicherten Zustand (die Stelle läuft seit `stand_ms` weiter, falls es lief).
    pub fn aus_zustand(zustand: LeinwandZustand) -> Self {
        Self { zustand }
    }

    pub fn zustand(&self) -> &LeinwandZustand {
        &self.zustand
    }

    /// Die aktuelle Stelle in Sekunden.
    pub fn position(&self, jetzt_ms: u64) -> f64 {
        self.zustand.position_um(jetzt_ms)
    }

    /// Wendet eine Nachricht an. `Ok(true)` heißt: geändert, an alle auf dem Dach schicken.
    pub fn anwenden(&mut self, nachricht: &Client, wer: &str, jetzt_ms: u64) -> Result<bool, String> {
        let jetzt = self.position(jetzt_ms);
        let neu = match nachricht {
            Client::VideoSetzen { url } => {
                let video = video_aus_url(url)?;
                LeinwandZustand {
                    video_id: Some(video.id),
                    laeuft: true,
                    position_s: video.start_s,
                    stand_ms: jetzt_ms,
                    von: None,
                }
            }
            Client::Abspielen { position_s } => {
                self.braucht_video()?;
                if self.zustand.laeuft && position_s.is_none() {
                    return Ok(false);
                }
                self.stelle(true, position_s.unwrap_or(jetzt), jetzt_ms)
            }
            Client::Pausieren { position_s } => {
                self.braucht_video()?;
                if !self.zustand.laeuft && position_s.is_none() {
                    return Ok(false);
                }
                self.stelle(false, position_s.unwrap_or(jetzt), jetzt_ms)
            }
            Client::Springen { position_s } => {
                self.braucht_video()?;
                self.stelle(self.zustand.laeuft, *position_s, jetzt_ms)
            }
            Client::Leeren => {
                if self.zustand.video_id.is_none() {
                    return Ok(false);
                }
                LeinwandZustand { stand_ms: jetzt_ms, ..Default::default() }
            }
        };
        self.zustand = LeinwandZustand { von: Some(kuerzen(wer, 24)), ..neu };
        Ok(true)
    }

    fn braucht_video(&self) -> Result<(), String> {
        match self.zustand.video_id {
            Some(_) => Ok(()),
            None => Err("Auf der Leinwand läuft gerade kein Video".into()),
        }
    }

    fn stelle(&self, laeuft: bool, position_s: f64, jetzt_ms: u64) -> LeinwandZustand {
        let position_s = if position_s.is_finite() { position_s.max(0.0) } else { 0.0 };
        LeinwandZustand { laeuft, position_s, stand_ms: jetzt_ms, ..self.zustand.clone() }
    }
}

/// Was ein Browser tun soll, damit sein Player zur Leinwand passt.
#[derive(Debug, Clone, PartialEq)]
pub enum Abgleich {
    /// Nichts zu tun.
    Passt,
    /// Ein anderes Video laden, ab `ab_s`, abspielen oder nicht.
    Laden { video_id: String, ab_s: f64, abspielen: bool },
    /// Zur Stelle springen.
    Springen { auf_s: f64 },
    Abspielen,
    Pausieren,
    /// Player wegräumen.
    Leeren,
}

/// Vergleicht den lokalen Player (`video`, `laeuft`, `stelle_s`) mit dem Server-Zustand zu `jetzt_ms`
/// (Server-Uhr, also lokale Zeit plus Uhrenversatz) und sagt, was zu tun ist.
pub fn abgleichen(
    soll: &LeinwandZustand,
    jetzt_ms: u64,
    video: Option<&str>,
    laeuft: bool,
    stelle_s: f64,
) -> Vec<Abgleich> {
    let Some(soll_id) = soll.video_id.as_deref() else {
        return if video.is_some() { vec![Abgleich::Leeren] } else { vec![Abgleich::Passt] };
    };
    let soll_stelle = soll.position_um(jetzt_ms);
    if video != Some(soll_id) {
        return vec![Abgleich::Laden { video_id: soll_id.to_string(), ab_s: soll_stelle, abspielen: soll.laeuft }];
    }
    let mut schritte = Vec::new();
    if (stelle_s - soll_stelle).abs() > ERLAUBTE_ABWEICHUNG_S {
        schritte.push(Abgleich::Springen { auf_s: soll_stelle });
    }
    if soll.laeuft != laeuft {
        schritte.push(if soll.laeuft { Abgleich::Abspielen } else { Abgleich::Pausieren });
    }
    if schritte.is_empty() {
        schritte.push(Abgleich::Passt);
    }
    schritte
}

pub(crate) fn kuerzen(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setzen(l: &mut Leinwand, url: &str, t: u64) {
        assert!(l.anwenden(&Client::VideoSetzen { url: url.into() }, "Luna", t).unwrap());
    }

    #[test]
    fn video_setzen_laeuft_ab_startzeit() {
        let mut l = Leinwand::neu();
        setzen(&mut l, "https://youtu.be/dQw4w9WgXcQ?t=30", 1_000);
        let z = l.zustand();
        assert_eq!(z.video_id.as_deref(), Some("dQw4w9WgXcQ"));
        assert!(z.laeuft);
        assert_eq!(l.position(1_000), 30.0);
        assert_eq!(l.position(11_000), 40.0);
        assert_eq!(z.von.as_deref(), Some("Luna"));
    }

    #[test]
    fn pause_und_weiter_halten_die_stelle() {
        let mut l = Leinwand::neu();
        setzen(&mut l, "dQw4w9WgXcQ", 0);
        assert!(l.anwenden(&Client::Pausieren { position_s: None }, "a", 5_000).unwrap());
        assert_eq!(l.position(60_000), 5.0);
        assert!(!l.anwenden(&Client::Pausieren { position_s: None }, "a", 6_000).unwrap());
        assert!(l.anwenden(&Client::Abspielen { position_s: None }, "b", 70_000).unwrap());
        assert_eq!(l.position(72_000), 7.0);
        assert!(l.anwenden(&Client::Springen { position_s: 100.0 }, "b", 80_000).unwrap());
        assert_eq!(l.position(81_000), 101.0);
        assert!(l.anwenden(&Client::Pausieren { position_s: Some(-3.0) }, "b", 90_000).unwrap());
        assert_eq!(l.position(99_000), 0.0);
    }

    #[test]
    fn ohne_video_gibt_es_fehler() {
        let mut l = Leinwand::neu();
        assert!(l.anwenden(&Client::Abspielen { position_s: None }, "a", 0).is_err());
        assert!(l.anwenden(&Client::VideoSetzen { url: "https://vimeo.com/1".into() }, "a", 0).is_err());
        assert!(!l.anwenden(&Client::Leeren, "a", 0).unwrap());
        setzen(&mut l, "dQw4w9WgXcQ", 0);
        assert!(l.anwenden(&Client::Leeren, "a", 0).unwrap());
        assert_eq!(l.zustand().video_id, None);
    }

    #[test]
    fn abgleich_sagt_was_zu_tun_ist() {
        let mut l = Leinwand::neu();
        setzen(&mut l, "dQw4w9WgXcQ", 0);
        let z = l.zustand().clone();
        assert_eq!(
            abgleichen(&z, 10_000, None, false, 0.0),
            vec![Abgleich::Laden { video_id: "dQw4w9WgXcQ".into(), ab_s: 10.0, abspielen: true }]
        );
        assert_eq!(abgleichen(&z, 10_000, Some("dQw4w9WgXcQ"), true, 9.5), vec![Abgleich::Passt]);
        assert_eq!(abgleichen(&z, 10_000, Some("dQw4w9WgXcQ"), true, 3.0), vec![Abgleich::Springen { auf_s: 10.0 }]);
        assert_eq!(abgleichen(&z, 10_000, Some("dQw4w9WgXcQ"), false, 10.0), vec![Abgleich::Abspielen]);
        assert_eq!(abgleichen(&LeinwandZustand::default(), 0, Some("dQw4w9WgXcQ"), true, 1.0), vec![Abgleich::Leeren]);
    }

    #[test]
    fn zwei_zuschauer_sehen_dieselbe_stelle() {
        let mut l = Leinwand::neu();
        setzen(&mut l, "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m", 50_000);
        // Zwei Browser mit verschiedenen Uhren, beide auf Server-Zeit umgerechnet.
        let a = l.zustand().position_um(65_000);
        let json = serde_json::to_string(l.zustand()).unwrap();
        let b: LeinwandZustand = serde_json::from_str(&json).unwrap();
        assert_eq!(a, b.position_um(65_000));
        assert_eq!(a, 75.0);
    }
}
