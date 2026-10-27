//! Das Wetter einer echten Stadt von open-meteo.com (frei, ohne Schlüssel): erst den Ort suchen,
//! dann alle 15 Minuten die aktuelle Lage holen.

use std::time::Duration;

use protokoll::einstellungen::Himmel;
use serde::Deserialize;

use super::himmel::aus_wmo;

const ORT_SUCHE: &str = "https://geocoding-api.open-meteo.com/v1/search";
const VORHERSAGE: &str = "https://api.open-meteo.com/v1/forecast";
pub const ABSTAND: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone)]
pub struct Ort {
    pub breite: f32,
    pub laenge: f32,
    pub name: String,
}

#[derive(Deserialize)]
struct Suche {
    #[serde(default)]
    results: Vec<Treffer>,
}

#[derive(Deserialize)]
struct Treffer {
    latitude: f32,
    longitude: f32,
    name: String,
    #[serde(default)]
    country: Option<String>,
}

#[derive(Deserialize)]
struct Antwort {
    current: Aktuell,
    #[serde(default)]
    utc_offset_seconds: i32,
}

#[derive(Deserialize)]
struct Aktuell {
    temperature_2m: f32,
    weather_code: u32,
}

fn holen(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

fn kodieren(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Sucht eine Stadt ("Berlin", "Hamburg, Deutschland" – nur der Teil vor dem Komma wird gesucht).
pub async fn ort_suchen(stadt: &str) -> Result<Ort, String> {
    let name = stadt.split(',').next().unwrap_or(stadt).trim().to_string();
    let url = format!(
        "{ORT_SUCHE}?name={}&count=1&language=de&format=json",
        kodieren(&name)
    );
    let text = tokio::task::spawn_blocking(move || holen(&url))
        .await
        .map_err(|e| e.to_string())??;
    let s: Suche = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let t = s
        .results
        .into_iter()
        .next()
        .ok_or_else(|| format!("Die Stadt {stadt} kennt open-meteo nicht"))?;
    Ok(Ort {
        breite: t.latitude,
        laenge: t.longitude,
        name: match t.country {
            Some(l) => format!("{}, {l}", t.name),
            None => t.name,
        },
    })
}

/// Holt das aktuelle Wetter für `ort`.
pub async fn wetter(ort: &Ort) -> Result<Himmel, String> {
    let url = format!(
        "{VORHERSAGE}?latitude={}&longitude={}&current=temperature_2m,weather_code&timezone=auto",
        ort.breite, ort.laenge
    );
    let text = tokio::task::spawn_blocking(move || holen(&url))
        .await
        .map_err(|e| e.to_string())??;
    let a: Antwort = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let (wetter, staerke) = aus_wmo(a.current.weather_code);
    Ok(Himmel {
        breite: ort.breite,
        laenge: ort.laenge,
        utc_versatz: a.utc_offset_seconds / 60,
        wetter,
        staerke,
        stadt: Some(ort.name.clone()),
        temperatur: Some(a.current.temperature_2m),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn kodieren() {
        assert_eq!(super::kodieren("Köln a"), "K%C3%B6ln%20a");
    }
}
