//! Markdown als HTML (Bücherregal, GitHub-Texte). Eingebettetes HTML wird nicht ausgeführt, sondern als
//! Text gezeigt; Links öffnen in einem neuen Tab; relative Bilder gehen über eine Basis-Adresse.

use leptos::prelude::*;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};

/// Wandelt Markdown in sicheres HTML. `bild_basis` macht aus einem relativen Bildpfad eine Adresse.
pub fn zu_html(text: &str, bild_basis: Option<&dyn Fn(&str) -> String>) -> String {
    let mut optionen = Options::empty();
    optionen.insert(Options::ENABLE_TABLES);
    optionen.insert(Options::ENABLE_STRIKETHROUGH);
    optionen.insert(Options::ENABLE_TASKLISTS);
    let ereignisse = Parser::new_ext(text, optionen).map(|e| match e {
        // Kein fremdes HTML in unsere Seite: als Text zeigen.
        Event::Html(h) | Event::InlineHtml(h) => Event::Text(h),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let sicher = if dest_url
                .trim_start()
                .to_lowercase()
                .starts_with("javascript:")
            {
                CowStr::from("#")
            } else {
                dest_url
            };
            Event::Start(Tag::Link {
                link_type,
                dest_url: sicher,
                title,
                id,
            })
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let relativ = !dest_url.contains("://") && !dest_url.starts_with("data:");
            let ziel = match (relativ, bild_basis) {
                (true, Some(f)) => CowStr::from(f(&dest_url)),
                _ => dest_url,
            };
            Event::Start(Tag::Image {
                link_type,
                dest_url: ziel,
                title,
                id,
            })
        }
        andere => andere,
    });
    let mut aus = String::new();
    html::push_html(&mut aus, ereignisse);
    aus.replace(
        "<a href=",
        "<a target=\"_blank\" rel=\"noopener noreferrer\" href=",
    )
}

/// Macht aus einem relativen Bildpfad eine Adresse.
pub type BildBasis = Box<dyn Fn(&str) -> String>;

/// Ein Markdown-Text, gesetzt mit den Klassen in `.markdown` (crates/client/stil/thema.css).
#[component]
pub fn Markdown(
    #[prop(into)] text: String,
    #[prop(optional)] bild_basis: Option<BildBasis>,
) -> impl IntoView {
    let html = zu_html(&text, bild_basis.as_deref());
    view! { <div class="markdown" inner_html=html></div> }
}

#[cfg(test)]
mod tests {
    #[test]
    fn kein_fremdes_html() {
        let h = super::zu_html(
            "# Titel\n\n<script>alert(1)</script>\n\n[x](javascript:alert(1)) und <b>fett</b>",
            None,
        );
        assert!(h.contains("<h1>Titel</h1>"));
        assert!(!h.contains("<script>"));
        assert!(!h.contains("<b>"));
        assert!(!h.contains("href=\"javascript"));
    }
}
