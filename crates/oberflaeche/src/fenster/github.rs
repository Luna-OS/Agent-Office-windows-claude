//! 🐙 GitHub: Issues und Pull Requests der Etage.

use leptos::prelude::*;
use protokoll::github;

use crate::modal::{Fenster, Rahmen};

pub fn empfangen(_m: &github::Server) {}

#[component]
pub fn Github() -> impl IntoView {
    view! {
        <Rahmen fenster=Fenster::Github titel="🐙 GitHub" breite="max-w-4xl">
            <p class="text-muted-foreground">"Wird geladen …"</p>
        </Rahmen>
    }
}
