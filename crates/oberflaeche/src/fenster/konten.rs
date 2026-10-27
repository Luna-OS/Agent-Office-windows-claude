//! 🔑 Anmeldungen: das eigene Claude und GitHub für die eigenen Arbeiter.

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};

pub fn link_erhalten(_pfad: &str) {}

#[component]
pub fn Konten() -> impl IntoView {
    view! {
        <Rahmen fenster=Fenster::Konten titel="🔑 Anmeldungen" breite="max-w-2xl">
            <p class="text-muted-foreground">"Wird geladen …"</p>
        </Rahmen>
    }
}
