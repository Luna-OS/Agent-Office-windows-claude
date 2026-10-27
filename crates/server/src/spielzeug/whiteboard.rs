//! Das Whiteboard: Excalidraw-Elemente, zusammengeführt wie bei Excalidraws Live-Zusammenarbeit
//! (höhere `version` gewinnt, bei Gleichstand die kleinere `versionNonce`).

use serde_json::Value;

pub const MAX_ELEMENTE: usize = 5000;
const MAX_ELEMENT_BYTES: usize = 64 * 1024;

fn version(e: &Value) -> (i64, i64) {
    (e.get("version").and_then(Value::as_i64).unwrap_or(0), e.get("versionNonce").and_then(Value::as_i64).unwrap_or(0))
}

/// Ist `neu` neuer als `alt`?
pub fn neuer(neu: &Value, alt: &Value) -> bool {
    let (vn, nn) = version(neu);
    let (va, na) = version(alt);
    vn > va || (vn == va && nn < na)
}

fn id(e: &Value) -> Option<&str> {
    e.get("id").and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 64)
}

/// Führt `eingang` in `elemente` zusammen und gibt die übernommenen Elemente zurück (zum Weitergeben).
pub fn zusammenfuehren(elemente: &mut Vec<Value>, eingang: Vec<Value>) -> Vec<Value> {
    let mut uebernommen = Vec::new();
    for e in eingang {
        let Some(eid) = id(&e).map(String::from) else { continue };
        if !e.is_object() || serde_json::to_vec(&e).map(|b| b.len()).unwrap_or(usize::MAX) > MAX_ELEMENT_BYTES {
            continue;
        }
        let anzahl = elemente.len();
        match elemente.iter_mut().find(|x| id(x) == Some(eid.as_str())) {
            Some(alt) if neuer(&e, alt) => {
                *alt = e.clone();
                uebernommen.push(e);
            }
            Some(_) => {}
            None if anzahl < MAX_ELEMENTE => {
                elemente.push(e.clone());
                uebernommen.push(e);
            }
            None => {}
        }
    }
    uebernommen
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hoehere_version_gewinnt() {
        let mut el = vec![json!({"id":"a","version":2,"versionNonce":5})];
        assert!(zusammenfuehren(&mut el, vec![json!({"id":"a","version":1,"versionNonce":1})]).is_empty());
        assert_eq!(zusammenfuehren(&mut el, vec![json!({"id":"a","version":2,"versionNonce":3})]).len(), 1);
        assert_eq!(zusammenfuehren(&mut el, vec![json!({"id":"b","version":1,"versionNonce":3}), json!({"version":1})]).len(), 1);
        assert_eq!(el.len(), 2);
    }
}
