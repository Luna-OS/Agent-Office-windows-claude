//! Medien im echten Browser (lyra): die YouTube-Leinwand auf dem Dach (Overlay sichtbar und synchron),
//! Voice zwischen zwei Tabs (verbunden, Ton kommt an, M macht wirklich stumm) und die Jukebox.
//!
//! Der Client schreibt seinen Medien-Stand für Tests in die Seite: `<div class="oao-leinwand" data-…>`,
//! `<html data-voice="…" data-jukebox="…">` (siehe crates/client/src/medien_anbindung.rs).

use std::path::Path;
use std::time::{Duration, Instant};

use ende_zu_ende::office::{client, fotos};
use ende_zu_ende::{Browser, Seite, TestOffice};
use serde_json::{Value, json};

const LANG: Duration = Duration::from_secs(30);
/// Ein Video, das YouTube einbetten lässt (das Demovideo der IFrame-API).
const VIDEO: &str = "https://youtu.be/M7lc1UVf-VE?t=10";

fn bereit() -> bool {
    let ok = ende_zu_ende::browser::chromium_finden().is_some() && client().is_some();
    if !ok {
        eprintln!("übersprungen: kein Chromium oder kein gebauter Client in dist/");
    }
    ok
}

/// Öffnet einen Eintrag aus dem Menü (Tab).
async fn menue(s: &Seite, eintrag: &str) -> anyhow::Result<()> {
    s.taste("Tab").await?;
    s.warte_auf("document.querySelector('[role=dialog]')", LANG)
        .await?;
    s.klick(&format!("text={eintrag}")).await
}

/// Liest ein JSON aus einem `data-…`-Attribut von `<html>`.
async fn stand(s: &Seite, attribut: &str) -> anyhow::Result<Value> {
    let text = s
        .js(&format!(
            "document.documentElement.dataset.{attribut} || 'null'"
        ))
        .await?;
    Ok(serde_json::from_str(text.as_str().unwrap_or("null"))?)
}

/// Höchster Wert von `f` über `dauer`, alle 50 ms gelesen (Pegel schwanken von Messung zu Messung).
async fn hoechster(s: &Seite, dauer: Duration, f: impl Fn(&Value) -> f64) -> anyhow::Result<f64> {
    let start = Instant::now();
    let mut max: f64 = 0.0;
    while start.elapsed() < dauer {
        max = max.max(f(&stand(s, "voice").await?));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(max)
}

/// Hintergrund-Tabs liefern headless kein Mikrofon: beide Tabs als aktiv und im Fokus ausgeben.
async fn aktiv_halten(s: &Seite) -> anyhow::Result<()> {
    s.rufen(
        "Emulation.setFocusEmulationEnabled",
        json!({ "enabled": true }),
    )
    .await?;
    s.rufen("Page.setWebLifecycleState", json!({ "state": "active" }))
        .await?;
    Ok(())
}

/// Ein Sinuston als WAV (16 Bit, mono): der Fake-Mikrofon-Eingang für Voice.
fn ton_wav(datei: &Path) -> std::io::Result<()> {
    let rate = 48_000u32;
    let proben: Vec<i16> = (0..rate * 5)
        .map(|i| {
            (12_000.0 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / rate as f64).sin()) as i16
        })
        .collect();
    let daten = proben.len() as u32 * 2;
    let mut w = Vec::with_capacity(44 + daten as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + daten).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&daten.to_le_bytes());
    for p in proben {
        w.extend_from_slice(&p.to_le_bytes());
    }
    std::fs::write(datei, w)
}

/// Ist YouTube von hier erreichbar? (Ohne Netz wird der Leinwand-Test übersprungen.)
async fn youtube_erreichbar() -> bool {
    matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect("www.youtube.com:443")
        )
        .await,
        Ok(Ok(_))
    )
}

