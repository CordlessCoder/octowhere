//! Records what the window shows to an animated GIF, encoded on a thread of its own so the
//! window keeps its pace.

use std::{
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use gif::{DisposalMethod, Encoder, Frame, Repeat};

use crate::{HEIGHT, WIDTH};

/// GIF delays count centiseconds, and browsers stretch any delay under two of them, so the
/// window is sampled every 20 ms.
const FRAME_US: u64 = 20_000;
const FRAME_CS: u16 = 2;
/// The most samples one frame's delay can hold.
const MAX_SAMPLES: u16 = u16::MAX / FRAME_CS;
/// NeuQuant's trade of quality for speed, used when a frame has over 256 colours.
const QUANTIZE_SPEED: i32 = 10;

pub struct Recording {
    next_sample: u64,
    /// The last frame taken, not yet sent, since its delay grows while the window stays the same.
    held: Vec<u32>,
    samples: u16,
    frames: Sender<(Vec<u32>, u16)>,
    encoder: JoinHandle<()>,
}

impl Recording {
    /// Starts recording, leaving the pixels in `knock_out` transparent in every frame.
    pub fn start(path: PathBuf, now: u64, pixels: &[u32], knock_out: Option<&[bool]>) -> Self {
        let (frames, received) = mpsc::channel();
        let knock_out = knock_out.map(<[bool]>::to_vec);
        Self {
            next_sample: now + FRAME_US,
            held: pixels.to_vec(),
            samples: 1,
            frames,
            encoder: thread::spawn(move || encode(path, knock_out, received)),
        }
    }

    /// Samples the window for every frame period that has passed by `now`.
    pub fn sample(&mut self, now: u64, pixels: &[u32]) {
        while now >= self.next_sample {
            self.next_sample += FRAME_US;
            if self.held == pixels && self.samples < MAX_SAMPLES {
                self.samples += 1;
            } else {
                let held = std::mem::replace(&mut self.held, pixels.to_vec());
                // A failed send means the encoder stopped, and it has said why.
                let _ = self.frames.send((held, self.samples));
                self.samples = 1;
            }
        }
    }

    /// Sends the last frame. The file is complete once the returned thread ends.
    pub fn finish(self) -> JoinHandle<()> {
        let _ = self.frames.send((self.held, self.samples));
        self.encoder
    }
}

fn encode(path: PathBuf, knock_out: Option<Vec<bool>>, frames: Receiver<(Vec<u32>, u16)>) {
    let written = (|| -> Result<(), gif::EncodingError> {
        let file = BufWriter::new(File::create(&path)?);
        let mut encoder = Encoder::new(file, WIDTH as u16, HEIGHT as u16, &[])?;
        encoder.set_repeat(Repeat::Infinite)?;
        let mut previous: Option<Vec<u32>> = None;
        for (pixels, samples) in frames {
            // Each frame after the first covers only what changed, over the frame before.
            let (left, top, right, bottom) = match &previous {
                None => (0, 0, WIDTH, HEIGHT),
                Some(previous) => changed(previous, &pixels).unwrap_or((0, 0, 1, 1)),
            };
            // A knocked-out pixel is transparent in every frame, so one covering an earlier frame
            // still shows nothing.
            let mut rgba: Vec<u8> = (top..bottom)
                .flat_map(|y| y * WIDTH + left..y * WIDTH + right)
                .flat_map(|index| {
                    let [_, r, g, b] = pixels[index].to_be_bytes();
                    let shown = knock_out.as_ref().is_none_or(|mask| mask[index]);
                    [r, g, b, if shown { 0xff } else { 0 }]
                })
                .collect();
            let mut frame = Frame::from_rgba_speed(
                (right - left) as u16,
                (bottom - top) as u16,
                &mut rgba,
                QUANTIZE_SPEED,
            );
            frame.left = left as u16;
            frame.top = top as u16;
            frame.delay = samples * FRAME_CS;
            frame.dispose = DisposalMethod::Keep;
            encoder.write_frame(&frame)?;
            previous = Some(pixels);
        }
        Ok(())
    })();
    match written {
        Ok(()) => println!("saved {}", path.display()),
        Err(error) => eprintln!("recording {}: {error}", path.display()),
    }
}

/// The bounds of the pixels that differ, as left, top, right and bottom, the last two exclusive.
fn changed(previous: &[u32], pixels: &[u32]) -> Option<(usize, usize, usize, usize)> {
    let differs = |y: usize| previous[y * WIDTH..][..WIDTH] != pixels[y * WIDTH..][..WIDTH];
    let top = (0..HEIGHT).find(|&y| differs(y))?;
    let bottom = (0..HEIGHT).rfind(|&y| differs(y))? + 1;
    let column_differs =
        |x: usize| (top..bottom).any(|y| previous[y * WIDTH + x] != pixels[y * WIDTH + x]);
    let left = (0..WIDTH).find(|&x| column_differs(x))?;
    let right = (0..WIDTH).rfind(|&x| column_differs(x))? + 1;
    Some((left, top, right, bottom))
}
