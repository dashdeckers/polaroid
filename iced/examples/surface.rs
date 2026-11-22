//! Cards to drag, type in, pick from, raise and delete on a surface to pan and zoom.

use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Point, Widget};
use polaroid_iced::surface;

const KINDS: [&str; 3] = ["filter", "select", "join"];

pub fn main() -> iced::Result {
    iced::run(Example::update, Example::view)
}

struct Example {
    cards: Vec<Card>,
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
        }
    }
}

impl Example {
    fn update(
        &mut self,
        message: Message,
    ) {
        let (Message::Moved(id, _)
        | Message::Named(id, _)
        | Message::Picked(id, _)
        | Message::Raised(id)
        | Message::Deleted(id)) = message;
        let Some(index) = self
            .cards
            .iter()
            .position(|card| card.id == id)
        else {
            return;
        };
        let mut card = self
            .cards
            .remove(index);
        match message {
            Message::Moved(_, at) => card.at = at,
            Message::Named(_, name) => card.name = name,
            Message::Picked(_, kind) => card.kind = Some(kind),
            Message::Raised(_) => {
                self.cards
                    .push(card);
                return;
            }
            Message::Deleted(_) => return,
        }
        self.cards
            .insert(index, card);
    }

    fn view(&self) -> impl Widget<Message> {
        let mut cards = surface().on_move(Message::Moved);
        for card in &self.cards {
            let id = card.id;
            let content = column![
                text!("card {id}"),
                text_input("name", &card.name).on_input(move |name| Message::Named(id, name)),
                pick_list(card.kind, KINDS, |kind| kind.to_string())
                    .on_select(move |kind| Message::Picked(id, kind))
                    .placeholder("kind"),
                row![
                    button("raise").on_press(Message::Raised(id)),
                    button("delete").on_press(Message::Deleted(id)),
                ]
                .spacing(8),
            ]
            .spacing(8);
            let content = container(content)
                .width(220)
                .padding(12)
                .style(container::bordered_box);
            let on_surface = polaroid_iced::card(id, card.at, content)
                .inputs([24.0])
                .output(24.0);
            cards = cards
                .push(on_surface)
                .wire(id, id + 1, 0);
        }
        cards
    }
}
