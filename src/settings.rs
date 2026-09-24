//! Settings kept across restarts, as a sequential-storage map in the partition table's `nvs`
//! partition. Nothing else in the firmware uses that partition, and the map is not ESP-IDF's NVS
//! format.
//!
//! A flash write stops the cache both cores run from, so the display core must wait in RAM
//! for its length. Core 0 asks with [`with_display_core_held`], and the display core's loop owes
//! a call to [`hold_display_core_if_asked`] between frames, outside any critical section.

use core::sync::atomic::{AtomicU32, Ordering};

use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_time::{Duration, Timer};
use esp_bootloader_esp_idf::partitions::{
    self, DataPartitionSubType, NorFlashRegion, PartitionType,
};
use esp_storage::FlashStorage;
use octowhere_ui::{
    tz::DATABASE,
    ui::clock::{ZoneId, ZoneMode},
};
use sequential_storage::{
    cache::{Cache, Uncached},
    map::{MapConfig, MapStorage},
};

const KEY_ZONE_MODE: u8 = 1;
const KEY_MANUAL_ZONE: u8 = 2;
const KEY_AUTOMATIC_ZONE: u8 = 3;
/// Long enough for a key and the longest zone name, rounded up to the flash's word.
const ITEM_BUFFER: usize = 64;

/// What the settings held when the firmware started.
#[derive(Clone, Copy, Debug, Default)]
pub struct Saved {
    pub zone_mode: ZoneMode,
    pub manual_zone: Option<ZoneId>,
    /// The zone GNSS last placed the device in.
    pub automatic_zone: Option<ZoneId>,
}

/// One setting to save.
#[derive(Clone, Copy, Debug)]
pub enum Write {
    AutomaticZone(ZoneId),
}

/// Names the zone, since a `ZoneId` changes when the zone data is rebuilt.
impl defmt::Format for Write {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::AutomaticZone(zone) => {
                defmt::write!(f, "AutomaticZone({=str})", DATABASE.zone(*zone).name);
            }
        }
    }
}

pub struct Store {
    flash: FlashStorage<'static>,
}

type Map<'r, 'a, 'd> = MapStorage<
    u8,
    BlockingAsync<NorFlashRegion<'r, 'a, 'd>>,
    Cache<Uncached, Uncached, Uncached, u8>,
>;

impl Store {
    /// # Safety
    ///
    /// Every flash operation through the store must run while the display core is not running,
    /// or inside [`with_display_core_held`].
    pub unsafe fn new(flash: FlashStorage<'static>) -> Self {
        // SAFETY: the caller keeps the display core off flash for every operation.
        Self {
            flash: unsafe { flash.multicore_ignore() },
        }
    }

    fn with_map<R>(&mut self, f: impl FnOnce(&mut Map<'_, '_, '_>) -> R) -> Option<R> {
        let mut table = [0; partitions::PARTITION_TABLE_MAX_LEN];
        let table = partitions::read_partition_table(&mut self.flash, &mut table).ok()?;
        let nvs = table
            .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
            .ok()??;
        let mut region = nvs.as_flash_region(&mut self.flash);
        let length = region.partition_size() as u32;
        let config = MapConfig::try_new(0..length).ok()?;
        let flash = region.as_nor_flash().ok()?;
        let mut map = MapStorage::new(BlockingAsync::new(flash), config, Cache::new_uncached());
        Some(f(&mut map))
    }

    /// Reads every setting, taking a default for any missing or unreadable.
    pub fn load(&mut self) -> Saved {
        self.with_map(|map| {
            let mut buffer = [0; ITEM_BUFFER];
            let mut zone = |map: &mut Map<'_, '_, '_>, key| {
                let name = embassy_futures::block_on(map.fetch_item::<&[u8]>(&mut buffer, &key));
                let name = core::str::from_utf8(name.ok()??).ok()?;
                DATABASE.find(name).map(|zone| zone.id)
            };
            let manual_zone = zone(map, KEY_MANUAL_ZONE);
            let automatic_zone = zone(map, KEY_AUTOMATIC_ZONE);
            let mut buffer = [0; ITEM_BUFFER];
            let mode = embassy_futures::block_on(map.fetch_item::<u8>(&mut buffer, &KEY_ZONE_MODE));
            Saved {
                // A manual choice needs its zone, which a rebuilt zone table may have dropped.
                zone_mode: match mode {
                    Ok(Some(1)) if manual_zone.is_some() => ZoneMode::Manual,
                    _ => ZoneMode::Automatic,
                },
                manual_zone,
                automatic_zone,
            }
        })
        .unwrap_or_default()
    }

    /// Saves one setting, and says whether it reached the flash.
    pub fn save(&mut self, write: Write) -> bool {
        self.with_map(|map| {
            let mut buffer = [0; ITEM_BUFFER];
            match write {
                Write::AutomaticZone(zone) => {
                    let name = DATABASE.zone(zone).name.as_bytes();
                    embassy_futures::block_on(map.store_item(
                        &mut buffer,
                        &KEY_AUTOMATIC_ZONE,
                        &name,
                    ))
                }
            }
            .is_ok()
        })
        .unwrap_or(false)
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
