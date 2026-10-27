//! 👥 Team: Konten, Rollen und Einladungslinks (für Admins).

use leptos::prelude::*;
use protokoll::konten;

use crate::modal::{Fenster, Rahmen};

pub fn eingeladen(_einladung: Option<konten::Einladung>, fehler: Option<String>) {
    if let Some(f) = fehler {
        crate::zustand::fehler(f);
    }
}

#[component]
pub fn Team() -> impl IntoView {
    view! {
        <Rahmen fenster=Fenster::Team titel="👥 Team" breite="max-w-3xl">
            <p class="text-muted-foreground">"Wird geladen …"</p>
        </Rahmen>
    }
}
