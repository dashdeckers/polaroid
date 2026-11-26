//! The look and feel of polaroid in one place: colours, text, scales, and the style of each widget.
//!
//! The roles, surfaces, shapes and state layers are those of Material Design 3, as iced_md3
//! (github.com/ColdMaterial/iced_md3, MIT, © 2026 tpaau) brings them to iced 0.14.

use std::borrow::Cow;
use std::time::Duration;

use iced::theme::palette::{self, Palette, Seed};
use iced::widget::text::LineHeight;
use iced::{Border, Color, Padding, Shadow, Theme, Vector, border};

/// The editor's theme while the system is light.
pub fn light() -> Theme {
    themed("Polaroid Light", &LIGHT)
}

/// The editor's theme while the system is dark.
pub fn dark() -> Theme {
    themed("Polaroid Dark", &DARK)
}

/// A theme of `scheme`, whose palette has the styles of iced itself drawn in its roles too.
fn themed(
    name: &'static str,
    scheme: &'static Scheme,
) -> Theme {
    let seed = Seed {
        background: scheme.surface,
        text: scheme.on_surface,
        primary: scheme.primary.color,
        success: scheme.success.color,
        warning: scheme.warning.color,
        danger: scheme.error.color,
    };
    Theme::custom_with_fn(name, seed, |_| scheme.palette())
}

/// What the editor runs with, and a simulator of it: the faces loaded, and how text is set.
pub fn settings() -> iced::Settings {
    iced::Settings {
        fonts: rustd3_fonts::ALL
            .iter()
            .map(|font| Cow::Borrowed(font.data()))
            .collect(),
        font: font::SANS,
        text_size: size::TEXT.into(),
        line_height: LINE_HEIGHT,
        ..iced::Settings::default()
    }
}

/// A colour and what reads on it, and its quieter container and what reads on that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Role {
    pub color: Color,
    pub on: Color,
    pub container: Color,
    pub on_container: Color,
}

/// The colour roles of Material Design 3, with two more: for what succeeded, and what warns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scheme {
    pub primary: Role,
    pub secondary: Role,
    pub tertiary: Role,
    pub error: Role,
    pub success: Role,
    pub warning: Role,
    /// The window, and what is drawn on it, plainly and more quietly.
    pub surface: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    /// The containers that lie on the window, from the one that stands out least to the most.
    pub surface_container_lowest: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,
    /// The line around what is typed in or pressed, and the quieter one that divides.
    pub outline: Color,
    pub outline_variant: Color,
    /// A surface that stands out against all the others, as a tooltip, and what is on it.
    pub inverse_surface: Color,
    pub inverse_on_surface: Color,
    pub shadow: Color,
    pub is_dark: bool,
}

/// A colour of six hexadecimal digits, as a constant.
const fn hex(rgb: u32) -> Color {
    Color::from_packed_rgb8(rgb)
}

/// The baseline scheme of Material Design 3 on a light system.
pub const LIGHT: Scheme = Scheme {
    primary: Role {
        color: hex(0x6750A4),
        on: hex(0xFFFFFF),
        container: hex(0xEADDFF),
        on_container: hex(0x4F378B),
    },
    secondary: Role {
        color: hex(0x625B71),
        on: hex(0xFFFFFF),
        container: hex(0xE8DEF8),
        on_container: hex(0x4A4458),
    },
    tertiary: Role {
        color: hex(0x7D5260),
        on: hex(0xFFFFFF),
        container: hex(0xFFD8E4),
        on_container: hex(0x633B48),
    },
    error: Role {
        color: hex(0xB3261E),
        on: hex(0xFFFFFF),
        container: hex(0xF9DEDC),
        on_container: hex(0x8C1D18),
    },
    success: Role {
        color: hex(0x386A20),
        on: hex(0xFFFFFF),
        container: hex(0xB8F397),
        on_container: hex(0x205107),
    },
    warning: Role {
        color: hex(0x7C5800),
        on: hex(0xFFFFFF),
        container: hex(0xFFDEA6),
        on_container: hex(0x5E4200),
    },
    surface: hex(0xFEF7FF),
    on_surface: hex(0x1D1B20),
    on_surface_variant: hex(0x49454F),
    surface_container_lowest: hex(0xFFFFFF),
    surface_container_low: hex(0xF7F2FA),
    surface_container: hex(0xF3EDF7),
    surface_container_high: hex(0xECE6F0),
    surface_container_highest: hex(0xE6E0E9),
    outline: hex(0x79747E),
    outline_variant: hex(0xCAC4D0),
    inverse_surface: hex(0x322F35),
    inverse_on_surface: hex(0xF5EFF7),
    shadow: hex(0x000000),
    is_dark: false,
};

/// The baseline scheme of Material Design 3 on a dark system.
pub const DARK: Scheme = Scheme {
    primary: Role {
        color: hex(0xD0BCFF),
        on: hex(0x381E72),
        container: hex(0x4F378B),
        on_container: hex(0xEADDFF),
    },
    secondary: Role {
        color: hex(0xCCC2DC),
        on: hex(0x332D41),
        container: hex(0x4A4458),
        on_container: hex(0xE8DEF8),
    },
    tertiary: Role {
        color: hex(0xEFB8C8),
        on: hex(0x492532),
        container: hex(0x633B48),
        on_container: hex(0xFFD8E4),
    },
    error: Role {
        color: hex(0xF2B8B5),
        on: hex(0x601410),
        container: hex(0x8C1D18),
        on_container: hex(0xF9DEDC),
    },
    success: Role {
        color: hex(0x9CD67D),
        on: hex(0x0C3900),
        container: hex(0x205107),
        on_container: hex(0xB8F397),
    },
    warning: Role {
        color: hex(0xF8BD49),
        on: hex(0x422C00),
        container: hex(0x5E4200),
        on_container: hex(0xFFDEA6),
    },
    surface: hex(0x141218),
    on_surface: hex(0xE6E0E9),
    on_surface_variant: hex(0xCAC4D0),
    surface_container_lowest: hex(0x0F0D13),
    surface_container_low: hex(0x1D1B20),
    surface_container: hex(0x211F26),
    surface_container_high: hex(0x2B2930),
    surface_container_highest: hex(0x36343B),
    outline: hex(0x938F99),
    outline_variant: hex(0x49454F),
    inverse_surface: hex(0xE6E0E9),
    inverse_on_surface: hex(0x322F35),
    shadow: hex(0x000000),
    is_dark: true,
};

