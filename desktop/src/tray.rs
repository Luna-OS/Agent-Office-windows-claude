//! Symbol im Infobereich (Windows) bzw. in der Menüleiste (macOS): Anzeigen, Browser, Beenden.

use crate::fenster::{Ereignis, Wo};
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const SYMBOL: &[u8] = include_bytes!("../ui/tray.png");

fn symbol() -> Option<Icon> {
    let bild = image::load_from_memory(SYMBOL).ok()?.into_rgba8();
    let (b, h) = bild.dimensions();
    Icon::from_rgba(bild.into_raw(), b, h).ok()
}

pub fn erstellen(proxy: EventLoopProxy<Ereignis>) -> Option<TrayIcon> {
    let anzeigen = MenuItem::new("Connector anzeigen", true, None);
    let browser = MenuItem::new("Office auf diesem Computer im Browser öffnen", true, None);
    let beenden = MenuItem::new("Beenden", true, None);
    let menue = Menu::new();
    menue.append_items(&[&anzeigen, &browser, &beenden]).ok()?;
    let (id_anzeigen, id_browser, id_beenden) = (anzeigen.id().clone(), browser.id().clone(), beenden.id().clone());

    let p = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let ereignis = if e.id == id_anzeigen {
            Ereignis::Anzeigen
        } else if e.id == id_browser {
            Ereignis::LokalStart(Wo::Browser)
        } else if e.id == id_beenden {
            Ereignis::Beenden
        } else {
            return;
        };
        let _ = p.send_event(ereignis);
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            let _ = proxy.send_event(Ereignis::Anzeigen);
        }
    }));

    let mut bauer = TrayIconBuilder::new().with_menu(Box::new(menue)).with_tooltip("OpenAgenticOffice");
    if let Some(s) = symbol() {
        bauer = bauer.with_icon(s);
    }
    bauer.build().ok()
}
