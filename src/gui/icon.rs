//! The icon of each block: its typst, drawn by rustd3 in the colours of the theme as an SVG.

use std::cell::OnceCell;
use std::collections::BTreeMap;

use iced::Theme;
use iced::widget::svg;
use polaroid_iced::theme::{self, Scheme};
use rustd3_figure::text::{self, Fonts};

/// The size of the text of an icon that gives it none, in the points an icon is drawn in.
const TEXT: f32 = 10.0;

/// The icons of the blocks in the colours of a theme, each drawn once it is asked for.
pub struct Icons {
    /// What names the colours of the theme in typst, which the typst of every icon follows.
    tones: String,
    /// The typst of the icon of each block.
    sources: BTreeMap<&'static str, &'static str>,
    /// What typst drew of each icon, many blocks sharing one; none of one that typst rejects.
    drawn: BTreeMap<&'static str, OnceCell<Option<svg::Handle>>>,
}

impl Icons {
    /// The icons of `sources`, each a block and the typst of its icon, in the colours of `theme`.
    pub fn new(
        sources: impl IntoIterator<Item = (&'static str, &'static str)>,
        theme: &Theme,
    ) -> Self {
        let sources: BTreeMap<_, _> = sources
            .into_iter()
            .collect();
        let tones = theme::icon::tones(Scheme::of(theme))
            .iter()
            .map(|(name, tone)| {
                let [r, g, b, _] = tone.into_rgba8();
                format!("#let {name} = rgb({r}, {g}, {b})\n")
            })
            .collect();
        let drawn = sources
            .values()
            .map(|source| (*source, OnceCell::new()))
            .collect();
        Icons {
            tones,
            sources,
            drawn,
        }
    }

    /// The icon of `block`, if it is a block and typst drew its icon.
    pub fn of(
        &self,
        block: &str,
    ) -> Option<svg::Handle> {
        let source = self
            .sources
            .get(block)?;
        self.drawn[source]
            .get_or_init(|| {
                let markup = format!(
                    "{}#set text(size: {TEXT}pt, fill: ink)\n{source}",
                    self.tones
                );
                let drawn = text::draw(&markup, &Fonts::bundled())
                    .map_err(|e| e.to_string())
                    .and_then(|icon| {
                        let options = rustd3_figure::svg::Options::default();
                        rustd3_figure::svg::write(&icon, &options).map_err(|e| e.to_string())
                    });
                match drawn {
                    Ok(drawn) => Some(svg::Handle::from_memory(drawn.into_bytes())),
                    Err(e) => {
                        log::warn!("the icon of {block}: {e}");
                        None
                    }
                }
            })
            .clone()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/icon.rs"]
mod tests;
