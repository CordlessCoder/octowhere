use embedded_graphics_core::{geometry::Size, primitives::Rectangle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillRegion {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub source_x: usize,
    pub source_y: usize,
}

/// Return the visible part of a signed drawing rectangle and its source offset.
pub const fn clipped_fill_region(
    screen_width: usize,
    screen_height: usize,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> Option<FillRegion> {
    if width == 0 || height == 0 {
        return None;
    }

    let left = left as i64;
    let top = top as i64;
    let right = left + width as i64;
    let bottom = top + height as i64;
    let visible_left = if left < 0 { 0 } else { left };
    let visible_top = if top < 0 { 0 } else { top };
    let visible_right = if right > screen_width as i64 {
        screen_width as i64
    } else {
        right
    };
    let visible_bottom = if bottom > screen_height as i64 {
        screen_height as i64
    } else {
        bottom
    };

    if visible_left >= visible_right || visible_top >= visible_bottom {
        return None;
    }

    Some(FillRegion {
        x: visible_left as usize,
        y: visible_top as usize,
        width: (visible_right - visible_left) as usize,
        height: (visible_bottom - visible_top) as usize,
        source_x: (visible_left - left) as usize,
        source_y: (visible_top - top) as usize,
    })
}

/// Consume one input item for every source pixel and emit only visible pixels.
pub fn for_each_visible_color<I, T, F>(screen: Size, area: Rectangle, colors: I, mut draw: F)
where
    I: IntoIterator<Item = T>,
    F: FnMut(usize, usize, T),
{
    let Some(region) = clipped_fill_region(
        screen.width as usize,
        screen.height as usize,
        area.top_left.x,
        area.top_left.y,
        area.size.width,
        area.size.height,
    ) else {
        return;
    };

    let mut colors = colors.into_iter();
    for source_y in 0..area.size.height as usize {
        for source_x in 0..area.size.width as usize {
            let Some(color) = colors.next() else {
                return;
            };
            if source_y >= region.source_y
                && source_y < region.source_y + region.height
                && source_x >= region.source_x
                && source_x < region.source_x + region.width
            {
                draw(
                    region.x + source_x - region.source_x,
                    region.y + source_y - region.source_y,
                    color,
                );
            }
        }
    }
}
