//! Settings kept across restarts, in an ekv database on the partition table's `storage`
//! partition, and beside them the mesh's: this device's key pair and name, and its group.
//! Nothing else uses that partition.
//!
//! A flash write stops the cache both cores run from, so the display core must wait in RAM
//! for its length. Core 0 asks with [`with_display_core_held`], and the display core's loop owes
//! a call to [`hold_display_core_if_asked`] between frames, outside any critical section.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicU32, Ordering};

use defmt::warn;
use ekv::{Config, Database, MountError, ReadError, flash::PageID};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use esp_bootloader_esp_idf::partitions::{self, DataPartitionSubType, PartitionType};
use esp_storage::{FlashStorage, FlashStorageError};
use octowhere_mesh::{
    IDS,
    members::{
        GONE_LEN, Gone, Group, Member, Name, RECORD_FIXED_LEN, RECORD_MAX_LEN, STORED_HEADER_LEN,
        Slot, decode_stored_header,
    },
    rekey::{self, Rekey},
    seal::Key,
};
use octowhere_ui::{
    tz::DATABASE,
    ui::{
        clock::{ZoneId, ZoneMode},
        rest::{AlwaysOn, Timeout},
        second::choice_label,
    },
};

/// The partition's label in `partitions.csv`.
const PARTITION: &str = "storage";
const PAGE_SIZE: u32 = ekv::config::PAGE_SIZE as u32;

// Keys, in the order a transaction must write them.
/// Off, the dim level or a percentage, as `AlwaysOn::to_byte` gives it.
const KEY_ALWAYS_ON: &[u8] = b"always_on";
const KEY_AUTOMATIC_ZONE: &[u8] = b"automatic_zone";
const KEY_BRIGHTNESS: &[u8] = b"brightness";
const KEY_MANUAL_ZONE: &[u8] = b"manual_zone";
/// The screen timeout in seconds, little-endian, 0 for never.
const KEY_TIMEOUT: &[u8] = b"timeout";
const KEY_ZONE_MODE: &[u8] = b"zone_mode";
/// Every settings key, ascending, which clearing deletes.
const SETTINGS_KEYS: [&[u8]; 6] = [
    KEY_ALWAYS_ON,
    KEY_AUTOMATIC_ZONE,
    KEY_BRIGHTNESS,
    KEY_MANUAL_ZONE,
    KEY_TIMEOUT,
    KEY_ZONE_MODE,
];
/// The group's header, as `Group::encode_stored_header` writes it.
const KEY_GROUP: &[u8] = b"group";
/// A removal under way, the old keys kept and those declined, as `Rekey::encode` writes them.
/// It sorts after every member's key.
const KEY_REKEY: &[u8] = b"group.rekey";
/// This device's Ed25519 seed, which its keys come from (`octowhere_mesh::identity`). It is in
/// the clear: `context/LORA-PROTOCOL.md` defers flash encryption. A device that stored an X25519
/// secret here before reads it as a seed, which gives it a new identity.
const KEY_IDENTITY: &[u8] = b"identity";
const KEY_NAME: &[u8] = b"name";
/// The end of the block of message sequence numbers reserved last, little-endian: every number
/// below it may have been used.
const KEY_SEQUENCE: &[u8] = b"sequence";
/// The first byte of each mesh value. It has stayed 1 as the layouts grew, since each grew by
/// fields added at the end, which the readers take as missing where an older value stops. A
/// layout that cannot be read that way needs a new version.
const MESH_VERSION: u8 = 1;
const MODE_AUTOMATIC: u8 = 0;
const MODE_MANUAL: u8 = 1;
/// Long enough for the longest zone name.
const VALUE_BUFFER: usize = 64;
/// Long enough for the longest mesh value, a removal's state, behind its version byte.
const MESH_BUFFER: usize = 1 + rekey::STORED_MAX;

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
    pub always_on: Option<AlwaysOn>,
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
    AlwaysOn(AlwaysOn),
    /// Forget every setting.
    Clear,
}

