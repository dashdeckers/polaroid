//! The images in the cells of a frame, as a plot leaves them there: a gallery to show on a card.

#[cfg(feature = "plot")]
use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap};

use iced::widget::{button, column, image, row, slider, space, svg, text};
use iced::{Center, Element, Fill, Widget};
use polaroid_iced::pick_list;
use polaroid_iced::theme::size;
#[cfg(feature = "plot")]
use rustd3_iced::Plot;

use super::look::{self, Help};
#[cfg(feature = "plot")]
use crate::Drawer;
use crate::polars::prelude::{Column, DataFrame, SortOptions};

/// The space between the lines of a gallery.
const GAP: f32 = 6.0;

/// An image to show: of pixels, or of shapes, which stay sharp at any size.
#[derive(Debug)]
enum Picture {
    /// With how tall it is for its width, which is known before it is loaded.
    Pixels(image::Handle, f32),
    Shapes(svg::Handle, f32),
}

impl Picture {
    /// The image that `bytes` are, if they are a PNG or an SVG.
    fn of(bytes: &[u8]) -> Option<Self> {
        let (wide, tall) = size(bytes)?;
        let tall = tall / wide;
        match kind(bytes)? {
            "png" => Some(Self::Pixels(
                image::Handle::from_bytes(bytes.to_vec()),
                tall,
            )),
            _ => Some(Self::Shapes(svg::Handle::from_memory(bytes.to_vec()), tall)),
        }
    }

    /// It, as wide as `width` and as tall as that makes it.
    fn view<'a, Message: 'a>(
        &self,
        width: f32,
    ) -> Element<'a, Message> {
        match self {
            Self::Pixels(handle, tall) => image(handle)
                .width(width)
                .height(width * tall)
                .boxed(),
            Self::Shapes(handle, tall) => svg(handle.clone())
                .width(width)
                .height(width * tall)
                .boxed(),
        }
    }
}

/// How wide and tall the image that `bytes` are says it is.
fn size(bytes: &[u8]) -> Option<(f32, f32)> {
    if kind(bytes)? == "png" {
        // in the header of a PNG, after its signature and the name of its first chunk
        let side = |at: usize| {
            let side = bytes.get(at..at + 4)?;
            Some(u32::from_be_bytes(
                side.try_into()
                    .ok()?,
            ) as f32)
        };
        return side(16).zip(side(20));
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    let side = |name: &str| {
        // past the name, the equals sign and the quote
        let from = head.find(&format!(" {name}="))? + name.len() + 3;
        let value = head.get(from..)?;
        value[..value.find(['\'', '"'])?]
            .parse()
            .ok()
    };
    side("width").zip(side("height"))
}

/// The kind of image that `bytes` are, as the extension of its files; none if they are none.
fn kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG") {
        Some("png")
    } else if bytes.starts_with(b"<svg") {
        Some("svg")
    } else {
        None
    }
}

/// A column that tells the rows of a gallery apart: its values in order, and that of each row.
#[derive(Debug)]
struct Attribute {
    name: String,
    is_numeric: bool,
    values: Vec<String>,
    /// Which of the values each row has.
    rows: Vec<usize>,
}

impl Attribute {
    /// The values of `column` and where they are; none if it has one value or they cannot be told.
    fn of(column: &Column) -> Option<Self> {
        let values = column
            .unique_stable()
            .and_then(|values| values.sort(SortOptions::default()))
            .ok()?;
        let shown = |column: &Column| -> Vec<String> {
            column
                .as_materialized_series()
                .iter()
                .map(|value| {
                    value
                        .str_value()
                        .into_owned()
                })
                .collect()
        };
        let values = shown(&values);
        let places: HashMap<&str, usize> = values
            .iter()
            .enumerate()
            .map(|(place, value)| (value.as_str(), place))
            .collect();
        let rows = shown(column)
            .iter()
            .map(|value| places[value.as_str()])
            .collect();
        (values.len() > 1).then(|| Attribute {
            name: column
                .name()
                .to_string(),
            is_numeric: column
                .dtype()
                .is_numeric(),
            values,
            rows,
        })
    }
}

