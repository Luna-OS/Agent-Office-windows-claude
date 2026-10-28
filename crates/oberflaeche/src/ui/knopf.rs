//! Button aus rust-ui (`ui/button.rs`).

use leptos::prelude::*;
use tw_merge::tw_merge;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum KnopfArt {
    #[default]
    Standard,
    Gefahr,
    Warnung,
    Erfolg,
    Umriss,
    Zweitrangig,
    Geist,
    Link,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum KnopfGroesse {
    #[default]
    Standard,
    Klein,
    Gross,
    Symbol,
    SymbolKlein,
}

impl KnopfArt {
    pub const fn klassen(self) -> &'static str {
        match self {
            Self::Standard => "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90",
            Self::Gefahr => "bg-destructive text-white shadow-xs hover:bg-destructive/90",
            Self::Warnung => "bg-warning text-warning-foreground hover:bg-warning/90",
            Self::Erfolg => "bg-success text-success-foreground hover:bg-success/90",
            Self::Umriss => {
                "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground"
            }
            Self::Zweitrangig => {
                "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80"
            }
            Self::Geist => "hover:bg-accent hover:text-accent-foreground",
            Self::Link => "text-primary underline-offset-4 hover:underline",
        }
    }
}

impl KnopfGroesse {
    pub const fn klassen(self) -> &'static str {
        match self {
            Self::Standard => "h-9 px-4 py-2",
            Self::Klein => "h-8 rounded-md gap-1.5 px-3",
            Self::Gross => "h-10 rounded-md px-6",
            Self::Symbol => "size-9",
            Self::SymbolKlein => "size-8 rounded-md",
        }
    }
}

/// Ein Knopf. Ereignisse und Attribute hängt man direkt an: `<Knopf on:click=… attr:title="…">`.
#[component]
pub fn Knopf(
    #[prop(optional)] art: KnopfArt,
    #[prop(optional)] groesse: KnopfGroesse,
    #[prop(into, optional)] class: String,
    #[prop(into, optional)] deaktiviert: Signal<bool>,
    #[prop(optional)] absenden: bool,
    children: Children,
) -> impl IntoView {
    let klassen = tw_merge!(
        "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-semibold transition-all disabled:pointer-events-none disabled:opacity-50 shrink-0 outline-none focus-visible:ring-ring/50 focus-visible:ring-[3px] hover:cursor-pointer active:scale-[0.98] select-none",
        art.klassen(),
        groesse.klassen(),
        class
    );
    view! {
        <button type=if absenden { "submit" } else { "button" } class=klassen disabled=deaktiviert>
            {children()}
        </button>
    }
}
