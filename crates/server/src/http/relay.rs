//! Dienste-Tunnel: `ssh -L 5173:localhost:4600 office@rechner` landet auf dem Port des Office, und der
//! Host-Header (localhost:5173) sagt, welcher Server eines Arbeiters gemeint ist. So erreicht das Team
//! jeden Dienst über den einen Port, den sein SSH-Schlüssel weiterleiten darf, und nur angemeldet.
//! Im Tailnet ist es https://<office>.ts.net:5173, das Tailscale Serve ebenfalls hierher zeigt.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::anmeldung::sitzung::COOKIE;
use crate::office::OfficeRef;

/// Gesetzt auf allem, was das Office weiterreicht, damit ein Dienst, der zurück aufs Office zeigt, keine Schleife baut.
const WEITERGEREICHT: &str = "x-office-relay";
/// Hierhin schickt das Anmeldeformular auf einem Tunnel.
pub const ANMELDE_PFAD: &str = "/__office/anmeldung";

fn host_port(host: &str) -> Option<(&str, u16)> {
    let (h, p) = host.rsplit_once(':')?;
    Some((h, p.parse().ok()?))
}

fn ist_lokal(h: &str) -> bool {
    let h = h.to_lowercase();
    h == "localhost" || h == "127.0.0.1" || h == "[::1]" || h.ends_with(".localhost")
}

/// Der Dienst-Port einer Anfrage, die über einen Tunnel oder das Tailnet kam.
pub fn tunnel_port(o: &OfficeRef, headers: &axum::http::HeaderMap) -> Option<u16> {
    if headers.contains_key(WEITERGEREICHT) {
        return None;
    }
    let host = headers.get(header::HOST)?.to_str().ok()?;
    let (h, port) = host_port(host)?;
    let passt = ist_lokal(h)
        || o.konfig
            .tailscale_host
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(h));
    (passt && port != o.konfig.port && o.konfig.port != 0).then_some(port)
}

/// Angemeldet bei diesem Office, auf irgendeinem Port dieses Hosts (Cookies kennen keine Ports).
fn angemeldet(o: &OfficeRef, headers: &axum::http::HeaderMap) -> bool {
    if o.zugang.offen() {
        return true;
    }
    let Some(c) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    c.split(';')
        .filter_map(|t| t.trim().split_once('='))
        .any(|(k, v)| {
            (k == COOKIE || k.strip_prefix(COOKIE).is_some_and(|r| r.starts_with('_')))
                && o.zugang
                    .sitzungen
                    .pruefen(v)
                    .is_some_and(|s| o.zugang.gueltig(&s))
        })
}

/// Der Cookie-Header ohne die Sitzungs-Cookies des Office (gehen den Dienst nichts an).
fn ohne_office_cookies(c: &str) -> Option<String> {
    let rest: Vec<&str> = c
        .split(';')
        .map(str::trim)
        .filter(|t| {
            !t.split('=')
                .next()
                .is_some_and(|k| k == COOKIE || k.starts_with(&format!("{COOKIE}_")))
        })
        .collect();
    (!rest.is_empty()).then(|| rest.join("; "))
}

/// Middleware ganz außen: Anfragen über einen Tunnel gehen an den Dienst, alles andere ans Office.
pub async fn weiterleiten(State(o): State<OfficeRef>, mut req: Request, next: Next) -> Response {
    let Some(port) = tunnel_port(&o, req.headers()) else {
        return next.run(req).await;
    };
    if req.uri().path() == ANMELDE_PFAD {
        *req.uri_mut() = Uri::from_static("/api/anmeldung");
        return next.run(req).await;
    }
    if !angemeldet(&o, req.headers()) {
        return anmeldeseite(port, &o);
    }
    if !o.dienst_ports().contains(&port) {
        return seite(
            StatusCode::SERVICE_UNAVAILABLE,
            "💤 Läuft nicht",
            &format!(
                "<p>Auf Port {port} antwortet gerade nichts. Vielleicht hat der Arbeiter seinen Server gestoppt – schau aufs 🌐 Dienste-Board im Office oder bitte ihn, ihn neu zu starten.</p>"
            ),
            "",
        );
    }
    weiterreichen(req, port).await
}

