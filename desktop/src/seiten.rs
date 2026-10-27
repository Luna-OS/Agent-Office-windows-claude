//! Die Seite des Connectors: reines HTML ohne Skripte. Eingaben gehen als Formular (GET) an
//! `/aktion/<name>`, das Fenster fängt sie ab (siehe `fenster.rs`).

#[derive(Default, Clone)]
pub struct LokalAnsicht {
    pub url: String,
    pub bereit: bool,
    pub fuer_andere: bool,
    pub passwort: Option<String>,
    pub adressen: Vec<String>,
}

#[derive(Default, Clone)]
pub struct Ansicht {
    pub meldung: String,
    pub lobbys: Vec<String>,
    pub lokal: Option<LokalAnsicht>,
    pub durchleitung: Option<String>,
}

pub fn escape(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const STIL: &str = "body{font:15px system-ui,sans-serif;background:#1c1b22;color:#eee;margin:0;padding:24px}\
main{max-width:560px;margin:auto}h1{font-size:22px}h2{font-size:16px;margin:22px 0 8px;color:#e8c547}\
.karte{background:#2a2933;border-radius:10px;padding:12px 14px;margin:8px 0}\
input[type=text]{width:100%;box-sizing:border-box;padding:8px;border-radius:6px;border:1px solid #555;background:#15141a;color:#eee}\
button{padding:7px 12px;border:0;border-radius:6px;background:#e8c547;color:#222;font-weight:600;cursor:pointer;margin:6px 6px 0 0}\
button.leise{background:#44434f;color:#eee}form{margin:0;display:inline}.melde{background:#3a3220;border-left:4px solid #e8c547}\
small{color:#aaa}code{background:#15141a;padding:2px 5px;border-radius:4px}";

fn knopf(aktion: &str, text: &str, felder: &[(&str, &str)], leise: bool) -> String {
    let versteckt: String =
        felder.iter().map(|(n, w)| format!("<input type=hidden name=\"{n}\" value=\"{}\">", escape(w))).collect();
    let klasse = if leise { " class=leise" } else { "" };
    format!("<form action=\"/aktion/{aktion}\">{versteckt}<button{klasse}>{text}</button></form>")
}

fn lobby_karten(lobbys: &[String]) -> String {
    if lobbys.is_empty() {
        return "<p><small>Noch keine Lobby gemerkt.</small></p>".into();
    }
    lobbys
        .iter()
        .map(|l| {
            format!(
                "<div class=karte><b>{}</b><br>{}{}{}</div>",
                escape(l),
                knopf("beitreten", "🌐 Im Browser öffnen", &[("adresse", l), ("wo", "browser")], false),
                knopf("beitreten", "🪟 Im Fenster öffnen", &[("adresse", l), ("wo", "fenster")], true),
                knopf("vergessen", "Vergessen", &[("adresse", l)], true),
            )
        })
        .collect()
}

fn lokal_bereich(lokal: &Option<LokalAnsicht>) -> String {
    let Some(l) = lokal else {
        return format!(
            "<div class=karte>Das Office läuft dann auf diesem Computer, mit eurem Claude-Abo und eurer GitHub CLI.<br>{}{}</div>",
            knopf("lokal_start", "🖥️ Auf diesem Computer starten (Browser)", &[("wo", "browser")], false),
            knopf("lokal_start", "🪟 Im Fenster starten", &[("wo", "fenster")], true),
        );
    };
    let status = if l.bereit { "läuft" } else { "startet …" };
    let mut s = format!("<div class=karte>Das Office auf diesem Computer <b>{status}</b> unter <code>{}</code>.<br>", escape(&l.url));
    if l.bereit {
        s += &knopf("lokal_oeffnen", "🌐 Im Browser öffnen", &[("wo", "browser")], false);
        s += &knopf("lokal_oeffnen", "🪟 Im Fenster öffnen", &[("wo", "fenster")], true);
    }
    if l.fuer_andere {
        s += &knopf("fuer_andere", "🔒 Lobby wieder schließen", &[("an", "nein")], true);
    } else {
        s += &knopf("fuer_andere", "👥 Lobby für andere öffnen", &[("an", "ja")], true);
    }
    s += &knopf("lokal_stop", "⏹️ Beenden", &[], true);
    if l.fuer_andere {
        let adressen: Vec<String> = l.adressen.iter().map(|a| format!("<code>{}</code>", escape(a))).collect();
        s += &format!(
            "<p>Einladung für Mitspieler: Adresse {} und Passwort <code>{}</code>.<br><small>Besser: im Office unter ☰ → 🔑 Konten einen Einladungslink (/beitreten#…) erzeugen.</small></p>",
            adressen.join(" oder "),
            escape(l.passwort.as_deref().unwrap_or("")),
        );
    }
    s + "</div>"
}

pub fn startseite(a: &Ansicht) -> String {
    let meldung = if a.meldung.is_empty() { String::new() } else { format!("<div class=\"karte melde\">{}</div>", escape(&a.meldung)) };
    let durch = a.durchleitung.as_ref().map_or(String::new(), |z| {
        format!("<p><small>Durchleitung aktiv: <code>http://127.0.0.1:48600</code> → <code>{}</code>. Lass den Connector laufen, damit Sprache und Bildschirm teilen im Browser gehen.</small></p>", escape(z))
    });
    format!(
        "<!doctype html><html lang=de><meta charset=utf-8><title>OpenAgenticOffice – Connector</title><style>{STIL}</style><body><main>\
<h1>🏢 OpenAgenticOffice</h1>{meldung}\
<h2>🌐 Lobby beitreten</h2>\
<form action=\"/aktion/beitreten\" style=\"display:block\"><input type=text name=adresse placeholder=\"100.78.107.162:4600 oder Einladungslink (…/beitreten#…)\" autofocus>\
<button name=wo value=browser>Verbinden (Browser)</button><button class=leise name=wo value=fenster>Verbinden (Fenster)</button></form>\
{lobbys}\
<h2>🖥️ Ohne Server</h2>{lokal}{durch}\
<p><small>Anmelden mit deinem eigenen Claude und GitHub tust du im Office selbst (☰ → 🔐 Deine Anmeldungen).</small></p>\
</main></body></html>",
        lobbys = lobby_karten(&a.lobbys),
        lokal = lokal_bereich(&a.lokal),
    )
}

pub fn warteseite() -> String {
    "<!doctype html><meta charset=utf-8><meta http-equiv=refresh content=\"1;url=/\"><body style=\"background:#1c1b22;color:#eee;font:15px system-ui;padding:24px\">Einen Moment …".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_schuetzt_html() {
        assert_eq!(escape("<a href=\"x\">&"), "&lt;a href=&quot;x&quot;&gt;&amp;");
    }

    #[test]
    fn startseite_zeigt_lobbys_und_meldung() {
        let a = Ansicht { meldung: "Hallo".into(), lobbys: vec!["http://x:4600".into()], ..Default::default() };
        let h = startseite(&a);
        assert!(h.contains("Hallo") && h.contains("http://x:4600") && h.contains("Lobby beitreten"));
    }
}
