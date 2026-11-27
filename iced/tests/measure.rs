//! Text as it is measured: where its wrapped lines are cut in two.

use iced::advanced::text::Span;
use polaroid_iced::theme::{font, size};
use polaroid_iced::{cut, width_of};

/// What `spans` say, one after another.
fn written(spans: &[Span<'_>]) -> String {
    spans
        .iter()
        .map(|span| &*span.text)
        .collect()
}

#[test]
fn spans_are_cut_where_a_line_of_them_ends_and_keep_their_look_on_both_sides() {
    let spans: [Span<'_>; 2] = [
        Span::new("one two "),
        Span::new("three four five").font(font::MONO),
    ];
    // as wide as two words of the code, which leaves the first two a line of their own
    let width = width_of("three four", size::TEXT, font::MONO) + 1.0;
    let cut = |lines| cut(&spans, size::TEXT, font::SANS, width, lines);

    let (first, rest) = cut(1);
    assert_eq!(written(&first), "one two ");
    assert_eq!(written(&rest), "three four five");
    // a span that the cut is in is on both sides of it
    let (first, rest) = cut(2);
    assert_eq!(written(&first), "one two three four ");
    assert_eq!(written(&rest), "five");
    assert_eq!(first[1].font, Some(font::MONO));
    assert_eq!(rest[0].font, Some(font::MONO));
    // with no more lines than asked for, nothing is left
    let (first, rest) = cut(3);
    assert_eq!(written(&first), "one two three four five");
    assert!(rest.is_empty());
}