async fn weiterreichen(mut req: Request, port: u16) -> Response {
    let lokal = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .and_then(host_port)
        .is_some_and(|(h, _)| ist_lokal(h));
    let pfad = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "/".into());
    let Ok(ziel) = format!("http://127.0.0.1:{port}{pfad}").parse::<Uri>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let eingehend = hyper::upgrade::on(&mut req);
    let (mut teile, body) = req.into_parts();
    teile.uri = ziel;
    teile
        .headers
        .insert(WEITERGEREICHT, HeaderValue::from_static("1"));
    // Aus dem Tailnet bekommt der Dienst den Host, den er über einen Tunnel sähe (Vite & Co. lehnen fremde Namen ab).
    if !lokal && let Ok(h) = HeaderValue::from_str(&format!("localhost:{port}")) {
        teile.headers.insert(header::HOST, h);
    }
    match teile
        .headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(ohne_office_cookies)
    {
        Some(Some(c)) => {
            if let Ok(v) = HeaderValue::from_str(&c) {
                teile.headers.insert(header::COOKIE, v);
            }
        }
        Some(None) => {
            teile.headers.remove(header::COOKIE);
        }
        None => {}
    }
    let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build_http();
    let mut antwort = match client.request(Request::from_parts(teile, body)).await {
        Ok(a) => a,
        Err(_) => {
            return seite(
                StatusCode::BAD_GATEWAY,
                "Keine Antwort",
                &format!(
                    "<p>Der Server auf Port {port} antwortete nicht. Vielleicht startet er gerade neu – gleich noch einmal versuchen.</p>"
                ),
                "",
            );
        }
    };
    if antwort.status() == StatusCode::SWITCHING_PROTOCOLS {
        // WebSockets (Hot Reload und Co.): beide Seiten upgraden und Bytes durchreichen.
        let ausgehend = hyper::upgrade::on(&mut antwort);
        tokio::spawn(async move {
            if let (Ok(a), Ok(b)) = (eingehend.await, ausgehend.await) {
                let (mut a, mut b) = (TokioIo::new(a), TokioIo::new(b));
                let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
            }
        });
    }
    antwort.map(Body::new)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STIL: &str = "body{margin:0;min-height:100vh;display:grid;place-items:center;background:#bfe3ff;font:16px/1.5 system-ui,sans-serif;color:#2b2d42}\
main{background:#fffaf3;border:3px solid #2b2d42;border-radius:18px;box-shadow:0 6px 0 #2b2d42;padding:28px 32px;max-width:440px;margin:16px}\
h1{margin:0 0 8px;font-size:22px}form{display:flex;flex-wrap:wrap;gap:8px}input{flex:1;min-width:0;font:inherit;padding:8px 12px;border:2px solid #2b2d42;border-radius:10px}\
button{font:inherit;font-weight:800;padding:8px 16px;border:2px solid #2b2d42;border-radius:10px;background:#ffd166;cursor:pointer}.f{color:#c1121f;font-weight:700;min-height:1.5em}";

fn seite(status: StatusCode, titel: &str, inhalt: &str, skript: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"de\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{} · OpenAgenticOffice</title><style>{STIL}</style></head><body><main><h1>{}</h1>{inhalt}</main>{}</body></html>",
        esc(titel),
        esc(titel),
        if skript.is_empty() {
            String::new()
        } else {
            format!("<script>{skript}</script>")
        }
    );
    let mut r = (status, Html(html)).into_response();
    let h = r.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    r
}

fn anmeldeseite(port: u16, o: &OfficeRef) -> Response {
    let (konten, gemeinsam) = {
        let k = o.zugang.konten.lock().unwrap();
        (k.gibt_es_welche(), k.gemeinsames_passwort())
    };
    let name = konten || !gemeinsam;
    let feld = if name {
        format!(
            "<input id=\"n\" placeholder=\"{}\" autocomplete=\"username\">",
            if gemeinsam {
                "Dein Name (freiwillig)"
            } else {
                "Dein Name"
            }
        )
    } else {
        String::new()
    };
    let inhalt = format!(
        "<p>Das ist der Server eines Arbeiters auf Port {port}, erreicht über das Office. Melde dich an, um ihn zu sehen.</p><form id=\"f\">{feld}<input id=\"p\" type=\"password\" placeholder=\"Passwort\" autocomplete=\"current-password\"><button>Anmelden</button></form><p class=\"f\" id=\"e\"></p>"
    );
    let skript = format!(
        "document.getElementById('f').addEventListener('submit',async(e)=>{{e.preventDefault();const f=document.getElementById('e');f.textContent='';const n=document.getElementById('n');\
try{{const r=await fetch('{ANMELDE_PFAD}',{{method:'POST',headers:{{'content-type':'application/json'}},body:JSON.stringify({{name:n?n.value:'',passwort:document.getElementById('p').value}})}});\
if(r.ok)location.reload();else f.textContent=(await r.json().catch(()=>({{}}))).fehler||'Anmeldung fehlgeschlagen'}}catch{{f.textContent='Das Office ist nicht erreichbar'}}}})"
    );
    seite(
        StatusCode::UNAUTHORIZED,
        "🔒 Beim Office anmelden",
        &inhalt,
        &skript,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_und_hosts() {
        assert_eq!(
            ohne_office_cookies("a=1; oao_sitzung_4600=x; b=2").as_deref(),
            Some("a=1; b=2")
        );
        assert_eq!(ohne_office_cookies("oao_sitzung=x"), None);
        assert!(ist_lokal("app.localhost"));
        assert!(!ist_lokal("example.com"));
        assert_eq!(host_port("localhost:5173"), Some(("localhost", 5173)));
    }
}
