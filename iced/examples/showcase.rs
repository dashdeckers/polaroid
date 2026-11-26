//! The widgets together: cards on a surface beside a list of them, and a menu to add more.

use iced::advanced::widget::Id;
use iced::widget::{button, column, container, row, stack, text, text_input};
use iced::{Fill, Point, Widget};
use polaroid_iced::theme::{self, Tone, extent, spacing};
use polaroid_iced::{menu, pick_list, split, surface};

const KINDS: [&str; 3] = ["filter", "select", "join"];

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .settings(theme::settings())
        .theme(theme::light())
        .run()
}

struct Example {
    cards: Vec<Card>,
    /// The id of the next card.
    next: u32,
    /// Where the menu is open: on the surface, and among the cards.
    menu: Option<(Point, Point)>,
}

struct Card {
    id: u32,
    at: Point,
    name: String,
    kind: Option<&'static str>,
}

#[derive(Debug, Clone)]
enum Message {
    Moved(u32, Point),
    Named(u32, String),
    Picked(u32, &'static str),
    Raised(u32),
    Deleted(u32),
    Menu(Point, Point),
    CloseMenu,
    /// A card of this kind, at this point among the cards.
    Added(&'static str, Point),
    Cleared,
}

impl Default for Example {
    fn default() -> Self {
        let card = |id: u32| Card {
            id,
            at: Point::new(40.0 + 120.0 * id as f32, 40.0 + 70.0 * id as f32),
            name: String::new(),
            kind: None,
        };
        Self {
            cards: (0..4)
                .map(card)
                .collect(),
            next: 4,
            menu: None,
        }
    }
}

impl Example {
    fn update(
        &mut self,
        message: Message,
    ) {
        self.menu = None;
        match message {
            Message::Moved(id, at) => self.change(id, |card| card.at = at),
            Message::Named(id, name) => self.change(id, |card| card.name = name),
            Message::Picked(id, kind) => self.change(id, |card| card.kind = Some(kind)),
            // the last card is drawn on top
            Message::Raised(id) => self
                .cards
                .sort_by_key(|card| card.id == id),
            Message::Deleted(id) => self
                .cards
                .retain(|card| card.id != id),
            Message::Menu(at, point) => self.menu = Some((at, point)),
            Message::CloseMenu => {}
            Message::Added(kind, at) => {
                self.cards
                    .push(Card {
                        id: self.next,
                        at,
                        name: String::new(),
                        kind: Some(kind),
                    });
                self.next += 1;
            }
            Message::Cleared => self.cards.clear(),
        }
    }

    fn change(
        &mut self,
        id: u32,
        change: impl FnOnce(&mut Card),
    ) {
        let card = self
            .cards
            .iter_mut()
            .find(|card| card.id == id);
        if let Some(card) = card {
            change(card);
        }
    }

    fn view(&self) -> impl Widget<Message> {
        // a right press beside the cards asks for the menu, which adds a card there
        let mut cards = surface()
            .on_move(Message::Moved)
            .on_menu(Message::Menu);
        for card in &self.cards {
            let id = card.id;
            let content = column![
                // the port of its input is level with its title
                container(text!("card {id}")).id("title"),
                text_input("name", &card.name)
                    .on_input(move |name| Message::Named(id, name))
                    .padding(theme::FIELD)
                    .style(theme::text_input::toned(Tone::Given)),
                pick_list(card.kind, KINDS.to_vec(), |kind: &&str| kind.to_string())
                    .on_select(move |kind| Message::Picked(id, kind))
                    .placeholder("kind"),
                row![
                    button("raise")
                        .style(theme::button::tonal)
                        .on_press(Message::Raised(id)),
                    button("delete")
                        .style(theme::button::text)
                        .on_press(Message::Deleted(id)),
                ]
                .spacing(spacing::S),
            ]
            .spacing(spacing::S);
            let content = container(content)
                .width(extent::M)
                .padding(spacing::M)
                .style(theme::container::card);
            let on_surface = polaroid_iced::card(id, card.at, content)
                .ports([Id::new("title")])
                .output(24.0);
            cards = cards
                .push(on_surface)
                .wire(id, id + 1, 0);
        }
        let menu = self
            .menu
            .map(|(at, point)| {
                let kinds = KINDS
                    .iter()
                    .map(|kind| (kind.to_string(), Message::Added(kind, point)))
                    .collect();
                let all = vec![("delete".to_owned(), Message::Cleared)];
                let groups = vec![
                    ("New card".to_owned(), kinds),
                    ("All cards".to_owned(), all),
                ];
                menu(at, groups).on_close(Message::CloseMenu)
            });

        // the cards by name, from the bottom one up, beside the surface they lie on
        let names = self
            .cards
            .iter()
            .map(|card| text!("card {}: {}", card.id, card.name).boxed());
        let list = container(column(names).spacing(spacing::XS))
            .padding(spacing::M)
            .width(Fill);
        split(list, stack![cards, menu]).width(extent::M)
    }
}