/// The mesh's state as stored.
#[derive(Default)]
pub struct MeshSaved {
    pub seed: Option<[u8; 32]>,
    pub name: Option<Name>,
    pub group: Option<Box<Group>>,
    pub sequence: Option<u32>,
    pub rekey: Option<Box<Rekey>>,
}

pub use octowhere_node::GroupWrite;

/// Writes `slot` at `id`, or deletes the key if it is stored and `slot` is `None`, and keeps
/// `stored` in step. A gone record's value is shorter than any member's, which tells them apart.
async fn write_member(
    transaction: &mut ekv::WriteTransaction<'_, Partition, NoopRawMutex>,
    stored: &mut u32,
    id: u8,
    slot: Option<&Slot>,
) -> Result<(), ekv::WriteError<FlashStorageError>> {
    let mut value = [0; 1 + RECORD_MAX_LEN];
    value[0] = MESH_VERSION;
    let len = match slot {
        Some(Slot::Member(member)) => {
            let mut record = [0; RECORD_MAX_LEN];
            let len = member.encode(id, &mut record);
            value[1..1 + len].copy_from_slice(&record[..len]);
            len
        }
        Some(Slot::Gone(gone)) => {
            let mut record = [0; GONE_LEN];
            gone.encode(id, &mut record);
            value[1..1 + GONE_LEN].copy_from_slice(&record);
            GONE_LEN
        }
        None if *stored & 1 << id != 0 => {
            transaction.delete(&member_key(id)).await?;
            *stored &= !(1 << id);
            return Ok(());
        }
        None => return Ok(()),
    };
    transaction
        .write(&member_key(id), &value[..1 + len])
        .await?;
    *stored |= 1 << id;
    Ok(())
}

/// Reads a slot's stored value, after its version byte.
fn read_slot(id: u8, value: &[u8]) -> Option<Slot> {
    if value.len() > RECORD_FIXED_LEN {
        let (at, member) = Member::decode(value)?;
        (at == id).then_some(Slot::Member(member))
    } else {
        let (at, gone) = Gone::decode(value)?;
        (at == id).then_some(Slot::Gone(gone))
    }
}

/// A member's key: `group.member.` and its id in two digits, which sorts after `group`.
fn member_key(id: u8) -> [u8; 15] {
    let mut key = *b"group.member.00";
    key[13] = b'0' + id / 10;
    key[14] = b'0' + id % 10;
    key
}

/// Names the zone, since a `ZoneId` changes when the zone data is rebuilt.
impl defmt::Format for Write {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::AutomaticZone(zone) => {
                defmt::write!(f, "AutomaticZone({=str})", DATABASE.zone(*zone).name);
            }
            Self::ManualZone(zone) => {
                defmt::write!(f, "ManualZone({=str})", DATABASE.zone(*zone).name)
            }
            Self::Automatic => defmt::write!(f, "Automatic"),
            Self::Brightness(level) => defmt::write!(f, "Brightness({})", level),
            Self::Timeout(timeout) => defmt::write!(f, "Timeout({=str})", timeout.label()),
            Self::AlwaysOn(choice) => defmt::write!(f, "AlwaysOn({=str})", &*choice_label(*choice)),
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

    async fn read(
        &mut self,
        page: PageID,
        offset: usize,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        let at = self.at(page, offset);
        self.flash.read(at, data)
    }

    async fn write(&mut self, page: PageID, offset: usize, data: &[u8]) -> Result<(), Self::Error> {
        let at = self.at(page, offset);
        // Not `write`, which reads, erases and rewrites the whole sector around `data`.
        self.flash.write_nor(at, data)
    }
}

