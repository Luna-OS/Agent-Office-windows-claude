//! Spielsachen im Office: Bilder an den Wänden, Hund, Basketball, Autos in der Garage, Arcade,
//! Whiteboard, Dachbar-Spiele (Darts/Axt), Golf. (Die Jukebox ist in `medien`.)

use serde::{Deserialize, Serialize};

use crate::praesenz::Barspiel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wand {
    Nord,
    Sued,
    Ost,
    West,
}

/// Wo und wie ein Bild hängt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DekorPlatz {
    /// Das Bild im Netz (http/https).
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titel: Option<String>,
    pub wand: Wand,
    /// Mitte entlang der Wand: x an Nord/Süd, z an Ost/West.
    pub u: f32,
    /// Höhe der Mitte über dem Boden.
    pub y: f32,
    /// Größe in Metern.
    pub b: f32,
    pub h: f32,
    /// Index in die Rahmen des Clients.
    pub rahmen: u32,
}

/// Ein Bild an der Wand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dekor {
    pub id: String,
    #[serde(flatten)]
    pub platz: DekorPlatz,
    pub von: String,
    pub um: u64,
}

/// Teiländerung eines Bildes; fehlende Felder bleiben.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DekorAenderung {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wand: Option<Wand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rahmen: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HundTun {
    Stehen,
    Sitzen,
    Liegen,
    Schlafen,
    Schnueffeln,
    Bellen,
    Wedeln,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rasse {
    Welpe,
    Corgi,
    Dackel,
    Mops,
    Shiba,
}

/// Was der Hund der Etage gerade tut: ein Wegstück, zu Beginn jedes Stücks gesendet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hund {
    pub name: String,
    /// Index in die Fellfarben des Clients.
    pub fell: u32,
    pub rasse: Rasse,
    /// Von wo er beim Start des Stücks war, dann Punkt für Punkt ([x, z]). Nie leer.
    pub weg: Vec<[f32; 2]>,
    /// m/s entlang des Wegs.
    pub tempo: f32,
    /// Wie lange das Stück beim Senden schon läuft, in ms.
    pub vergangen_ms: u64,
    pub tun: HundTun,
    /// Blickrichtung am Ziel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blick: Option<f32>,
    /// Bellen: der Arbeiter, der wartet. Schlafen: unter wessen Tisch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arbeiter: Option<String>,
    /// Wem er hinterhertrottet (Verbindungs-ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folgt: Option<String>,
    /// Wedeln: wer ihn gestreichelt hat (Name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gestreichelt_von: Option<String>,
}

/// Ein Wurf des Basketballs: Start, Geschwindigkeit, Werfer, seit wann.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wurf {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    pub vergangen_ms: u64,
}

/// Der Basketball: wer ihn hat, oder wie er zuletzt geworfen wurde; beides leer = unter dem Korb.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Ball {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wurf: Option<Wurf>,
    /// Wer zuletzt geworfen hat (Verbindungs-ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub werfer: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoSitz {
    Fahrer,
    Beifahrer,
}

/// Ein Auto in der Garage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Auto {
    pub x: f32,
    pub z: f32,
    pub rot_y: f32,
    pub tempo: f32,
    pub lenkung: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fahrer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beifahrer: Option<String>,
}

/// Ein Bild des Spiels am Arcade-Automaten (ein Tetris-artiges Spiel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArcadeBild {
    /// Der Schacht Zeile für Zeile von oben, ein Zeichen pro Zelle: '0' leer, '1'–'7' Farbe, '8' Landeplatz.
    pub zellen: String,
    pub naechstes: u8,
    pub halten: u8,
    pub punkte: u32,
    pub reihen: u32,
    pub level: u32,
    /// Gelandete Teile in diesem Spiel.
    pub teile: u32,
    /// "laeuft", "pause" oder "vorbei".
    pub zustand: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bestenliste {
    pub spiel: String,
    pub name: String,
    pub farbe: String,
    pub punkte: u32,
    pub reihen: u32,
    pub level: u32,
    pub um: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArcadeSpieler {
    pub id: String,
    pub name: String,
    pub spiel: String,
}

/// Wer am Automaten steht, und die Bestenliste des Gebäudes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Arcade {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spieler: Option<ArcadeSpieler>,
    pub bestenliste: Vec<Bestenliste>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bild: Option<ArcadeBild>,
}

