//! Badge aus rust-ui (`ui/badge.rs`).

use leptos::prelude::*;
use tw_merge::tw_merge;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum AbzeichenArt {
    #[default]
    Standard,
    Zweitrangig,
    Gefahr,
    Erfolg,
    Warnung,
    Umriss,
}

impl AbzeichenArt {
    pub const fn klassen(self) -> &'static str {
        match self {
            Self::Standard => "border-transparent bg-primary text-primary-foreground",
            Self::Zweitrangig => "border-transparent bg-secondary text-secondary-foreground",
            Self::Gefahr => "border-transparent bg-destructive text-white",
            Self::Erfolg => "border-transparent bg-success text-success-foreground",
            Self::Warnung => "border-transparent bg-warning text-warning-foreground",
            Self::Umriss => "text-foreground",
        }
    }
}

#[component]
pub fn Abzeichen(#[prop(optional)] art: AbzeichenArt, #[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let k = tw_merge!(
        "inline-flex items-center justify-center rounded-md border px-2 py-0.5 text-xs font-semibold w-fit whitespace-nowrap shrink-0 gap-1",
        art.klassen(),
        class
    );
    view! { <span class=k>{children()}</span> }
}
