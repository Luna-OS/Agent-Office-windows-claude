//! Die Fensterregel im echten Browser, für jedes Fenster, das sich ohne die Welt öffnen lässt:
//! oben rechts ein ✕; Esc schließt es; ✕ schließt es; und ist danach keins mehr offen, will die Welt
//! die Maus sofort wieder (ohne Extra-Klick). Fenster, die nur aus der Welt aufgehen (Anlegen, Schild,
//! Heimschicken …), prüft `oberflaeche::fenster::regel` am Code: Sie liegen alle im selben Rahmen.

use std::time::Duration;

use ende_zu_ende::office::client;
use ende_zu_ende::{Browser, Seite, TestOffice};
use serde_json::json;

const LANG: Duration = Duration::from_secs(20);

fn bereit() -> bool {
    if ende_zu_ende::browser::chromium_finden().is_none() {
        eprintln!("übersprungen: kein Chromium (OFFICE_CHROMIUM setzen)");
        return false;
    }
    if client().is_none() {
        eprintln!("übersprungen: kein gebauter Client in dist/ (trunk build in crates/client)");
        return false;
    }
    true
}

/// Wie viele Fenster offen sind.
async fn offen(s: &Seite) -> anyhow::Result<u64> {
    Ok(s.js("document.querySelectorAll('[role=dialog]').length")
        .await?
        .as_u64()
        .unwrap_or(0))
}

/// Klickt (echte Geste) den sichtbaren Knopf, dessen Text `text` enthält: im obersten Fenster, sonst irgendwo.
async fn knopf(s: &Seite, text: &str) -> anyhow::Result<()> {
    let ort = s
        .js(&format!(
            "(() => {{ const d = [...document.querySelectorAll('[role=dialog]')].pop() || document;
                const b = [...d.querySelectorAll('button,a')].filter(e => e.offsetParent !== null && e.innerText.includes({t})).shift();
                if (!b) return null; b.scrollIntoView({{block: 'center'}}); const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; }})()",
            t = json!(text)
        ))
        .await?;
    let Some(a) = ort.as_array() else {
        anyhow::bail!("Kein Knopf „{text}“ gefunden")
    };
    s.klick_bei(a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0))
        .await
}

/// Ein Schritt zum Öffnen: `taste:Tab`, `knopf:Text`, `frei` (Maus aus der Welt holen).
async fn schritt(s: &Seite, sch: &str) -> anyhow::Result<()> {
    match sch.split_once(':') {
        Some(("taste", t)) => s.taste(t).await,
        Some(("knopf", t)) => knopf(s, t).await,
        _ if sch == "frei" => s.js("document.exitPointerLock()").await.map(|_| ()),
        _ => anyhow::bail!("Unbekannter Schritt {sch}"),
    }
}

