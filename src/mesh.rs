//! Runs the location mesh on the radio: sends this node's packet in its slot, listens to the other
//! slots, and keeps the timebase the slots are placed on. Pairing takes the radio over, on a
//! channel of its own, until it ends. `octowhere_mesh` holds the protocol and
//! `context/LORA-PROTOCOL.md` the design.

use alloc::boxed::Box;
use core::{
    cell::{Cell, RefCell},
    sync::atomic::{AtomicU32, Ordering},
};

use defmt::{debug, info, warn};
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_sync::{
    blocking_mutex::{Mutex as BlockingMutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::gpio::Input;
use lc76g::FixQuality;
use octowhere::{
    settings::GroupWrite,
    ui::group::view::{
        Answer, GroupView, MemberView, MeshView, PairingView, Position, Refused, Request,
    },
};
use octowhere_mesh::{
    IDS,
    clock::{Clock, Taken},
    members::{Group, Member, Merged, Name},
    packet::{Builder, Entry, Hdop, Header, MAX_PACKET, Plain, Quality, Record, Source, Timebase},
    pair::{End, Identity, MAX_FRAME, Pairing, Phase, Role},
    schedule::{
        GUARD_US, ROUND_US, airtime_us, base_of, named_slot, next_slot, next_slot_in, round_at,
    },
    seal::{self, Key, SIV_LEN},
    table::{Merge, Table},
};
use sx127xlora::{
    driver::Sx127xError,
    registers::{
        FIFO_ADDR_PTR, FIFO_TX_BASE_ADDR, FIFO_TX_BASE_ADDR_VALUE, IRQ_FLAGS,
        SYNC_WORD as SYNC_WORD_REGISTER,
    },
    types::{DeviceMode, OCP, PowerRamp, RxDone, RxPacket, TxConfig, TxDone},
};

use super::{GPS_TIME, LoraPath, SensorLora};

/// Band O's lower 125 kHz channel.
const FREQUENCY_HZ: u32 = 869_462_500;
/// Off the SX127x's reset value `0x12`, which another network on this channel uses, and off
/// LoRaWAN's `0x34` and Meshtastic's `0x2B`. Neither nibble is zero.
const SYNC_WORD: u8 = 0x6C;
/// On PA_BOOST, which the module's antenna is on.
const POWER_DBM: u8 = 17;
/// Band O's upper 125 kHz channel, which pairing has to itself.
const PAIR_FREQUENCY_HZ: u32 = 869_587_500;
const PAIR_SYNC_WORD: u8 = 0xA6;
/// PA_BOOST's lowest. Two devices side by side overload each other's receiver at +17 dBm.
const PAIR_POWER_DBM: u8 = 2;
/// How long before its slot the node loads its packet and switches the antenna to transmit.
const PREPARE_US: i64 = 30_000;
/// How long after a packet ends `DIO0`'s RxDone is seen, plus how long after its slot's start a
/// sender's transmission begins: half how late a root hears the nodes timing from it, on two
/// boards whose `DIO0` follows the radio (2026-10-01). Polling the flags adds half a poll.
const ARRIVAL_LATENCY_US: i64 = 1_050;
/// The longest a transmission can take, with margin for `DIO0`.
const SEND_TIMEOUT_US: i64 = 500_000;
/// Positions a packet carries at most: as many as fit beside the neighbours record.
const MAX_ENTRIES: usize = 24;
/// How often the radio's flags are read where `DIO0` does not follow them.
const POLL_US: u64 = 1_000;
/// How long a pairing waits for the group to be stored before it counts as a failure.
const STORE_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest a pairing listens before it runs its timers again.
const PAIR_LISTEN_US: i64 = 250_000;
/// How long a device that founded a group, without hearing the last acknowledgement, listens
/// for the joining device under the group's key. That device waits 30 s for done, sweeps for
/// three rounds and sends by its next floor round, about 5 minutes in all.
const FOUNDING_WAIT_US: i64 = 10 * 60 * 1_000_000;
const IRQ_TX_DONE: u8 = 0x08;
const IRQ_RX_DONE: u8 = 0x40;

/// The latest fix and the UTC second it was made in, from `gnss_task`.
pub static FIX: BlockingMutex<CriticalSectionRawMutex, Cell<Option<Fix>>> =
    BlockingMutex::new(Cell::new(None));
/// The RTC's UTC seconds and when they were read, from `sensor_task`, while its oscillator has
/// not stopped.
pub static RTC_TIME: BlockingMutex<CriticalSectionRawMutex, Cell<Option<(i64, Instant)>>> =
    BlockingMutex::new(Cell::new(None));
/// What the user asks of the mesh.
pub static COMMANDS: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
/// What the screens show of the mesh, as last published, and a signal that it changed.
static VIEW: BlockingMutex<CriticalSectionRawMutex, RefCell<Option<Box<MeshView>>>> =
    BlockingMutex::new(RefCell::new(None));
pub static VIEW_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Counts the views published, so a reader can tell when there is a new one.
static VIEWS: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
pub struct Fix {
    /// Degrees × 10⁷.
    pub latitude: i32,
    pub longitude: i32,
    /// UTC seconds.
    pub stamp: u32,
    pub quality: FixQuality,
    pub hdop_milli: Option<u32>,
}

#[derive(Clone, Copy, defmt::Format)]
pub enum Command {
    /// Starts pairing to add a device to the group, founding one if this node has none.
    Add,
    /// Starts pairing to join a group. Only a node in no group can.
    Join,
    /// Adding: picks a device found, by its place in the list.
    Choose(u8),
    /// Adding: picks the device found announcing from this address.
    ChooseMac([u8; 6]),
    /// The codes match.
    Accept,
    Decline,
    /// The codes differ.
    Mismatch,
    Cancel,
    /// Forgets the group.
    Leave,
    Rename(Name),
}

impl From<Request> for Command {
    fn from(request: Request) -> Self {
        match request {
            Request::Add => Self::Add,
            Request::Join => Self::Join,
            Request::Choose(mac) => Self::ChooseMac(mac),
            Request::Accept => Self::Accept,
            Request::Decline => Self::Decline,
            Request::Mismatch => Self::Mismatch,
            Request::Cancel => Self::Cancel,
            Request::Leave => Self::Leave,
            Request::Rename(name) => Self::Rename(name),
        }
    }
}

/// The mesh's state as the firmware starts.
pub struct Start {
    pub me: Identity,
    pub group: Option<Box<Group>>,
}

/// Passes what the screens asked on to the mesh. Returns false when the queue is full.
pub fn request(request: Request) -> bool {
    COMMANDS.try_send(request.into()).is_ok()
}

/// Copies the mesh as last published into `into`, if it changed after the view counted `seen`,
/// and returns its count.
pub fn view_since(seen: u32, into: &mut MeshView) -> Option<u32> {
    let count = VIEWS.load(Ordering::Acquire);
    if count == seen {
        return None;
    }
    VIEW.lock(|view| view.borrow().as_deref().map(|view| into.copy_from(view)))?;
    Some(count)
}

/// A view to fill, on the heap. A view is a couple of kilobytes, and the radio's task runs on
/// whatever stack the frame loop left, so none is built or copied on the stack.
#[inline(never)]
fn blank_view() -> Box<MeshView> {
    Box::default()
}

/// Gives `group` a group with no members, outside [`Shown::fill`], so its frame does not hold
/// one on every call.
#[cold]
#[inline(never)]
fn blank_group(group: &mut Option<GroupView>) -> &mut GroupView {
    group.insert(GroupView {
        own: 0,
        members: [None; IDS as usize],
    })
}

/// Publishes `view` when it differs from what the screens were last shown. It trades places
/// with the view it replaces, so it holds an older one after.
fn publish(view: &mut Box<MeshView>) {
    let changed = VIEW.lock(|current| {
        let mut current = current.borrow_mut();
        match &mut *current {
            Some(current) if **current == **view => return false,
            Some(current) => core::mem::swap(current, view),
            None => *current = Some(core::mem::replace(view, blank_view())),
        }
        VIEWS.fetch_add(1, Ordering::Release);
        true
    });
    if changed {
        VIEW_CHANGED.signal(());
    }
}

/// Publishes the stored identity and group before the radio is known, for the screens to show
/// from the start.
pub fn publish_start(start: &Start) {
    let mut view = blank_view();
    Shown::new(false).fill(&mut view, &start.me, start.group.as_deref(), local());
    publish(&mut view);
}

/// What the screens are shown beyond the group itself.
struct Shown {
    radio: bool,
    sessions: u32,
    pairing: Option<PairingView>,
    answered: u32,
    answer: Option<Answer>,
    /// When each id was last heard sending, on the local clock.
    heard: [Option<i64>; IDS as usize],
    /// The newest position held for each id, as its UTC second. Kept past the table's expiry,
    /// so an old position shows as old rather than never received.
    positions: [Option<u32>; IDS as usize],
}

impl Shown {
    fn new(radio: bool) -> Self {
        Self {
            radio,
            sessions: 0,
            pairing: None,
            answered: 0,
            answer: None,
            heard: [None; IDS as usize],
            positions: [None; IDS as usize],
        }
    }

    fn answer(&mut self, answer: Answer) {
        self.answered += 1;
        self.answer = Some(answer);
    }

    /// Notes the stamps of the positions `table` holds.
    fn positions(&mut self, table: &Table) {
        for entry in table.entries() {
            if let Some(held) = self.positions.get_mut(usize::from(entry.id)) {
                *held = Some(held.map_or(entry.stamp, |held| held.max(entry.stamp)));
            }
        }
    }

    /// Makes `view` the view of `me` in `group` at local time `now`. Times in UTC become local
    /// times on the stage's clock, which is the same as this one.
    fn fill(&self, view: &mut MeshView, me: &Identity, group: Option<&Group>, now: i64) {
        view.radio = self.radio;
        view.mac = me.mac;
        view.name = me.name;
        view.sessions = self.sessions;
        view.pairing.clone_from(&self.pairing);
        view.answered = self.answered;
        view.answer = self.answer;
        let Some(group) = group else {
            view.group = None;
            return;
        };
        let utc = utc_now(now);
        let local_at = |stamp: u32| utc.map(|utc| now - (utc - i64::from(stamp)) * 1_000_000);
        let shown = match &mut view.group {
            Some(shown) => shown,
            None => blank_group(&mut view.group),
        };
        shown.own = group.own();
        for (index, slot) in shown.members.iter_mut().enumerate() {
            let id = index as u8;
            *slot = group.member(id).map(|member| MemberView {
                name: member.name,
                mac: member.mac,
                // A device that knew no UTC dated the record 0.
                joined: (member.joined != 0)
                    .then(|| local_at(member.joined))
                    .flatten(),
                heard: if id == group.own() {
                    None
                } else {
                    self.heard[index]
                },
                position: match self.positions[index] {
                    None => Position::Never,
                    Some(stamp) => local_at(stamp).map_or(Position::Unknown, Position::At),
                },
            });
        }
    }
}

fn pairing_view(session: u32, pairing: &Pairing) -> PairingView {
    PairingView {
        session,
        role: pairing.role(),
        phase: pairing.phase(),
        deadline: pairing.deadline(),
        candidates: pairing.candidates().collect(),
        peer: pairing.peer(),
        peer_name: pairing.peer_name(),
        group: pairing
            .group()
            .map(|group| (group.own(), group.count() as u8)),
        refused: None,
    }
}

/// A session the mesh would not start.
fn refused(session: u32, role: Role, refused: Refused) -> PairingView {
    PairingView {
        session,
        role,
        phase: Phase::Searching,
        deadline: None,
        candidates: heapless::Vec::new(),
        peer: None,
        peer_name: None,
        group: None,
        refused: Some(refused),
    }
}

/// Forgets the group, once that is stored. A node in no group has nothing to forget.
async fn leave(group: &mut Option<Group>) -> bool {
    if group.is_none() {
        return true;
    }
    let left = super::save_group(GroupWrite::Leave).await;
    if left {
        *group = None;
    }
    left
}

/// Stores `name` as this device's, and only then takes it up, so a failed write leaves the
/// name that is stored.
async fn rename(me: &mut Identity, group: Option<&mut Group>, name: Name) -> bool {
    if !super::save_group(GroupWrite::Name(name)).await {
        return false;
    }
    me.name = name;
    if let Some(group) = group {
        group.rename(name, utc_seconds(local()));
    }
    true
}

/// Random bytes from the hardware's true random source, or `None` without it. `async_main`
/// enables it at boot.
pub fn random<const N: usize>() -> Option<[u8; N]> {
    let trng = esp_hal::rng::Trng::try_new().ok()?;
    let mut bytes = [0; N];
    trng.read(&mut bytes);
    Some(bytes)
}

fn local() -> i64 {
    Instant::now().as_micros() as i64
}

fn until(local: i64) -> Timer {
    Timer::at(Instant::from_micros(local.max(0) as u64))
}

/// UTC seconds at local time `now`, from GNSS or the RTC, or 0 with neither.
fn utc_seconds(now: i64) -> u32 {
    utc_now(now).map_or(0, |utc| utc.clamp(0, i64::from(u32::MAX)) as u32)
}

/// UTC seconds at local time `now`, from GNSS or the RTC.
fn utc_now(now: i64) -> Option<i64> {
    GPS_TIME
        .lock(Cell::get)
        .map(|gps| now - gps.offset)
        .or_else(|| rtc_now(now))
        .map(|utc| utc / 1_000_000)
}

pub struct Mesh {
    lora: SensorLora,
    dio0: Input<'static>,
    /// `DIO0` rose for the last flag it was mapped to. Where it did not, the flags are polled.
    dio0_follows: bool,
    path: LoraPath,
    me: Identity,
    group: Option<Group>,
    /// A group this device founded in a pairing whose last acknowledgement it never heard, and
    /// the local time it stops listening for the joining device. A packet under the group's key
    /// shows that device stored it.
    founding: Option<Box<(Group, i64)>>,
    /// The ids whose member records changed and are not yet queued to be stored, as a set.
    unsaved: u32,
    /// The group's key or this node's id changed, so the whole group is to be stored.
    unsaved_group: bool,
    clock: Clock,
    table: Table,
    /// The timebase time the next own slot is looked for from, past the last one decided.
    after: i64,
    timebase_shown: Option<Timebase>,
    /// What the screens are shown.
    shown: Shown,
    /// The view [`publish`] fills.
    view: Box<MeshView>,
}

impl Mesh {
    pub async fn new(lora: SensorLora, dio0: Input<'static>, path: LoraPath, start: Start) -> Self {
        let group = start.group.map(|group| *group);
        let own = group.as_ref().map_or(0, Group::own);
        match &group {
            Some(group) => info!(
                "[MESH] id={} name={} members={} mac={=[u8]:02x}",
                own,
                group.me().name,
                group.count(),
                start.me.mac
            ),
            None => info!(
                "[MESH] in no group, name={} mac={=[u8]:02x}",
                start.me.name, start.me.mac
            ),
        }
        let now = local();
        let mut mesh = Self {
            lora,
            dio0,
            dio0_follows: true,
            path,
            me: start.me,
            group,
            founding: None,
            unsaved: 0,
            unsaved_group: false,
            clock: Clock::new(own, now),
            table: Table::new(own),
            after: i64::MIN,
            timebase_shown: None,
            shown: Shown::new(true),
            view: blank_view(),
        };
        let tuned = mesh.tune(FREQUENCY_HZ, SYNC_WORD, POWER_DBM).await;
        info!("[MESH] tuned={}", tuned);
        mesh.publish();
        mesh
    }

    #[inline(never)]
    fn publish(&mut self) {
        self.shown
            .fill(&mut self.view, &self.me, self.group.as_ref(), local());
        publish(&mut self.view);
    }

    async fn tune(&mut self, frequency: u32, sync_word: u8, power: u8) -> bool {
        let lora = &mut self.lora;
        let _ = lora.set_device_mode(DeviceMode::STDBY).await;
        let tuned = lora.set_frequency(frequency).await.is_ok()
            && lora.write(SYNC_WORD_REGISTER, sync_word).await.is_ok();
        let powered = match TxConfig::new(OCP::new(true, 120), power, PowerRamp::Us40, false) {
            Ok(config) => lora.configure_tx(config).await.is_ok(),
            Err(_) => false,
        };
        self.idle_receive().await;
        tuned && powered
    }

    /// Puts the radio in standby with its antenna on the receive path and `DIO0` on RxDone,
    /// whatever was interrupted.
    async fn idle_receive(&mut self) {
        let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
        let _ = self.lora.clear_all_interrupts().await;
        let _ = self.lora.map_dio0::<RxDone>().await;
        let _ = self.path.receive().await;
    }

    pub async fn run(mut self) -> ! {
        loop {
            self.publish();
            let command = if self.group.is_some() {
                match select(self.step(), COMMANDS.receive()).await {
                    Either::First(()) => continue,
                    Either::Second(command) => {
                        // The step may have stopped anywhere, a transmission included.
                        self.idle_receive().await;
                        command
                    }
                }
            } else if self.founding.is_some() {
                match select(self.await_joiner(), COMMANDS.receive()).await {
                    Either::First(()) => continue,
                    Either::Second(command) => {
                        self.idle_receive().await;
                        command
                    }
                }
            } else {
                let _ = self.lora.set_device_mode(DeviceMode::SLEEP).await;
                COMMANDS.receive().await
            };
            self.command(command).await;
        }
    }

    async fn command(&mut self, command: Command) {
        info!("[MESH] command {}", command);
        match command {
            Command::Add => self.pair(Role::Add).await,
            Command::Join if self.group.is_some() => {
                warn!("[MESH] in a group; it must leave before it can join another");
                self.shown.sessions += 1;
                self.shown.pairing =
                    Some(refused(self.shown.sessions, Role::Join, Refused::InGroup));
            }
            Command::Join => self.pair(Role::Join).await,
            Command::Leave => {
                self.founding = None;
                let left = leave(&mut self.group).await;
                if left {
                    self.unsaved = 0;
                    self.unsaved_group = false;
                    info!("[MESH] left the group");
                } else {
                    warn!("[MESH] not left");
                }
                self.shown.answer(Answer::Left(left));
            }
            Command::Rename(name) => {
                let group = match &mut self.founding {
                    Some(founding) => Some(&mut founding.0),
                    None => self.group.as_mut(),
                };
                let saved = rename(&mut self.me, group, name).await;
                info!("[MESH] renamed {} saved={}", name, saved);
                if saved && let Some(group) = &self.group {
                    self.unsaved |= 1 << group.own();
                }
                self.shown.answer(Answer::Renamed(saved));
            }
            Command::Choose(_)
            | Command::ChooseMac(_)
            | Command::Accept
            | Command::Decline
            | Command::Mismatch
            | Command::Cancel => warn!("[MESH] no pairing to take it"),
        }
    }

    /// Queues what changed in the group to be stored, as far as the queue has room.
    fn queue_unsaved(&mut self) {
        let Some(group) = &self.group else { return };
        if self.unsaved_group {
            if super::queue_group_write(GroupWrite::Group(Box::new(group.clone()))) {
                self.unsaved_group = false;
                self.unsaved = 0;
            }
            return;
        }
        while self.unsaved != 0 {
            let id = self.unsaved.trailing_zeros() as u8;
            let member = group.member(id).copied();
            if !super::queue_group_write(GroupWrite::Member { id, member }) {
                return;
            }
            self.unsaved &= !(1 << id);
        }
    }

    /// Waits for the node's next slot and sends in it, listening meanwhile.
    async fn step(&mut self) {
        self.queue_unsaved();
        let Some(own) = self.group.as_ref().map(Group::own) else {
            return;
        };
        let now = local();
        self.take_readings(own);
        self.clock.tick(now, rtc_now(now));
        let timebase = self.clock.at(now).map(|(_, timebase)| timebase);
        if timebase != self.timebase_shown {
            log_timebase(timebase, self.clock.is_sweeping(now));
            self.timebase_shown = timebase;
        }
        let Some((time, _)) = self.clock.at(now) else {
            let end = self.clock.sweep_ends(now).unwrap_or(now + ROUND_US);
            self.listen(end).await;
            return;
        };
        let (round, start) = next_slot((time + PREPARE_US).max(self.after), own);
        let send_at = start + (now - time);
        if self.listen(send_at - PREPARE_US).await {
            self.after = i64::MIN;
            return;
        }
        self.after = start + 1;
        let sending =
            self.table.wants_to_send(round) || self.group.as_ref().is_some_and(Group::has_unsent);
        info!(
            "[MESH] round={} sending={} sweeping={}",
            round,
            sending,
            self.clock.is_sweeping(local())
        );
        if sending {
            self.send(round, start, timebase, send_at).await;
        }
    }

    fn take_readings(&mut self, own: u8) {
        let gps = GPS_TIME.lock(Cell::get);
        if let Some(gps) = gps {
            self.clock.gps(gps.offset, gps.updated.as_micros() as i64);
            if let Some(fix) = FIX.lock(Cell::get) {
                self.table.set_own(Entry {
                    id: own,
                    latitude: fix.latitude,
                    longitude: fix.longitude,
                    stamp: fix.stamp,
                    quality: match fix.quality {
                        FixQuality::Autonomous => Quality::Autonomous,
                        FixQuality::Differential
                        | FixQuality::Pps
                        | FixQuality::Rtk
                        | FixQuality::FloatRtk => Quality::Differential,
                        _ => Quality::Estimated,
                    },
                    hdop: Hdop::from_milli(fix.hdop_milli.unwrap_or(u32::MAX)),
                });
                self.shown.positions(&self.table);
            }
        }
        if let Some((time, _)) = self.clock.at(local()) {
            self.table.expire((time / 1_000_000) as u32);
        }
    }

    /// Listens until local time `end`: throughout in a sweep or without a timebase, and otherwise
    /// in a window round the slot of each other member and of each id heard lately.
    /// Returns whether a packet moved the node to another timebase or another id, which moves
    /// every slot.
    async fn listen(&mut self, end: i64) -> bool {
        loop {
            let now = local();
            if now >= end {
                return false;
            }
            let (open, close) = match self.clock.at(now) {
                Some((time, _)) if !self.clock.is_sweeping(now) => {
                    let offset = now - time;
                    let from = time - GUARD_US - airtime_us(MAX_PACKET) + 1;
                    let window =
                        next_slot_in(from, self.listened(round_at(from))).map(|(_, start)| {
                            (
                                start - GUARD_US + offset,
                                start + GUARD_US + airtime_us(MAX_PACKET) + offset,
                            )
                        });
                    let sweep = self.clock.next_sweep(now).unwrap_or(i64::MAX);
                    match window {
                        Some((open, close)) if open < sweep => {
                            (open.max(now), close.min(sweep).min(end))
                        }
                        // The sweep round opens first: wait for it, and sweep from there.
                        _ => (sweep.max(now).min(end), sweep.max(now).min(end)),
                    }
                }
                _ => (
                    now,
                    self.clock.sweep_ends(now).map_or(end, |ends| ends.min(end)),
                ),
            };
            if open >= end {
                let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
                until(end).await;
                return false;
            }
            if open > now {
                let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
                until(open).await;
            }
            if self.lora.rx(None).await.is_err() {
                warn!("[MESH] receive start failed");
            }
            while self.wait_for(IRQ_RX_DONE, close).await {
                if self.receive().await {
                    let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
                    return true;
                }
            }
            let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
        }
    }

    /// The ids whose slots the node listens to outside a sweep in `round`: the group's other
    /// members, and any other id heard in the rounds a neighbour counts for. A member the group
    /// does not know of yet is found by a sweep.
    fn listened(&self, round: i64) -> u32 {
        let Some(group) = &self.group else {
            return 0;
        };
        (group.ids() | self.table.neighbours(round)) & !(1 << group.own())
    }

    /// Reads the packet `DIO0` reported, and when its RxDone was seen.
    async fn read_packet(&mut self) -> Option<(RxPacket, i64)> {
        let done = local();
        let packet = self.lora.rx_packet().await;
        let flags = self.lora.read(IRQ_FLAGS).await.ok();
        let _ = self.lora.clear_all_interrupts().await;
        match packet {
            Ok(packet) => Some((packet, done)),
            Err(error) => {
                warn!(
                    "[MESH] receive failed: {} flags={}",
                    defmt::Debug2Format(&error),
                    flags
                );
                // The driver finds no RxDone, so DIO0 rose for nothing.
                if matches!(error, Sx127xError::PacketNotReady) && self.dio0_follows {
                    warn!(
                        "[MESH] DIO0 is high with no flag raised; polling the radio's flags from here"
                    );
                    self.dio0_follows = false;
                }
                None
            }
        }
    }

    /// Takes the packet `DIO0` reported. Returns whether it moved the node to its timebase, or
    /// to another id.
    async fn receive(&mut self) -> bool {
        let Some((packet, done)) = self.read_packet().await else {
            return false;
        };
        self.take(&packet, done)
    }

    /// Takes a packet whose RxDone was seen at local time `done`. Returns whether it moved the
    /// node to its timebase, or to another id.
    fn take(&mut self, packet: &RxPacket, done: i64) -> bool {
        let Some(group) = &mut self.group else {
            return false;
        };
        let len = packet.length;
        let mut bytes = packet.payload;
        let Ok(plain) = seal::open(group.key(), &mut bytes[..len]) else {
            info!("[MESH] not ours len={} rssi={}", len, packet.rssi);
            return false;
        };
        let Ok(plain) = Plain::parse(plain) else {
            warn!("[MESH] malformed len={}", len);
            return false;
        };
        let header = plain.header;
        let polled = if self.dio0_follows {
            0
        } else {
            POLL_US as i64 / 2
        };
        let start = done - airtime_us(len) - ARRIVAL_LATENCY_US - polled;
        let arrival = self.clock.arrival(&header, start, done);
        if let Some(heard) = self.shown.heard.get_mut(usize::from(header.sender)) {
            *heard = Some(done);
        }
        let round = round_at(named_slot(header.base, header.sender));
        self.table.heard(header.sender, round);
        let now = self
            .clock
            .at(done)
            .map(|(time, _)| (time / 1_000_000) as u32);
        let (mut entries, mut news, mut neighbours, mut moved) = (0, 0, 0, false);
        let mut carried = [None; IDS as usize];
        let mut heard_record = None;
        for record in plain.records() {
            match record {
                Record::Positions(positions) => {
                    for entry in positions {
                        entries += 1;
                        if let Some(stamp) = carried.get_mut(usize::from(entry.id)) {
                            *stamp = Some(entry.stamp);
                        }
                        if matches!(self.table.merge(entry, now), Merge::New | Merge::Newer) {
                            news += 1;
                        }
                    }
                }
                Record::Neighbours(set) => neighbours = set,
                Record::Member(id, member) => {
                    heard_record = Some((id, member));
                    match group.merge(id, member, now.unwrap_or_else(|| utc_seconds(done))) {
                        Merged::Unchanged => {}
                        Merged::Changed { vacated } => {
                            info!("[MESH] member {} is {} now", id, member.name);
                            self.unsaved |= 1 << id;
                            if let Some(vacated) = vacated {
                                self.unsaved |= 1 << vacated;
                            }
                        }
                        Merged::Renumbered { from, to } => {
                            warn!(
                                "[MESH] another device keeps id {}; this one moves to {}",
                                from, to
                            );
                            self.clock.renumber(to);
                            self.table.renumber(to);
                            self.unsaved_group = true;
                            moved = true;
                        }
                    }
                }
                Record::Other(..) => {}
            }
        }
        if self
            .table
            .covered_by(header.sender, &carried, neighbours, round)
            && let Some((id, member)) = heard_record
        {
            group.covered(id, &member);
        }
        self.shown.positions(&self.table);
        self.publish();
        info!(
            "[MESH] heard id={} timebase={} hops={} taken={} late_us={} len={} rssi={} snr={} entries={} new={} neighbours={=u32:#010x}",
            header.sender,
            header.timebase.source,
            header.timebase.hops,
            arrival.taken,
            arrival.late_us,
            len,
            packet.rssi,
            packet.snr,
            entries,
            news,
            neighbours,
        );
        arrival.taken == Taken::Adopted || moved
    }

    /// Sends this node's packet in its slot in `round`, which starts at timebase time `start` and
    /// local time `send_at`.
    async fn send(&mut self, round: i64, start: i64, timebase: Option<Timebase>, send_at: i64) {
        let (Some(timebase), Some(group)) = (timebase, &mut self.group) else {
            return;
        };
        let base = base_of(start);
        let header = Header {
            sender: group.own(),
            timebase,
            base,
            phase: 0,
        };
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Builder::new(&mut packet[SIV_LEN..], &header);
        let neighbours = self.table.neighbours(round);
        let _ = builder.neighbours(neighbours);
        // Ahead of the positions, so a busy table never crowds it out.
        let (member_id, member) = group.next_record();
        let _ = builder.member(member_id, &member);
        let mut entries = [Entry {
            id: 0,
            latitude: 0,
            longitude: 0,
            stamp: 0,
            quality: Quality::Reserved,
            hdop: Hdop::from_milli(0),
        }; MAX_ENTRIES];
        let room = builder.room_for_entries().min(MAX_ENTRIES);
        let n = self.table.digest(base, &mut entries[..room]);
        if n > 0 && builder.positions(&entries[..n]).is_err() {
            warn!("[MESH] positions did not fit");
        }
        let plain_len = builder.finish();
        let len = seal::seal(group.key(), &mut packet, plain_len);

        let Some((done, flags)) = self.transmit(&packet[..len], Some(send_at)).await else {
            return;
        };
        self.table.sent(&entries[..n]);
        info!(
            "[MESH] sent round={} len={} entries={} member={} neighbours={=u32:#010x} done={} flags={}",
            round, len, n, member_id, neighbours, done, flags
        );
    }

    /// Sends `packet`, at local time `at` or at once. Returns whether TxDone was seen and the
    /// flags after, and leaves the radio as [`Mesh::idle_receive`] does.
    async fn transmit(&mut self, packet: &[u8], at: Option<i64>) -> Option<(bool, Option<u8>)> {
        if self.load(packet).await.is_err() {
            warn!("[MESH] loading a {}-byte packet failed", packet.len());
            self.idle_receive().await;
            return None;
        }
        let _ = self.path.transmit().await;
        if let Some(at) = at {
            until(at).await;
        }
        let started = local();
        if let Some(at) = at
            && started - at > 1_000
        {
            warn!("[MESH] sent {}us late", started - at);
        }
        let _ = self.lora.set_device_mode(DeviceMode::TX).await;
        let done = self.wait_for(IRQ_TX_DONE, started + SEND_TIMEOUT_US).await;
        let flags = self.lora.read(IRQ_FLAGS).await.ok();
        if self.dio0_follows && !done && flags.is_some_and(|flags| flags & IRQ_TX_DONE != 0) {
            warn!("[MESH] DIO0 did not rise for TxDone; polling the radio's flags from here");
            self.dio0_follows = false;
        }
        self.idle_receive().await;
        Some((done, flags))
    }

    /// Runs a pairing in `role` on the pairing channel until it ends, then returns to the mesh's
    /// channel. A node that joins, or founds a group, starts its timebase afresh.
    async fn pair(&mut self, role: Role) {
        if self.founding.take().is_some() {
            info!("[MESH] no longer listening for the device a founding left unconfirmed");
        }
        self.shown.sessions += 1;
        let session = self.shown.sessions;
        let (Some(nonce), Some(founding)) = (random::<16>(), random::<32>()) else {
            warn!("[PAIR] no true random source; not pairing");
            self.shown.pairing = Some(refused(session, role, Refused::NoRandom));
            return;
        };
        if !self
            .tune(PAIR_FREQUENCY_HZ, PAIR_SYNC_WORD, PAIR_POWER_DBM)
            .await
        {
            warn!("[PAIR] tuning to the pairing channel failed");
        }
        let now = local();
        let had_group = self.group.is_some();
        let mut pairing = match role {
            Role::Join => Pairing::join(&self.me, nonce, now),
            Role::Add => {
                let utc = utc_seconds(now);
                let group = self.group.clone().unwrap_or_else(|| {
                    Group::found(
                        Key::new(founding),
                        Member {
                            public: self.me.public(),
                            joined: utc,
                            changed: utc,
                            mac: self.me.mac,
                            name: self.me.name,
                        },
                    )
                });
                Pairing::add(&self.me, group, nonce, now, utc)
            }
        };
        let mut frame = [0u8; MAX_FRAME];
        let mut shown = None;
        let mut listening = false;
        let mut saving: Option<Instant> = None;
        loop {
            let now = local();
            if let Some(len) = pairing.poll(now, &mut frame) {
                let sent = self.transmit(&frame[..len], None).await;
                debug!(
                    "[PAIR] sent kind={} len={} done={}",
                    frame[1],
                    len,
                    sent.is_some_and(|(done, _)| done)
                );
                listening = false;
                continue;
            }
            let phase = pairing.phase();
            if shown != Some(phase) {
                log_pairing(&pairing, phase);
                shown = Some(phase);
            }
            let view = pairing_view(session, &pairing);
            if self.shown.pairing.as_ref() != Some(&view) {
                self.shown.pairing = Some(view);
                // A pairing that is done has stored its group, which the screens show at once.
                let group = match phase {
                    Phase::Done(_) => pairing.group(),
                    _ => self.group.as_ref(),
                };
                self.shown.fill(&mut self.view, &self.me, group, now);
                publish(&mut self.view);
            }
            if pairing.is_over(now) {
                break;
            }
            if phase == Phase::Storing && saving.is_none() {
                let group = pairing
                    .group()
                    .expect("a pairing storing has a group")
                    .clone();
                super::GROUP_SAVED.reset();
                super::send_group_write(GroupWrite::Group(Box::new(group))).await;
                saving = Some(Instant::now());
                continue;
            }
            if phase == Phase::Storing
                && saving.is_some_and(|started| started.elapsed() > STORE_TIMEOUT)
            {
                warn!("[PAIR] storing the group timed out");
                pairing.stored(false, now);
                continue;
            }
            if !listening {
                if self.lora.rx(None).await.is_err() {
                    warn!("[PAIR] receive start failed");
                }
                listening = true;
            }
            let wake = pairing.wake_at().min(now + PAIR_LISTEN_US);
            match select3(
                self.wait_for(IRQ_RX_DONE, wake),
                COMMANDS.receive(),
                super::GROUP_SAVED.wait(),
            )
            .await
            {
                Either3::First(true) => {
                    if let Some((packet, done)) = self.read_packet().await {
                        #[cfg(feature = "pair-inject")]
                        if inject::is_deaf() {
                            info!("[PAIR] deaf to kind={}", packet.payload[1]);
                            continue;
                        }
                        debug!(
                            "[PAIR] heard kind={} len={} rssi={} snr={}",
                            packet.payload[1], packet.length, packet.rssi, packet.snr
                        );
                        let mut payload = packet.payload;
                        let started = local();
                        pairing.receive(&mut payload[..packet.length], done);
                        let took = local() - started;
                        if took > 5_000 {
                            info!("[PAIR] a frame took {}us to take", took);
                        }
                    }
                }
                Either3::First(false) => {}
                Either3::Second(command) => {
                    let now = local();
                    info!("[PAIR] command {}", command);
                    match command {
                        Command::Choose(index) => pairing.choose(usize::from(index), now),
                        Command::ChooseMac(mac) => {
                            let index = pairing.candidates().position(|found| found == mac);
                            if let Some(index) = index {
                                pairing.choose(index, now);
                            }
                        }
                        Command::Accept => pairing.accept(now),
                        Command::Decline => pairing.reject(false, now),
                        Command::Mismatch => pairing.reject(true, now),
                        Command::Cancel => pairing.cancel(now),
                        _ => warn!("[PAIR] not while pairing"),
                    }
                }
                Either3::Third(ok) => {
                    if saving.is_some() && phase == Phase::Storing {
                        pairing.stored(ok, local());
                    }
                }
            }
        }
        match (pairing.phase(), pairing.group()) {
            (Phase::Done(_), Some(group)) => {
                self.group = Some(group.clone());
                self.unsaved = 0;
                self.unsaved_group = false;
                if role == Role::Join || !had_group {
                    self.restart(group.own());
                }
            }
            (Phase::Ended(End::Unconfirmed), Some(group)) if !had_group => {
                info!("[MESH] listening for the joining device under the founded group's key");
                self.founding = Some(Box::new((group.clone(), local() + FOUNDING_WAIT_US)));
            }
            _ => {}
        }
        if !self.tune(FREQUENCY_HZ, SYNC_WORD, POWER_DBM).await {
            warn!("[MESH] tuning back to the mesh's channel failed");
        }
    }

    /// Starts the timebase, the table and what the screens show of them afresh, at id `own`.
    fn restart(&mut self, own: u8) {
        self.clock = Clock::new(own, local());
        self.table = Table::new(own);
        self.after = i64::MIN;
        self.timebase_shown = None;
        self.shown.heard = [None; IDS as usize];
        self.shown.positions = [None; IDS as usize];
    }

    /// Listens throughout for a packet under the group a founding left unconfirmed, until its
    /// wait ends. One means the joining device stored the group, which this device then takes
    /// up and stores.
    async fn await_joiner(&mut self) {
        let Some(end) = self.founding.as_ref().map(|founding| founding.1) else {
            return;
        };
        if self.lora.rx(None).await.is_err() {
            warn!("[MESH] receive start failed");
        }
        while self.wait_for(IRQ_RX_DONE, end).await {
            let Some((packet, done)) = self.read_packet().await else {
                continue;
            };
            let Some(founded) = self.founding.as_ref().map(|founding| &founding.0) else {
                return;
            };
            let mut bytes = packet.payload;
            if seal::open(founded.key(), &mut bytes[..packet.length]).is_err() {
                info!("[MESH] not ours len={} rssi={}", packet.length, packet.rssi);
                continue;
            }
            let (founded, _) = *self.founding.take().expect("checked above");
            info!("[MESH] heard the joining device; the founded group is this device's");
            let own = founded.own();
            self.group = Some(founded);
            self.unsaved = 0;
            self.unsaved_group = true;
            self.restart(own);
            self.take(&packet, done);
            let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
            return;
        }
        info!("[MESH] the joining device was not heard; this device founded no group");
        self.founding = None;
        let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
    }

    /// Waits until local time `deadline` for the radio to raise `flag`, the one `DIO0` is mapped
    /// to. Returns whether it did.
    async fn wait_for(&mut self, flag: u8, deadline: i64) -> bool {
        if self.dio0_follows {
            return matches!(
                select(self.dio0.wait_for_high(), until(deadline)).await,
                Either::First(())
            );
        }
        loop {
            if self
                .lora
                .read(IRQ_FLAGS)
                .await
                .is_ok_and(|flags| flags & flag != 0)
            {
                return true;
            }
            if local() >= deadline {
                return false;
            }
            Timer::after(Duration::from_micros(POLL_US)).await;
        }
    }

    /// Puts the packet in the radio's FIFO, ready to send on one mode change.
    async fn load(&mut self, packet: &[u8]) -> Result<(), ()> {
        let lora = &mut self.lora;
        lora.set_device_mode(DeviceMode::STDBY)
            .await
            .map_err(|_| ())?;
        lora.map_dio0::<TxDone>().await.map_err(|_| ())?;
        lora.clear_all_interrupts().await.map_err(|_| ())?;
        lora.write(FIFO_TX_BASE_ADDR, FIFO_TX_BASE_ADDR_VALUE)
            .await
            .map_err(|_| ())?;
        lora.write(FIFO_ADDR_PTR, FIFO_TX_BASE_ADDR_VALUE)
            .await
            .map_err(|_| ())?;
        lora.write_fifo(packet).await.map_err(|_| ())?;
        lora.set_payload_length(packet.len() as u8)
            .await
            .map_err(|_| ())
    }
}

fn log_pairing(pairing: &Pairing, phase: Phase) {
    let left = pairing
        .deadline()
        .map_or(0, |deadline| (deadline - local()).max(0) / 1_000_000);
    match phase {
        Phase::Found => {
            for (i, mac) in pairing.candidates().enumerate() {
                info!("[PAIR] found {}: {=[u8]:02x}", i, mac);
            }
        }
        Phase::Compare { code } => info!(
            "[PAIR] code {=u32:06} with {=[u8]:02x}; {}s to confirm",
            code,
            pairing.peer().unwrap_or_default(),
            left
        ),
        _ => {}
    }
    info!("[PAIR] {} {} ({}s left)", pairing.role(), phase, left);
}

/// The RTC's UTC in microseconds at local time `now`, to the RTC's whole second.
fn rtc_now(now: i64) -> Option<i64> {
    RTC_TIME
        .lock(Cell::get)
        .map(|(seconds, read)| seconds * 1_000_000 + now - read.as_micros() as i64)
}

fn log_timebase(timebase: Option<Timebase>, sweeping: bool) {
    match timebase {
        None => info!("[MESH] no timebase, sweeping={}", sweeping),
        Some(Timebase {
            source: Source::Gps,
            hops,
        }) => info!("[MESH] timebase GPS hops={}", hops),
        Some(Timebase {
            source: Source::Node(root),
            hops,
        }) => info!("[MESH] timebase root={} hops={}", root, hops),
    }
}

/// Stands in for the mesh on a board whose radio did not answer: it keeps this device's name and
/// can leave the group, and tells the screens there is no radio to pair with.
#[embassy_executor::task]
pub async fn offline(start: Start) {
    let mut me = start.me;
    let mut group = start.group.map(|group| *group);
    let mut shown = Shown::new(false);
    let mut view = blank_view();
    loop {
        shown.fill(&mut view, &me, group.as_ref(), local());
        publish(&mut view);
        match COMMANDS.receive().await {
            Command::Leave => {
                let left = leave(&mut group).await;
                shown.answer(Answer::Left(left));
            }
            Command::Rename(name) => {
                let saved = rename(&mut me, group.as_mut(), name).await;
                if saved && let Some(group) = &group {
                    let id = group.own();
                    super::queue_group_write(GroupWrite::Member {
                        id,
                        member: group.member(id).copied(),
                    });
                }
                shown.answer(Answer::Renamed(saved));
            }
            command => warn!("[MESH] no radio for {}", command),
        }
    }
}

/// Lets a debugger give the mesh commands in place of the screens; `tools/pair-inject.py`
/// does.
#[cfg(feature = "pair-inject")]
pub mod inject {
    use core::sync::atomic::{AtomicU32, Ordering};

    use embassy_time::{Duration, Instant, Timer};
    use octowhere_mesh::members::Name;

    use super::{COMMANDS, Command};

    /// The command's code in the low byte and its argument in the next. The debugger writes it
    /// last.
    #[unsafe(no_mangle)]
    static OCTOWHERE_PAIR_COMMAND: AtomicU32 = AtomicU32::new(0);
    /// A new name's bytes, little-endian in each word; the command's argument is its length.
    #[unsafe(no_mangle)]
    static OCTOWHERE_PAIR_NAME: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
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
                10 => {
                    let now = Instant::now().as_millis() as u32;
                    DEAF_UNTIL_MS.store(now + 1000 * u32::from(argument), Ordering::Relaxed);
                    defmt::info!("[PAIR] deaf for {}s", argument);
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
