//! The theme by itself: what reads on what, how colours mix, and the faces of its text and signs.

use iced::Color;
use iced::advanced::graphics::text::cosmic_text::skrifa::{FontRef, MetadataProvider};
use polaroid_iced::theme::{self, DARK, LIGHT, Scheme, contrast, font, hue, ink, over, sign, size};

/// What WCAG asks of text on its ground, and of what is not text.
const TEXT: f32 = 4.5;
const SHAPE: f32 = 3.0;

/// A spread of colours through all there are, six steps along each of red, green and blue.
fn hues() -> impl Iterator<Item = Color> {
    let steps = || (0..6u8).map(|step| step * 51);
    steps()
        .flat_map(move |r| steps().flat_map(move |g| steps().map(move |b| (r, g, b))))
        .map(|(r, g, b)| Color::from_rgb8(r, g, b))
}

#[test]
fn a_theme_gives_its_scheme_back() {
    assert_eq!(Scheme::of(&theme::light()), &LIGHT);
    assert_eq!(Scheme::of(&theme::dark()), &DARK);
}

#[test]
fn every_role_reads_on_its_colour_and_on_its_container() {
    for scheme in [&LIGHT, &DARK] {
        for role in [
            scheme.primary,
            scheme.secondary,
            scheme.tertiary,
            scheme.error,
            scheme.success,
            scheme.warning,
        ] {
            assert!(contrast(role.on, role.color) >= TEXT, "{role:?}");
            assert!(
                contrast(role.on_container, role.container) >= TEXT,
                "{role:?}"
            );
        }
    }
}

#[test]
fn text_reads_on_every_surface() {
    for scheme in [&LIGHT, &DARK] {
        for ground in [
            scheme.surface,
            scheme.surface_container_lowest,
            scheme.surface_container_low,
            scheme.surface_container,
            scheme.surface_container_high,
            scheme.surface_container_highest,
        ] {
            assert!(contrast(scheme.on_surface, ground) >= TEXT, "{ground:?}");
            assert!(
                contrast(scheme.on_surface_variant, ground) >= TEXT,
                "{ground:?}"
            );
            // a note of what is unsaved, and of what is refused
            for note in [
                scheme.warning.color,
                scheme.error.color,
            ] {
                assert!(contrast(note, ground) >= TEXT, "{note:?} on {ground:?}");
            }
        }
        let tooltip = contrast(scheme.inverse_on_surface, scheme.inverse_surface);
        assert!(tooltip >= TEXT);
        // a placeholder, and a value that is the default, in the well of a field
        let quiet = contrast(scheme.outline, scheme.surface_container_lowest);
        assert!(quiet >= TEXT, "{quiet}");
    }
}

#[test]
fn a_layer_over_a_colour_mixes_them_by_its_opacity() {
    let (base, layer) = (Color::BLACK, Color::WHITE);
    assert_eq!(over(base, layer, 0.0), base);
    assert_eq!(over(base, layer, 1.0), layer);
    assert_eq!(over(base, layer, 0.5), Color::from_rgb(0.5, 0.5, 0.5));
    // over nothing it is the layer alone, as see-through as it is laid
    let alone = over(Color::TRANSPARENT, layer, 0.08);
    assert_eq!(alone, layer.scale_alpha(0.08));
    assert_eq!(over(Color::TRANSPARENT, layer, 0.0), Color::TRANSPARENT);
}

#[test]
fn text_reads_on_every_hue_and_on_its_shades() {
    for hue in hues().chain([hue::NEUTRAL]) {
        // a chip, and the cap of a pill
        assert!(contrast(ink(hue), hue) >= TEXT, "{hue:?}");
        for scheme in [&LIGHT, &DARK] {
            let (body, well) = (hue::body(scheme, hue), hue::well(scheme, hue));
            assert!(contrast(scheme.on_surface, body) >= TEXT, "{hue:?}");
            assert!(contrast(scheme.on_surface, well) >= TEXT, "{hue:?}");
            // a value that is the default is quieter there, and still made out
            assert!(contrast(scheme.outline, well) >= SHAPE, "{hue:?}");
        }
    }
}

#[test]
fn the_sans_face_draws_every_sign_itself() {
    let sans = FontRef::new(rustd3_fonts::DEJAVU_SANS.data()).unwrap();
    for sign in sign::ALL {
        for glyph in sign.chars() {
            // one it lacks would be drawn in a face of the system, which may be an emoji
            assert!(
                sans.charmap()
                    .map(glyph)
                    .is_some(),
                "{sign}"
            );
        }
    }
    // what is left out is written in code too
    let mono = FontRef::new(rustd3_fonts::DEJAVU_SANS_MONO.data()).unwrap();
    for glyph in sign::ELLIPSIS.chars() {
        assert!(
            mono.charmap()
                .map(glyph)
                .is_some()
        );
    }
}

#[test]
fn text_is_set_once_in_the_faces_of_the_plots() {
    use iced::font::Family;

    let settings = theme::settings();
    assert_eq!(settings.fonts.len(), rustd3_fonts::ALL.len());
    assert_eq!(settings.font, font::SANS);
    assert_eq!(font::SANS.family, Family::Name(rustd3_fonts::SANS));
    assert_eq!(font::MONO.family, Family::Name(rustd3_fonts::MONO));
    assert_eq!(settings.text_size.0, size::TEXT);
    assert_eq!(settings.line_height, theme::LINE_HEIGHT);
    // a line of a field is whole pixels, so the rows of a menu are crisp
    assert_eq!(theme::LINE, theme::LINE.round());
}
