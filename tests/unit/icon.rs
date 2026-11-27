use polaroid_iced::theme;

use super::*;
use crate::Registry;

/// What typst drew of the icon of a block whose typst is `source`, in the colours of `theme`.
fn drawn(
    source: &'static str,
    theme: &Theme,
) -> Option<svg::Handle> {
    Icons::new([("block", source)], theme).of("block")
}

/// A colour as an SVG writes it.
fn hex(color: iced::Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Whether the SVG that `icon` is holds `written`.
fn holds(
    icon: &svg::Handle,
    written: &str,
) -> bool {
    let iced::advanced::svg::Data::Bytes(bytes) = icon.data() else {
        return false;
    };
    String::from_utf8_lossy(bytes).contains(written)
}

#[test]
fn every_block_has_an_icon_and_what_is_no_block_has_none() {
    let registry = Registry::with_polars();
    let icons = Icons::new(registry.icons(), &theme::light());
    for (block, _) in registry.icons() {
        assert!(
            icons
                .of(block)
                .is_some(),
            "{block}"
        );
    }
    assert!(
        icons
            .of("no such block")
            .is_none()
    );
}

#[test]
fn an_icon_is_drawn_in_the_colours_of_the_theme_by_their_names() {
    for theme in [theme::light(), theme::dark()] {
        for (name, tone) in theme::icon::tones(Scheme::of(&theme)) {
            let source = format!("#rect(width: 4pt, height: 4pt, fill: {name})");
            let icon = drawn(source.leak(), &theme).unwrap();
            assert!(holds(&icon, &hex(tone)), "{name}");
        }
    }
}

#[test]
fn the_text_of_an_icon_is_in_its_ink() {
    let theme = theme::dark();
    let icon = drawn("A", &theme).unwrap();
    assert!(holds(&icon, &hex(Scheme::of(&theme).on_surface)));
}

#[test]
fn an_icon_that_typst_rejects_is_none() {
    assert!(drawn("#no_such_function()", &theme::light()).is_none());
}