impl Scheme {
    /// The scheme of a theme: the dark one, or the light.
    pub fn of(theme: &Theme) -> &'static Self {
        match theme
            .palette()
            .is_dark
        {
            true => &DARK,
            false => &LIGHT,
        }
    }

    /// The colour `role` glows in: its own on a dark window, and its container on a light one.
    pub fn glow(
        &self,
        role: Role,
    ) -> Color {
        match self.is_dark {
            true => role.color,
            false => role.container,
        }
    }

    /// `color` halfway to the window, as the ring of what is known but not yet done.
    pub fn pale(
        &self,
        color: Color,
    ) -> Color {
        over(self.surface, color, opacity::FADED)
    }

    /// Its roles as the palette of iced, which the styles of iced itself are drawn with.
    fn palette(&self) -> Palette {
        let pair = |color, text| palette::Pair { color, text };
        let surface = |color| pair(color, self.on_surface);
        let swatch = |role: Role| palette::Swatch {
            base: pair(role.color, role.on),
            weak: pair(role.container, role.on_container),
            strong: pair(over(role.color, role.on, opacity::HOVERED), role.on),
        };
        Palette {
            background: palette::Background {
                base: surface(self.surface),
                weakest: surface(self.surface_container_low),
                weaker: surface(self.surface_container),
                weak: surface(self.surface_container_high),
                neutral: surface(self.surface_container_highest),
                strong: surface(self.outline_variant),
                stronger: surface(over(self.outline_variant, self.outline, opacity::FADED)),
                strongest: surface(self.outline),
            },
            primary: swatch(self.primary),
            // what iced writes quietly in, as a placeholder is
            secondary: palette::Swatch {
                base: pair(self.on_surface_variant, self.surface),
                weak: pair(self.outline, self.surface),
                strong: pair(self.on_surface, self.surface),
            },
            success: swatch(self.success),
            warning: swatch(self.warning),
            danger: swatch(self.error),
            is_dark: self.is_dark,
        }
    }
}

/// How much of one colour lies over another, or is left of it.
pub mod opacity {
    /// The colour of its content over what is hovered, and over what is pressed.
    pub const HOVERED: f32 = 0.08;
    pub const PRESSED: f32 = 0.12;
    /// What is left of the content of what cannot be pressed, and of its container.
    pub const DISABLED: f32 = 0.38;
    pub const DISABLED_CONTAINER: f32 = 0.12;
    /// The ink over every other row of a table.
    pub const STRIPE: f32 = 0.04;
    /// What is left of a selection behind its text, and of a line that is only hinted at.
    pub const FAINT: f32 = 0.25;
    /// Halfway: a colour that is dimmed, or mixed evenly with what it lies on.
    pub const FADED: f32 = 0.5;
    /// The paper over what is out of date, and what is left of a colour that glows.
    pub const VEIL: f32 = 0.6;
}

/// `layer` laid over `base` this opaque, as a state layer is drawn, or a tint.
pub fn over(
    base: Color,
    layer: Color,
    opacity: f32,
) -> Color {
    let alpha = opacity + base.a * (1.0 - opacity);
    if alpha == 0.0 {
        return Color::TRANSPARENT;
    }
    let mix = |under: f32, above: f32| (above * opacity + under * base.a * (1.0 - opacity)) / alpha;
    Color {
        r: mix(base.r, layer.r),
        g: mix(base.g, layer.g),
        b: mix(base.b, layer.b),
        a: alpha,
    }
}

/// How far apart two colours are to the eye, as WCAG measures it: from 1 to 21.
pub fn contrast(
    one: Color,
    other: Color,
) -> f32 {
    let (one, other) = (
        one.relative_luminance() + 0.05,
        other.relative_luminance() + 0.05,
    );
    one.max(other) / one.min(other)
}

/// The ink that reads best on `color`: black, or white.
pub fn ink(color: Color) -> Color {
    match contrast(Color::BLACK, color) > contrast(Color::WHITE, color) {
        true => Color::BLACK,
        false => Color::WHITE,
    }
}

/// The faces of text: rustd3's DejaVu Sans, and its Mono for code and for columns of numbers.
pub mod font {
    use iced::Font;
    use iced::font::Weight;

    pub const SANS: Font = Font::new(rustd3_fonts::SANS);
    pub const BOLD: Font = SANS.weight(Weight::Bold);
    pub const MONO: Font = Font::new(rustd3_fonts::MONO);
}

/// The sizes of text: of a note beside, of what is read and typed, of a label, and of a heading.
pub mod size {
    pub const NOTE: f32 = 12.0;
    pub const TEXT: f32 = 13.0;
    pub const TITLE: f32 = 14.0;
    pub const HEADING: f32 = 16.0;
}