/// The images of a frame row by row, and the columns beside them that tell the rows apart.
#[derive(Debug, Default)]
pub struct Gallery {
    /// The column that holds the images.
    column: String,
    pictures: Vec<Option<Picture>>,
    attributes: Vec<Attribute>,
    /// What draws the figure of a row over again, to zoom and pan, if a Plot drew the images.
    #[cfg(feature = "plot")]
    live: Option<Live>,
}

/// The Plot that drew the images of a frame, and what it drew them of.
#[cfg(feature = "plot")]
#[derive(Debug)]
struct Live {
    drawer: Drawer,
    frame: DataFrame,
    /// What the image of each row was drawn of, once it is asked for; none if it cannot be told.
    values: Vec<OnceCell<Option<DataFrame>>>,
}

#[cfg(feature = "plot")]
impl Live {
    /// What the image of `row` was drawn of, which is found once.
    fn values(
        &self,
        row: usize,
    ) -> Option<&DataFrame> {
        self.values
            .get(row)?
            .get_or_init(|| {
                self.drawer
                    .values(&self.frame, row)
                    .ok()
                    .flatten()
            })
            .as_ref()
    }
}

/// What is shown of a gallery: the value picked of an attribute, and a row of those left.
#[derive(Debug, Default)]
pub struct State {
    /// The place of the value picked, by the name of its attribute.
    picked: BTreeMap<String, usize>,
    row: usize,
    /// Whether it steps through the rows left by itself.
    pub is_played: bool,
    /// Whether the row is shown as a plot to zoom and pan, and not as its image.
    is_live: bool,
    /// The point of that plot under the cursor: where it is among the values of the row.
    hovered: Option<usize>,
}

/// The state of a gallery that nothing was asked of yet.
pub static UNTOUCHED: State = State {
    picked: BTreeMap::new(),
    row: 0,
    is_played: false,
    is_live: false,
    hovered: None,
};

#[derive(Debug, Clone)]
pub enum Message {
    /// Only the rows that have this value of this attribute are shown, or any if it is none.
    Pick(String, Option<usize>),
    /// This one of the rows that are left is shown.
    Row(usize),
    /// This row of the frame is shown, whatever was picked.
    Show(usize),
    /// It steps through the rows left by itself, or stops if it does.
    Play,
    /// The next of the rows left is shown, and after the last the first.
    Step,
    /// The image shown is to be written to a file.
    Save,
    /// The row is shown as a plot to zoom and pan, or as its image again if it is.
    Live,
    /// The point of the plot that the cursor is over, if it is over one.
    #[cfg_attr(not(feature = "plot"), allow(dead_code))]
    Hover(Option<usize>),
}

impl State {
    /// Does what `message` asks to what is shown of `gallery`.
    pub fn update(
        &mut self,
        message: Message,
        gallery: &Gallery,
    ) {
        // the point under the cursor is one of the row that was shown
        if !matches!(message, Message::Hover(_)) {
            self.hovered = None;
        }
        match message {
            Message::Row(row) => self.row = row,
            Message::Pick(name, value) => {
                match value {
                    Some(value) => self
                        .picked
                        .insert(name, value),
                    None => self
                        .picked
                        .remove(&name),
                };
                // other rows are left, of which the first is as good as any
                self.row = 0;
            }
            Message::Show(row) => {
                let place = |state: &State| {
                    gallery
                        .rows(state)
                        .iter()
                        .position(|left| *left == row)
                };
                // among all rows, if it is not one of those left
                if place(self).is_none() {
                    self.picked.clear();
                }
                self.row = place(self).unwrap_or_default();
            }
            Message::Play => self.is_played = !self.is_played,
            Message::Step => {
                let left = gallery
                    .rows(self)
                    .len()
                    .max(1);
                self.row = (self.row + 1) % left;
            }
            Message::Save => {}
            Message::Live => self.is_live = !self.is_live,
            Message::Hover(point) => self.hovered = point,
        }
    }
}

