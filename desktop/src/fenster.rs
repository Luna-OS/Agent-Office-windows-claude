//! Das Fenster mit seiner WebView. Der Connector ist eine eigene Seite (`oao://`), deren Formulare
//! als Aktionen beim Hauptprogramm ankommen.

use crate::seiten::{self, Ansicht};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tao::event_loop::{EventLoop, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::http::{Request, Response};
use wry::{WebView, WebViewBuilder};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wo {
    Browser,
    Fenster,
}

/// Alles, was Fenster, Tray und Hintergrund-Threads dem Hauptprogramm mitteilen.
#[derive(Debug)]
pub enum Ereignis {
    Beitreten {
        adresse: String,
        wo: Wo,
    },
    LobbyGeprueft {
        basis: String,
        rest: String,
        wo: Wo,
        erreichbar: bool,
    },
    Vergessen(String),
    LokalStart(Wo),
    LokalOeffnen(Wo),
    LokalStop,
    FuerAndere(bool),
    ServerBereit {
        wo: Option<Wo>,
    },
    Anzeigen,
    Beenden,
}

#[cfg(windows)]
pub const START_URL: &str = "http://oao.localhost/";
#[cfg(not(windows))]
pub const START_URL: &str = "oao://localhost/";

pub fn fenster_bauen(schleife: &EventLoop<Ereignis>) -> Window {
    WindowBuilder::new()
        .with_title("OpenAgenticOffice")
        .with_inner_size(tao::dpi::LogicalSize::new(900.0, 720.0))
        .build(schleife)
        .expect("Das Fenster ließ sich nicht öffnen")
}

pub fn webview_bauen(
    fenster: &Window,
    ansicht: Arc<Mutex<Ansicht>>,
    proxy: EventLoopProxy<Ereignis>,
) -> WebView {
    let bauer = WebViewBuilder::new()
        .with_custom_protocol("oao".into(), move |_id, anfrage| {
            antwort(&anfrage, &ansicht, &proxy)
        })
        .with_url(START_URL);
    #[cfg(windows)]
    let bauer = {
        use wry::WebViewBuilderExtWindows;
        bauer.with_additional_browser_args(BROWSER_SCHALTER)
    };
    bauer
        .build(fenster)
        .expect("Die Ansicht ließ sich nicht erzeugen (WebView2 bzw. WebKit fehlt?)")
}

/// Schalter für WebView2. Steht die Grafikkarte auf Chromiums Sperrliste, gibt es dort kein WebGPU, und
/// das 3D-Office bleibt leer; im Fenster läuft nur das Office, also schalten wir es immer frei. Die ersten
/// beiden sind wrys Vorgaben (ohne Mini-Menü und SmartScreen, Ton ohne Klick), die eigene Schalter ersetzen.
#[cfg(windows)]
const BROWSER_SCHALTER: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
    --autoplay-policy=no-user-gesture-required --enable-unsafe-webgpu --ignore-gpu-blocklist";

fn html(seite: String) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Cow::Owned(seite.into_bytes()))
        .expect("Antwort bauen")
}

fn antwort(
    anfrage: &Request<Vec<u8>>,
    ansicht: &Arc<Mutex<Ansicht>>,
    proxy: &EventLoopProxy<Ereignis>,
) -> Response<Cow<'static, [u8]>> {
    let pfad = anfrage.uri().path();
    let Some(name) = pfad.strip_prefix("/aktion/") else {
        return html(seiten::startseite(
            &ansicht.lock().map(|a| a.clone()).unwrap_or_default(),
        ));
    };
    let felder = felder_lesen(anfrage.uri().query().unwrap_or(""));
    let feld = |n: &str| {
        felder
            .iter()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let wo = if feld("wo") == "fenster" {
        Wo::Fenster
    } else {
        Wo::Browser
    };
    let ereignis = match name {
        "beitreten" => Some(Ereignis::Beitreten {
            adresse: feld("adresse"),
            wo,
        }),
        "vergessen" => Some(Ereignis::Vergessen(feld("adresse"))),
        "lokal_start" => Some(Ereignis::LokalStart(wo)),
        "lokal_oeffnen" => Some(Ereignis::LokalOeffnen(wo)),
        "lokal_stop" => Some(Ereignis::LokalStop),
        "fuer_andere" => Some(Ereignis::FuerAndere(feld("an") == "ja")),
        _ => None,
    };
    if let Some(e) = ereignis {
        let _ = proxy.send_event(e);
    }
    html(seiten::warteseite())
}

/// `a=b&c=d` mit Prozent-Kodierung lesen.
pub fn felder_lesen(abfrage: &str) -> Vec<(String, String)> {
    abfrage
        .split('&')
        .filter(|t| !t.is_empty())
        .map(|t| {
            let (k, v) = t.split_once('=').unwrap_or((t, ""));
            (entkodieren(k), entkodieren(v))
        })
        .collect()
}

fn entkodieren(t: &str) -> String {
    let b = t.as_bytes();
    let mut aus = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => aus.push(b' '),
            b'%' => match t
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                Some(x) => {
                    aus.push(x);
                    i += 2;
                }
                None => aus.push(b'%'),
            },
            x => aus.push(x),
        }
        i += 1;
    }
    String::from_utf8_lossy(&aus).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn felder_mit_prozent_und_plus() {
        let f = felder_lesen("adresse=http%3A%2F%2Fx%3A4600%2Fbeitreten%23ab&wo=fenster&n=a+b");
        assert_eq!(f[0].1, "http://x:4600/beitreten#ab");
        assert_eq!(f[1], ("wo".to_string(), "fenster".to_string()));
        assert_eq!(f[2].1, "a b");
    }

    #[test]
    fn kaputte_kodierung_bleibt_stehen() {
        assert_eq!(felder_lesen("a=%zz")[0].1, "%zz");
        assert_eq!(felder_lesen("a=100%")[0].1, "100%");
    }
}
