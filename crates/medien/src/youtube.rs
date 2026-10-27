//! YouTube-URLs lesen: Video-ID und Startzeit aus allen üblichen Formen.
//!
//! `watch?v=`, `youtu.be/`, `shorts/`, `embed/`, `live/`, `v/`, `m.`/`music.`/`www.`, `youtube-nocookie.com`,
//! Startzeit als `t=90`, `t=1m30s`, `t=1h2m3s`, `start=90`, auch hinter `#`. Eine nackte ID geht auch.

/// Eine gelesene YouTube-Adresse.
#[derive(Debug, Clone, PartialEq)]
pub struct YoutubeVideo {
    pub id: String,
    /// Startzeit in Sekunden (0, wenn keine angegeben ist).
    pub start_s: f64,
}

/// Liest eine YouTube-URL oder eine nackte Video-ID. Gibt eine deutsche Fehlermeldung zurück, wenn nicht.
pub fn video_aus_url(eingabe: &str) -> Result<YoutubeVideo, String> {
    let s = eingabe.trim();
    if s.is_empty() {
        return Err("Bitte eine YouTube-Adresse eingeben".into());
    }
    if s.len() > 2048 {
        return Err("Die Adresse ist zu lang".into());
    }
    if gueltige_id(s) {
        return Ok(YoutubeVideo { id: s.to_string(), start_s: 0.0 });
    }
    let ohne_schema = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .or_else(|| s.strip_prefix("//"))
        .unwrap_or(s);
    let (wirt, rest) = match ohne_schema.find(['/', '?', '#']) {
        Some(i) => (&ohne_schema[..i], &ohne_schema[i..]),
        None => (ohne_schema, ""),
    };
    let wirt = wirt.to_ascii_lowercase();
    let wirt = wirt.split(':').next().unwrap_or("");
    let (pfad, abfrage, anker) = zerlegen(rest);
    let parameter: Vec<(String, String)> = paare(abfrage).chain(paare(anker)).collect();
    let param = |name: &str| parameter.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());

    let id = if wirt == "youtu.be" || wirt == "www.youtu.be" {
        pfad.trim_start_matches('/').split('/').next().map(str::to_string)
    } else if ist_youtube(wirt) {
        let teile: Vec<&str> = pfad.split('/').filter(|t| !t.is_empty()).collect();
        match teile.as_slice() {
            ["shorts" | "embed" | "live" | "v" | "e", id, ..] => Some(id.to_string()),
            ["attribution_link", ..] => param("u")
                .and_then(|u| video_aus_url(&format!("https://www.youtube.com{u}")).ok())
                .map(|v| v.id),
            _ => param("v").map(str::to_string),
        }
    } else {
        return Err("Das ist keine YouTube-Adresse".into());
    };
    let id = id.filter(|i| gueltige_id(i)).ok_or("In der Adresse steckt keine gültige Video-ID")?;
    let start_s = param("t").or_else(|| param("start")).and_then(zeit_lesen).unwrap_or(0.0);
    Ok(YoutubeVideo { id, start_s })
}

fn ist_youtube(wirt: &str) -> bool {
    matches!(
        wirt,
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtube-nocookie.com" | "www.youtube-nocookie.com"
    )
}

/// Eine Video-ID: genau 11 Zeichen aus `A-Z a-z 0-9 - _`.
pub fn gueltige_id(s: &str) -> bool {
    s.len() == 11 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `/pfad?abfrage#anker` in seine drei Teile.
fn zerlegen(rest: &str) -> (&str, &str, &str) {
    let (vor_anker, anker) = rest.split_once('#').unwrap_or((rest, ""));
    let (pfad, abfrage) = vor_anker.split_once('?').unwrap_or((vor_anker, ""));
    (pfad, abfrage, anker)
}

fn paare(s: &str) -> impl Iterator<Item = (String, String)> + '_ {
    s.split('&').filter(|p| !p.is_empty()).map(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        (k.to_string(), prozent_dekodieren(v))
    })
}

fn prozent_dekodieren(s: &str) -> String {
    let b = s.as_bytes();
    let mut aus = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 1 {
            if let Some(x) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                aus.push(x);
                i += 3;
                continue;
            }
        }
        aus.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&aus).into_owned()
}

/// `90`, `90s`, `1m30s`, `1h2m3s`, `01:30`, `1:02:03` in Sekunden.
pub fn zeit_lesen(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        return (n.is_finite() && n >= 0.0).then_some(n);
    }
    if s.contains(':') {
        let mut summe = 0.0;
        for teil in s.split(':') {
            summe = summe * 60.0 + teil.parse::<u32>().ok()? as f64;
        }
        return Some(summe);
    }
    let mut summe = 0.0;
    let mut zahl = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' => zahl.push(c),
            'h' | 'm' | 's' => {
                let n: f64 = zahl.parse().ok()?;
                zahl.clear();
                summe += n * match c {
                    'h' => 3600.0,
                    'm' => 60.0,
                    _ => 1.0,
                };
            }
            _ => return None,
        }
    }
    if !zahl.is_empty() {
        summe += zahl.parse::<f64>().ok()?;
    }
    Some(summe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(url: &str) -> (String, f64) {
        let v = video_aus_url(url).unwrap_or_else(|e| panic!("{url}: {e}"));
        (v.id, v.start_s)
    }

    #[test]
    fn alle_formen() {
        let id = "dQw4w9WgXcQ".to_string();
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "http://youtube.com/watch?v=dQw4w9WgXcQ&list=PL123&index=2",
            "youtube.com/watch?feature=share&v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ&si=abc",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?si=xyz",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://youtube.com/shorts/dQw4w9WgXcQ?feature=share",
            "https://www.youtube.com/embed/dQw4w9WgXcQ?rel=0",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ?feature=shared",
            "https://www.youtube.com/v/dQw4w9WgXcQ",
            "  https://WWW.YouTube.com/watch?v=dQw4w9WgXcQ  ",
            "dQw4w9WgXcQ",
        ] {
            assert_eq!(ok(url), (id.clone(), 0.0), "{url}");
        }
    }

    #[test]
    fn startzeiten() {
        assert_eq!(ok("https://youtu.be/dQw4w9WgXcQ?t=42").1, 42.0);
        assert_eq!(ok("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m30s").1, 90.0);
        assert_eq!(ok("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1h2m3s").1, 3723.0);
        assert_eq!(ok("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=90s").1, 90.0);
        assert_eq!(ok("https://www.youtube.com/embed/dQw4w9WgXcQ?start=15").1, 15.0);
        assert_eq!(ok("https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=2m").1, 120.0);
        assert_eq!(ok("https://youtube.com/shorts/dQw4w9WgXcQ?t=7").1, 7.0);
        assert_eq!(zeit_lesen("1:02:03"), Some(3723.0));
        assert_eq!(zeit_lesen("abc"), None);
    }

    #[test]
    fn fehler() {
        for url in [
            "",
            "https://vimeo.com/12345",
            "https://www.youtube.com/watch?v=kurz",
            "https://www.youtube.com/channel/UC123",
            "https://youtu.be/",
            "https://evil.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com.evil.com/watch?v=dQw4w9WgXcQ",
        ] {
            assert!(video_aus_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn attribution_link() {
        assert_eq!(
            ok("https://www.youtube.com/attribution_link?a=x&u=%2Fwatch%3Fv%3DdQw4w9WgXcQ%26feature%3Dshare").0,
            "dQw4w9WgXcQ"
        );
    }
}