async fn alle_zu(s: &Seite) -> anyhow::Result<()> {
    for _ in 0..6 {
        if offen(s).await? == 0 {
            return Ok(());
        }
        s.taste("Escape").await?;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    anyhow::bail!("Fenster ließen sich nicht schließen")
}

async fn oeffnen(s: &Seite, schritte: &[&str]) -> anyhow::Result<u64> {
    for sch in schritte {
        schritt(s, sch).await?;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    s.warte_auf("document.querySelector('[role=dialog]')", LANG)
        .await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    offen(s).await
}

/// Schließt das oberste Fenster (`wie`: Esc oder ✕) und prüft, was danach gilt.
async fn schliessen(s: &Seite, name: &str, wie: &str, n: u64) -> anyhow::Result<Vec<String>> {
    let mut maengel = Vec::new();
    let vorher = s.js("window.__mausFangen").await?.as_u64().unwrap_or(0);
    if wie == "Esc" {
        s.taste("Escape").await?;
    } else {
        let ort = s
            .js("(() => { const x = [...document.querySelectorAll('[role=dialog]')].pop().querySelector('.schliessen');
                const r = x.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()")
            .await?;
        s.klick_bei(
            ort[0].as_f64().unwrap_or(0.0),
            ort[1].as_f64().unwrap_or(0.0),
        )
        .await?;
    }
    if s.warte_auf(
        &format!(
            "document.querySelectorAll('[role=dialog]').length === {}",
            n - 1
        ),
        Duration::from_secs(5),
    )
    .await
    .is_err()
    {
        maengel.push(format!("{name}: {wie} schließt das Fenster nicht"));
        return Ok(maengel);
    }
    if n == 1
        && s.warte_auf(
            &format!("window.__mausFangen > {vorher} && window.__mausZiel === 'welt'"),
            Duration::from_secs(5),
        )
        .await
        .is_err()
    {
        maengel.push(format!(
            "{name}: nach {wie} will die Welt die Maus nicht sofort wieder"
        ));
    }
    Ok(maengel)
}

#[tokio::test(flavor = "multi_thread")]
async fn jedes_fenster_hat_kreuz_und_esc() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let t = TestOffice::starten(Some("geheim123")).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url("/")).await?;
    s.warte_auf("document.querySelector('input[type=password]')", LANG)
        .await?;
    s.klick("input[type=password]").await?;
    s.tippe("geheim123").await?;
    s.klick("text=Anmelden").await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;

    // Ein Arbeiter (eine Shell), damit es ein Terminal zu öffnen gibt: über eine zweite Verbindung
    // aus der angemeldeten Seite (sie hat das Sitzungs-Cookie).
    s.js("(() => { const w = new WebSocket(location.origin.replace('http', 'ws') + '/ws?name=Helfer');
        w.onopen = () => w.send(JSON.stringify({ domaene: 'arbeiter', nachricht: { typ: 'anlegen', tisch: 'desk-1', art: 'shell' } }));
        window.__helfer = w; return 1; })()")
        .await?;
    s.warte_auf(
        "document.querySelector('[aria-label=Arbeiter] li button .font-bold')",
        LANG,
    )
    .await?;
    let name = s
        .js(r"document.querySelector('[aria-label=Arbeiter] li button .font-bold').innerText.replace(/^\S+\s+/, '').trim()")
        .await?
        .as_str()
        .unwrap_or_default()
        .to_string();
    eprintln!("Arbeiter: {name}");

    let menue = |e: &'static str| {
        vec![
            "taste:Tab",
            Box::leak(format!("knopf:{e}").into_boxed_str()),
        ]
    };
    let mut faelle: Vec<(String, Vec<&str>)> = vec![
        ("Menü".into(), vec!["taste:Tab"]),
        ("Befehlspalette".into(), vec!["taste:/"]),
        (
            "Terminal".into(),
            vec!["frei", Box::leak(format!("knopf:{name}").into_boxed_str())],
        ),
        (
            "Repo-Auswahl (über den Etagen)".into(),
            vec!["taste:Tab", "knopf:Etagen", "knopf:+ Projekt hinzufügen"],
        ),
        (
            "Pult-Agent (über dem Aufgabenboard)".into(),
            vec!["taste:Tab", "knopf:Aufgabenboard", "knopf:Agent fragen"],
        ),
    ];
    for e in [
        "Aufgabenboard",
        "Agenten",
        "GitHub",
        "Etagen",
        "Bücherregal",
        "Dienste",
        "Nutzung und Limits",
        "Meeting",
        "Bildschirm teilen",
        "Dach-Leinwand",
        "Whiteboard",
        "Arcade",
        "Minesweeper",
        "Jukebox",
        "Team",
        "Anmeldungen",
        "Prompts",
        "Einstellungen",
        "Steuerung",
    ] {
        faelle.push((e.to_string(), menue(e)));
    }

    let mut maengel = Vec::new();
    for (name, schritte) in &faelle {
        eprintln!("Fenster: {name}");
        alle_zu(&s).await?;
        let n = oeffnen(&s, schritte).await?;
        // Das ✕ sitzt oben rechts im obersten Fenster.
        let lage = s
            .js("(() => { const d = [...document.querySelectorAll('[role=dialog]')].pop(); const x = d && d.querySelector('.schliessen');
                if (!x) return null; const a = d.getBoundingClientRect(), b = x.getBoundingClientRect();
                return [a.right - b.right, b.top - a.top, x.innerText.trim()]; })()")
            .await?;
        match lage.as_array() {
            Some(l)
                if l[0].as_f64().unwrap_or(99.0) <= 32.0
                    && l[1].as_f64().unwrap_or(99.0) <= 40.0
                    && l[2] == "✕" => {}
            _ => maengel.push(format!("{name}: kein ✕ oben rechts ({lage})")),
        }
        maengel.extend(schliessen(&s, name, "Esc", n).await?);
        alle_zu(&s).await?;
        let n = oeffnen(&s, schritte).await?;
        maengel.extend(schliessen(&s, name, "✕", n).await?);
    }
    alle_zu(&s).await?;
    eprintln!("{} Fenster geprüft.", faelle.len());
    assert!(maengel.is_empty(), "Mängel:\n  {}", maengel.join("\n  "));
    Ok(())
}