/// How tall a line of text is for its size.
const LEADING: f32 = 1.375;
pub const LINE_HEIGHT: LineHeight = LineHeight::Relative(LEADING);

/// The height of a line of text of `size`, in whole pixels.
pub const fn line(size: f32) -> f32 {
    (size * LEADING).ceil()
}

/// The height of a line that holds a field, or an item of a menu.
pub const LINE: f32 = line(size::TEXT) + FIELD.top + FIELD.bottom;

/// The signs that are written as text, each a glyph of the sans face.
pub mod sign {
    /// What lays something open, and what closes it again.
    pub const OPEN: &str = "▸";
    pub const CLOSE: &str = "▾";
    /// What adds one more, and what deletes something, or clears it.
    pub const ADD: &str = "+";
    pub const DELETE: &str = "×";
    /// What leads on to more, as an item of a menu that opens another.
    pub const ONWARD: &str = "›";
    /// The handle that a drag up or down adjusts by, of a sign that no font draws as an emoji.
    pub const SCRUB: &str = "⇕";
    /// The grip that something is dragged by.
    pub const GRIP: &str = "≡";
    /// What is left out, and what parts two things on a line.
    pub const ELLIPSIS: &str = "…";
    pub const PART: &str = "·";

    /// Every sign.
    pub const ALL: [&str; 9] = [
        OPEN, CLOSE, ADD, DELETE, ONWARD, SCRUB, GRIP, ELLIPSIS, PART,
    ];
}

/// The spaces between and around things, from the least to the most.
pub mod spacing {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
}

/// How round corners are, from a tick box to a dialog, and a pill.
pub mod radius {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 28.0;
    /// Half the height of anything, which every renderer rounds it to.
    pub const FULL: f32 = u32::MAX as f32;
}

/// How thick lines are: an outline or a divider, a wire or what is typed in, and a status.
pub mod stroke {
    pub const THIN: f32 = 1.0;
    pub const MEDIUM: f32 = 2.0;
    pub const THICK: f32 = 3.0;
}

/// The side of a dot of a colour: on a tile, down a card or in a list, and to pick one from.
pub mod dot {
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 18.0;
}

/// The side of a drawn sign: a chevron, one in a line of text, one in a button, and on a tile.
pub mod icon {
    pub const S: f32 = 12.0;
    pub const M: f32 = 16.0;
    pub const L: f32 = 20.0;
    pub const XL: f32 = 40.0;
}

/// The height of what is pressed: a button that is a sign, a pill or a switch, and one afloat.
pub mod control {
    pub const S: f32 = 24.0;
    pub const M: f32 = 36.0;
    pub const L: f32 = 40.0;
}

/// How wide or tall what has no size of its own is, from the least of a field to an open card.
pub mod extent {
    /// The least of a field, and a slider beside one.
    pub const XXS: f32 = 48.0;
    pub const XS: f32 = 64.0;
    /// A column of a table, the least of a pane, and the most of what scrolls inside a card.
    pub const S: f32 = 140.0;
    /// What opens to pick from, narrow and wide, and the most that a tooltip takes.
    pub const M: f32 = 200.0;
    pub const L: f32 = 280.0;
    /// A pane beside the cards, and an open card.
    pub const XL: f32 = 320.0;
    pub const XXL: f32 = 480.0;
}

/// How long things take: a step of a drag, a slide, a rest before a drop, and a pause.
pub mod motion {
    use super::Duration;

    pub const INSTANT: Duration = Duration::from_millis(100);
    pub const SHORT: Duration = Duration::from_millis(200);
    pub const MEDIUM: Duration = Duration::from_millis(400);
    pub const LONG: Duration = Duration::from_millis(500);
}

/// The ring around what has the focus of the keyboard: how thick, and how far outside it.
pub mod focus {
    pub const WIDTH: f32 = super::stroke::THICK;
    pub const OUTSET: f32 = super::spacing::XXS;
}

/// The cards and the wires between them.
pub mod surface {
    /// How far a port reaches from its middle, and the side of the corner that resizes a card.
    pub const PORT: f32 = 5.0;
    pub const GRIP: f32 = 14.0;
    /// The radius of a dot of a grip, and how round a card is.
    pub const SPECK: f32 = 1.5;
    pub const CORNER: f32 = super::radius::M;
    /// How far a wire runs level at the least before it bends.
    pub const BEND: f32 = 40.0;
    /// Where the first card is laid from the corner, and the room beside and below each.
    pub const EDGE: f32 = 20.0;
    pub const BESIDE: f32 = 70.0;
    pub const BELOW: f32 = 160.0;
    /// How much one step of the wheel zooms, and how far out and in.
    pub const ZOOM_STEP: f32 = 1.1;
    pub const ZOOM: (f32, f32) = (0.25, 2.5);
    /// How much of the smaller of two cards lies on the other for a drop to swap them.
    pub const OVERLAP: f32 = super::opacity::FADED;
}

/// A drag that adjusts a number: how far for each step, and how it steps more as it is faster.
pub mod scrub {
    pub const STEP: f32 = 6.0;
    /// The pixels a second up to which a drag steps evenly.
    pub const SLOW: f32 = 200.0;
    /// How steeply it steps more beyond that, and the most it ever does.
    pub const POWER: f32 = 1.5;
    pub const MOST: f32 = 40.0;
}

/// How far a press is dragged before it is a drag.
pub const DRAG: f32 = spacing::XS;

/// The bars that scroll what is too long for its box: how thick they are.
pub const BAR: f32 = 6.0;

/// The room around the text of a field to type in, or of a list to pick from.
pub const FIELD: Padding = Padding {
    top: spacing::XS,
    right: spacing::S,
    bottom: spacing::XS,
    left: spacing::S,
};