/// Ein Excalidraw-Element; der Server braucht nur `id`, `version` und `versionNonce`, der Rest reist mit.
pub type WbElement = serde_json::Value;

/// Wo jemandes Maus auf dem Whiteboard ist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WbZeiger {
    pub x: f32,
    pub y: f32,
    /// "pointer" oder "laser".
    pub werkzeug: String,
    /// "up" oder "down".
    pub taste: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ausgewaehlt: Vec<String>,
}

/// Der Spielzeug-Stand einer Etage, beim Betreten.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Stand {
    pub dekor: Vec<Dekor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hund: Option<Hund>,
    pub ball: Ball,
    pub autos: Vec<Auto>,
    pub arcade: Arcade,
    pub whiteboard: Vec<WbElement>,
    /// Wer das Whiteboard offen hat (Verbindungs-IDs).
    pub whiteboard_leute: Vec<String>,
}

/// Nachrichten vom Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Client {
    DekorHinzufuegen {
        platz: DekorPlatz,
    },
    DekorAendern {
        id: String,
        aenderung: DekorAenderung,
    },
    DekorEntfernen {
        id: String,
    },
    /// Den Hund streicheln (muss in Reichweite sein).
    HundStreicheln,
    /// Den Hund umbenennen ("" gibt ihm seinen ersten Namen zurück).
    HundName {
        name: String,
    },
    /// Den Basketball nehmen oder fangen.
    BallNehmen,
    /// Den Ball werfen (oder fallen lassen).
    BallWerfen {
        x: f32,
        y: f32,
        z: f32,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    AutoEinsteigen {
        auto: u32,
        sitz: AutoSitz,
    },
    AutoAussteigen,
    AutoFahren {
        auto: u32,
        x: f32,
        z: f32,
        rot_y: f32,
        tempo: f32,
        lenkung: f32,
    },
    Hupen,
    /// An den Automaten treten (mit `spiel` weiterspielen, sonst neues Spiel).
    ArcadeSpielen {
        #[serde(default)]
        spiel: Option<String>,
    },
    ArcadeVerlassen,
    ArcadeBild {
        bild: ArcadeBild,
    },
    WbOeffnen,
    WbSchliessen,
    WbAendern {
        elemente: Vec<WbElement>,
    },
    WbZeiger {
        zeiger: WbZeiger,
    },
    /// Golf-Abschlag vom Balkon.
    Golf {
        gier: f32,
        neigung: f32,
        kraft: f32,
    },
    /// Ein Wurf an der Dachbar: Treffer (u, v) auf der Scheibe, steckt, n-ter Wurf der Runde.
    Barwurf {
        spiel: Barspiel,
        u: f32,
        v: f32,
        steckt: bool,
        n: u32,
    },
}

/// Nachrichten an den Browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum Server {
    /// Alles auf einmal, nach `etagen::Server::Betreten`.
    Stand {
        stand: Box<Stand>,
    },
    Dekor {
        liste: Vec<Dekor>,
    },
    Hund {
        hund: Hund,
    },
    Ball {
        ball: Ball,
    },
    /// `antwort`: an den, der ein-/aussteigen wollte, ob es klappte oder nicht.
    Autos {
        autos: Vec<Auto>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        antwort: Option<bool>,
    },
    AutoBewegt {
        auto: u32,
        x: f32,
        z: f32,
        rot_y: f32,
        tempo: f32,
        lenkung: f32,
    },
    Hupe {
        auto: u32,
    },
    Arcade {
        arcade: Arcade,
    },
    ArcadeBild {
        bild: ArcadeBild,
    },
    WbAenderung {
        elemente: Vec<WbElement>,
    },
    WbLeute {
        leute: Vec<String>,
    },
    WbZeiger {
        id: String,
        zeiger: WbZeiger,
    },
    Golf {
        id: String,
        gier: f32,
        neigung: f32,
        kraft: f32,
    },
    Barwurf {
        id: String,
        spiel: Barspiel,
        u: f32,
        v: f32,
        steckt: bool,
        n: u32,
    },
}
