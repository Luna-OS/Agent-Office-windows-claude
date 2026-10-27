//! Speichern im Datenordner: JSON-Dateien, atomar geschrieben (erst .tmp, dann umbenennen).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Liest eine JSON-Datei; fehlt sie oder ist sie kaputt, gibt es den Standardwert (kaputt wird gemeldet).
pub fn laden<T: DeserializeOwned + Default>(pfad: &Path) -> T {
    match std::fs::read(pfad) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(wert) => wert,
            Err(e) => {
                tracing::warn!("{} ist nicht lesbar ({e}), fange leer an", pfad.display());
                let _ = std::fs::copy(pfad, pfad.with_extension("kaputt.json"));
                T::default()
            }
        },
        Err(_) => T::default(),
    }
}

/// Schreibt eine JSON-Datei atomar. Fehler werden gemeldet, aber nicht weitergereicht:
/// ein voller Datenträger soll das Office nicht anhalten.
pub fn speichern<T: Serialize + ?Sized>(pfad: &Path, wert: &T) {
    if let Err(e) = versuchen(pfad, wert) {
        tracing::error!("Konnte {} nicht speichern: {e}", pfad.display());
    }
}

fn versuchen<T: Serialize + ?Sized>(pfad: &Path, wert: &T) -> std::io::Result<()> {
    if let Some(dir) = pfad.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = pfad.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(wert).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, pfad)
}

/// Jetzt in ms seit 1970.
pub fn jetzt_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Eine zufällige ID aus `laenge` Hex-Zeichen.
pub fn zufalls_id(laenge: usize) -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    (0..laenge).map(|_| char::from_digit(rng.random_range(0..16), 16).unwrap_or('0')).collect()
}

/// Ein zufälliges Geheimnis, base64url, aus `bytes` Zufallsbytes.
pub fn geheimnis(bytes: usize) -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut b = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut b);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn rundreise() {
        let dir = tempfile::tempdir().unwrap();
        let pfad = dir.path().join("a/b.json");
        let mut m = BTreeMap::new();
        m.insert("x".to_string(), 1u32);
        speichern(&pfad, &m);
        let wieder: BTreeMap<String, u32> = laden(&pfad);
        assert_eq!(wieder, m);
        let leer: BTreeMap<String, u32> = laden(&dir.path().join("fehlt.json"));
        assert!(leer.is_empty());
    }
}
