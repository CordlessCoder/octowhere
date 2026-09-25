//! Settings kept across restarts, in an ekv database on the partition table's `storage`
//! partition. Nothing else uses that partition.
//!
//! A flash write stops the cache both cores run from, so the display core must wait in RAM
//! for its length. Core 0 asks with [`with_display_core_held`], and the display core's loop owes
//! a call to [`hold_display_core_if_asked`] between frames, outside any critical section.

use core::sync::atomic::{AtomicU32, Ordering};

use defmt::warn;
use ekv::{Config, Database, MountError, ReadError, flash::PageID};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use embedded_storage::nor_flash::{NorFlash as _, ReadNorFlash as _};
use esp_bootloader_esp_idf::partitions::{self, DataPartitionSubType, PartitionType};
use esp_storage::{FlashStorage, FlashStorageError};
use octowhere_ui::{
    tz::DATABASE,
    ui::{
        clock::{ZoneId, ZoneMode},
        rest::Timeout,
    },
};

/// The partition's label in `partitions.csv`.
const PARTITION: &str = "storage";
const PAGE_SIZE: u32 = ekv::config::PAGE_SIZE as u32;

// Keys, in the order a transaction must write them.
const KEY_ALWAYS_ON: &[u8] = b"always_on";
const KEY_AUTOMATIC_ZONE: &[u8] = b"automatic_zone";
const KEY_BRIGHTNESS: &[u8] = b"brightness";
const KEY_MANUAL_ZONE: &[u8] = b"manual_zone";
/// The screen timeout in seconds, little-endian, 0 for never.
const KEY_TIMEOUT: &[u8] = b"timeout";
const KEY_ZONE_MODE: &[u8] = b"zone_mode";
const MODE_AUTOMATIC: u8 = 0;
const MODE_MANUAL: u8 = 1;
/// Long enough for the longest zone name.
const VALUE_BUFFER: usize = 64;

/// What the settings held when the firmware started.
#[derive(Clone, Copy, Debug, Default)]
pub struct Saved {
    pub zone_mode: ZoneMode,
    pub manual_zone: Option<ZoneId>,
    /// The zone GNSS last placed the device in.
    pub automatic_zone: Option<ZoneId>,
    /// The display's level, out of 255.
    pub brightness: Option<u8>,
    pub timeout: Option<Timeout>,
    pub always_on: Option<bool>,
}

/// One change to save.
#[derive(Clone, Copy, Debug)]
pub enum Write {
    /// Where GNSS placed the device.
    AutomaticZone(ZoneId),
    /// A zone chosen by hand, which puts the zone in manual mode.
    ManualZone(ZoneId),
    /// Back to automatic mode.
    Automatic,
    Brightness(u8),
    Timeout(Timeout),
    AlwaysOn(bool),
    /// Forget every setting.
    Clear,
}

/// Names the zone, since a `ZoneId` changes when the zone data is rebuilt.
impl defmt::Format for Write {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::AutomaticZone(zone) => {
                defmt::write!(f, "AutomaticZone({=str})", DATABASE.zone(*zone).name);
            }
            Self::ManualZone(zone) => defmt::write!(f, "ManualZone({=str})", DATABASE.zone(*zone).name),
            Self::Automatic => defmt::write!(f, "Automatic"),
            Self::Brightness(level) => defmt::write!(f, "Brightness({})", level),
            Self::Timeout(timeout) => defmt::write!(f, "Timeout({=str})", timeout.label()),
            Self::AlwaysOn(on) => defmt::write!(f, "AlwaysOn({})", on),
            Self::Clear => defmt::write!(f, "Clear"),
        }
    }
}

/// The `storage` partition, in the pages ekv asks for.
struct Partition {
    flash: FlashStorage<'static>,
    offset: u32,
    pages: usize,
}

impl Partition {
    fn at(&self, page: PageID, offset: usize) -> u32 {
        self.offset + page.index() as u32 * PAGE_SIZE + offset as u32
    }
}

