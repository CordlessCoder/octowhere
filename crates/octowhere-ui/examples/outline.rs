//! Renders sample sheets of outlined text, the hollow ring and a halo at radius 1 and 2, in the
//! faces and sizes the screens use. They show the primitive, not a screen. From the repository
//! root:
//!
//! ```text
//! cargo +stable run --manifest-path crates/octowhere-ui/Cargo.toml \
//!   --target x86_64-unknown-linux-gnu --example outline -- [out-dir]
//! ```
//!
//! The output directory defaults to `target/renders` under the crate.

use std::{fs::File, io::BufWriter, path::PathBuf};

use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use octowhere_ui::{
    chrome::{self, Color, FB, FontdueRenderer},
    ui::screens,
};

struct Sample {
    name: &'static str,
    text: &'static str,
    size: u32,
    font: usize,
    /// Each variant's pen on the baseline: hollow at radius 1 and 2, then the halos.
    pens: [Point; 4],
}

const SAMPLES: [Sample; 3] = [
    Sample {
        name: "shapiro-40",
        text: "OCTO",
        size: 40,
        font: chrome::SHAPIRO,
        pens: [Point::new(70, 150), Point::new(250, 150), Point::new(70, 320), Point::new(250, 320)],
    },
    Sample {
        name: "fraktion-bold-136",
        text: "48",
        size: 136,
        font: chrome::FRAKTION_BOLD,
        pens: [Point::new(60, 200), Point::new(245, 200), Point::new(60, 370), Point::new(245, 370)],
    },
    Sample {
        name: "fraktion-16",
        text: "WED 25 SEP",
        size: 16,
        font: chrome::FRAKTION,
        pens: [Point::new(80, 150), Point::new(260, 150), Point::new(80, 320), Point::new(260, 320)],
    },
];

const LABELS: [&str; 4] = ["HOLLOW, 1 PX", "HOLLOW, 2 PX", "HALO, 1 PX", "HALO, 2 PX"];

fn main() {
    let out = std::env::args_os().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/renders"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("creating the output directory");
    let base = FontdueRenderer::new(chrome::FontdueRendererCtx::new_rc(), 12, chrome::GRAY, chrome::FONTS);
    for sample in &SAMPLES {
        let mut fb = FB::boxed();
        screens::clear(&mut *fb).unwrap();
        let mut style = base.clone();
        (style.font_size, style.font_index) = (sample.size, sample.font);
        let mut label = base.clone();
        label.font_index = chrome::FRAKTION;
        for (i, (&pen, text)) in sample.pens.iter().zip(LABELS).enumerate() {
            let radius = if i % 2 == 0 { 1 } else { 2 };
            if i < 2 {
                style.text_color = chrome::WHITE;
                style.draw_outline_on_baseline(sample.text, pen, radius, &mut *fb).unwrap();
            } else {
                style.text_color = chrome::ORANGE;
                style.draw_outline_on_baseline(sample.text, pen, radius, &mut *fb).unwrap();
                style.text_color = chrome::WHITE;
                style.draw_on_baseline(sample.text, pen, &mut *fb).unwrap();
            }
            label.draw_on_baseline(text, pen + Point::new(0, 30), &mut *fb).unwrap();
        }
        label.draw_on_baseline("OUTLINE SAMPLES", Point::new(175, 60), &mut *fb).unwrap();
        write_png(&fb, &out.join(format!("outline-{}.png", sample.name)));
    }
}

fn write_png(fb: &FB, path: &std::path::Path) {
    let pixels: Vec<u8> = (0..466)
        .flat_map(|y| (0..466).map(move |x| Point::new(x, y)))
        .flat_map(|point| {
            let color = Rgb888::from(fb.pixel(point).unwrap_or(Color::BLACK));
            [color.r(), color.g(), color.b()]
        })
        .collect();
    let file = BufWriter::new(File::create(path).expect("creating the PNG"));
    let mut encoder = png::Encoder::new(file, 466, 466);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("writing the PNG header");
    writer.write_image_data(&pixels).expect("writing the PNG");
    println!("{}", path.display());
}