pub struct Store {
    /// `None` when the partition table has no storage partition.
    database: Option<Database<Partition, NoopRawMutex>>,
    /// The ids whose member keys the database holds, as a set, so a write touches only those
    /// that change.
    stored_members: u32,
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
                        && entry.partition_type()
                            == PartitionType::Data(DataPartitionSubType::Undefined)
                })
            })
            .map(|entry| (entry.offset(), entry.len()));
        let Some((offset, length)) = found else {
            warn!("[SETTINGS] no {=str} partition", PARTITION);
            return Self {
                database: None,
                stored_members: 0,
            };
        };
        let pages = ((length / PAGE_SIZE) as usize).min(ekv::config::MAX_PAGE_COUNT);
        let mut config = Config::default();
        config.random_seed = random_seed;
        let database = Database::new(
            Partition {
                flash,
                offset,
                pages,
            },
            config,
        );
        Self {
            database: Some(database),
            stored_members: 0,
        }
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
                DATABASE
                    .find(core::str::from_utf8(&name).ok()?)
                    .map(|zone| zone.id)
            };
            let always_on = value(KEY_ALWAYS_ON)
                .await
                .and_then(|byte| AlwaysOn::from_byte(*byte.first()?));
            let automatic_zone = zone(value(KEY_AUTOMATIC_ZONE).await);
            let brightness = value(KEY_BRIGHTNESS)
                .await
                .and_then(|level| level.first().copied());
            let manual_zone = zone(value(KEY_MANUAL_ZONE).await);
            let timeout = value(KEY_TIMEOUT)
                .await
                .and_then(|seconds| Some(u16::from_le_bytes(seconds.as_slice().try_into().ok()?)))
                .and_then(Timeout::from_seconds);
            let mode = value(KEY_ZONE_MODE)
                .await
                .and_then(|mode| mode.first().copied());
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

    /// Reads the mesh's state. Call it after [`Store::load`], which mounts the database.
    pub fn load_mesh(&mut self) -> MeshSaved {
        let Some(database) = &self.database else {
            return MeshSaved::default();
        };
        embassy_futures::block_on(async {
            let transaction = database.read_transaction().await;
            let mut buffer = [0; MESH_BUFFER];
            let mut value = async |key: &[u8]| -> Option<heapless::Vec<u8, MESH_BUFFER>> {
                match transaction.read(key, &mut buffer).await {
                    Ok(length) => match buffer[..length].split_first() {
                        Some((&MESH_VERSION, rest)) => heapless::Vec::from_slice(rest).ok(),
                        _ => {
                            warn!("[SETTINGS] {=[u8]:a} has another layout", key);
                            None
                        }
                    },
                    Err(ReadError::KeyNotFound) => None,
                    Err(_) => {
                        warn!("[SETTINGS] {=[u8]:a} unreadable", key);
                        None
                    }
                }
            };
            let group = value(KEY_GROUP).await;
            let mut slots = [None; IDS as usize];
            let mut stored_members = 0;
            if group.is_some() {
                for id in 0..IDS {
                    let Some(record) = value(&member_key(id)).await else {
                        continue;
                    };
                    stored_members |= 1 << id;
                    match read_slot(id, &record) {
                        Some(slot) => slots[usize::from(id)] = Some(slot),
                        None => warn!("[SETTINGS] member {} unreadable", id),
                    }
                }
            }
            let group = group.and_then(|group| {
                let (key, own, generation) = decode_stored_header(&group)?;
                Group::restore(key, generation, own, slots)
            });
            let seed = value(KEY_IDENTITY)
                .await
                .and_then(|seed| seed.as_slice().try_into().ok());
            let name = value(KEY_NAME).await.and_then(|name| Name::new(&name));
            let sequence = value(KEY_SEQUENCE)
                .await
                .and_then(|end| Some(u32::from_le_bytes(end.as_slice().try_into().ok()?)));
            let rekey = match &group {
                Some(_) => value(KEY_REKEY).await.and_then(|bytes| {
                    let rekey = Rekey::decode(&bytes);
                    if rekey.is_none() {
                        warn!("[SETTINGS] {=[u8]:a} has another layout", KEY_REKEY);
                    }
                    rekey.map(Box::new)
                }),
                None => None,
            };
            self.stored_members = stored_members;
            MeshSaved {
                seed,
                name,
                group: group.map(Box::new),
                sequence,
                rekey,
            }
        })
    }

    /// Saves one change to the mesh's state, and says whether it reached the flash.
    pub fn save_mesh(&mut self, write: &GroupWrite) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        embassy_futures::block_on(async {
            let mut transaction = database.write_transaction().await;
            let mut value = [0; MESH_BUFFER];
            value[0] = MESH_VERSION;
            let mut stored = self.stored_members;
            // A transaction takes its keys in ascending order.
            let written = match write {
                GroupWrite::Identity(seed) => {
                    value[1..33].copy_from_slice(seed);
                    transaction.write(KEY_IDENTITY, &value[..33]).await
                }
                GroupWrite::Name(name) => {
                    let name = name.as_bytes();
                    value[1..1 + name.len()].copy_from_slice(name);
                    transaction.write(KEY_NAME, &value[..1 + name.len()]).await
                }
                GroupWrite::Sequence(end) => {
                    value[1..5].copy_from_slice(&end.to_le_bytes());
                    transaction.write(KEY_SEQUENCE, &value[..5]).await
                }
                GroupWrite::Rekey(rekey) => {
                    let mut bytes = [0; rekey::STORED_MAX];
                    let len = rekey.encode(&mut bytes);
                    value[1..1 + len].copy_from_slice(&bytes[..len]);
                    transaction.write(KEY_REKEY, &value[..1 + len]).await
                }
                GroupWrite::Group(group, rekey) => {
                    let mut header = [0; STORED_HEADER_LEN];
                    group.encode_stored_header(&mut header);
                    value[1..1 + STORED_HEADER_LEN].copy_from_slice(&header);
                    let mut written = transaction
                        .write(KEY_GROUP, &value[..1 + STORED_HEADER_LEN])
                        .await;
                    for id in 0..IDS {
                        if written.is_err() {
                            break;
                        }
                        written =
                            write_member(&mut transaction, &mut stored, id, group.slot(id)).await;
                    }
                    if let (Ok(()), Some(rekey)) = (&written, rekey) {
                        let mut bytes = [0; rekey::STORED_MAX];
                        let len = rekey.encode(&mut bytes);
                        value[1..1 + len].copy_from_slice(&bytes[..len]);
                        written = transaction.write(KEY_REKEY, &value[..1 + len]).await;
                    }
                    written
                }
                GroupWrite::Slot { id, slot } => {
                    write_member(&mut transaction, &mut stored, *id, slot.as_ref()).await
                }
                GroupWrite::Leave => {
                    let mut written = transaction.delete(KEY_GROUP).await;
                    for id in 0..IDS {
                        if written.is_ok() {
                            written = write_member(&mut transaction, &mut stored, id, None).await;
                        }
                    }
                    if written.is_ok() {
                        written = transaction.delete(KEY_REKEY).await;
                    }
                    written
                }
            };
            let saved = written.is_ok() && transaction.commit().await.is_ok();
            if saved {
                self.stored_members = stored;
            }
            saved
        })
    }

    /// Saves one change, and says whether it reached the flash.
    pub fn save(&mut self, write: Write) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        embassy_futures::block_on(async {
            if let Write::Clear = write {
                // The group and the mesh's identity are not settings, and stay.
                let mut transaction = database.write_transaction().await;
                for key in SETTINGS_KEYS {
                    if transaction.delete(key).await.is_err() {
                        return false;
                    }
                }
                return transaction.commit().await.is_ok();
            }
            let name = |zone: ZoneId| DATABASE.zone(zone).name.as_bytes();
            let mut transaction = database.write_transaction().await;
            // A transaction takes its keys in ascending order, and commits them all or none.
            let written = match write {
                Write::AutomaticZone(zone) => {
                    transaction.write(KEY_AUTOMATIC_ZONE, name(zone)).await
                }
                Write::ManualZone(zone) => {
                    match transaction.write(KEY_MANUAL_ZONE, name(zone)).await {
                        Ok(()) => transaction.write(KEY_ZONE_MODE, &[MODE_MANUAL]).await,
                        error => error,
                    }
                }
                Write::Automatic => transaction.write(KEY_ZONE_MODE, &[MODE_AUTOMATIC]).await,
                Write::Brightness(level) => transaction.write(KEY_BRIGHTNESS, &[level]).await,
                Write::Timeout(timeout) => {
                    transaction
                        .write(KEY_TIMEOUT, &timeout.seconds().to_le_bytes())
                        .await
                }
                Write::AlwaysOn(choice) => {
                    transaction.write(KEY_ALWAYS_ON, &[choice.to_byte()]).await
                }
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