// esp-storage is blocking, so none of these ever wait.
impl ekv::flash::Flash for Partition {
    type Error = FlashStorageError;

    fn page_count(&self) -> usize {
        self.pages
    }

    async fn erase(&mut self, page: PageID) -> Result<(), Self::Error> {
        let at = self.at(page, 0);
        self.flash.erase(at, at + PAGE_SIZE)
    }

    async fn read(&mut self, page: PageID, offset: usize, data: &mut [u8]) -> Result<(), Self::Error> {
        let at = self.at(page, offset);
        self.flash.read(at, data)
    }

    async fn write(&mut self, page: PageID, offset: usize, data: &[u8]) -> Result<(), Self::Error> {
        let at = self.at(page, offset);
        self.flash.write(at, data)
    }
}

pub struct Store {
    /// `None` when the partition table has no storage partition.
    database: Option<Database<Partition, NoopRawMutex>>,
}

impl Store {
    /// Finds the storage partition. Touches no flash beyond reading the partition table.
    ///
    /// # Safety
    ///
    /// Every flash operation through the store must run while the display core is not running,
    /// or inside [`with_display_core_held`].
    pub unsafe fn new(flash: FlashStorage<'static>, random_seed: u32) -> Self {
        // SAFETY: the caller keeps the display core off flash for every operation.
        let mut flash = unsafe { flash.multicore_ignore() };
        let mut table = [0; partitions::PARTITION_TABLE_MAX_LEN];
        let found = partitions::read_partition_table(&mut flash, &mut table)
            .ok()
            .and_then(|table| {
                table.iter().find(|entry| {
                    entry.label_as_str() == PARTITION
                        && entry.partition_type() == PartitionType::Data(DataPartitionSubType::Undefined)
                })
            })
            .map(|entry| (entry.offset(), entry.len()));
        let Some((offset, length)) = found else {
            warn!("[SETTINGS] no {=str} partition", PARTITION);
            return Self { database: None };
        };
        let pages = ((length / PAGE_SIZE) as usize).min(ekv::config::MAX_PAGE_COUNT);
        let mut config = Config::default();
        config.random_seed = random_seed;
        let database = Database::new(Partition { flash, offset, pages }, config);
        Self { database: Some(database) }
    }

    /// Reads every setting, taking a default for any missing or unreadable. Formats the
    /// partition if it holds no database, as on first boot.
    pub fn load(&mut self) -> Saved {
        let Some(database) = &self.database else {
            return Saved::default();
        };
        embassy_futures::block_on(async {
            match database.mount().await {
                Ok(()) => {}
                Err(MountError::Corrupted) => {
                    warn!("[SETTINGS] no database, formatting");
                    if database.format().await.is_err() {
                        return Saved::default();
                    }
                }
                Err(MountError::Flash(_)) => return Saved::default(),
            }
            let transaction = database.read_transaction().await;
            let mut buffer = [0; VALUE_BUFFER];
            let mut value = async |key: &[u8]| -> Option<heapless::Vec<u8, VALUE_BUFFER>> {
                match transaction.read(key, &mut buffer).await {
                    Ok(length) => heapless::Vec::from_slice(&buffer[..length]).ok(),
                    Err(ReadError::KeyNotFound) => None,
                    Err(_) => {
                        warn!("[SETTINGS] {=[u8]:a} unreadable", key);
                        None
                    }
                }
            };
            let zone = |name: Option<heapless::Vec<u8, VALUE_BUFFER>>| {
                let name = name?;
                DATABASE.find(core::str::from_utf8(&name).ok()?).map(|zone| zone.id)
            };
            let always_on = value(KEY_ALWAYS_ON).await.and_then(|on| on.first().map(|&on| on != 0));
            let automatic_zone = zone(value(KEY_AUTOMATIC_ZONE).await);
            let brightness = value(KEY_BRIGHTNESS).await.and_then(|level| level.first().copied());
            let manual_zone = zone(value(KEY_MANUAL_ZONE).await);
            let timeout = value(KEY_TIMEOUT)
                .await
                .and_then(|seconds| Some(u16::from_le_bytes(seconds.as_slice().try_into().ok()?)))
                .and_then(Timeout::from_seconds);
            let mode = value(KEY_ZONE_MODE).await.and_then(|mode| mode.first().copied());
            Saved {
                // A manual choice needs its zone, which a rebuilt zone table may have dropped.
                zone_mode: match mode {
                    Some(MODE_MANUAL) if manual_zone.is_some() => ZoneMode::Manual,
                    _ => ZoneMode::Automatic,
                },
                manual_zone,
                automatic_zone,
                brightness,
                timeout,
                always_on,
            }
        })
    }

