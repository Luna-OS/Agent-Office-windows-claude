//! Wer darf herein: das gemeinsame Office-Passwort, eigene Konten, Sitzungen.
//!
//! Ohne Passwort und nur auf 127.0.0.1 ist das Office offen (jeder Browser auf diesem Rechner ist drin).
//! Lauscht es im Netz und hat noch kein Passwort, wird eines erzeugt und einmal angezeigt.

pub mod konten;
pub mod passwort;
pub mod sitzung;

use std::path::PathBuf;
use std::sync::Mutex;

use protokoll::konten::{Ich, KontoKurz, Rolle};
use serde::{Deserialize, Serialize};

use crate::konfig::Konfig;
use crate::speicher::{laden, speichern};
pub use konten::Konten;
pub use sitzung::{Sitzung, Sitzungen};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Gespeichert {
    /// Hash des gemeinsamen Passworts; keiner = offen (nur lokal erlaubt).
    passwort: Option<String>,
    /// Das erzeugte Passwort wurde über /beanspruchen schon einmal gezeigt.
    beansprucht: bool,
}

pub struct Zugang {
    datei: PathBuf,
    d: Mutex<Gespeichert>,
    /// Klartext eines erzeugten Passworts, bis es über /beanspruchen abgeholt wurde (nur im Speicher).
    erzeugt: Mutex<Option<String>>,
    pub konten: Mutex<Konten>,
    pub sitzungen: Sitzungen,
    pub beanspruch_token: Option<String>,
}

impl Zugang {
    /// Lädt alles aus dem Datenordner. Gibt ein neu erzeugtes Passwort zurück, das angezeigt werden muss.
    pub fn starten(k: &Konfig) -> (Self, Option<String>) {
        let datei = k.daten.join("zugang.json");
        let mut d: Gespeichert = laden(&datei);
        let mut neu = None;
        if let Some(pw) = &k.passwort {
            if !d
                .passwort
                .as_deref()
                .is_some_and(|h| passwort::pruefen(pw, h))
            {
                d.passwort = Some(passwort::hashen(pw));
            }
        } else if k.passwort_zuruecksetzen || (d.passwort.is_none() && !k.nur_lokal()) {
            let pw = passwort::erzeugen();
            d.passwort = Some(passwort::hashen(&pw));
            d.beansprucht = false;
            neu = Some(pw);
        }
        speichern(&datei, &d);
        let sitzungen = Sitzungen::laden(&k.daten, d.passwort.as_deref().unwrap_or(""));
        let z = Self {
            datei,
            erzeugt: Mutex::new(neu.clone().filter(|_| k.beanspruch_token.is_some())),
            d: Mutex::new(d),
            konten: Mutex::new(Konten::laden(&k.daten)),
            sitzungen,
            beanspruch_token: k.beanspruch_token.clone(),
        };
        (z, neu)
    }

    /// Kein Passwort nötig (nur auf diesem Rechner erreichbar).
    pub fn offen(&self) -> bool {
        self.d.lock().unwrap().passwort.is_none()
    }

    pub fn passwort_pruefen(&self, pw: &str) -> bool {
        match &self.d.lock().unwrap().passwort {
            Some(h) => passwort::pruefen(pw, h),
            None => false,
        }
    }

    /// Setzt ein neues gemeinsames Passwort (meldet alle Sitzungen ohne Konto ab).
    pub fn passwort_setzen(&self, pw: &str) {
        let h = passwort::hashen(pw);
        self.sitzungen.passwort_geaendert(&h);
        let mut d = self.d.lock().unwrap();
        d.passwort = Some(h);
        speichern(&self.datei, &*d);
    }

    /// Das erzeugte Passwort einmalig abholen (Einmal-Link /beanspruchen?t=…).
    pub fn beanspruchen(&self, token: &str) -> Result<String, &'static str> {
        let Some(erwartet) = &self.beanspruch_token else {
            return Err("Dieses Office hat keinen Beanspruch-Link.");
        };
        let mut d = self.d.lock().unwrap();
        let mut erzeugt = self.erzeugt.lock().unwrap();
        if d.beansprucht || erzeugt.is_none() {
            return Err(
                "Dieses Office wurde schon beansprucht. Melde dich mit dem gespeicherten Passwort an.",
            );
        }
        if !passwort::gleich(token.as_bytes(), erwartet.as_bytes()) {
            return Err("Dieser Beanspruch-Link ist nicht gültig.");
        }
        d.beansprucht = true;
        speichern(&self.datei, &*d);
        Ok(erzeugt.take().unwrap_or_default())
    }

    pub fn beanspruchbar(&self) -> bool {
        self.beanspruch_token.is_some()
            && self.erzeugt.lock().unwrap().is_some()
            && !self.d.lock().unwrap().beansprucht
    }

    /// Gilt diese Sitzung noch? Ein entzogenes Konto oder das ausgeschaltete gemeinsame Passwort wirken sofort.
    pub fn gueltig(&self, s: &Sitzung) -> bool {
        let konten = self.konten.lock().unwrap();
        match &s.konto {
            Some(id) => konten.get(id).is_some(),
            None => self.offen() || konten.gemeinsames_passwort(),
        }
    }

    /// Die Sitzung zu einem Cookie-Header; im offenen Office auch ohne Cookie.
    pub fn aus_cookies(&self, cookies: Option<&str>, host: Option<&str>) -> Option<Sitzung> {
        let name = sitzung::cookie_name(host);
        let aus_cookie = cookies
            .and_then(|c| sitzung::cookie_lesen(c, &name))
            .and_then(|w| self.sitzungen.pruefen(w))
            .filter(|s| self.gueltig(s));
        aus_cookie.or_else(|| self.offen().then_some(Sitzung { konto: None }))
    }

    /// Als wer jemand angemeldet ist. Mit dem gemeinsamen Passwort ist man Admin (wie im Original).
    pub fn ich(&self, s: &Sitzung) -> Ich {
        let konten = self.konten.lock().unwrap();
        match s.konto.as_deref().and_then(|id| konten.get(id)) {
            Some(k) => Ich {
                konto: Some(KontoKurz {
                    name: k.name.clone(),
                    rolle: k.rolle,
                }),
                admin: k.rolle == Rolle::Admin,
            },
            None => Ich {
                konto: None,
                admin: s.konto.is_none(),
            },
        }
    }

    pub fn konto_name(&self, s: &Sitzung) -> Option<String> {
        let id = s.konto.as_deref()?;
        self.konten.lock().unwrap().get(id).map(|k| k.name.clone())
    }
}
