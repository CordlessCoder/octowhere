//! Records what the window shows to an animated GIF, or to an MP4 through `ffmpeg`, encoded on a
//! thread of its own so the window keeps its pace.

use std::{
    error::Error,
    fs::File,
    io::{BufWriter, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use gif::{DisposalMethod, Encoder, Frame, Repeat};

use crate::HEIGHT;

/// GIF delays count centiseconds, and browsers stretch any delay under two of them, so the
/// window is sampled every 20 ms. An MP4 runs at the same rate.
const FRAME_US: u64 = 20_000;
const FRAMES_PER_SECOND: u64 = 1_000_000 / FRAME_US;
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

/// What a recording is saved as, from its path's extension.
#[derive(Clone, Copy)]
pub enum Format {
    Gif,
    Mp4,
}

impl Format {
    pub fn of(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "gif" => Some(Self::Gif),
            "mp4" => Some(Self::Mp4),
            _ => None,
        }
    }
}

impl Recording {
    /// Starts recording. A GIF leaves the pixels in `knock_out` transparent in every frame; an
    /// MP4 has no transparency and keeps them as they show.
    ///
    /// # Panics
    ///
    /// If `path` ends in neither `.gif` nor `.mp4`.
    pub fn start(path: PathBuf, now: u64, width: usize, pixels: &[u32], knock_out: Option<&[bool]>) -> Self {
        let format = Format::of(&path)
            .unwrap_or_else(|| panic!("{} is neither a .gif nor an .mp4", path.display()));
        let (frames, received) = mpsc::channel();
        let knock_out = knock_out.map(<[bool]>::to_vec);
        Self {
            next_sample: now + FRAME_US,
            held: pixels.to_vec(),
            samples: 1,
            frames,
            encoder: thread::spawn(move || {
                let written = match format {
                    Format::Gif => encode_gif(&path, width, knock_out, received),
                    Format::Mp4 => encode_mp4(&path, width, received),
                };
                match written {
                    Ok(()) => println!("saved {}", path.display()),
                    Err(error) => eprintln!("recording {}: {error}", path.display()),
                }
            }),
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

fn encode_gif(
    path: &Path,
    width: usize,
    knock_out: Option<Vec<bool>>,
    frames: Receiver<(Vec<u32>, u16)>,
) -> Result<(), Box<dyn Error>> {
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = Encoder::new(file, width as u16, HEIGHT as u16, &[])?;
    encoder.set_repeat(Repeat::Infinite)?;
    let mut previous: Option<Vec<u32>> = None;
    for (pixels, samples) in frames {
        // Each frame after the first covers only what changed, over the frame before.
        let (left, top, right, bottom) = match &previous {
            None => (0, 0, width, HEIGHT),
            Some(previous) => changed(width, previous, &pixels).unwrap_or((0, 0, 1, 1)),
        };
        // A knocked-out pixel is transparent in every frame, so one covering an earlier frame
        // still shows nothing.
        let mut rgba: Vec<u8> = (top..bottom)
            .flat_map(|y| y * width + left..y * width + right)
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
}

/// Streams a raw frame per frame period to `ffmpeg`, which encodes H.264.
fn encode_mp4(path: &Path, width: usize, frames: Receiver<(Vec<u32>, u16)>) -> Result<(), Box<dyn Error>> {
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24"])
        .args(["-video_size", &format!("{width}x{HEIGHT}")])
        .args(["-framerate", &FRAMES_PER_SECOND.to_string(), "-i", "-"])
        .args(["-c:v", "libx264", "-tune", "animation", "-crf", "18"])
        // 4:2:0 with the index at the front is what browsers and players all take.
        .args(["-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| format!("starting ffmpeg: {error}"))?;
    let mut input = BufWriter::new(ffmpeg.stdin.take().expect("ffmpeg's stdin is piped"));
    for (pixels, samples) in frames {
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|pixel| {
                let [_, r, g, b] = pixel.to_be_bytes();
                [r, g, b]
            })
            .collect();
        for _ in 0..samples {
            input.write_all(&rgb)?;
        }
    }
    drop(input);
    let status = ffmpeg.wait()?;
    if !status.success() {
        return Err(format!("ffmpeg {status}").into());
    }
    Ok(())
}

/// The bounds of the pixels that differ, as left, top, right and bottom, the last two exclusive.
fn changed(width: usize, previous: &[u32], pixels: &[u32]) -> Option<(usize, usize, usize, usize)> {
    let differs = |y: usize| previous[y * width..][..width] != pixels[y * width..][..width];
    let top = (0..HEIGHT).find(|&y| differs(y))?;
    let bottom = (0..HEIGHT).rfind(|&y| differs(y))? + 1;
    let column_differs =
        |x: usize| (top..bottom).any(|y| previous[y * width + x] != pixels[y * width + x]);
    let left = (0..width).find(|&x| column_differs(x))?;
    let right = (0..width).rfind(|&x| column_differs(x))? + 1;
    Some((left, top, right, bottom))
}
