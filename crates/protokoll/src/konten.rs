//! Anmeldung, Spieler, Rollen (Admin), Einladungen, eigene Anmeldungen (Claude/GitHub), Lobby.
//!
//! Die eigentliche Anmeldung läuft über HTTP (`/api/anmeldung`, `/api/beitreten`, `/api/link`);
//! hier stehen nur die Nachrichten, die im laufenden Office über den WebSocket gehen.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rolle {
    Admin,
    Mitglied,
}

/// Name und Rolle eines Kontos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KontoKurz {
    pub name: String,
    pub rolle: Rolle,
}

/// Als wer dieser Browser angemeldet ist.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Ich {
    /// Das eigene Konto; fehlt bei Anmeldung mit dem gemeinsamen Passwort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub konto: Option<KontoKurz>,
    /// Darf einladen, Konten sehen und entziehen.
    pub admin: bool,
}

/// Worauf sich jemand für die eigenen Arbeiter anmeldet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnmeldeArt {
    Claude,
    Github,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnmeldeStatus {
    /// Angemeldet.
    Ok,
    /// Noch nicht.
    Keine,
    /// Meldet sich gerade an oder wird geprüft.
    Laeuft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnmeldeWeg {
    /// Eigener Login im eigenen Ordner auf dem Rechner des Office.
    Login,
    /// Eingefügter Token.
    Token,
    /// Die Anmeldung des Office-Rechners selbst.
    Office,
}

/// Eine laufende Anmeldung: die Seite zum Öffnen, GitHubs Einmal-Code, ob Claudes Code schon geschickt wurde.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AnmeldungOffen {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default)]
    pub geschickt: bool,
}

/// Eine der eigenen Anmeldungen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anmeldung {
    pub status: AnmeldeStatus,
    pub weg: AnmeldeWeg,
    /// Als wer: E-Mail und Plan bei Claude, @login bei GitHub.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offen: Option<AnmeldungOffen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
}

/// Die eigenen Claude- und GitHub-Anmeldungen (nur mit Konto).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anmeldungen {
    pub claude: Anmeldung,
    pub github: Anmeldung,
    /// Ob man die Anmeldungen des Office-Rechners nutzen darf: Admins beide, Mitglieder Claude, wenn geteilt.
    pub office_claude: bool,
    pub office_github: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KontoInfo {
    pub id: String,
    pub name: String,
    pub rolle: Rolle,
    pub erstellt_um: u64,
    pub erstellt_von: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zuletzt_da: Option<u64>,
    /// Gerade im Office.
    pub online: bool,
}

/// Ein Einmal-Link, der ein Konto anlegt: /beitreten#<token>.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Einladung {
    pub id: String,
    pub token: String,
    /// Der Name des Kontos; fehlt er, wählt ihn, wer den Link öffnet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub rolle: Rolle,
    pub erstellt_von: String,
    pub erstellt_um: u64,
    pub gueltig_bis: u64,
}

/// Alle Konten, für Admins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KontenStand {
    pub konten: Vec<KontoInfo>,
    pub einladungen: Vec<Einladung>,
    /// Ob das gemeinsame Passwort noch hereinlässt.
    pub gemeinsames_passwort: bool,
    /// Ob Mitglieder das Claude des Office-Rechners nutzen dürfen.
    pub claude_teilen: bool,
    /// Wo andere Rechner das Office erreichen (für Einladungslinks); leer, wenn nur dieser Rechner.
    #[serde(default)]
    pub adressen: Vec<String>,
}

/// Wer per SSH-Tunnel ins Office darf (nur bei Offices, die mit deploy/provision.sh aufgesetzt sind).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TeamStand {
    /// Warum Einladungen hier nicht gehen, wenn nicht.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nicht_verfuegbar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fehler: Option<String>,
    /// Wohin getunnelt wird: office@203.0.113.7 oder ssh://office@host:port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<String>,
    /// Port des Office auf dem Rechner (Ziel des Tunnels).
    pub port: u16,
    /// Wie das Deploy-Skript aufgerufen wurde (z. B. "deploy/aws.sh --name x"), für vorgeschlagene Befehle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy: Option<String>,
    /// SHA256-Fingerabdruck des ED25519-Host-Schlüssels, zum Prüfen beim ersten Verbinden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerabdruck: Option<String>,
    pub mitglieder: Vec<TeamMitglied>,
    /// Name im Tailscale-Netz: alle dort öffnen https://<name>.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tailnet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamMitglied {
    /// GitHub-Name (oder der Name, unter dem ein Schlüssel eingeladen wurde).
    pub name: String,
    pub schluessel: u32,
}

/// Nachrichten vom Browser. Alles mit Konten/Einladungen ist nur für Admins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    KontenHolen,
    Einladen {
        #[serde(default)]
        name: Option<String>,
        rolle: Rolle,
    },
    EinladungZurueckziehen { einladung: String },
    Entziehen { konto: String },
    RolleSetzen { konto: String, rolle: Rolle },
    /// Gemeinsames Passwort an/aus.
    GemeinsamesPasswort { an: bool },
    /// Mitglieder dürfen das Claude des Office-Rechners nutzen.
    ClaudeTeilen { an: bool },
    /// Eigene Anmeldungen (nur mit Konto).
    AnmeldungenHolen,
    AnmeldungStarten { art: AnmeldeArt },
    /// Der Code von Claudes Anmeldeseite.
    AnmeldungCode { code: String },
    AnmeldungAbbrechen { art: AnmeldeArt },
    AnmeldungToken { art: AnmeldeArt, token: String },
    AnmeldungOffice { art: AnmeldeArt },
    Abmelden { art: AnmeldeArt },
    /// Einen Einmal-Link für einen anderen Browser (Connector), fünf Minuten gültig; Antwort `Link`.
    LinkErzeugen,
    /// SSH-Team (Admins): Stand holen, die SSH-Schlüssel eines GitHub-Nutzers einladen, jemanden entfernen.
    TeamHolen,
    TeamEinladen { github: String },
    TeamEntfernen { name: String },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// An Admins, auf Nachfrage und bei jeder Änderung.
    Konten { stand: KontenStand },
    /// An den, der eingeladen hat.
    Eingeladen {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        einladung: Option<Einladung>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fehler: Option<String>,
    },
    /// Die eigene Rolle hat sich geändert.
    Ich { ich: Ich },
    Anmeldungen { stand: Anmeldungen },
    /// Dafür braucht es erst eine eigene Anmeldung.
    AnmeldungNoetig { art: AnmeldeArt, warum: String },
    /// Antwort auf `LinkErzeugen`: Pfad wie "/anmelden#schluessel=…".
    Link { pfad: String },
    Team { stand: TeamStand },
    /// An den, der eingeladen hat.
    TeamEingeladen {
        github: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schluessel: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fehler: Option<String>,
    },
}