impl Gallery {
    /// The images of `column` of `frame`, if it holds any, with the plain columns beside it.
    pub fn of(
        frame: &DataFrame,
        column: &str,
    ) -> Self {
        let cells = frame
            .column(column)
            .and_then(|cells| cells.binary());
        let Ok(cells) = cells else {
            return Self::default();
        };
        let pictures: Vec<_> = cells
            .iter()
            .map(|cell| cell.and_then(Picture::of))
            .collect();
        if pictures
            .iter()
            .all(Option::is_none)
        {
            return Self::default();
        }
        // what an image was drawn of is no attribute of it, nor are the images
        let attributes = frame
            .columns()
            .iter()
            .filter(|column| {
                let dtype = column.dtype();
                !dtype.is_nested() && !dtype.is_binary()
            })
            .filter_map(Attribute::of)
            .collect();
        Gallery {
            column: column.to_owned(),
            pictures,
            attributes,
            #[cfg(feature = "plot")]
            live: None,
        }
    }

    /// It, with the rows of `frame` to show as the plots that `drawer` draws of them.
    #[cfg(feature = "plot")]
    pub fn live(
        mut self,
        frame: &DataFrame,
        drawer: Drawer,
    ) -> Self {
        self.live = Some(Live {
            drawer,
            // its columns are shared with the frame, and not copied
            frame: frame.clone(),
            values: (0..frame.height())
                .map(|_| OnceCell::new())
                .collect(),
        });
        self
    }

