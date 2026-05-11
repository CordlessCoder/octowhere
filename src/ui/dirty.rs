use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use esp_println::dbg;

// TODO: Coalesce full grid entries during iteration

#[derive(Clone, Debug)]
pub struct DirtyAreas<
    const WIDTH: usize,
    const HEIGHT: usize,
    const CELLS_X: usize,
    const CELLS_Y: usize,
    const N: usize,
> {
    grid: [Rectangle; N],
    full: bool,
}

#[inline]
fn bounding_box(a: &Rectangle, b: &Rectangle) -> Rectangle {
    // dbg!(a, b);
    let Some(bottom_right_a) = a.bottom_right() else {
        return *b;
    };
    let Some(bottom_right_b) = b.bottom_right() else {
        return *a;
    };
    let top_left = a.top_left.component_min(b.top_left);
    let bottom_right = bottom_right_b.component_max(bottom_right_a);
    let size = Size::new(
        (bottom_right.x - top_left.x + 1) as u32,
        (bottom_right.y - top_left.y + 1) as u32,
    );
    Rectangle::new(top_left, size)
}

impl<
    const WIDTH: usize,
    const HEIGHT: usize,
    const CELLS_X: usize,
    const CELLS_Y: usize,
    const N: usize,
> DirtyAreas<WIDTH, HEIGHT, CELLS_X, CELLS_Y, N>
{
    const CELL_WIDTH: u32 = (WIDTH / CELLS_X) as u32;
    const CELL_HEIGHT: u32 = (HEIGHT / CELLS_Y) as u32;
    const _CHECK_CELL: () = assert!(Self::CELL_WIDTH != 0 && Self::CELL_HEIGHT != 0);

    /// Static assertion that N is correct.
    // MSRV: remove N when constant generic expressions are stabilized
    const _CHECK_N: () = assert!(
        N == CELLS_X * CELLS_Y,
        "Invalid N: it must be equal to CELLS_HEIGHT * CELLS_WIDTH"
    );

    #[must_use]
    pub const fn new() -> Self {
        Self {
            full: false,
            grid: [Rectangle::zero(); N],
        }
    }

    #[must_use]
    pub const fn new_full() -> Self {
        Self {
            full: true,
            grid: [Rectangle::zero(); N],
        }
    }

    pub fn clear(&mut self) {
        self.grid
            .iter_mut()
            .for_each(|rect| *rect = Rectangle::zero());
        self.full = false;
    }

    fn cell(x: u32, y: u32) -> Rectangle {
        let width = if x == const { CELLS_X as u32 - 1 } {
            Self::CELL_WIDTH + WIDTH as u32 % CELLS_X as u32
        } else {
            Self::CELL_WIDTH
        };
        let height = if y == const { CELLS_Y as u32 - 1 } {
            Self::CELL_HEIGHT + HEIGHT as u32 % CELLS_Y as u32
        } else {
            Self::CELL_HEIGHT
        };
        Rectangle::new(
            Point::new(
                (x * Self::CELL_WIDTH) as i32,
                (y * Self::CELL_HEIGHT) as i32,
            ),
            Size { width, height },
        )
    }

    #[inline]
    fn get_mut(&mut self, x: u32, y: u32) -> &mut Rectangle {
        assert!(x < CELLS_X as u32 && y < CELLS_Y as u32);
        &mut self.grid[(y * CELLS_Y as u32 + x) as usize]
    }

    #[inline]
    fn get(&self, x: u32, y: u32) -> Rectangle {
        assert!(x < CELLS_X as u32 && y < CELLS_Y as u32);
        self.grid[(y * CELLS_Y as u32 + x) as usize]
    }

    #[inline]
    fn is_rect_full(rect: &Rectangle) -> bool {
        rect.size.width == Self::CELL_WIDTH && rect.size.height == Self::CELL_HEIGHT
    }

    pub fn add(&mut self, rect: Rectangle) {
        let mut log = false;

        // if rect.top_left.x < 0 || rect.top_left.y < 0 {
        //     log = true;
        // }
        if log {
            esp_println::println!("Adding {rect:?} to {self:?}");
        }
        if self.is_full() {
            return;
        }
        let Some(bottom_right) = rect.bottom_right() else {
            return;
        };
        if bottom_right.x <= 0 || bottom_right.y <= 0 {
            return;
        }
        let x_start = rect.top_left.x.max(0) as u32 / Self::CELL_WIDTH;
        let y_start = rect.top_left.y.max(0) as u32 / Self::CELL_HEIGHT;
        let x_end = bottom_right.x as u32 / Self::CELL_WIDTH;
        let y_end = bottom_right.y as u32 / Self::CELL_HEIGHT;

        let x_range = x_start..=x_end.min(CELLS_X as u32 - 1);
        let y_range = y_start..=y_end.min(CELLS_Y as u32 - 1);
        if log {
            dbg!(&x_range, &y_range);
        }
        for x in x_range {
            for y in y_range.clone() {
                let current = self.get_mut(x, y);
                // dbg!(x, y);
                let cell_box = Self::cell(x, y);

                let within_cell = rect.intersection(&cell_box);
                if log {
                    dbg!(x, y, cell_box, within_cell);
                }
                *current = bounding_box(&within_cell, current);
            }
        }
        if log {
            esp_println::println!("After: {self:?}");
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Rectangle> {
        self.grid
            .iter()
            .copied()
            .filter(|rect| !rect.is_zero_sized())
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.full
    }

    pub fn make_full(&mut self) {
        self.full = true
    }

    pub fn extend(&mut self, other: &Self) {
        self.full = self.full || other.full;
        if self.full {
            return;
        }
        self.grid
            .iter_mut()
            .zip(other.grid)
            .for_each(|(rect, other)| *rect = bounding_box(rect, &other));
    }
}

impl<
    const WIDTH: usize,
    const HEIGHT: usize,
    const CELLS_X: usize,
    const CELLS_Y: usize,
    const N: usize,
> Default for DirtyAreas<WIDTH, HEIGHT, CELLS_X, CELLS_Y, N>
{
    fn default() -> Self {
        Self::new()
    }
}