/// The room around the label of a button, and of a small one among the lines of a card.
pub const BUTTON: Padding = Padding {
    top: 6.0,
    right: spacing::M,
    bottom: 6.0,
    left: spacing::M,
};
pub const DENSE: Padding = Padding {
    top: spacing::XXS,
    right: spacing::S,
    bottom: spacing::XXS,
    left: spacing::S,
};

/// How far a halo reaches out from what it rings, and each ring of many beyond the one inside it.
pub const HALO: f32 = spacing::XS;

/// A glow of `color` around what is lit: a shadow that falls nowhere.
pub fn halo(color: Color) -> Shadow {
    Shadow {
        color: color.scale_alpha(opacity::VEIL),
        offset: Vector::ZERO,
        blur_radius: 2.0 * HALO,
    }
}

/// How high a surface lies over the window: the container it is filled with, and its shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Elevation {
    /// Just over the window, as a card.
    Raised,
    /// Over all else, as a menu.
    Floating,
}

impl Elevation {
    /// The container it is filled with.
    pub fn surface(
        self,
        scheme: &Scheme,
    ) -> Color {
        match self {
            Self::Raised => scheme.surface_container_low,
            Self::Floating => scheme.surface_container,
        }
    }

    /// The shadow it casts, one where Material Design has two, after iced_md3.
    pub fn shadow(
        self,
        scheme: &Scheme,
    ) -> Shadow {
        let (down, blur, alpha) = match self {
            Self::Raised => (1.0, 3.0, 0.15),
            Self::Floating => (2.0, 6.0, 0.18),
        };
        Shadow {
            color: scheme
                .shadow
                .scale_alpha(alpha),
            offset: Vector::new(0.0, down),
            blur_radius: blur,
        }
    }
}

/// The line around a surface that is set apart, as round as `radius`.
fn outlined(
    scheme: &Scheme,
    radius: f32,
) -> Border {
    border::rounded(radius)
        .color(scheme.outline_variant)
        .width(stroke::THIN)
}

/// How a value stands, which the field that holds it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Given,
    /// It is the default, and written quietly.
    Default,
    /// It is typed or edited and not yet taken, and outlined as a warning.
    Unsaved,
    /// There is none where there has to be one, or it is refused: outlined as an error.
    Refused,
}

/// What a field is doing, which its outline shows.
#[derive(Clone, Copy)]
enum Hold {
    Rest,
    Hovered,
    /// It is typed in, or open.
    Active,
}

/// The well of a field, its outline and what is written in it, on the panel of `tint` if any.
fn field(
    scheme: &Scheme,
    hold: Hold,
    tone: Tone,
    tint: Option<Color>,
    is_disabled: bool,
) -> (Color, Border, Color) {
    let well = match tint {
        Some(tint) => hue::well(scheme, tint),
        None => scheme.surface_container_lowest,
    };
    let (line, width) = match (tone, hold, tint) {
        (Tone::Unsaved, ..) => (scheme.warning.color, stroke::MEDIUM),
        (Tone::Refused, ..) => (scheme.error.color, stroke::MEDIUM),
        (_, Hold::Active, _) => (scheme.primary.color, stroke::MEDIUM),
        (_, Hold::Hovered, Some(tint)) => (tint, stroke::THIN),
        (_, Hold::Hovered, None) => (scheme.outline, stroke::THIN),
        (_, Hold::Rest, Some(tint)) => (tint.scale_alpha(opacity::FADED), stroke::THIN),
        (_, Hold::Rest, None) => (scheme.outline_variant, stroke::THIN),
    };
    let written = match (is_disabled, tone) {
        (true, _) => scheme
            .on_surface
            .scale_alpha(opacity::DISABLED),
        (false, Tone::Default) => scheme.outline,
        (false, _) => scheme.on_surface,
    };
    let outline = border::rounded(radius::XS)
        .color(line)
        .width(width);
    (well, outline, written)
}

/// The buttons: pills, laid over with the colour of their content as they are hovered or pressed.
pub mod button {
    use iced::widget::button::{Status, Style};
    use iced::{Color, Theme, border};

    use super::{Elevation, Scheme, opacity, over, radius, stroke};

    /// The button of the action that matters most, filled with the primary colour.
    pub fn filled(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        let primary = scheme.primary;
        styled(scheme, primary.color, primary.on, status)
    }

    /// A button of an action beside the one that matters most, in the secondary container.
    pub fn tonal(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        let secondary = scheme.secondary;
        styled(scheme, secondary.container, secondary.on_container, status)
    }

    /// Filled while it is on and tonal while it is off: the button of a mode, or of a toggle.
    pub fn selected(is_on: bool) -> fn(&Theme, Status) -> Style {
        match is_on {
            true => filled,
            false => tonal,
        }
    }

    /// A button that is its label alone, in the primary colour.
    pub fn text(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        styled(scheme, Color::TRANSPARENT, scheme.primary.color, status)
    }

    /// A button that is a sign, as + or −, in the quieter colour of what is on a surface.
    pub fn icon(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        styled(
            scheme,
            Color::TRANSPARENT,
            scheme.on_surface_variant,
            status,
        )
    }

    /// A sign that deletes, as ×, in the colour of an error.
    pub fn danger(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        styled(scheme, Color::TRANSPARENT, scheme.error.color, status)
    }

