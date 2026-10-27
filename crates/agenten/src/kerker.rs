//! Der Kerker einer Etage: auf Karten, die heimgeschickte Arbeiter einsperren (das sagt der Server
//! mit [`crate::Agenten::kerker_aktiv_setzen`]), landet jeder dort, für immer. Die Welt rechnet aus,
//! wie weit er seitdem verfallen ist; hier steht nur wer und wann. `<etage>/kerker.json`.

use std::path::Path;

use protokoll::agenten::{Gefangener, Kerker};
use protokoll::arbeiter::ArbeiterInfo;

/// So viele behält der Kerker mit Namen; die davor sind nur noch eine Zahl, Knochen auf dem Haufen.
pub const MAX_GEFANGENE: usize = 200;

pub fn laden(ordner: &Path) -> Kerker {
    std::fs::read_to_string(ordner.join("kerker.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn speichern(ordner: &Path, k: &Kerker) {
    let _ = std::fs::create_dir_all(ordner);
    if let Ok(text) = serde_json::to_string_pretty(k) {
        let _ = std::fs::write(ordner.join("kerker.json"), text);
    }
}

/// Sperrt einen Arbeiter ein, ab `um`.
pub fn einsperren(k: &mut Kerker, info: &ArbeiterInfo, um: u64) {
    k.gefangene.retain(|g| g.id != info.id);
    let gearbeitet_ms = info.gearbeitet_ms + info.arbeitet_seit.map_or(0, |s| um.saturating_sub(s));
    k.gefangene.push(Gefangener {
        id: info.id.clone(),
        name: info.name.clone(),
        farbe: info.farbe.clone(),
        um,
        gearbeitet_ms,
    });
    let ueber = k.gefangene.len().saturating_sub(MAX_GEFANGENE);
    if ueber > 0 {
        k.gefangene.drain(..ueber);
        k.knochen += ueber as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voll_wird_zu_knochen() {
        let mut k = Kerker::default();
        let mut info = crate::provider::claude::tests::info();
        for i in 0..(MAX_GEFANGENE + 3) {
            info.id = format!("a{i}");
            einsperren(&mut k, &info, i as u64);
        }
        assert_eq!(k.gefangene.len(), MAX_GEFANGENE);
        assert_eq!(k.knochen, 3);
        assert_eq!(k.gefangene[0].id, "a3");
        let dir = tempfile::tempdir().unwrap();
        speichern(dir.path(), &k);
        assert_eq!(laden(dir.path()), k);
    }
}