    /// The plot of `row` to zoom and pan, as wide as `width`, over what says the point hovered.
    ///
    /// The cards are drawn `zoom` times their size, which the plot draws its pixels for.
    #[cfg(feature = "plot")]
    fn plot<'a>(
        &'a self,
        state: &State,
        row: usize,
        width: f32,
        zoom: f32,
    ) -> Option<Element<'a, Message>> {
        let live = self.live.as_ref()?;
        let values = live.values(row)?;
        let figure = live
            .drawer
            .figure(values)
            .ok()?;
        let (wide, tall) = live.drawer.size;
        let plot = Plot::new(figure)
            .width(width)
            .height(width * (tall / wide) as f32)
            .magnification(zoom)
            .on_hover(|hover| Message::Hover(hover.map(|hover| hover.index)));
        // the values of the point, each under the name of its column
        let point = state
            .hovered
            .filter(|point| *point < values.height())
            .map(|point| {
                let says: Vec<String> = values
                    .columns()
                    .iter()
                    .filter_map(|column| {
                        let value = column
                            .get(point)
                            .ok()?;
                        Some(format!("{} = {value}", column.name()))
                    })
                    .collect();
                says.join(",  ")
            });
        let says = point.unwrap_or_else(|| "over a point, its values show here".to_owned());
        Some(
            column![
                Element::from(plot),
                text(says).size(size::NOTE)
            ]
            .spacing(GAP)
            .boxed(),
        )
    }

    /// Whether the rows can be shown as plots to zoom and pan.
    fn is_drawn(&self) -> bool {
        #[cfg(feature = "plot")]
        return self.live.is_some();
        #[cfg(not(feature = "plot"))]
        false
    }

    /// The value that is picked of an attribute, if it is one of its values.
    fn picked(
        state: &State,
        attribute: &Attribute,
    ) -> Option<usize> {
        state
            .picked
            .get(&attribute.name)
            .copied()
            .filter(|value| {
                *value
                    < attribute
                        .values
                        .len()
            })
    }

    /// The rows that have the value picked of every attribute of which one is picked.
    pub fn rows(
        &self,
        state: &State,
    ) -> Vec<usize> {
        (0..self.pictures.len())
            .filter(|row| {
                self.attributes
                    .iter()
                    .all(|attribute| {
                        Self::picked(state, attribute)
                            .is_none_or(|value| attribute.rows[*row] == value)
                    })
            })
            .collect()
    }

    /// The row whose image is shown, if any is left; a Run since may have left fewer rows.
    fn shown(
        &self,
        state: &State,
    ) -> Option<usize> {
        let rows = self.rows(state);
        let last = rows
            .len()
            .checked_sub(1)?;
        Some(rows[state.row.min(last)])
    }

    /// The image shown, as it lies in `frame`, and the extension of a file of its kind.
    pub fn export<'a>(
        &self,
        state: &State,
        frame: &'a DataFrame,
    ) -> Option<(&'a [u8], &'static str)> {
        let bytes = frame
            .column(&self.column)
            .ok()?
            .binary()
            .ok()?
            .get(self.shown(state)?)?;
        Some((bytes, kind(bytes)?))
    }

    /// The image of a row that is left, as wide as `width`.
    ///
    /// To browse, it is under the attributes and over the rows left, and a plot to zoom and pan
    /// if that is asked for, which draws for cards shown `zoom` times their size.
    pub fn view<'a>(
        &'a self,
        state: &State,
        width: f32,
        is_browsed: bool,
        zoom: f32,
        help: Help,
    ) -> Option<Element<'a, Message>> {
        if self
            .pictures
            .is_empty()
        {
            return None;
        }
        let picture = self
            .shown(state)
            .and_then(|row| self.pictures[row].as_ref())
            .map(|picture| picture.view(width));
        if !is_browsed {
            return picture;
        }
        #[cfg(feature = "plot")]
        let picture = self
            .shown(state)
            .filter(|_| state.is_live)
            .and_then(|row| self.plot(state, row, width, zoom))
            .or(picture);
        #[cfg(not(feature = "plot"))]
        let _ = zoom;

        let names = look::names(
            self.attributes
                .iter()
                .map(|attribute| {
                    attribute
                        .name
                        .as_str()
                }),
        );
        let attributes = self
            .attributes
            .iter()
            .map(|attribute| {
                let filter = self.filter(state, attribute);
                look::labelled(&attribute.name, names, None, filter)
            });
        let save = picture
            .is_some()
            .then(|| {
                help.of(
                    small("Save", Message::Save),
                    "Writes the image shown to a file.",
                )
            });
        let live = (self.is_drawn() && picture.is_some()).then(|| {
            let label = match state.is_live {
                true => "Image",
                false => "Live",
            };
            help.of(
                small(label, Message::Live),
                "Shows the row as a plot to zoom with the wheel and pan with a drag, or as its \
                 image again.",
            )
        });
        let left = self
            .rows(state)
            .len();
        let browse = match left {
            0 => row![text("no row has these values").size(size::NOTE)],
            1 => row![
                space::horizontal(),
                live,
                save
            ],
            _ => {
                let at = state
                    .row
                    .min(left - 1);
                let play = match state.is_played {
                    true => "Stop",
                    false => "Play",
                };
                row![
                    help.of(
                        small(play, Message::Play),
                        "Steps through the images of the rows that are left, five a second.",
                    ),
                    stepped(left, at, format!("{} of {left}", at + 1), Message::Row),
                    live,
                    save,
                ]
            }
        };
        Some(
            column![
                column(attributes).spacing(5),
                picture,
                browse
                    .spacing(GAP)
                    .align_y(Center),
            ]
            .spacing(GAP)
            .boxed(),
        )
    }

    /// What picks a value of `attribute`: a slider through numbers, and else a list; and a clear.
    fn filter<'a>(
        &'a self,
        state: &State,
        attribute: &'a Attribute,
    ) -> Element<'a, Message> {
        let picked = Self::picked(state, attribute);
        let values = &attribute.values;
        let pick = |value| {
            Message::Pick(
                attribute
                    .name
                    .clone(),
                value,
            )
        };
        let pick_one = move |value| pick(Some(value));
        let control = match attribute.is_numeric {
            true => {
                let value = picked.map_or("any", |value| values[value].as_str());
                let place = picked.unwrap_or_default();
                stepped(values.len(), place, value.to_owned(), pick_one)
            }
            false => {
                let places: Vec<usize> = (0..values.len()).collect();
                pick_list(picked, places, |place: &usize| values[*place].clone())
                    .on_select(pick_one)
                    .placeholder("any")
                    .width(Fill)
                    .stepped()
            }
        };
        let clear = picked.map(|_| small("×", pick(None)));
        row![control, clear]
            .spacing(5)
            .align_y(Center)
            .boxed()
    }
}

fn small<'a>(
    label: &'a str,
    message: Message,
) -> impl Widget<Message> + 'a {
    button(text(label))
        .padding([2, 6])
        .style(button::text)
        .on_press(message)
}

/// A slider through `len` places that is at `place`, beside what says where that is.
fn stepped<'a>(
    len: usize,
    place: usize,
    says: String,
    on_step: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let last = len.saturating_sub(1) as u32;
    row![
        slider(0..=last, place as u32, move |place| on_step(place as usize)),
        text(says).size(size::NOTE),
    ]
    .spacing(GAP)
    .align_y(Center)
    .boxed()
}

#[cfg(test)]
#[path = "../../tests/unit/gallery.rs"]
mod tests;