    /// A button that is a sign on a ground of `color`, in the ink that reads there.
    pub fn on(color: Color) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            styled(
                Scheme::of(theme),
                Color::TRANSPARENT,
                super::ink(color),
                status,
            )
        }
    }

    /// A bar across what it lays open or closes, as a drawer: a step over the card, less round.
    pub fn bar(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        let style = styled(
            scheme,
            scheme.surface_container_high,
            scheme.on_surface_variant,
            status,
        );
        Style {
            border: border::rounded(radius::S),
            ..style
        }
    }

    /// A round button afloat over all else, filled while it is on.
    pub fn floating(is_on: bool) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| Style {
            shadow: Elevation::Floating.shadow(Scheme::of(theme)),
            ..selected(is_on)(theme, status)
        }
    }

    /// A dot of `color` to pick, ringed while it is picked or hovered.
    pub fn swatch(
        color: Color,
        is_picked: bool,
    ) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let ring = match is_picked || status == Status::Hovered {
                true => Scheme::of(theme).on_surface,
                false => Color::TRANSPARENT,
            };
            Style {
                background: Some(color.into()),
                border: border::rounded(radius::FULL)
                    .color(ring)
                    .width(stroke::MEDIUM),
                ..Style::default()
            }
        }
    }

    /// Where a button lies in a pill of buttons: at its round start or end, or between them.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Segment {
        First,
        Inner,
        Last,
    }

    /// `style` for a button at `at` in a pill of buttons, whose sides that meet are barely round.
    pub fn segment(
        style: fn(&Theme, Status) -> Style,
        at: Segment,
    ) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let style = style(theme, status);
            Style {
                border: style
                    .border
                    .rounded(corners(at)),
                ..style
            }
        }
    }

    /// How round the corners of a button are at `at` in a pill of buttons.
    pub fn corners(at: Segment) -> border::Radius {
        let (left, right) = match at {
            Segment::First => (radius::FULL, radius::XS),
            Segment::Inner => (radius::XS, radius::XS),
            Segment::Last => (radius::XS, radius::FULL),
        };
        border::Radius {
            top_left: left,
            bottom_left: left,
            top_right: right,
            bottom_right: right,
        }
    }

    /// A pill of `container` with `content` on it, which lies over it as it is hovered or pressed.
    fn styled(
        scheme: &Scheme,
        container: Color,
        content: Color,
        status: Status,
    ) -> Style {
        let (background, text_color) = match status {
            Status::Active => (container, content),
            Status::Hovered => (over(container, content, opacity::HOVERED), content),
            Status::Pressed => (over(container, content, opacity::PRESSED), content),
            Status::Disabled => {
                let faded = match container == Color::TRANSPARENT {
                    true => container,
                    false => scheme
                        .on_surface
                        .scale_alpha(opacity::DISABLED_CONTAINER),
                };
                let content = scheme
                    .on_surface
                    .scale_alpha(opacity::DISABLED);
                (faded, content)
            }
        };
        Style {
            background: Some(background.into()),
            text_color,
            border: border::rounded(radius::FULL),
            ..Style::default()
        }
    }
}

/// The fields to type in.
pub mod text_input {
    use iced::widget::text_input::{Status, Style};
    use iced::{Color, Theme, border};

    use super::{Hold, Scheme, Tone, radius};

    /// A field whose value stands as `tone`: a well, outlined, in the primary colour while used.
    pub fn toned(tone: Tone) -> impl Fn(&Theme, Status) -> Style {
        styled(tone, None)
    }

    /// Such a field on a panel of `hue`, as a tag or a group has one.
    pub fn tinted(
        tone: Tone,
        hue: Color,
    ) -> impl Fn(&Theme, Status) -> Style {
        styled(tone, Some(hue))
    }

    fn styled(
        tone: Tone,
        tint: Option<Color>,
    ) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let scheme = Scheme::of(theme);
            let hold = match status {
                Status::Active | Status::Disabled => Hold::Rest,
                Status::Hovered => Hold::Hovered,
                Status::Focused { .. } => Hold::Active,
            };
            let is_disabled = status == Status::Disabled;
            let (well, border, value) = super::field(scheme, hold, tone, tint, is_disabled);
            Style {
                background: well.into(),
                border,
                // quieter than what is typed, as a value that is the default is shown too
                placeholder: scheme.outline,
                value,
                selection: scheme
                    .primary
                    .container,
            }
        }
    }

    /// A field with no box of its own, as a name on a pill, whose line shows in use or in a tone.
    pub fn bare(tone: Tone) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let style = toned(tone)(theme, status);
            let width = match (status, tone) {
                (Status::Active | Status::Disabled, Tone::Given | Tone::Default) => 0.0,
                _ => style.border.width,
            };
            Style {
                background: Color::TRANSPARENT.into(),
                border: border::rounded(radius::FULL)
                    .color(style.border.color)
                    .width(width),
                ..style
            }
        }
    }
}

/// The lists to pick from.
pub mod pick_list {
    use iced::widget::pick_list::{Status, Style};
    use iced::{Color, Theme};

    use super::{Hold, Scheme, Tone};

    /// A list whose pick stands as `tone`, drawn as a field is, in the primary colour while open.
    pub fn toned(tone: Tone) -> impl Fn(&Theme, Status) -> Style {
        styled(tone, None)
    }

    /// Such a list on a panel of `hue`, as a tag or a group has one.
    pub fn tinted(
        tone: Tone,
        hue: Color,
    ) -> impl Fn(&Theme, Status) -> Style {
        styled(tone, Some(hue))
    }

