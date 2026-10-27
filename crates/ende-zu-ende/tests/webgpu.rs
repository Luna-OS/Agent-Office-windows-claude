//! Kann das kopflose Chromium WebGPU? (Voraussetzung für die 3D-Tests.)

#[tokio::test(flavor = "multi_thread")]
async fn webgpu_adapter() -> anyhow::Result<()> {
    if ende_zu_ende::browser::chromium_finden().is_none() {
        return Ok(());
    }
    let t = ende_zu_ende::TestOffice::starten(None).await?;
    let b = ende_zu_ende::Browser::starten().await?;
    let s = b.seite(&t.url("/api/gesundheit")).await?;
    let v = s.js("(async () => { if (!navigator.gpu) return 'kein navigator.gpu'; const a = await navigator.gpu.requestAdapter(); if (!a) return 'kein Adapter'; const i = a.info || {}; return 'Adapter: ' + [i.vendor, i.architecture, i.description].join(' / '); })()").await?;
    eprintln!("WebGPU: {v}");
    assert!(v.as_str().unwrap_or("").starts_with("Adapter"), "WebGPU fehlt im Test-Chromium: {v}");
    Ok(())
}
