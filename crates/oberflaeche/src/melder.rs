//! Wer wartet, soll es merken, auch im Hintergrund: der Tab-Titel zählt die Arbeiter, die auf jemanden
//! warten (auf allen Etagen), und ist der Tab nicht zu sehen, kommt eine Desktop-Benachrichtigung.

use std::collections::HashSet;

use leptos::prelude::*;
use protokoll::arbeiter::ArbeiterStatus;

use crate::modal::{self, Fenster};
use crate::zustand::zustand;

const TITEL: &str = "OpenAgenticOffice";

fn benachrichtigen(titel: &str, text: &str, arbeiter: String) {
    let Ok(n) = web_sys::Notification::new_with_options(titel, &{
        let o = web_sys::NotificationOptions::new();
        o.set_body(text);
        o.set_tag(&arbeiter);
        o
    }) else {
        return;
    };
    let klick = wasm_bindgen::prelude::Closure::<dyn Fn()>::new(move || {
        let _ = window().focus();
        modal::oeffnen(Fenster::Terminal(arbeiter.clone()));
    });
    n.set_onclick(Some(wasm_bindgen::JsCast::unchecked_ref(klick.as_ref())));
    klick.forget();
}

/// Beim Start einmal aufrufen.
pub fn starten() {
    let z = zustand();
    // Der Titel: wartende hier und auf den anderen Etagen.
    Effect::new(move |_| {
        let hier = z.arbeiter.with(|l| {
            l.iter()
                .filter(|a| a.status == ArbeiterStatus::Wartet)
                .count() as u32
        });
        let etage = z.etage.get();
        let anderswo: u32 = z.etagen.with(|l| {
            l.iter()
                .filter(|e| Some(&e.id) != etage.as_ref())
                .map(|e| e.wartend)
                .sum()
        });
        let n = hier + anderswo;
        document().set_title(&if n > 0 {
            format!("({n}) {TITEL}")
        } else {
            TITEL.to_string()
        });
    });
    // Benachrichtigungen: nur für den Übergang zu "wartet", nur wenn der Tab versteckt ist und man es erlaubt hat.
    let schon = StoredValue::new(HashSet::<String>::new());
    Effect::new(move |_| {
        let wartend: Vec<_> = z.arbeiter.with(|l| {
            l.iter()
                .filter(|a| a.status == ArbeiterStatus::Wartet)
                .map(|a| (a.id.clone(), a.name.clone(), a.hinweis.clone()))
                .collect()
        });
        let versteckt = document().hidden();
        let erlaubt =
            web_sys::Notification::permission() == web_sys::NotificationPermission::Granted;
        let neu: Vec<_> = wartend
            .iter()
            .filter(|(id, ..)| !schon.with_value(|s| s.contains(id)))
            .cloned()
            .collect();
        schon.set_value(wartend.iter().map(|(id, ..)| id.clone()).collect());
        if versteckt && erlaubt {
            for (id, name, hinweis) in neu {
                benachrichtigen(
                    &format!("{name} braucht dich"),
                    &hinweis.unwrap_or_else(|| "Eine Rückfrage oder Erlaubnis ist offen.".into()),
                    id,
                );
            }
        }
    });
}

/// Fragt nach der Erlaubnis für Benachrichtigungen (aus einem Klick heraus).
pub fn erlauben() {
    if let Ok(p) = web_sys::Notification::request_permission() {
        leptos::task::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(p).await;
            crate::zustand::info(
                if web_sys::Notification::permission() == web_sys::NotificationPermission::Granted {
                    "Benachrichtigungen sind an."
                } else {
                    "Benachrichtigungen sind aus (im Browser erlauben)."
                },
            );
        });
    }
}