/// Wartet auf eine Bedingung an der Leinwand; schlägt sie fehl, mit Stand und Foto.
async fn leinwand_warten(s: &Seite, bedingung: &str, name: &str) -> anyhow::Result<()> {
    if let Err(e) = s.warte_auf(bedingung, Duration::from_secs(120)).await {
        let stand = s
            .js("(() => { const h = document.querySelector('.oao-leinwand'); return h ? JSON.stringify(Object.assign({sichtbar: getComputedStyle(h).visibility, transform: h.style.transform}, h.dataset)) : 'kein Overlay'; })()")
            .await?;
        s.foto(&fotos().join(format!("medien-leinwand-{name}-fehler.png")))
            .await?;
        anyhow::bail!(
            "{e}
Stand der Leinwand: {stand}"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn leinwand_auf_dem_dach() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    if !youtube_erreichbar().await {
        eprintln!("übersprungen: youtube.com nicht erreichbar");
        return Ok(());
    }
    let t = TestOffice::starten(None).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url("/?name=Leinwand")).await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;

    eprintln!("Schritt: aufs Dach (man kommt mit Blick auf die Leinwand an)");
    menue(&s, "Etagen").await?;
    s.klick("knopf=Dachterrasse").await?;
    s.warte_auf("document.querySelector('.oao-leinwand') || true", LANG)
        .await?;
    tokio::time::sleep(Duration::from_secs(3)).await;

    eprintln!("Schritt: Video auf die Leinwand");
    menue(&s, "Dach-Leinwand").await?;
    s.klick("[role=dialog] input").await?;
    s.tippe(VIDEO).await?;
    s.taste("Enter").await?;

    eprintln!("Schritt: das Overlay liegt sichtbar auf der Leinwand und das Video läuft");
    let laeuft = "(() => { const h = document.querySelector('.oao-leinwand'); if (!h) return false; const d = h.dataset;         return d.video === 'M7lc1UVf-VE' && d.bereit === 'ja' && d.laeuft === 'ja' && d.imBlick === 'ja'         && getComputedStyle(h).visibility === 'visible' && h.style.transform.startsWith('matrix')         && parseFloat(d.stelle) > 10.5; })()";
    leinwand_warten(&s, laeuft, "laeuft").await?;
    s.foto(&fotos().join("medien-leinwand.png")).await?;

    // Ein laufendes Video unter Last (Software-Rendering, parallele Builds) spielt langsamer als
    // Echtzeit; die Synchronisation prüfen wir deshalb an einer festen Stelle: angehalten.
    eprintln!("Schritt: Pause – der Player springt an die Stelle des Servers und hält dort an");
    menue(&s, "Dach-Leinwand").await?;
    s.klick("knopf=Pause").await?;
    let angehalten = "(() => { const d = document.querySelector('.oao-leinwand').dataset;         return d.laeuft === 'nein' && Math.abs(parseFloat(d.stelle) - parseFloat(d.soll)) < 1.5; })()";
    leinwand_warten(&s, angehalten, "angehalten").await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn voice_zwischen_zwei_tabs() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let ordner = tempfile::tempdir()?;
    let wav = ordner.path().join("ton.wav");
    ton_wav(&wav)?;
    let t = TestOffice::starten(None).await?;
    let b = Browser::starten_mit(&[format!(
        "--use-file-for-fake-audio-capture={}",
        wav.display()
    )])
    .await?;
    let a = b.seite(&t.url("/?name=Anna")).await?;
    let c = b.seite(&t.url("/?name=Bert")).await?;
    for s in [&a, &c] {
        s.warte_auf("document.getElementById('hud')", LANG).await?;
        aktiv_halten(s).await?;
    }

    eprintln!("Schritt: beide treten mit V dem Voice bei");
    for s in [&a, &c] {
        s.taste("v").await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let verbunden = "(() => { const v = JSON.parse(document.documentElement.dataset.voice || 'null'); \
        return v && v.aktiv && v.verbindungen.length === 1 && v.verbindungen[0][1] === 'connected' && v.verbindungen[0][2]; })()";
    for s in [&a, &c] {
        s.warte_auf(verbunden, LANG).await?;
    }

    eprintln!("Schritt: Bert hört Anna");
    let von_anna = |v: &Value| v["verbindungen"][0][3].as_f64().unwrap_or(0.0);
    let vorher = hoechster(&c, Duration::from_secs(5), von_anna).await?;
    anyhow::ensure!(vorher > 0.015, "Bert hört Anna nicht (Pegel {vorher:.3})");

    eprintln!("Schritt: Anna drückt M – bei Bert kommt nichts mehr an");
    a.taste("m").await?;
    a.warte_auf(
        "JSON.parse(document.documentElement.dataset.voice).stumm === true",
        LANG,
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let stumm = hoechster(&c, Duration::from_secs(3), von_anna).await?;
    anyhow::ensure!(
        stumm < 0.005,
        "Anna ist stumm, aber Bert hört sie noch (Pegel {stumm:.3})"
    );

    eprintln!("Schritt: noch einmal M – Anna ist wieder zu hören");
    a.taste("m").await?;
    a.warte_auf(
        "JSON.parse(document.documentElement.dataset.voice).stumm === false",
        LANG,
    )
    .await?;
    let wieder = hoechster(&c, Duration::from_secs(5), von_anna).await?;
    anyhow::ensure!(
        wieder > 0.015,
        "Nach dem Entstummen hört Bert Anna nicht (Pegel {wieder:.3})"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn jukebox_spielt() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let t = TestOffice::starten(None).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url("/?name=Jukebox")).await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;
    // Audio startet erst nach einer echten Taste (M ist außerhalb des Voice harmlos).
    s.taste("m").await?;

    eprintln!("Schritt: Kaffeepause auflegen – die Musik ist zu hören");
    menue(&s, "Jukebox").await?;
    s.klick("knopf=Kaffeepause").await?;
    let spielt = |id: &str| {
        format!(
            "(() => {{ const j = JSON.parse(document.documentElement.dataset.jukebox || 'null'); return j && j.spielt === '{id}' && j.pegel > 0; }})()"
        )
    };
    s.warte_auf(&spielt("coffee-break"), LANG).await?;

    eprintln!("Schritt: Nächstes, dann Aus");
    s.klick("knopf=⏭ Nächstes").await?;
    s.warte_auf(&spielt("late-commit"), LANG).await?;
    s.klick("knopf=⏹ Aus").await?;
    s.warte_auf(
        "JSON.parse(document.documentElement.dataset.jukebox).spielt === null",
        LANG,
    )
    .await?;
    let j = stand(&s, "jukebox").await?;
    anyhow::ensure!(j["spielt"].is_null(), "Jukebox läuft nach Aus noch: {j}");
    Ok(())
}