    fn styled(
        tone: Tone,
        tint: Option<Color>,
    ) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let scheme = Scheme::of(theme);
            let hold = match status {
                Status::Active | Status::Disabled => Hold::Rest,
                Status::Hovered => Hold::Hovered,
                Status::Opened { .. } => Hold::Active,
            };
            let is_disabled = status == Status::Disabled;
            let (well, border, text_color) = super::field(scheme, hold, tone, tint, is_disabled);
            Style {
                text_color,
                placeholder_color: scheme.outline,
                handle_color: scheme.on_surface_variant,
                background: well.into(),
                border,
            }
        }
    }
}

/// The lists that open over all else: a menu, and the options of a list to pick from.
pub mod menu {
    use iced::{Background, Border, Color, Shadow, Theme, border};

    use super::{Scheme, container, opacity, radius, spacing};

    /// The room above the first row and below the last, and beside the mark of the row that is lit.
    pub const EDGE: f32 = spacing::XS;
    /// The room between a row and what is said at its end.
    pub const GAP: f32 = spacing::M;

    /// What a list is drawn with.
    #[derive(Debug, Clone, Copy)]
    pub struct Style {
        pub background: Background,
        pub border: Border,
        pub shadow: Shadow,
        pub text: Color,
        /// The mark under the row that is lit, how round it is, and what is written on it.
        pub lit: Color,
        pub mark: Border,
        pub on_lit: Color,
        /// A row that cannot be picked, and what is said at the end of a row.
        pub disabled: Color,
        pub aside: Color,
    }

    /// A list as a menu is: a row is lit in the secondary container.
    pub fn list(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        let secondary = scheme.secondary;
        let menu = container::menu(theme);
        Style {
            background: menu
                .background
                .unwrap_or(Color::TRANSPARENT.into()),
            border: menu.border,
            shadow: menu.shadow,
            text: scheme.on_surface,
            lit: secondary.container,
            mark: border::rounded(radius::XS),
            on_lit: secondary.on_container,
            disabled: scheme
                .on_surface
                .scale_alpha(opacity::DISABLED),
            aside: scheme.on_surface_variant,
        }
    }
}

/// The roles that the modes of the editor glow in on their switch: live, delayed, and by hand.
pub mod mode {
    use super::{Role, Scheme};

    pub fn live(scheme: &Scheme) -> Role {
        scheme.success
    }

    pub fn delay(scheme: &Scheme) -> Role {
        scheme.warning
    }

    pub fn manual(scheme: &Scheme) -> Role {
        scheme.primary
    }
}

/// The boxes to tick.
pub mod checkbox {
    use iced::widget::checkbox::{Status, Style};
    use iced::{Color, Theme, border};

    use super::{Scheme, Tone, opacity, radius, stroke};

    /// A box whose tick stands as `tone`: filled once ticked, grey as the default, red if refused.
    pub fn toned(tone: Tone) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let scheme = Scheme::of(theme);
            let (is_checked, line, opacity) = match status {
                Status::Active { is_checked } => (is_checked, scheme.on_surface_variant, 1.0),
                Status::Hovered { is_checked } => (is_checked, scheme.on_surface, 1.0),
                Status::Disabled { is_checked } => {
                    (is_checked, scheme.on_surface, opacity::DISABLED)
                }
            };
            let lit = match tone {
                Tone::Default => scheme.outline,
                _ => scheme.primary.color,
            };
            let (fill, line) = match (is_checked, tone) {
                (true, Tone::Refused) => (lit, scheme.error.color),
                (true, _) => (lit, lit),
                (false, Tone::Refused) => (Color::TRANSPARENT, scheme.error.color),
                (false, Tone::Default) => (Color::TRANSPARENT, scheme.outline),
                (false, _) => (Color::TRANSPARENT, line),
            };
            Style {
                background: fill
                    .scale_alpha(opacity)
                    .into(),
                icon_color: scheme.primary.on,
                border: border::rounded(radius::XXS)
                    .color(line.scale_alpha(opacity))
                    .width(stroke::MEDIUM),
                text_color: None,
            }
        }
    }
}

/// The switches.
pub mod toggler {
    use iced::widget::toggler::{Status, Style, default};
    use iced::{Color, Theme};

    use super::{Scheme, stroke};

    /// A switch filled with `color` while it is on, and outlined while it is off.
    pub fn lit(color: Color) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let scheme = Scheme::of(theme);
            let (Status::Active { is_toggled }
            | Status::Hovered { is_toggled }
            | Status::Disabled { is_toggled }) = status;
            let (track, thumb, line) = match is_toggled {
                true => (color, scheme.surface_container_lowest, color),
                false => (
                    scheme.surface_container_highest,
                    scheme.outline,
                    scheme.outline,
                ),
            };
            Style {
                background: track.into(),
                background_border_width: stroke::MEDIUM,
                background_border_color: line,
                foreground: thumb.into(),
                foreground_border_width: 0.0,
                foreground_border_color: Color::TRANSPARENT,
                ..default(theme, status)
            }
        }
    }
}

/// The sliders.
pub mod slider {
    use iced::Theme;
    use iced::widget::slider::{Handle, HandleShape, Rail, Status, Style};

    use super::{Scheme, dot, opacity, over, radius, spacing};

    /// A rail filled with the primary colour up to a round handle of it.
    pub fn line(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let scheme = Scheme::of(theme);
        let primary = scheme.primary;
        let layer = match status {
            Status::Active => 0.0,
            Status::Hovered => opacity::HOVERED,
            Status::Dragged => opacity::PRESSED,
        };
        let lit = over(primary.color, primary.on, layer);
        Style {
            rail: Rail {
                backgrounds: (
                    lit.into(),
                    scheme
                        .surface_container_highest
                        .into(),
                ),
                width: spacing::XS,
                border: iced::border::rounded(radius::FULL),
            },
            handle: Handle {
                shape: HandleShape::Circle {
                    radius: dot::M / 2.0,
                },
                background: lit.into(),
                border_width: 0.0,
                border_color: lit,
            },
        }
    }
}

