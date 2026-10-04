//! Runs the mesh's node, `octowhere_node`, on this board: its radio, clock and random source,
//! the device around it and the store it keeps its group in, and the task that stands in for it
//! when the radio does not answer.

mod device;
mod radio;
mod random;
mod time;

pub use octowhere_node::{Fix, Mesh, Start};

pub use self::device::{
    BoardCommands, BoardDevice, BoardGroupStore, FIX, RTC_TIME, VIEW_CHANGED, lend_messages,
    quality, request, view_since,
};
pub use self::radio::BoardRadio;
pub use self::random::{BoardRandom, random};
pub use self::time::BoardTime;
use self::time::local;

/// Publishes the stored identity and group before the radio is known, for the screens to show
/// from the start.
pub fn publish_start(start: &Start) {
    octowhere_node::publish_start(start, local(), &BoardDevice);
}

/// Stands in for the mesh on a board whose radio did not answer: it keeps this device's name and
/// can leave the group, and tells the screens there is no radio to pair with.
#[embassy_executor::task]
pub async fn offline(start: Start) {
    octowhere_node::offline(
        start,
        &BoardTime,
        &BoardDevice,
        &BoardGroupStore,
        &BoardCommands,
    )
    .await
}

/// Lets a debugger give the mesh commands in place of the screens; `tools/pair-inject.py`
/// does.
#[cfg(feature = "pair-inject")]
pub mod inject {
    use core::sync::atomic::{AtomicU32, Ordering};

    use embassy_time::{Duration, Instant, Timer};
    use octowhere_mesh::members::Name;

    use octowhere_node::{Command, view::Text};

    use super::device::COMMANDS;

    /// The command's code in the low byte and its argument in the next. The debugger writes it
    /// last.
    #[unsafe(no_mangle)]
    static OCTOWHERE_PAIR_COMMAND: AtomicU32 = AtomicU32::new(0);
    /// A new name's bytes, little-endian in each word; the command's argument is its length.
    #[unsafe(no_mangle)]
    static OCTOWHERE_PAIR_NAME: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
    /// A message's text, little-endian in each word; its length is the command's third byte.
    #[unsafe(no_mangle)]
    static OCTOWHERE_PAIR_TEXT: [AtomicU32; 40] = [const { AtomicU32::new(0) }; 40];
    /// The local time in milliseconds until which a pairing drops every frame it hears.
    static DEAF_UNTIL_MS: AtomicU32 = AtomicU32::new(0);

    /// Whether a pairing is to drop the frames it hears, to lose them on purpose.
    pub fn is_deaf() -> bool {
        (Instant::now().as_millis() as u32) < DEAF_UNTIL_MS.load(Ordering::Relaxed)
    }

    #[embassy_executor::task]
    pub async fn task() {
        loop {
            Timer::after(Duration::from_millis(100)).await;
            let word = OCTOWHERE_PAIR_COMMAND.swap(0, Ordering::Acquire);
            let argument = (word >> 8) as u8;
            let command = match word & 0xff {
                0 => continue,
                1 => Command::Add,
                2 => Command::Join,
                3 => Command::Choose(argument),
                4 => Command::Accept,
                5 => Command::Decline,
                6 => Command::Mismatch,
                7 => Command::Cancel,
                8 => Command::Leave,
                9 => {
                    let mut bytes = [0; 16];
                    for (i, word) in OCTOWHERE_PAIR_NAME.iter().enumerate() {
                        bytes[4 * i..4 * i + 4]
                            .copy_from_slice(&word.load(Ordering::Relaxed).to_le_bytes());
                    }
                    match Name::new(&bytes[..usize::from(argument).min(16)]) {
                        Some(name) => Command::Rename(name),
                        None => {
                            defmt::warn!("[MESH] injected name refused");
                            continue;
                        }
                    }
                }
                11 => Command::Refresh,
                13 => Command::Remove(argument),
                14 => Command::Keep(argument),
                15 => Command::Phantom,
                12 => {
                    let mut bytes = [0; 160];
                    for (i, word) in OCTOWHERE_PAIR_TEXT.iter().enumerate() {
                        bytes[4 * i..4 * i + 4]
                            .copy_from_slice(&word.load(Ordering::Relaxed).to_le_bytes());
                    }
                    let len = usize::from((word >> 16) as u8).min(160);
                    match Text::new(&bytes[..len]) {
                        // 255 is the whole group.
                        Some(text) => Command::Send {
                            to: (argument != u8::MAX).then_some(argument),
                            text,
                        },
                        None => {
                            defmt::warn!("[MESH] injected text refused");
                            continue;
                        }
                    }
                }
                10 => {
                    let now = Instant::now().as_millis() as u32;
                    DEAF_UNTIL_MS.store(now + 1000 * u32::from(argument), Ordering::Relaxed);
                    defmt::info!("[MESH] deaf for {}s", argument);
                    continue;
                }
                other => {
                    defmt::warn!("[MESH] injected command {} unknown", other);
                    continue;
                }
            };
            COMMANDS.send(command).await;
        }
    }
}
