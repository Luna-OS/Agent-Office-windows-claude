//! Passwörter: scrypt-Hash mit Salz, Vergleich in konstanter Zeit.

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD as B64;
use rand::RngCore;

pub const PASSWORT_MIN: usize = 8;
pub const PASSWORT_MAX: usize = 512;

/// Ein gespeicherter Passwort-Hash: "salz$hash", beides base64.
pub fn hashen(passwort: &str) -> String {
    let mut salz = [0u8; 16];
    rand::rng().fill_bytes(&mut salz);
    format!("{}${}", B64.encode(salz), B64.encode(ableiten(passwort, &salz)))
}

/// Passt `passwort` zum gespeicherten Hash?
pub fn pruefen(passwort: &str, gespeichert: &str) -> bool {
    let Some((salz, hash)) = gespeichert.split_once('$') else { return false };
    let (Ok(salz), Ok(hash)) = (B64.decode(salz), B64.decode(hash)) else { return false };
    let passwort = &passwort[..passwort.len().min(PASSWORT_MAX)];
    gleich(&ableiten(passwort, &salz), &hash)
}

/// Prüft gegen einen Wegwerf-Hash, damit ein unbekannter Name gleich lange dauert wie ein bekannter.
pub fn blind_pruefen(passwort: &str) {
    let _ = ableiten(passwort, b"0123456789abcdef");
}

fn ableiten(passwort: &str, salz: &[u8]) -> [u8; 32] {
    // N = 2^14, r = 8, p = 1: wie Node's scrypt-Standard.
    let params = scrypt::Params::new(14, 8, 1, 32).expect("gültige scrypt-Parameter");
    let mut aus = [0u8; 32];
    scrypt::scrypt(passwort.as_bytes(), salz, &params, &mut aus).expect("Ausgabelänge passt");
    aus
}

/// Vergleich in konstanter Zeit.
pub fn gleich(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Ein neues, gut tippbares Passwort (für `--passwort-zuruecksetzen`).
pub fn erzeugen() -> String {
    const ZEICHEN: &[u8] = b"abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut b = [0u8; 18];
    rand::rng().fill_bytes(&mut b);
    let s: String = b.iter().map(|x| ZEICHEN[*x as usize % ZEICHEN.len()] as char).collect();
    format!("{}-{}-{}", &s[0..6], &s[6..12], &s[12..18])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_und_pruefen() {
        let h = hashen("geheim123");
        assert!(pruefen("geheim123", &h));
        assert!(!pruefen("falsch", &h));
        assert!(!pruefen("x", "kaputt"));
    }

    #[test]
    fn erzeugtes_passwort_ist_lang_genug() {
        assert!(erzeugen().len() >= PASSWORT_MIN);
    }
}