/// The containers that set something apart.
pub mod container {
    use iced::widget::container::Style;
    use iced::{Color, Theme, border};

    use super::{Elevation, Scheme, opacity, outlined, radius, stroke, surface};

    /// A card, a little over the surface: rounded, outlined, and with a shadow.
    pub fn card(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        Style::default()
            .background(Elevation::Raised.surface(scheme))
            .border(outlined(scheme, surface::CORNER))
            .shadow(Elevation::Raised.shadow(scheme))
    }

    /// How what a card stands for went: found sound or not, or done and well or not.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Ring {
        Sound,
        Unsound,
        Done,
        Failed,
    }

    /// A card ringed in the colour of how it went, if anything is known of that.
    pub fn ringed(ring: Option<Ring>) -> impl Fn(&Theme) -> Style {
        move |theme| {
            let card = card(theme);
            let scheme = Scheme::of(theme);
            let (success, error) = (scheme.success.color, scheme.error.color);
            // paler for what is only found, and not done
            let color = match ring {
                None => return card,
                Some(Ring::Sound) => scheme.pale(success),
                Some(Ring::Unsound) => scheme.pale(error),
                Some(Ring::Done) => success,
                Some(Ring::Failed) => error,
            };
            let border = card
                .border
                .color(color)
                .width(stroke::THICK);
            card.border(border)
        }
    }

    /// The tile of a card, a step over it, and as round: alone it is the card, and open its corner.
    pub fn tile(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        Style::default()
            .background(scheme.surface_container_high)
            .border(outlined(scheme, surface::CORNER))
    }

    /// A part of a card that shows what it gives, as its table or its plot: outlined, in a well.
    pub fn section(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        Style::default()
            .background(scheme.surface_container_lowest)
            .border(outlined(scheme, radius::S))
    }

    /// What a tooltip says, on the inverse surface, which stands out over all else.
    pub fn tooltip(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        Style::default()
            .background(scheme.inverse_surface)
            .color(scheme.inverse_on_surface)
            .border(border::rounded(radius::XS))
            .shadow(Elevation::Floating.shadow(scheme))
    }

    /// What opens over all else to pick from, as a menu does.
    pub fn menu(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        Style::default()
            .background(Elevation::Floating.surface(scheme))
            .color(scheme.on_surface)
            .border(outlined(scheme, radius::S))
            .shadow(Elevation::Floating.shadow(scheme))
    }

    /// A sheet beside the surface, as the drawer is.
    pub fn sheet(theme: &Theme) -> Style {
        let scheme = Scheme::of(theme);
        Style::default().background(Elevation::Raised.surface(scheme))
    }

    /// A box that holds an error, in the error container, outlined in the error colour.
    pub fn error(theme: &Theme) -> Style {
        let error = Scheme::of(theme).error;
        let outline = border::rounded(radius::S)
            .color(error.color)
            .width(stroke::THIN);
        Style::default()
            .background(error.container)
            .color(error.on_container)
            .border(outline)
    }

    /// A veil of `paper` over what lies on it and is out of date.
    pub fn veil(paper: Color) -> impl Fn(&Theme) -> Style {
        move |_| Style::default().background(paper.scale_alpha(opacity::VEIL))
    }

    /// What a plot says at the cursor: a box of its own `paper`, outlined faintly in its `ink`.
    pub fn readout(
        ink: Color,
        paper: Color,
    ) -> impl Fn(&Theme) -> Style {
        move |_| {
            let outline = border::rounded(radius::XS)
                .color(ink.scale_alpha(opacity::FAINT))
                .width(stroke::THIN);
            Style::default()
                .background(paper)
                .color(ink)
                .border(outline)
        }
    }

    /// The line where what is dragged would drop, in the primary colour if it would drop there.
    pub fn drop(is_lit: bool) -> impl Fn(&Theme) -> Style {
        move |theme| Style {
            background: is_lit.then(|| {
                Scheme::of(theme)
                    .primary
                    .color
                    .into()
            }),
            border: border::rounded(radius::FULL),
            ..Style::default()
        }
    }

    /// The box of a group on a sheet, ringed in the primary colour where a drag would drop in it.
    pub fn group(
        is_boxed: bool,
        is_lit: bool,
    ) -> impl Fn(&Theme) -> Style {
        move |theme| {
            let scheme = Scheme::of(theme);
            let ring = match is_lit {
                true => border::rounded(radius::M)
                    .color(scheme.primary.color)
                    .width(stroke::MEDIUM),
                false => border::rounded(radius::M),
            };
            Style {
                background: is_boxed.then(|| {
                    scheme
                        .surface_container
                        .into()
                }),
                border: ring,
                ..Style::default()
            }
        }
    }
}

/// The bars that scroll.
pub mod scrollable {
    use iced::widget::scrollable::{Rail, Scroller, Status, Style, default};
    use iced::{Border, Color, Theme, border};

    use super::{Scheme, radius};

    /// Thin, round bars on no rail, which darken as they are hovered and dragged.
    pub fn thin(
        theme: &Theme,
        status: Status,
    ) -> Style {
        on(Color::TRANSPARENT)(theme, status)
    }

