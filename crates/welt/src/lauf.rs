//! Die Render-Schleife: Fenster öffnen, Features installieren, dann jeden Frame Eingang abarbeiten,
//! Tasten verteilen, Ticks laufen lassen, Beschriftungen zeichnen, rendern.

use kiss3d::prelude::*;

use crate::api::{self, EINGANG};
use crate::bau::himmel;
use crate::features;
use crate::register::Register;
use crate::welt::Kontext;
use crate::{eingabe, grafik};

/// Startet die Welt im Canvas `CANVAS_ID`. Kehrt nicht zurück, solange die Seite offen ist.
/// Im Browser: `wasm_bindgen_futures::spawn_local(welt::starten())`.
pub async fn starten() {
    let webgpu = grafik::vorbereiten().await;
    if !webgpu {
        grafik::kein_webgpu_melden();
        return;
    }
    let einrichtung = grafik::einrichtung(api::CANVAS_ID, webgpu);
    let mut fenster = Window::new_with_setup("OpenAgenticOffice", 1280, 720, einrichtung).await;
    fenster.set_background_color(himmel::himmelsfarbe(1.0));
    fenster.set_ambient(0.55);
    eingabe::installieren(api::CANVAS_ID);

    let mut szene = SceneNode3d::empty();
    let mut sonne = szene.add_light(Light::directional(Vec3::new(-0.4, -1.0, -0.5)));
    sonne.modify_light(|l| {
        l.intensity = 2.2;
        l.casts_shadows = false;
    });

    let mut kx = Kontext::neu(szene.clone());
    let mut reg: Register<Kontext> = Register::neu();
    features::installieren(&mut reg, &mut kx);
    kx.etage_bauen();

    let mut letzte = web_time::Instant::now();
    let mut stunde_alt = -1.0f32;
    loop {
        let jetzt = web_time::Instant::now();
        let dt = (jetzt - letzte).as_secs_f32().min(0.1);
        letzte = jetzt;
        kx.zeit += dt as f64;
        kx.css = grafik::css_groesse(&fenster);

        // Was von außen kam.
        let (nachrichten, bilder, zur_etage, perspektive) =
            EINGANG.with_borrow_mut(|e| (std::mem::take(&mut e.nachrichten), std::mem::take(&mut e.bilder), e.zur_etage.take(), e.perspektive.take()));
        if let Some(ich) = perspektive {
            kx.kamera.abstand = if ich { 0.0 } else { 3.0 };
        }
        kx.aenderungen = Default::default();
        for n in &nachrichten {
            let a = kx.zustand.anwenden(n);
            zusammenfuehren(&mut kx.aenderungen, a);
        }
        kx.bilder = bilder;
        if let Some(etage) = zur_etage {
            features::aufzug::fahren(&mut kx, &etage);
        }
        kx.neu_gebaut = false;
        if kx.aenderungen.etage || kx.aenderungen.grundriss {
            reg.alle_stoppen(&mut kx);
            kx.etage_bauen();
            if let Some(o) = kx.zustand.ankunft.take() {
                kx.spieler.versetzen(o.x, o.y, o.z, o.rot_y);
                if kx.zustand.auf_dem_dach() {
                    crate::features::aufzug::dach_ankunft(&mut kx);
                }
            }
        }

        // Tasten: erst die Aktivität und die Office-Tasten, E an dem, worauf man zielt.
        kx.eingabe = eingabe::abholen();
        let tasten = std::mem::take(&mut kx.eingabe.tasten);
        for (code, gedrueckt) in &tasten {
            let genommen = reg.taste_gedrueckt(&mut kx, code, *gedrueckt);
            if !genommen
                && *gedrueckt
                && code == "KeyE"
                && let Some(z) = kx.ziel.clone()
            {
                reg.nutzen(&mut kx, &z);
            }
            weiterreichen(&mut kx, &mut reg);
        }
        if kx.eingabe.klicks > 0
            && reg.laufen_frei()
            && let Some(z) = kx.ziel.clone()
        {
            reg.nutzen(&mut kx, &z);
            weiterreichen(&mut kx, &mut reg);
        }
        kx.eingabe.tasten = tasten;

        kx.beschriftungen.clear();
        kx.bewegliche.clear();
        kx.dinge.clear();
        kx.aktiv = reg.aktivitaet();
        kx.spieler_fest = !reg.laufen_frei();
        reg.frame(&mut kx, dt);
        weiterreichen(&mut kx, &mut reg);
        api::hinweis_aendern(reg.hinweis(&kx, kx.ziel.as_ref()));

        // Sonne und Himmel nach der Uhrzeit (einmal pro Minute reicht).
        let stunde = grafik::stunde();
        if (stunde - stunde_alt).abs() > 1.0 / 60.0 {
            stunde_alt = stunde;
            let s = himmel::sonnenstand(stunde);
            fenster.set_background_color(himmel::himmelsfarbe(s.tag));
            fenster.set_ambient(0.35 + 0.25 * s.tag);
            sonne.modify_light(|l| {
                l.light_type = kiss3d::light::LightType::Directional(-Vec3::from(s.richtung));
                l.intensity = 0.4 + 1.8 * s.tag;
            });
        }

        zeichnen_beschriftungen(&mut fenster, &kx);
        if !fenster.render_3d(&mut szene, &mut kx.kamera).await {
            break;
        }
    }
}

/// Was ein Feature im Kontext angestoßen hat, ins Register bringen.
fn weiterreichen(kx: &mut Kontext, reg: &mut Register<Kontext>) {
    if std::mem::take(&mut kx.alle_stoppen) {
        reg.alle_stoppen(kx);
    }
    if let Some(a) = kx.beginnen.take() {
        reg.beginnen(a);
    }
}

fn zusammenfuehren(z: &mut crate::zustand::Aenderungen, a: crate::zustand::Aenderungen) {
    z.etage |= a.etage;
    z.grundriss |= a.grundriss;
    z.personen |= a.personen;
    z.arbeiter |= a.arbeiter;
    z.spielzeug |= a.spielzeug;
    z.leinwand |= a.leinwand;
    z.emotes.extend(a.emotes);
    z.hupen.extend(a.hupen);
    z.golf.extend(a.golf);
    if a.sitz_belegt.is_some() {
        z.sitz_belegt = a.sitz_belegt;
    }
}

fn zeichnen_beschriftungen(fenster: &mut Window, kx: &Kontext) {
    let font = Font::default();
    let physisch = Vec2::new(fenster.width() as f32, fenster.height() as f32);
    let faktor = physisch.x / kx.css.x.max(1.0);
    for b in &kx.beschriftungen {
        let (p, vorn) = kx.kamera.projizieren(b.pos, kx.css);
        if !vorn {
            continue;
        }
        let abstand = (kx.kamera.auge() - Vec3::from(b.pos)).length();
        if abstand > 25.0 {
            continue;
        }
        let groesse = (28.0 - abstand * 0.6).clamp(14.0, 28.0) * faktor;
        let breite = b.text.chars().count() as f32 * groesse * 0.27;
        fenster.draw_text(&b.text, Vec2::new(p[0] * faktor - breite, p[1] * faktor - groesse), groesse, &font, b.farbe);
    }
}
