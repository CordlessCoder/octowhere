use embedded_graphics::{Pixel, prelude::*};
use octowhere_ui::chrome::{self, CoverageTarget, FontdueRenderer};

const SIZE: i32 = 466;

/// Records the most coverage drawn on each pixel, so a test sees the bytes a drawing produces
/// rather than colours rounded to the framebuffer's format.
struct Coverage(Vec<u8>);

impl Coverage {
    fn new() -> Self {
        Self(vec![0; (SIZE * SIZE) as usize])
    }

    fn at(&self, point: Point) -> u8 {
        let inside = (0..SIZE).contains(&point.x) && (0..SIZE).contains(&point.y);
        if inside { self.0[(point.y * SIZE + point.x) as usize] } else { 0 }
    }
}

impl OriginDimensions for Coverage {
    fn size(&self) -> Size {
        Size::new_equal(SIZE as u32)
    }
}

impl DrawTarget for Coverage {
    type Color = chrome::Color;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Self::Color>>>(&mut self, _: I) -> Result<(), Self::Error> {
        unreachable!("text draws through blend_row")
    }
}

impl CoverageTarget for Coverage {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], _: Self::Color) {
        for (x, &covered) in (x..).zip(coverage) {
            if (0..SIZE).contains(&x) && (0..SIZE).contains(&y) {
                let at = &mut self.0[(y * SIZE + x) as usize];
                *at = (*at).max(covered);
            }
        }
    }
}

fn renderer(size: u32, font: usize) -> FontdueRenderer<'static, chrome::Color> {
    let mut renderer = FontdueRenderer::new(chrome::FontdueRendererCtx::new_rc(), size, chrome::WHITE, chrome::FONTS);
    renderer.font_index = font;
    renderer
}

#[test]
fn an_outline_rings_the_ink_without_covering_it() {
    const ORIGIN: Point = Point::new(40, 300);
    for (size, font) in [(40, chrome::SHAPIRO), (136, chrome::FRAKTION_BOLD), (16, chrome::FRAKTION)] {
        let style = renderer(size, font);
        let mut glyphs = Coverage::new();
        style.draw_on_baseline("O4E", ORIGIN, &mut glyphs).unwrap();
        for radius in [1u8, 2] {
            let mut outline = Coverage::new();
            style.draw_outline_on_baseline("O4E", ORIGIN, radius, &mut outline).unwrap();
            let reach = i32::from(radius);
            let mut ring = 0;
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let point = Point::new(x, y);
                    let covered = outline.at(point);
                    if glyphs.at(point) == u8::MAX {
                        assert_eq!(covered, 0, "{size} px, radius {radius}: ring over solid ink at {point}");
                    }
                    if covered > 0 {
                        ring += 1;
                        let near = (-reach..=reach)
                            .any(|dy| (-reach..=reach).any(|dx| glyphs.at(point + Point::new(dx, dy)) > 0));
                        assert!(near, "{size} px, radius {radius}: ring farther than {radius} px from ink at {point}");
                    }
                }
            }
            assert!(ring > 0, "{size} px, radius {radius}: no ring");
        }
    }
}