    /// Saves one change, and says whether it reached the flash.
    pub fn save(&mut self, write: Write) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        embassy_futures::block_on(async {
            if let Write::Clear = write {
                return database.format().await.is_ok();
            }
            let name = |zone: ZoneId| DATABASE.zone(zone).name.as_bytes();
            let mut transaction = database.write_transaction().await;
            // A transaction takes its keys in ascending order, and commits them all or none.
            let written = match write {
                Write::AutomaticZone(zone) => transaction.write(KEY_AUTOMATIC_ZONE, name(zone)).await,
                Write::ManualZone(zone) => {
                    match transaction.write(KEY_MANUAL_ZONE, name(zone)).await {
                        Ok(()) => transaction.write(KEY_ZONE_MODE, &[MODE_MANUAL]).await,
                        error => error,
                    }
                }
                Write::Automatic => transaction.write(KEY_ZONE_MODE, &[MODE_AUTOMATIC]).await,
                Write::Brightness(level) => transaction.write(KEY_BRIGHTNESS, &[level]).await,
                Write::Timeout(timeout) => transaction.write(KEY_TIMEOUT, &timeout.seconds().to_le_bytes()).await,
                Write::AlwaysOn(on) => transaction.write(KEY_ALWAYS_ON, &[u8::from(on)]).await,
                Write::Clear => unreachable!("handled above"),
            };
            written.is_ok() && transaction.commit().await.is_ok()
        })
    }
}

const RUNNING: u32 = 0;
const ASKED: u32 = 1;
const HELD: u32 = 2;
static DISPLAY_CORE: AtomicU32 = AtomicU32::new(RUNNING);

/// Holds the display core in RAM while core 0 runs `f`, which may then use the flash.
///
/// Waits for the display core's next frame, so the frame loop must keep handing frames over
/// meanwhile: never call this from the frame loop.
pub async fn with_display_core_held<R>(f: impl FnOnce() -> R) -> R {
    /// Lets the display core go however the wait ends, including a dropped future.
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            DISPLAY_CORE.store(RUNNING, Ordering::Release);
        }
    }

    DISPLAY_CORE.store(ASKED, Ordering::Release);
    let _release = Release;
    while DISPLAY_CORE.load(Ordering::Acquire) != HELD {
        Timer::after(Duration::from_millis(1)).await;
    }
    f()
}

/// Waits in RAM, with this core's interrupts masked, while core 0 has asked to use the flash.
pub fn hold_display_core_if_asked() {
    if DISPLAY_CORE.load(Ordering::Acquire) != ASKED {
        return;
    }
    let mask = esp_hal::xtensa_lx::interrupt::disable();
    wait_in_ram();
    // SAFETY: restores the mask `disable` returned, outside any critical section.
    unsafe { esp_hal::xtensa_lx::interrupt::set_mask(mask) };
}

/// Nothing here may call into flash: core 0 starts on the flash as soon as it sees `HELD`.
#[esp_hal::ram]
fn wait_in_ram() {
    // Core 0 may have given up since the check; then there is nothing to hold for.
    if DISPLAY_CORE
        .compare_exchange(ASKED, HELD, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    while DISPLAY_CORE.load(Ordering::Acquire) == HELD {
        core::hint::spin_loop();
    }
}