    /// Such bars on rails of `paper`, which nothing that is scrolled shows through.
    pub fn on(paper: Color) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let scheme = Scheme::of(theme);
            let lit = |is_hovered: bool, is_dragged: bool| match (is_dragged, is_hovered) {
                (true, _) => scheme.on_surface_variant,
                (false, true) => scheme.outline,
                (false, false) => scheme.outline_variant,
            };
            let (horizontal, vertical) = match status {
                Status::Active { .. } => (lit(false, false), lit(false, false)),
                Status::Hovered {
                    is_horizontal_scrollbar_hovered,
                    is_vertical_scrollbar_hovered,
                    ..
                } => (
                    lit(is_horizontal_scrollbar_hovered, false),
                    lit(is_vertical_scrollbar_hovered, false),
                ),
                Status::Dragged {
                    is_horizontal_scrollbar_dragged,
                    is_vertical_scrollbar_dragged,
                    ..
                } => (
                    lit(false, is_horizontal_scrollbar_dragged),
                    lit(false, is_vertical_scrollbar_dragged),
                ),
            };
            let rail = |scroller: Color| Rail {
                background: Some(paper.into()),
                border: Border::default(),
                scroller: Scroller {
                    background: scroller.into(),
                    border: border::rounded(radius::FULL),
                },
            };
            Style {
                vertical_rail: rail(vertical),
                horizontal_rail: rail(horizontal),
                gap: Some(paper.into()),
                ..default(theme, status)
            }
        }
    }

    /// Such bars over an error, on the paper of its box.
    pub fn on_error(
        theme: &Theme,
        status: Status,
    ) -> Style {
        let paper = Scheme::of(theme)
            .error
            .container;
        on(paper)(theme, status)
    }
}

/// The colours of text.
pub mod text {
    use iced::Theme;
    use iced::widget::text::Style;

    use super::{Scheme, Tone, opacity};

    /// Text said quietly, beside what matters.
    pub fn muted(theme: &Theme) -> Style {
        toned(Tone::Default)(theme)
    }

    /// Text of what stands as `tone`: quiet, or in the colour of a warning or of an error.
    pub fn toned(tone: Tone) -> impl Fn(&Theme) -> Style {
        move |theme| {
            let scheme = Scheme::of(theme);
            let color = match tone {
                Tone::Given => scheme.on_surface,
                Tone::Default => scheme.on_surface_variant,
                Tone::Unsaved => scheme.warning.color,
                Tone::Refused => scheme.error.color,
            };
            Style {
                color: Some(color),
                selection: None,
            }
        }
    }

    /// Text of an error, as it lies in the error container, and selected in the error colour.
    pub fn error(theme: &Theme) -> Style {
        let error = Scheme::of(theme).error;
        Style {
            color: Some(error.on_container),
            selection: Some(
                error
                    .color
                    .scale_alpha(opacity::FAINT),
            ),
        }
    }
}

/// What takes a colour that the user gave it, as a tag or a group: a hue of a palette.
pub mod hue {
    use iced::widget::container::Style;
    use iced::{Color, Theme, border};

    use super::button::{Segment, corners};
    use super::{Scheme, control, ink, over, radius, spacing};

    /// The grey of what has no colour of its own.
    pub const NEUTRAL: Color = super::hex(0x8C8C8C);

    /// How much of a hue lies over the lowest surface in a body of it, light and dark.
    const BODY: (f32, f32) = (0.32, 0.22);
    /// How much of it lies there in a field on such a body.
    const WELL: f32 = 0.12;

    /// How round a panel of a hue is: as round as the pill in it, a step further out.
    pub const PANEL: f32 = control::M / 2.0 + spacing::XS;

    /// The lighter shade of `hue` that the body of a pill is, which the text of a surface reads on.
    pub fn body(
        scheme: &Scheme,
        hue: Color,
    ) -> Color {
        let opacity = match scheme.is_dark {
            true => BODY.1,
            false => BODY.0,
        };
        over(scheme.surface_container_lowest, hue, opacity)
    }

    /// The shade of `hue` that a field on such a body is, a well lighter than the body.
    pub fn well(
        scheme: &Scheme,
        hue: Color,
    ) -> Color {
        over(scheme.surface_container_lowest, hue, WELL)
    }

    /// A chip of `hue` itself, as a tag that something carries, in the ink that reads on it.
    pub fn chip(hue: Color) -> impl Fn(&Theme) -> Style {
        move |_| {
            Style::default()
                .background(hue)
                .color(ink(hue))
                .border(border::rounded(radius::FULL))
        }
    }

    /// A dot of `hue`.
    pub fn dot(hue: Color) -> impl Fn(&Theme) -> Style {
        move |_| {
            Style::default()
                .background(hue)
                .border(border::rounded(radius::FULL))
        }
    }

    /// The cap of a pill at `at`, in `hue` itself.
    pub fn cap(
        hue: Color,
        at: Segment,
    ) -> impl Fn(&Theme) -> Style {
        move |_| {
            Style::default()
                .background(hue)
                .color(ink(hue))
                .border(border::rounded(corners(at)))
        }
    }

    /// The body of a pill at `at`, in the lighter shade of `hue`.
    pub fn shade(
        hue: Color,
        at: Segment,
    ) -> impl Fn(&Theme) -> Style {
        move |theme| {
            let scheme = Scheme::of(theme);
            Style::default()
                .background(body(scheme, hue))
                .color(scheme.on_surface)
                .border(border::rounded(corners(at)))
        }
    }

    /// The panel that a pill grows into as it is laid open, in the lighter shade of `hue`.
    pub fn panel(hue: Color) -> impl Fn(&Theme) -> Style {
        move |theme| {
            let scheme = Scheme::of(theme);
            Style::default()
                .background(body(scheme, hue))
                .color(scheme.on_surface)
                .border(border::rounded(PANEL))
        }
    }
}
