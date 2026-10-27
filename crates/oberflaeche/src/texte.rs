//! Kleine Helfer für Texte: Status-Namen, "vor 5 Min.", Kürzen.

use protokoll::arbeiter::ArbeiterStatus;

/// Wie ein Status im UI heißt.
pub fn status(s: ArbeiterStatus) -> &'static str {
    match s {
        ArbeiterStatus::Startet => "startet",
        ArbeiterStatus::Bereit => "bereit",
        ArbeiterStatus::Arbeitet => "arbeitet",
        ArbeiterStatus::Wartet => "braucht dich",
        ArbeiterStatus::Fertig => "fertig",
        ArbeiterStatus::Beendet => "beendet",
        ArbeiterStatus::Offline => "schläft",
    }
}

/// Tailwind-Klassen für das Status-Abzeichen.
pub fn status_klassen(s: ArbeiterStatus) -> &'static str {
    match s {
        ArbeiterStatus::Startet | ArbeiterStatus::Bereit => "bg-info/20 text-foreground",
        ArbeiterStatus::Arbeitet => "bg-warning text-warning-foreground",
        ArbeiterStatus::Wartet => "bg-destructive text-white animate-pulse",
        ArbeiterStatus::Fertig => "bg-success text-success-foreground",
        ArbeiterStatus::Beendet | ArbeiterStatus::Offline => "bg-muted text-muted-foreground",
    }
}

/// Jetzt in ms seit 1970.
pub fn jetzt() -> u64 {
    js_sys::Date::now() as u64
}

/// "gerade eben", "vor 5 Min.", "vor 3 Std.", "vor 2 Tagen".
pub fn her(um: u64) -> String {
    let s = jetzt().saturating_sub(um) / 1000;
    match s {
        0..60 => "gerade eben".into(),
        60..3600 => format!("vor {} Min.", s / 60),
        3600..86400 => format!("vor {} Std.", s / 3600),
        _ => match s / 86400 {
            1 => "vor 1 Tag".into(),
            n => format!("vor {n} Tagen"),
        },
    }
}

/// Uhrzeit "14:05" in der Zeitzone des Browsers.
pub fn uhrzeit(um: u64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(um as f64));
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}

/// Höchstens `max` Zeichen, sonst mit "…".
pub fn kuerzen(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// "1,2k" für große Zahlen.
pub fn zahl_kurz(n: u64) -> String {
    match n {
        0..1000 => n.to_string(),
        1000..1_000_000 => format!("{:.1}k", n as f64 / 1000.0).replace('.', ","),
        _ => format!("{:.1} Mio.", n as f64 / 1_000_000.0).replace('.', ","),
    }
}

/// "$1,23".
pub fn dollar(d: f64) -> String {
    format!("${d:.2}").replace('.', ",")
}
