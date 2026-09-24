//! The status symbols the screens share: five rows of five modules, drawn outlined at any
//! module size.

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::chrome::{self, Color};

/// Five rows of five modules, bit 4 the leftmost.
pub type Glyph = [u8; 5];

pub const NO_DATA: Glyph = [0b11100, 0b11000, 0b00100, 0b00011, 0b00111];

/// Where an icon sits and how large its modules are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tile {
    pub corner: Point,
    pub module: i32,
    /// From the tile's edge to the first module.
    pub padding: i32,
}

impl Tile {
    #[must_use]
    pub const fn side(self) -> i32 {
        2 * self.padding + 5 * self.module
    }

    /// A quarter of the module, rounded half up, and never below 2 px.
    #[must_use]
    pub const fn frame(self) -> i32 {
        let frame = (self.module + 2) / 4;
        if frame < 2 { 2 } else { frame }
    }

    #[must_use]
    pub const fn bounds(self) -> Rectangle {
        Rectangle::new(self.corner, Size::new(self.side() as u32, self.side() as u32))
    }

    /// The modules of rows `from..to`.
    #[must_use]
    pub fn rows(self, from: u8, to: u8) -> Rectangle {
        Rectangle::new(
            self.corner + Point::new(self.padding, self.padding + i32::from(from) * self.module),
            Size::new(
                (5 * self.module) as u32,
                (i32::from(to.saturating_sub(from)) * self.module) as u32,
            ),
        )
    }

    /// A frame in `color` around black, with the first `rows` rows of `glyph`'s modules in
    /// `color`.
    pub fn draw<D: DrawTarget<Color = Color>>(
        self,
        glyph: &Glyph,
        color: Color,
        rows: u8,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let frame = self.frame();
        target.fill_solid(&self.bounds(), color)?;
        let inside = self.side() - 2 * frame;
        target.fill_solid(
            &Rectangle::new(
                self.corner + Point::new_equal(frame),
                Size::new_equal(inside as u32),
            ),
            chrome::BLACK,
        )?;
        for (row, bits) in glyph.iter().enumerate().take(usize::from(rows)) {
            for column in (0..5).filter(|column| bits & (0b10000 >> column) != 0) {
                let corner = self.corner
                    + Point::new(
                        self.padding + column * self.module,
                        self.padding + row as i32 * self.module,
                    );
                target.fill_solid(
                    &Rectangle::new(corner, Size::new_equal(self.module as u32)),
                    color,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_is_a_quarter_module_and_at_least_two_pixels() {
        let frame = |module| Tile { corner: Point::zero(), module, padding: 0 }.frame();
        assert_eq!([5, 8, 10, 16].map(frame), [2, 2, 3, 4]);
    }
}
