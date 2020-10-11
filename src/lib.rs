//! A simple, declarative, and modular TUI library.
#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![warn(clippy::pedantic, rust_2018_idioms)]
#![allow(
    clippy::module_name_repetitions,
    // `as u16` is used when we need to get the width of a string that is guaranteed not to exceed
    // u16.
    clippy::cast_possible_truncation,
    clippy::non_ascii_literal
)]

use std::fmt::{self, Display, Write as _};

pub use smartstring;
use unicode_width::UnicodeWidthChar;

pub mod backend;
pub mod buffer;
pub mod input;
pub mod style;
mod terminal;
mod vec2;

pub use backend::*;
pub use buffer::*;
pub use input::*;
pub use style::*;
pub use terminal::*;
pub use vec2::Vec2;

/// An element on the screen.
///
/// Elements are cheap, immutable, borrowed and short-lived. They are usually `Copy`.
pub trait Element<Event> {
    /// Draw the element to the output.
    fn draw(&self, output: &mut dyn Output);

    /// Get the ideal size that the element takes up on the screen given the size restrictions.
    ///
    /// Values will be capped at `maximum`; returning a greater value will have no effect.
    ///
    /// Don't just blindly return `maximum`, as some containers (e.g. scrolling containers) will
    /// pass in `Vec2 { x: u16::MAX, y: u16::MAX }` in `maximum`, and you will end up with a very
    /// small scrollbar. If your element is flexible return the smallest size possible.
    fn ideal_size(&self, maximum: Vec2<u16>) -> Vec2<u16>;

    /// React to the input and return an event if necessary.
    fn handle(&self, input: Input) -> Option<Event>;
}

impl<'a, E: Element<Event>, Event> Element<Event> for &'a E {
    fn draw(&self, output: &mut dyn Output) {
        (*self).draw(output)
    }
    fn ideal_size(&self, maximum: Vec2<u16>) -> Vec2<u16> {
        (*self).ideal_size(maximum)
    }
    fn handle(&self, input: Input) -> Option<Event> {
        (*self).handle(input)
    }
}

/// An output to which elements draw themselves.
pub trait Output {
    /// Get the size of the output.
    ///
    /// Attempting to draw content outside the size will silently fail.
    fn size(&self) -> Vec2<u16>;

    /// Write a single character to the output at a zero-indexed position.
    ///
    /// Failures are intentionally ignored and not detectable - the `Output`'s current state is
    /// completely opaque.
    ///
    /// - Drawing a control character will fail.
    /// - Drawing a character out of bounds will fail.
    /// - Drawing a double-width character to the last column of the screen will fail.
    /// - Drawing a zero-width character on top of an existing character will add it to it, ignoring
    /// the zero-width character's style.
    /// - Drawing a zero-width character on the second column of a double-width character will fail.
    /// - Drawing a single-width or double-width character on top of a single-width character will
    /// completely replace it.
    /// - Drawing a single-width or double-width character to either column of a double-width
    /// character will completely replace the columns drawn to, and any other columns previously
    /// occupied by the double-width character will retain the background color of the double-width
    /// character.
    fn write_char(&mut self, pos: Vec2<u16>, c: char, style: Style);

    /// Write a type implementing `Display` to the output at a zero-indexed position.
    ///
    /// If the string overflows the width of the terminal it will be cut off. Control characters
    /// will be ignored.
    ///
    /// # Panics
    ///
    /// This method will panic if the value's implementation of `Display` returns an error, which
    /// most implementations won't do.
    fn write(&mut self, pos: Vec2<u16>, value: &dyn Display, style: Style) {
        struct WriteToOutput<'a, O: ?Sized> {
            pos: Vec2<u16>,
            style: Style,
            output: &'a mut O,
        }
        impl<'a, O: Output + ?Sized> fmt::Write for WriteToOutput<'a, O> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                for c in s.chars() {
                    let width = match c.width() {
                        Some(width) => width,
                        None => continue,
                    } as u16;

                    self.output.write_char(self.pos, c, self.style);

                    self.pos.x += width;
                }

                Ok(())
            }
        }

        write!(
            &mut WriteToOutput {
                pos,
                style,
                output: self,
            },
            "{}",
            value
        )
        .expect("formatting failed");
    }

    /// Clear the output with one color.
    fn clear(&mut self, color: Color) {
        let size = self.size();
        let style = Style::new(Color::default(), color, Attributes::default());

        for x in 0..size.x {
            for y in 0..size.y {
                self.write_char(Vec2 { x, y }, ' ', style);
            }
        }
    }

    /// Set the title of the output.
    fn set_title(&mut self, title: &dyn Display);

    /// Set the cursor of the output, if there is one.
    fn set_cursor(&mut self, cursor: Option<Cursor>);
}

impl<'a, O: Output> Output for &'a mut O {
    fn size(&self) -> Vec2<u16> {
        (**self).size()
    }
    fn write_char(&mut self, pos: Vec2<u16>, c: char, style: Style) {
        (**self).write_char(pos, c, style)
    }
    fn set_title(&mut self, title: &dyn Display) {
        (**self).set_title(title)
    }
    fn set_cursor(&mut self, cursor: Option<Cursor>) {
        (**self).set_cursor(cursor)
    }
}

/// A terminal cursor.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Cursor {
    /// The shape of the cursor.
    pub shape: CursorShape,
    /// Whether the cursor blinks.
    pub blinking: bool,
    /// The zero-indexed position of the cursor.
    pub pos: Vec2<u16>,
}

/// The shape of a cursor.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum CursorShape {
    /// A bar to the left of the character.
    Bar,
    /// A full block over the character.
    Block,
    /// An underline under the character.
    Underline,
}
