//! Runs the location mesh on a radio: sends this node's packet in its slot, listens to the
//! other slots, and keeps the timebase the slots are placed on. Pairing takes the radio over, on
//! a channel of its own, until it ends. A refresh listens throughout for three rounds when the
//! screens ask. `octowhere_mesh` holds the protocol and `context/LORA-PROTOCOL.md` the design.

use alloc::boxed::Box;
use core::alloc::Allocator;

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn};
use embassy_futures::select::{Either, Either3, select, select3};
use octowhere_mesh::{
    IDS, Ids, Zeroable,
    absorb::{Event, State, When, absorb},
    clock::{Clock, SWEEP_US, Taken, UTC_BOUND_US},
    compose::{Sources, compose},
    members::{Gone, Group, Member, Name, Requests, fingerprint},
    messages::{
        self, BODY_MAX, Insert, Message, Pairwise, Sequence, Store, Summaries, TEXT_MAX, To, kind,
    },
    packet::{
        Builder, Entry, HEADER_LEN, Hdop, Header, MAX_PACKET, Plain, Quality, Record, Source,
        Timebase,
    },
    pair::{End, Identity, MAX_FRAME, Pairing, Phase, Role},
    rekey::{Learned, NewKey, Rekey, key_fingerprint},
    schedule::{
        GUARD_US, ROUND_US, SWEEP_EVERY, Schedule, airtime_us, is_sweep_round, round_at,
        round_start_s, second_at, stored_round_at,
    },
    seal::{self, Key, SIV_LEN},
    table::Table,
};

use crate::{
    GroupWrite,
    fmt::{Ascii, Mac},
    inbox::Inbox,
    removals::Removals,
    unsaved::{Due, Unsaved},
    view::{
        Answer, Decline, GroupView, MemberView, MeshView, MessagesView, PairingView, Position,
        RecoveryPhase, RecoveryView, RefreshPhase, RefreshView, Refused, RemovalStage, RemovalView,
        RemovalsView, Request, Text, Unremovable,
    },
};

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
/// How long a pairing waits for the group to be stored before it counts as a failure.
const STORE_TIMEOUT_US: i64 = 10 * 1_000_000;
/// A notice is a header alone.
const NOTICE_LEN: usize = SIV_LEN + HEADER_LEN;
/// The longest a pairing listens before it runs its timers again.
const PAIR_LISTEN_US: i64 = 250_000;
/// How long a device that founded a group, without hearing the last acknowledgement, listens
/// for the joining device under the group's key. That device waits 30 s for done, sweeps for
/// three rounds, then sends in its next slot, since it hears nobody: about 3½ minutes in all.
const FOUNDING_WAIT_US: i64 = 10 * 60 * 1_000_000;
/// How long after a failed write a founding's wait tries to store its group again.
const STORE_RETRY_US: i64 = 10 * 1_000_000;
/// The messages made here that can wait for a sequence number and a timebase.
const OUTBOX: usize = 8;
/// The slots a device that leaves sends its gone record in.
const LEAVE_REPEATS: u8 = 2;
/// How long a device that left tries to tell the others: its next slots, a round apart.
const LEAVE_WAIT_US: i64 = 3 * ROUND_US;

#[derive(Clone, Copy, Debug)]
pub struct Fix {
    /// Degrees × 10⁷.
    pub latitude: i32,
    pub longitude: i32,
    /// UTC seconds.
    pub stamp: u32,
    pub quality: Quality,
    pub hdop_milli: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    /// Listens throughout for three rounds.
    Refresh,
    /// Sends text to one member, privately, or with `None` to the whole group.
    Send {
        to: Option<u8>,
        text: Text,
    },
    /// Counts the message this device numbered so as read.
    Read(u32),
    /// Removes the member with this id from the group.
    Remove(u8),
    /// Removes the member at `id`, if it is still the device with this fingerprint.
    RemoveDevice {
        id: u8,
        device: [u8; 8],
    },
    /// Declines the removal whose new key has this fingerprint.
    KeepKey([u8; 8]),
    /// Declines the removal of the member with this id that another member asked for, before
    /// its switch or within a day after.
    Keep(u8),
    /// Enrols a member no device stands behind, for removing on a bench of two boards.
    #[cfg(feature = "phantom")]
    Phantom,
}

/// A message made here, waiting for a sequence number and a timebase: its kind and contents.
#[derive(Clone, Copy)]
struct Outgoing {
    to: To,
    /// A key message's generation: it goes out signed, with its generation in the clear.
    generation: Option<u16>,
    /// The number the screens know a text by, 0 for anything else.
    shown: u32,
    len: u8,
    plain: [u8; BODY_MAX],
}

impl Outgoing {
    fn new(to: To, generation: Option<u16>, plain: &[u8]) -> Self {
        let mut bytes = [0; BODY_MAX];
        bytes[..plain.len()].copy_from_slice(plain);
        Self {
            to,
            generation,
            shown: 0,
            len: plain.len() as u8,
            plain: bytes,
        }
    }
}

/// A value made in place in `alloc`, as the message store is: tens of kilobytes, too large to
/// build on the stack.
fn zeroed_in<T: Zeroable, A: Allocator>(alloc: A) -> Box<T, A> {
    // SAFETY: `Zeroable` promises that all zeroes is a valid `T`.
    unsafe { Box::new_zeroed_in(alloc).assume_init() }
}

/// What became of a message new to this node.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrival {
    /// Acted on, or there was nothing to act on.
    Done,
    /// Not yet: the sender's record or a timebase was missing, so it is tried again later.
    Later,
}

/// What [`Mesh::make`] made of a message.
enum Made {
    /// Its sequence number's block is not stored yet.
    Wait,
    Dropped,
    Message(Message),
}

/// The most a summary takes of a packet, so that it leaves room for messages.
const SUMMARY_MAX: usize = 120;

/// Which key a packet heard opened under.
enum Opened {
    Current,
    /// The one the group is about to switch to, which its sender already has.
    Pending {
        header: Header,
    },
    /// One the group switched away from, which its sender has not: the key, its generation, and
    /// that of the key message that catches its sender up.
    Old {
        header: Header,
        old: (Key, u16, u16),
    },
    Not,
}

/// What a step sends next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    /// A notice, at local time `at`.
    Notice { at: i64 },
    /// A packet under an old key, at local time `at`, in `round` of that key's order.
    Old { at: i64, round: i64 },
    /// This node's own packet, in its slot.
    Own,
}

impl Next {
    /// What goes out next, with this node's own slot at local time `send_at`: a notice due at
    /// local time `notice`, or else a packet under an old key due at the local time and round
    /// `old`, if it ends before the own slot with time to prepare for that; or else the own
    /// packet.
    fn choose(send_at: i64, notice: Option<i64>, old: Option<(i64, i64)>) -> Self {
        if let Some(at) = notice
            && at + airtime_us(NOTICE_LEN) + PREPARE_US < send_at
        {
            return Self::Notice { at };
        }
        if let Some((at, round)) = old
            && at + airtime_us(MAX_PACKET) + PREPARE_US < send_at
        {
            return Self::Old { at, round };
        }
        Self::Own
    }
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
            Request::Refresh => Self::Refresh,
            Request::Send { to, text } => Self::Send { to, text },
            Request::Read(id) => Self::Read(id),
            Request::Remove { id, device } => Self::RemoveDevice { id, device },
            Request::Keep { key } => Self::KeepKey(key),
        }
    }
}

/// The mesh's state as the firmware starts.
pub struct Start {
    pub me: Identity,
    pub group: Option<Box<Group>>,
    /// The end of the block of message sequence numbers reserved last.
    pub sequence: Option<u32>,
    /// The group's removals, as stored.
    pub rekey: Option<Box<Rekey>>,
}

/// A view to fill, on the heap. A view is a couple of kilobytes, and the radio's task runs on
/// whatever stack the frame loop left, so none is built or copied on the stack.
#[inline(never)]
pub fn blank_view() -> Box<MeshView> {
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

/// Publishes the stored identity and group before the radio is known, for the screens to show
/// from the start.
pub fn publish_start(start: &Start, now: i64, device: &impl Device) {
    let mut view = blank_view();
    Shown::new(false).fill(
        &mut view,
        &start.me,
        start.group.as_deref(),
        now,
        utc_now(device, now),
    );
    device.publish(&mut view);
}

/// What the screens are shown beyond the group itself.
struct Shown {
    radio: bool,
    sessions: u32,
    pairing: Option<PairingView>,
    answered: u32,
    answer: Option<Answer>,
    /// The node's `heard` and `refresh`, copied in as it renders. Protocol decisions read the
    /// node's own, never these.
    heard: [Option<i64>; IDS as usize],
    refresh: Option<RefreshView>,
    /// The newest position held for each id: its UTC second and its coordinates. Kept past the
    /// table's expiry, so an old position shows as old rather than never received.
    positions: [Option<(u32, (i32, i32))>; IDS as usize],
    /// A founding's wait, under way or ended, until another pairing starts.
    recovery: Option<RecoveryView>,
    removals: RemovalsView,
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
            refresh: None,
            recovery: None,
            removals: RemovalsView::default(),
        }
    }

    fn answer(&mut self, answer: Answer) {
        self.answered += 1;
        self.answer = Some(answer);
    }

    /// Notes the newest of the positions `table` holds.
    fn positions(&mut self, table: &Table) {
        for entry in table.entries() {
            if let Some(held) = self.positions.get_mut(usize::from(entry.id))
                && held.is_none_or(|(stamp, _)| entry.stamp >= stamp)
            {
                *held = Some((entry.stamp, (entry.latitude, entry.longitude)));
            }
        }
    }

    /// Makes `view` the view of `me` in `group` at local time `now`. Times in UTC become local
    /// times on the stage's clock, which is the same as this one.
    fn fill(
        &self,
        view: &mut MeshView,
        me: &Identity,
        group: Option<&Group>,
        now: i64,
        utc: Option<i64>,
    ) {
        // Named in full, so that a field added to the view cannot be left unfilled.
        let MeshView {
            radio,
            mac,
            name,
            group: shown_group,
            sessions,
            pairing,
            answered,
            answer,
            refresh,
            recovery,
            removals,
        } = view;
        *radio = self.radio;
        *mac = me.mac;
        *name = me.name;
        *sessions = self.sessions;
        pairing.clone_from(&self.pairing);
        *answered = self.answered;
        *answer = self.answer;
        *refresh = self.refresh;
        *recovery = self.recovery;
        *removals = self.removals;
        let Some(group) = group else {
            *shown_group = None;
            return;
        };
        let local_at = |stamp: u32| utc.map(|utc| now - (utc - i64::from(stamp)) * 1_000_000);
        let shown = match shown_group {
            Some(shown) => shown,
            None => blank_group(shown_group),
        };
        shown.own = group.own();
        for (index, slot) in shown.members.iter_mut().enumerate() {
            let id = index as u8;
            *slot = group.member(id).map(|member| MemberView {
                name: member.name,
                mac: member.mac,
                // PERF: a SHA-256 for every member at every publish.
                device: fingerprint(&member.public),
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
                    Some((stamp, _)) => local_at(stamp).map_or(Position::Unknown, Position::At),
                },
                coordinates: self.positions[index].map(|(_, coordinates)| coordinates),
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

/// Forgets the group, once that is stored, and hands it back to be told of the leaving. A node
/// in no group has nothing to forget.
async fn leave(group: &mut Option<Group>, store: &impl GroupStore) -> (bool, Option<Group>) {
    if group.is_none() {
        return (true, None);
    }
    if !store.save(GroupWrite::Leave).await {
        return (false, None);
    }
    (true, group.take())
}

/// Stores `name` as this device's, and only then takes it up, so a failed write leaves the
/// name that is stored.
async fn rename(
    me: &mut Identity,
    group: Option<&mut Group>,
    name: Name,
    utc: u32,
    store: &impl GroupStore,
) -> bool {
    if !store.save(GroupWrite::Name(name)).await {
        return false;
    }
    me.name = name;
    if let Some(group) = group {
        group.rename(name, utc, me);
    }
    true
}

/// UTC seconds at local time `now`, from GNSS or the RTC, or 0 with neither.
fn utc_seconds(device: &impl Device, now: i64) -> u32 {
    utc_now(device, now).map_or(0, |utc| utc.clamp(0, i64::from(u32::MAX)) as u32)
}

/// UTC seconds at local time `now`, from GNSS or the RTC.
fn utc_now(device: &impl Device, now: i64) -> Option<i64> {
    device
        .gps_time()
        .map(|gps| now - gps.offset)
        .or_else(|| device.rtc_utc(now))
        .map(|utc| utc / 1_000_000)
}

/// A group this device left, kept until it has told the others in its own slots.
struct Leaving {
    group: Group,
    gone: Gone,
    /// The slots it is still to be sent in.
    left: u8,
    /// When it is given up, on the local clock.
    until: i64,
}

/// A group founded in a pairing whose last acknowledgement never came. A packet under its key
/// shows the joining device stored it; this device then stores it too, and only then takes it
/// up.
struct Founding {
    group: Group,
    /// When the wait ends, on the local clock.
    until: i64,
    /// The joining device was heard, so only the write is left.
    heard: bool,
}

/// The longest packet a radio takes.
pub const RECEIVED_MAX: usize = 255;

/// A packet a radio received.
pub struct Received {
    pub payload: [u8; RECEIVED_MAX],
    pub length: usize,
    pub rssi: i16,
    pub snr: i16,
}

/// The radio a node runs on, at local times in microseconds.
pub trait Radio {
    /// Tunes to `frequency` with `sync_word`, sending at `power` dBm, and leaves the radio as
    /// [`Radio::idle_receive`] does. Returns whether every setting took.
    async fn tune(&mut self, frequency: u32, sync_word: u8, power: u8) -> bool;
    /// Puts the radio in standby, ready to receive, whatever was interrupted.
    async fn idle_receive(&mut self);
    async fn standby(&mut self);
    async fn sleep(&mut self);
    /// Starts receiving until the radio is put in standby. Returns whether it started.
    async fn start_receiving(&mut self) -> bool;
    /// Waits until local time `deadline` for a packet to arrive. Returns whether one did.
    async fn wait_received(&mut self, deadline: i64) -> bool;
    /// Reads the packet that arrived, and when its end was seen.
    async fn read_packet(&mut self) -> Option<(Received, i64)>;
    /// How late, on average, a packet's end is seen after it ends.
    fn seen_late_us(&self) -> i64;
    /// Sends `packet`, at local time `at` or at once, and leaves the radio as
    /// [`Radio::idle_receive`] does. Returns whether it was seen to finish, or `None` when it
    /// could not be loaded.
    async fn transmit(&mut self, packet: &[u8], at: Option<i64>) -> Option<bool>;
}

/// The local clock a node runs on, in microseconds.
pub trait Time {
    fn now(&self) -> i64;
    async fn until(&self, at: i64);
}

/// Where a node's keys and nonces come from.
pub trait Random {
    /// Fills `out` with random bytes. Returns false where there is no source.
    fn fill(&mut self, out: &mut [u8]) -> bool;

    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let mut bytes = [0; N];
        self.fill(&mut bytes).then_some(bytes)
    }
}

/// UTC on the local clock: UTC in microseconds is local time less `offset`, as fixes last
/// refined it at local time `updated`.
#[derive(Clone, Copy, Debug)]
pub struct GpsTime {
    pub offset: i64,
    pub updated: i64,
}

/// The device a node runs in: what its other parts know, and the screens it shows its state on.
pub trait Device {
    /// The latest fix, and the UTC second it was made in.
    fn fix(&self) -> Option<Fix>;
    fn gps_time(&self) -> Option<GpsTime>;
    /// The RTC's UTC in microseconds at local time `now`, to its whole second, unless its
    /// oscillator stopped.
    fn rtc_utc(&self, now: i64) -> Option<i64>;
    /// Shows the screens `view`, if it changed. It may trade places with an older view.
    fn publish(&self, view: &mut Box<MeshView>);
    /// Shows the screens `messages`. Returns false when they cannot take them now, to be offered
    /// again later.
    fn publish_messages(&self, messages: &MessagesView) -> bool;
}

/// Where a node's commands come from.
pub trait Commands {
    async fn receive(&self) -> Command;
}

/// Where a node's group and its removals are kept across restarts, a write at a time in the
/// order they were queued. A write can fail.
pub trait GroupStore {
    /// Queues `write` without waiting, and returns its number for [`GroupStore::result`], or `None`
    /// when the queue is full.
    fn queue(&self, write: GroupWrite) -> Option<u32>;
    /// Queues `write`, waiting for room, and returns its number for [`GroupStore::saved`].
    async fn send(&self, write: GroupWrite) -> u32;
    /// Whether the write numbered `number` was stored, once it is done.
    fn result(&self, number: u32) -> Option<bool>;
    /// Waits for the write numbered `number`, and says whether it was stored.
    async fn saved(&self, number: u32) -> bool;

    /// Stores `write`, and says whether it was. It waits however long the writes ahead of it
    /// take: one that gave up early would report a write as failed that may still land.
    async fn save(&self, write: GroupWrite) -> bool {
        let number = self.send(write).await;
        self.saved(number).await
    }
}

pub struct Mesh<R, T, G, D, S, A: Allocator> {
    radio: R,
    time: T,
    random: G,
    device: D,
    store: S,
    me: Identity,
    group: Option<Group>,
    /// A group this device founded in a pairing whose last acknowledgement it never heard.
    founding: Option<Box<Founding>>,
    /// A group this device left and has yet to tell.
    leaving: Option<Box<Leaving>>,
    /// The order the group's ids send in, from its key.
    schedule: Option<Box<Schedule>>,
    /// The member records the next packet asks for.
    requests: Requests,
    /// The members' addresses as a refresh under way started, which tell the members it learns
    /// from one that moved to another id.
    refresh_known: Option<Box<[Option<[u8; 6]>; IDS as usize]>>,
    /// What changed in the group and its removals and is not yet queued to be stored.
    unsaved: Unsaved,
    clock: Clock,
    table: Table,
    /// The timebase time the next own slot is looked for from, past the last one decided.
    after: i64,
    timebase_shown: Option<Timebase>,
    /// When each id was last heard sending, on the local clock.
    heard: [Option<i64>; IDS as usize],
    /// The refresh under way, or the last, while the group stays this device's.
    refresh: Option<RefreshView>,
    /// The refreshes started since boot, which number them: the screens take a session no
    /// higher than the last they saw for one already told.
    refreshes: u32,
    /// What the screens are shown.
    shown: Shown,
    /// The view [`publish`] fills.
    view: Box<MeshView>,
    /// Where a member heard on a timebase ranked below this node's places its slots, as its
    /// offset from the local timer, until a notice has gone to it.
    notice: Option<i64>,
    /// Every message the node holds.
    messages: Box<Store, A>,
    /// The messages the screens are shown.
    inbox: Inbox<A>,
    summaries: Summaries,
    pairwise: Box<Pairwise>,
    sequence: Sequence,
    outbox: Box<heapless::Deque<Outgoing, OUTBOX>>,
    removals: Removals<A>,
    /// The numbers of the writes queued without waiting, whose results are yet to be checked.
    writes: heapless::Vec<u32, 8>,
    /// A summary made before the slot it goes in, with the origin the next one starts from.
    summary: Option<Box<(heapless::Vec<u8, SUMMARY_MAX>, u8)>>,
}

impl<R: Radio, T: Time, G: Random, D: Device, S: GroupStore, A: Allocator + Clone>
    Mesh<R, T, G, D, S, A>
{
    pub async fn new(
        radio: R,
        time: T,
        random: G,
        device: D,
        store: S,
        alloc: A,
        start: Start,
    ) -> Self {
        let group = start.group.map(|group| *group);
        let own = group.as_ref().map_or(0, Group::own);
        let mut rekey = start.rekey.unwrap_or_default();
        // The stored state may still wait on an older key for a member removed since, as
        // earlier builds left it.
        if let Some(group) = &group {
            rekey.wait_only_for(group.ids());
        }
        match &group {
            Some(group) => info!(
                "[MESH] id={} name={} members={} mac={}",
                own,
                group.me().name,
                group.count(),
                Mac(&start.me.mac)
            ),
            None => info!(
                "[MESH] in no group, name={} mac={}",
                start.me.name,
                Mac(&start.me.mac)
            ),
        }
        let now = time.now();
        let mut mesh = Self {
            radio,
            time,
            random,
            device,
            store,
            me: start.me,
            group,
            founding: None,
            leaving: None,
            schedule: None,
            requests: Requests::default(),
            refresh_known: None,
            unsaved: Unsaved::default(),
            clock: Clock::new(own, now),
            table: Table::new(own),
            after: i64::MIN,
            timebase_shown: None,
            heard: [None; IDS as usize],
            refresh: None,
            refreshes: 0,
            shown: Shown::new(true),
            view: blank_view(),
            notice: None,
            messages: zeroed_in(alloc.clone()),
            inbox: Inbox::new(zeroed_in(alloc.clone())),
            summaries: Summaries::default(),
            pairwise: Box::default(),
            sequence: Sequence::new(start.sequence),
            outbox: Box::default(),
            removals: Removals::new(rekey, zeroed_in(alloc)),
            writes: heapless::Vec::new(),
            summary: None,
        };
        // A switch just before a restart may not have told the others yet, who wait for it.
        if let Some(group) = mesh.group.as_ref().filter(|group| group.generation() > 0) {
            mesh.removals.carry_on_key(group, &mesh.me, now);
        }
        let tuned = mesh.radio.tune(FREQUENCY_HZ, SYNC_WORD, POWER_DBM).await;
        info!("[MESH] tuned={}", tuned);
        mesh.sync_schedule();
        mesh.publish();
        mesh
    }

    /// Keeps the slot schedule the group's key gives. Call it once the group changes, before
    /// placing a slot.
    fn sync_schedule(&mut self) {
        let group = self
            .group
            .as_ref()
            .or(self.leaving.as_ref().map(|leaving| &leaving.group));
        match group {
            Some(group)
                if !self
                    .schedule
                    .as_ref()
                    .is_some_and(|schedule| schedule.is_for(group.key())) =>
            {
                self.schedule = Some(Box::new(Schedule::new(group.key())));
            }
            Some(_) => {}
            None => self.schedule = None,
        }
    }

    #[inline(never)]
    fn publish(&mut self) {
        self.show_pending();
        let now = self.time.now();
        let utc = self.utc(now);
        self.shown.heard = self.heard;
        self.shown.refresh = self.refresh;
        self.shown
            .fill(&mut self.view, &self.me, self.group.as_ref(), now, utc);
        self.device.publish(&mut self.view);
        self.show_messages();
    }

    /// Shows the screens the messages, if they changed since they were last shown.
    fn show_messages(&mut self) {
        if self.inbox.changed() && self.device.publish_messages(self.inbox.view()) {
            self.inbox.shown();
        }
    }

    /// Local time at the start of timebase round `round`, while there is a timebase.
    fn local_at_round(&self, round: u32) -> Option<i64> {
        let now = self.time.now();
        self.clock
            .at(now)
            .map(|(time, _)| now - (time - i64::from(round) * ROUND_US))
    }

    /// What the screens show of the removal of the member `new` names by `remover`.
    fn removal(&self, new: &NewKey, remover: u8, stage: RemovalStage) -> RemovalView {
        let name = |id: u8| {
            self.group
                .as_ref()
                .and_then(|group| group.member(id))
                .map_or_else(
                    || Name::from_mac(&[0, 0, 0, 0, 0, id]),
                    |member| member.name,
                )
        };
        RemovalView {
            key: key_fingerprint(&new.key),
            remover,
            remover_name: name(remover),
            removed: new.removed,
            removed_name: name(new.removed),
            device: new.fingerprint,
            stage,
        }
    }

    /// Shows the removal under way as it now is: it keeps its switch's time up to date.
    fn show_pending(&mut self) {
        let Some(pending) = self.removals.rekey.pending().cloned() else {
            self.show_declinable();
            return;
        };
        let key = key_fingerprint(&pending.new.key);
        let since = match self.shown.removals.current {
            Some(RemovalView {
                key: held,
                stage: RemovalStage::Pending { since, .. },
                ..
            }) if held == key => since,
            current => {
                // A rival that replaced the one shown: that one lost.
                if let Some(mut lost) =
                    current.filter(|current| matches!(current.stage, RemovalStage::Pending { .. }))
                {
                    lost.stage = RemovalStage::Lost;
                    self.shown.removals.rival = Some(lost);
                }
                self.time.now()
            }
        };
        let switch = self.local_at_round(pending.switch);
        self.shown.removals.current = Some(self.removal(
            &pending.new,
            pending.remover,
            RemovalStage::Pending { since, switch },
        ));
    }

    /// Shows the removal this device can still decline while nothing shows one, as after a
    /// restart, once there is a timebase to place it on.
    fn show_declinable(&mut self) {
        if self.shown.removals.current.is_some() {
            return;
        }
        let (Some(group), Some((undo, Some(last)))) =
            (&self.group, self.removals.rekey.declinable())
        else {
            return;
        };
        let (Some(remover), Some(at), Some(until)) = (
            self.removals.rekey.remover_of(group.generation()),
            self.local_at_round(undo.switched),
            self.local_at_round(undo.until + 1),
        ) else {
            return;
        };
        let name = |id: u8, member: Option<&Member>| {
            member.map_or_else(
                || Name::from_mac(&[0, 0, 0, 0, 0, id]),
                |member| member.name,
            )
        };
        self.shown.removals.current = Some(RemovalView {
            key: key_fingerprint(group.key()),
            remover,
            remover_name: name(remover, group.member(remover)),
            removed: last.removed,
            removed_name: name(last.removed, last.record.as_ref()),
            device: last.fingerprint,
            stage: RemovalStage::Switched {
                at,
                decline: Decline::Until(until),
            },
        });
    }

    /// Sets the stage of the removal shown, if it is the one with the new key `key`.
    fn show_stage(&mut self, key: [u8; 8], stage: RemovalStage) {
        if let Some(current) = &mut self.shown.removals.current
            && current.key == key
        {
            current.stage = stage;
        }
    }

    /// Local time at timebase second `stamp`, or now without a timebase.
    fn local_at(&self, stamp: u32) -> i64 {
        let now = self.time.now();
        self.clock
            .at(now)
            .map_or(now, |(time, _)| now - (time - i64::from(stamp) * 1_000_000))
    }

    pub async fn run(mut self, commands: &impl Commands) -> ! {
        if let Some(group) = &self.group {
            info!(
                "[MESH] id={} members={} generation={} removal pending={} declinable until={:?} old keys={}",
                group.own(),
                group.count(),
                group.generation(),
                self.removals.rekey.pending().is_some(),
                self.removals.rekey.undo_until(),
                self.removals.rekey.old().count()
            );
        }
        loop {
            self.sync_schedule();
            self.publish();
            let command = if self.group.is_some() {
                match select(self.step(), commands.receive()).await {
                    Either::First(()) => continue,
                    Either::Second(command) => {
                        // The step may have stopped anywhere, a transmission included.
                        self.radio.idle_receive().await;
                        command
                    }
                }
            } else if self.founding.is_some() {
                match select(self.await_joiner(), commands.receive()).await {
                    Either::First(()) => continue,
                    Either::Second(command) => {
                        self.radio.idle_receive().await;
                        command
                    }
                }
            } else if self.leaving.is_some() {
                match select(self.tell_leaving(), commands.receive()).await {
                    Either::First(()) => continue,
                    Either::Second(command) => {
                        // A pairing takes the radio; the others can still remove this device.
                        if matches!(command, Command::Add | Command::Join) {
                            self.leaving = None;
                        }
                        self.radio.idle_receive().await;
                        command
                    }
                }
            } else {
                self.radio.sleep().await;
                commands.receive().await
            };
            self.command(command, commands).await;
        }
    }

    async fn command(&mut self, command: Command, commands: &impl Commands) {
        info!("[MESH] command {:?}", command);
        match command {
            // A device added now would get the key the group is about to leave.
            Command::Add if self.removals.rekey.pending().is_some() => {
                warn!("[MESH] a removal is under way; adding waits for its switch");
                self.shown.sessions += 1;
                self.shown.pairing =
                    Some(refused(self.shown.sessions, Role::Add, Refused::Removing));
            }
            Command::Add => self.pair(Role::Add, commands).await,
            Command::Join if self.group.is_some() => {
                warn!("[MESH] in a group; it must leave before it can join another");
                self.shown.sessions += 1;
                self.shown.pairing =
                    Some(refused(self.shown.sessions, Role::Join, Refused::InGroup));
            }
            Command::Join => self.pair(Role::Join, commands).await,
            Command::Leave => {
                self.founding = None;
                self.shown.recovery = None;
                let now = self.time.now();
                let (left, group) = leave(&mut self.group, &self.store).await;
                if let Some(group) = group {
                    let gone = group.leaving(self.utc_seconds(now), &self.me);
                    self.leaving = Some(Box::new(Leaving {
                        group,
                        gone,
                        left: LEAVE_REPEATS,
                        until: now + LEAVE_WAIT_US,
                    }));
                }
                if left {
                    self.forget_messages();
                    self.unsaved.group_replaced();
                    self.refresh = None;
                    self.refresh_known = None;
                    info!("[MESH] left the group");
                } else {
                    warn!("[MESH] not left");
                }
                self.shown.answer(Answer::Left(left));
            }
            Command::Rename(name) => {
                let utc = self.utc_seconds(self.time.now());
                let group = match &mut self.founding {
                    Some(founding) => Some(&mut founding.group),
                    None => self.group.as_mut(),
                };
                let saved = rename(&mut self.me, group, name, utc, &self.store).await;
                info!("[MESH] renamed {} saved={}", name, saved);
                if saved && let Some(group) = &self.group {
                    self.unsaved.slot(group.own());
                }
                self.shown.answer(Answer::Renamed(saved));
            }
            Command::Refresh => self.start_refresh(),
            Command::Send { to, text } => self.queue_text(to, text),
            Command::Read(id) => {
                self.inbox.read(id);
                self.show_messages();
            }
            Command::Remove(id) => {
                let started = self.remove(id).await;
                self.shown.answer(Answer::Removing(started));
            }
            Command::RemoveDevice { id, device } => {
                let same = self
                    .group
                    .as_ref()
                    .and_then(|group| group.member(id))
                    .is_some_and(|member| fingerprint(&member.public) == device);
                let started = if same {
                    self.remove(id).await
                } else {
                    warn!("[REKEY] {} is another device now; not removed", id);
                    Err(Unremovable::Changed)
                };
                self.shown.answer(Answer::Removing(started));
            }
            Command::KeepKey(key) => {
                let removed = match (self.removals.rekey.pending(), &self.group) {
                    (Some(pending), _) if key_fingerprint(&pending.new.key) == key => {
                        Some(pending.new.removed)
                    }
                    (None, Some(group)) if key_fingerprint(group.key()) == key => {
                        self.removals.rekey.undo_removed()
                    }
                    _ => None,
                };
                match removed {
                    Some(removed) => self.keep(removed),
                    None => warn!("[REKEY] that removal cannot be declined now"),
                }
            }
            #[cfg(feature = "phantom")]
            Command::Phantom => {
                let now = self.utc_seconds(self.time.now());
                if let (Some(group), Some(seed), Some(mac)) = (
                    &mut self.group,
                    self.random.bytes::<32>(),
                    self.random.bytes::<6>(),
                ) && let Some(id) = group.lowest_free()
                {
                    let phantom =
                        Identity::new(seed, mac, Name::new(b"Phantom").expect("printable"));
                    let mut record = octowhere_mesh::members::Member {
                        public: phantom.public(),
                        joined: now,
                        changed: now,
                        mac,
                        name: phantom.name,
                        signature: [0; octowhere_mesh::identity::SIGNATURE_LEN],
                    };
                    record.sign(id, &phantom);
                    group.enrol(id, record);
                    self.unsaved.slot(id);
                    info!("[MESH] enrolled a phantom at {}", id);
                }
            }
            Command::Keep(removed) => self.keep(removed),
            Command::Choose(_)
            | Command::ChooseMac(_)
            | Command::Accept
            | Command::Decline
            | Command::Mismatch
            | Command::Cancel => warn!("[MESH] no pairing to take it"),
        }
    }

    /// Queues what changed in the group to be stored, as far as the queue has room. The ids
    /// that changed reach the removals first, so that a restart never keeps a change that
    /// declining the last removal would not forget.
    fn queue_unsaved(&mut self) {
        let mut failed = false;
        self.writes
            .retain(|&number| match self.store.result(number) {
                Some(saved) => {
                    failed |= !saved;
                    false
                }
                None => true,
            });
        if failed {
            warn!("[MESH] a write to the flash failed; the group is stored again");
            self.unsaved.everything();
        }
        if let Some(group) = &mut self.group
            && self.removals.rekey.changed(group.take_changed())
        {
            self.unsaved.rekey();
        }
        while let Some(due) = self.unsaved.due(self.group.is_some()) {
            let write = match due {
                Due::Group { rekey } => GroupWrite::Group(
                    Box::new(
                        self.group
                            .clone()
                            .expect("a whole group is due only with one"),
                    ),
                    rekey.then(|| self.removals.rekey.clone()),
                ),
                Due::Rekey => GroupWrite::Rekey(self.removals.rekey.clone()),
                Due::Slot(id) => GroupWrite::Slot {
                    id,
                    slot: self
                        .group
                        .as_ref()
                        .and_then(|group| group.slot(id).copied()),
                },
            };
            if !self.queue_write(write) {
                return;
            }
            self.unsaved.queued(due);
        }
    }

    /// Queues a write to the flash without waiting, and keeps its number to check its result.
    /// Returns false when the queue is full.
    fn queue_write(&mut self, write: GroupWrite) -> bool {
        let Some(number) = self.store.queue(write) else {
            return false;
        };
        if self.writes.push(number).is_err() {
            // Too many to follow: store everything again rather than miss a failure.
            self.writes.clear();
            self.unsaved.everything();
        }
        true
    }

    /// Queues the group and its removals to be stored in one write, as a change of key needs:
    /// apart, a restart between the two would pair the key with removals made for another.
    fn save_switch(&mut self) {
        self.unsaved.everything();
        self.queue_unsaved();
    }

    /// Waits for the node's next slot and sends in it, listening meanwhile.
    async fn step(&mut self) {
        self.queue_unsaved();
        if let Some((generation, remover)) = self.removals.take_refill() {
            self.refill_kept(generation, remover).await;
        }
        let Some(own) = self.group.as_ref().map(Group::own) else {
            return;
        };
        let now = self.time.now();
        self.take_readings(own);
        self.clock.tick(now, self.device.rtc_utc(now));
        self.update_refresh(now);
        let timebase = self.clock.at(now).map(|(_, timebase)| timebase);
        if timebase != self.timebase_shown {
            log_timebase(timebase, self.clock.is_sweeping(now));
            self.timebase_shown = timebase;
            // What the screens show of removals is timed on it.
            self.publish();
        }
        let Some((time, timebase)) = self.clock.at(now) else {
            let end = self.clock.sweep_ends(now).unwrap_or(now + ROUND_US);
            self.listen(end).await;
            return;
        };
        if self.removals.rekey.is_due(stored_round_at(time)) {
            self.switch_key();
            return;
        }
        if self.removals.rekey.expire(stored_round_at(time)) {
            info!("[REKEY] a day since the switch; the key before it is dropped");
            self.save_rekey();
            if let Some(current) = &mut self.shown.removals.current
                && let RemovalStage::Switched { decline, .. } = &mut current.stage
            {
                *decline = Decline::Expired;
            }
            self.publish();
        }
        if self.removals.to_tell() {
            self.tell_removed(time).await;
        }
        if self.removals.keys_due(own) {
            self.post_keys(time).await;
        }
        // A member this device removed again once its record is back.
        let again =
            self.removals.rekey.again() & self.group.as_ref().map_or(Ids::EMPTY, Group::ids);
        if let Some(id) = again.first()
            && self.removals.rekey.pending().is_none()
        {
            // One that does not start is tried again at the next step.
            _ = self.remove(id).await;
        }
        self.retry_unread(own);
        self.inbox.started(round_start_s(time));
        self.messages.expire(round_start_s(time), |_| {});
        self.inbox.prune(&self.messages);
        self.show_messages();
        if !self.outbox.is_empty() {
            self.post_outbox(time).await;
        }
        let Some((round, start)) = self
            .schedule
            .as_deref()
            .map(|schedule| schedule.next_slot((time + PREPARE_US).max(self.after), own))
        else {
            return;
        };
        let send_at = start + (now - time);
        match Next::choose(
            send_at,
            self.notice_at(now, own),
            self.old_slot_at(now, own),
        ) {
            Next::Notice { at } => {
                self.notice = None;
                if !self.listen(at - PREPARE_US).await {
                    self.send_notice(own, timebase, at).await;
                }
                return;
            }
            Next::Old { at, round } => {
                if !self.listen(at - PREPARE_US).await {
                    self.send_old(own, timebase, at, round).await;
                }
                return;
            }
            Next::Own => {}
        }
        self.prepare_summary();
        if self.listen(send_at - PREPARE_US).await {
            return;
        }
        self.after = start + 1;
        let sending = self.table.wants_to_send(round)
            || !self.requests.pending().is_empty()
            || self.group.as_ref().is_some_and(Group::has_unsent)
            || self.messages.has_unsent()
            || self.summary.is_some();
        info!(
            "[MESH] round={} sending={} sweeping={}",
            round,
            sending,
            self.clock.is_sweeping(self.time.now())
        );
        if sending {
            self.send(round, start, timebase, send_at).await;
        }
    }

    /// Sends the gone record of the group this device left in its next own slot, and forgets
    /// the group once it has gone out in [`LEAVE_REPEATS`] of them or the wait is over.
    async fn tell_leaving(&mut self) {
        let now = self.time.now();
        let (Some(leaving), Some((time, timebase)), Some(schedule)) =
            (&self.leaving, self.clock.at(now), self.schedule.as_deref())
        else {
            info!("[MESH] left with no timebase to tell the others on");
            self.leaving = None;
            return;
        };
        if now >= leaving.until || leaving.left == 0 {
            self.leaving = None;
            return;
        }
        let own = leaving.group.own();
        let (round, start) = schedule.next_slot((time + PREPARE_US).max(self.after), own);
        let send_at = start + (now - time);
        if send_at > leaving.until {
            self.leaving = None;
            return;
        }
        self.radio.standby().await;
        self.time.until(send_at - PREPARE_US).await;
        self.after = start + 1;
        let header = Header {
            sender: own,
            timebase,
            base: second_at(start),
            phase: 0,
            notice: false,
        };
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Builder::new(&mut packet[SIV_LEN..], &header);
        let _ = builder.neighbours(self.table.neighbours(round).bits());
        let _ = builder.gone(own, &leaving.gone);
        let plain_len = builder.finish();
        let len = seal::seal(leaving.group.key(), &mut packet, plain_len);
        let sent = self.radio.transmit(&packet[..len], Some(send_at)).await;
        info!(
            "[MESH] told the group it left round={} done={}",
            round,
            sent.is_some_and(|done| done)
        );
        if let Some(leaving) = &mut self.leaving {
            leaving.left -= 1;
        }
    }

    /// When this node's slot comes next in the order of the old key a packet under one would go
    /// out under, on the local timer, and its round. That is always in a sweep round, where
    /// every member listens throughout: one that switched to a rival key listens in no other
    /// round of the old key's order.
    fn old_slot_at(&self, now: i64, own: u8) -> Option<(i64, i64)> {
        if !self.removals.sends_old() {
            return None;
        }
        let (time, _) = self.clock.at(now)?;
        let from = time + 2 * PREPARE_US;
        let (key, _, _) = self.removals.old_packet(round_at(from))?;
        let schedule = Schedule::new(key);
        let (mut round, mut start) = schedule.next_slot(from, own);
        if !is_sweep_round(round) {
            // A sweep round's header goes in that round only.
            if !self.removals.catching_up() {
                return None;
            }
            round += SWEEP_EVERY - round.rem_euclid(SWEEP_EVERY);
            start = schedule.slot_start(round, own);
        }
        Some((start + (now - time), round))
    }

    /// Sends a packet under an old key at local time `at`, in this node's slot in that key's
    /// order: a header, and the key message of each member waiting for one on that key.
    async fn send_old(&mut self, own: u8, timebase: Timebase, at: i64, round: i64) {
        let Some((time, _)) = self.clock.at(at) else {
            return;
        };
        let Some((key, generation, ids)) = self.removals.old_packet(round) else {
            return;
        };
        let key = key.clone();
        let header = Header {
            sender: own,
            timebase,
            base: second_at(time),
            phase: 0,
            notice: false,
        };
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Builder::new(&mut packet[SIV_LEN..], &header);
        let (mut caught, mut lost) = (0u32, 0u32);
        for &(id, catches_up_with) in &ids {
            match self.removals.message_for(id, catches_up_with) {
                Some(message) if builder.message(message).is_ok() => caught |= 1 << id,
                Some(_) => {}
                // Gone since it was queued, as a rival's switch drops the losing key's.
                None => lost |= 1 << id,
            }
        }
        let plain_len = builder.finish();
        let len = seal::seal(&key, &mut packet, plain_len);
        let sent = self.radio.transmit(&packet[..len], Some(at)).await;
        self.removals.sent_old(caught, lost, round);
        info!(
            "[REKEY] sent under generation {} round={} caught={:#010x} len={} done={}",
            generation,
            round,
            caught,
            len,
            sent.is_some_and(|done| done)
        );
    }

    /// When the member a notice is for next listens for this node's slot, on the local timer.
    fn notice_at(&self, now: i64, own: u8) -> Option<i64> {
        let offset = self.notice?;
        // The member's round, on its own timebase, gives this node's slot there.
        let (_, start) = self
            .schedule
            .as_deref()?
            .next_slot(now - offset + 2 * PREPARE_US, own);
        Some(start + offset)
    }

    /// Sends a notice at local time `at`, when a member on a lower timebase listens for this
    /// node, which makes it sweep and so hear this node's own packets.
    async fn send_notice(&mut self, own: u8, timebase: Timebase, at: i64) {
        let mut packet = [0u8; NOTICE_LEN];
        let (Some(group), Some((time, _))) = (&self.group, self.clock.at(at)) else {
            return;
        };
        let header = Header {
            sender: own,
            timebase,
            base: second_at(time),
            phase: 0,
            notice: true,
        };
        let plain_len = Builder::new(&mut packet[SIV_LEN..], &header).finish();
        let len = seal::seal(group.key(), &mut packet, plain_len);
        let sent = self.radio.transmit(&packet[..len], Some(at)).await;
        info!("[MESH] notice sent done={}", sent.is_some_and(|done| done));
    }

    fn take_readings(&mut self, own: u8) {
        if let Some(gps) = self.device.gps_time() {
            self.clock.gps(gps.offset, gps.updated);
            if let Some(fix) = self.device.fix() {
                self.table.set_own(Entry {
                    id: own,
                    latitude: fix.latitude,
                    longitude: fix.longitude,
                    stamp: fix.stamp,
                    quality: fix.quality,
                    hdop: Hdop::from_milli(fix.hdop_milli.unwrap_or(u32::MAX)),
                });
                self.shown.positions(&self.table);
            }
        }
        if let Some((time, _)) = self.clock.at(self.time.now()) {
            self.table.expire(second_at(time));
        }
    }

    /// Listens until local time `end`: throughout in a sweep or without a timebase, and otherwise
    /// in a window round the slot of each other member and of each id heard lately.
    /// Returns whether a switch, or a packet that moved the node to another timebase or another
    /// id, moved every slot; the next own slot is then looked for afresh.
    async fn listen(&mut self, end: i64) -> bool {
        loop {
            let now = self.time.now();
            if self.update_refresh(now) {
                self.publish();
            }
            if self.switch_at(now).is_some_and(|at| at <= now) {
                self.switch_key();
                self.after = i64::MIN;
                return true;
            }
            if now >= end {
                return false;
            }
            let (open, close) = match self.clock.at(now) {
                Some((time, _)) if !self.clock.is_sweeping(now) => {
                    let offset = now - time;
                    let from = time - GUARD_US - airtime_us(MAX_PACKET) + 1;
                    let ids = self.listened(round_at(from));
                    let window = self
                        .schedule
                        .as_deref()
                        .and_then(|schedule| schedule.next_slot_in(from, ids))
                        .map(|(_, start)| {
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
            // A refresh's end is shown as it comes, and a switch is made as it comes, whatever
            // the window.
            let event = [self.refresh_until(), self.switch_at(now)]
                .into_iter()
                .flatten()
                .filter(|&at| at > now)
                .min();
            let close = match event {
                Some(at) => close.min(at),
                None => close,
            };
            if let Some(ends) = event
                && ends < open.min(end)
            {
                self.radio.standby().await;
                self.time.until(ends).await;
                continue;
            }
            if open >= end {
                self.radio.standby().await;
                self.time.until(end).await;
                return false;
            }
            if open > now {
                self.radio.standby().await;
                self.time.until(open).await;
            }
            if !self.radio.start_receiving().await {
                warn!("[MESH] receive start failed");
            }
            while self.radio.wait_received(close).await {
                if self.receive().await {
                    self.radio.standby().await;
                    self.after = i64::MIN;
                    return true;
                }
            }
            self.radio.standby().await;
        }
    }

    /// The ids whose slots the node listens to outside a sweep in `round`: the group's other
    /// members, and any other id heard in the rounds a neighbour counts for. A member the group
    /// does not know of yet is found by a sweep.
    fn listened(&self, round: i64) -> u32 {
        let Some(group) = &self.group else {
            return 0;
        };
        (group.ids() | self.table.neighbours(round))
            .without(group.own())
            .bits()
    }

    /// Takes the packet `DIO0` reported. Returns whether it moved the node to its timebase, or
    /// to another id.
    async fn receive(&mut self) -> bool {
        let Some((packet, done)) = self.radio.read_packet().await else {
            return false;
        };
        self.take(&packet, done)
    }

    /// Opens a sealed packet in place under the group's key. Says instead which other key
    /// opens it: the one the group is about to switch to, or an old one, which tells of a member
    /// that missed a switch.
    fn open(&self, packet: &mut [u8]) -> Opened {
        let Some(group) = &self.group else {
            return Opened::Not;
        };
        if seal::open(group.key(), packet).is_ok() {
            return Opened::Current;
        }
        if let Some(pending) = self.removals.rekey.pending()
            && let Ok(plain) = seal::open(&pending.new.key, packet)
            && let Ok(plain) = Plain::parse(plain)
        {
            return Opened::Pending {
                header: plain.header,
            };
        }
        for old in self.removals.rekey.old() {
            if let Ok(plain) = seal::open(&old.key, packet)
                && let Ok(plain) = Plain::parse(plain)
            {
                return Opened::Old {
                    header: plain.header,
                    old: (old.key.clone(), old.generation, old.catches_up_with()),
                };
            }
        }
        Opened::Not
    }

    /// Takes a packet whose RxDone was seen at local time `done`. Returns whether it moved the
    /// node to its timebase, or to another id.
    fn take(&mut self, packet: &Received, done: i64) -> bool {
        let len = packet.length;
        let mut bytes = packet.payload;
        match self.open(&mut bytes[..len]) {
            Opened::Current => {}
            // Nothing else in it is taken: this node sends under the old key, which the member
            // being removed reads, and its slot order and clock are still the old key's.
            Opened::Pending { header } => {
                info!("[REKEY] heard {} on the key to switch to", header.sender);
                if let Some(heard) = self.heard.get_mut(usize::from(header.sender)) {
                    *heard = Some(done);
                }
                return self.adopt_pending(&header, len, done);
            }
            Opened::Old { header, old } => {
                self.heard_on_old(&header, old, done);
                self.take_rivals(&bytes[SIV_LEN..len]);
                return false;
            }
            Opened::Not => {
                info!("[MESH] not ours len={} rssi={}", len, packet.rssi);
                return false;
            }
        }
        let utc = self.utc_seconds(done);
        let Some(group) = &mut self.group else {
            return false;
        };
        let Ok(plain) = Plain::parse(&bytes[SIV_LEN..len]) else {
            warn!("[MESH] malformed len={}", len);
            return false;
        };
        let header = plain.header;
        let polled = self.radio.seen_late_us();
        let start = done - airtime_us(len) - ARRIVAL_LATENCY_US - polled;
        let Some(slot) = self
            .schedule
            .as_deref()
            .map(|schedule| schedule.named_slot(header.base, header.sender))
        else {
            return false;
        };
        let ours = self.clock.at(done).map(|(_, timebase)| timebase.source);
        if header.notice {
            if ours.is_none_or(|ours| header.timebase.source.outranks(ours)) {
                info!(
                    "[MESH] notice from id={} timebase={:?}; sweeping",
                    header.sender, header.timebase.source
                );
                self.clock.sweep(done);
            }
            return false;
        }
        let arrival = self.clock.arrival(&header, slot, start, done);
        if arrival.taken == Taken::Ignored
            && self.notice.is_none()
            && ours.is_some_and(|ours| ours.outranks(header.timebase.source))
        {
            info!(
                "[MESH] id={} is on a lower timebase; a notice goes to it",
                header.sender
            );
            self.notice = Some(start - slot);
        }
        if let Some(heard) = self.heard.get_mut(usize::from(header.sender)) {
            *heard = Some(done);
        }
        if let Some(refresh) = &mut self.refresh
            && refresh.is_listening()
            && header.sender != group.own()
            && header.sender < IDS
        {
            refresh.heard |= 1 << header.sender;
        }
        let round = round_at(slot);
        self.table.heard(header.sender, round);
        // A clock started from a boot is no UTC to judge a record's time by.
        let now = self
            .clock
            .at(done)
            .filter(|(_, timebase)| timebase.source.is_utc())
            .map(|(time, _)| second_at(time));
        // Messages are judged by this node's clock: the header's own time is the sender's word.
        let round_s = self
            .clock
            .at(done)
            .map_or_else(|| round_start_s(slot), |(time, _)| round_start_s(time));
        let own = group.own();
        let absorbed = absorb(
            plain.records(),
            header.sender,
            When {
                round,
                now,
                utc,
                round_s,
            },
            State {
                group,
                table: &mut self.table,
                messages: &mut self.messages,
                requests: &mut self.requests,
                summaries: &mut self.summaries,
                rekey: &mut self.removals.rekey,
                me: &self.me,
            },
        );
        for &name in absorbed.carried_names() {
            if name.0 == own && header.sender != own {
                self.inbox.relayed(name);
            }
        }
        for event in absorbed.events() {
            match event {
                Event::Changed(id, name) => info!("[MESH] member {} is {} now", id, name),
                Event::Renumbered { from, to } => warn!(
                    "[MESH] another device keeps id {}; this one moves to {}",
                    from, to
                ),
                Event::Went(at) => info!("[MESH] member {} went", at),
            }
        }
        for id in (0..IDS).filter(|&id| absorbed.changed & 1 << id != 0) {
            self.unsaved.slot(id);
        }
        if let Some(to) = absorbed.renumbered {
            self.clock.renumber(to);
            self.unsaved.group();
        }
        for name in absorbed.arrivals() {
            let Some(message) = self.messages.get(*name).copied() else {
                continue;
            };
            if message.is_key() {
                self.keep_key(&message);
            }
            if self.arrived(&message, own) == Arrival::Later {
                self.messages.mark_unread(*name);
            }
        }
        if let Some(message) = absorbed.late_key {
            self.arrived(&message, own);
        }
        if absorbed.rekey_changed {
            if !self.removals.rekey.is_waiting() {
                info!("[REKEY] every member is on the new key; the old one is dropped");
                self.removals.all_on_new_key();
            }
            self.save_rekey();
        }
        self.shown.positions(&self.table);
        self.publish();
        info!(
            "[MESH] heard id={} timebase={:?} hops={} taken={:?} late_us={:?} len={} rssi={} snr={} entries={} new={} neighbours={:#010x} messages={}/{} summary={}",
            header.sender,
            header.timebase.source,
            header.timebase.hops,
            arrival.taken,
            arrival.late_us,
            len,
            packet.rssi,
            packet.snr,
            absorbed.entries,
            absorbed.news,
            absorbed.neighbours,
            absorbed.arrivals().len(),
            absorbed.carried,
            absorbed.summary,
        );
        arrival.taken == Taken::Adopted || absorbed.renumbered.is_some()
    }

    /// Moves to the clock of a packet under the key to switch to, if it outranks this node's. A
    /// node that restarted with no UTC finds the group's clock in no other packet once the
    /// others have switched. Returns whether it moved.
    fn adopt_pending(&mut self, header: &Header, len: usize, done: i64) -> bool {
        let ours = self.clock.at(done).map(|(_, timebase)| timebase.source);
        if ours.is_some_and(|ours| !header.timebase.source.outranks(ours)) {
            return false;
        }
        let Some(pending) = self.removals.rekey.pending() else {
            return false;
        };
        let slot = Schedule::new(&pending.new.key).named_slot(header.base, header.sender);
        let start = done - airtime_us(len) - ARRIVAL_LATENCY_US - self.radio.seen_late_us();
        let arrival = self.clock.arrival(header, slot, start, done);
        info!(
            "[REKEY] clock from the key to switch to: {:?} late_us={:?}",
            arrival.taken, arrival.late_us
        );
        arrival.taken == Taken::Adopted
    }

    /// Takes from a packet under an old key, opened, the key messages of this node's
    /// generation: rivals of the key it switched to, which reach it no other way once both
    /// parts of the group have switched. Nothing else in it is taken.
    fn take_rivals(&mut self, plain: &[u8]) {
        let (Some(group), Ok(plain), Some((time, _))) = (
            &self.group,
            Plain::parse(plain),
            self.clock.at(self.time.now()),
        ) else {
            return;
        };
        let (generation, own) = (group.generation(), group.own());
        for record in plain.records() {
            let Record::Message(message) = record else {
                continue;
            };
            if !message.is_key() || message.generation() != Some(generation) {
                continue;
            }
            match self.messages.insert(message, round_start_s(time)) {
                Insert::New => {
                    self.keep_key(&message);
                    if self.arrived(&message, own) == Arrival::Later {
                        self.messages.mark_unread(message.name());
                    }
                }
                Insert::Old if message.to() == To::Member(own) => {
                    self.arrived(&message, own);
                }
                _ => {}
            }
        }
    }

    /// Sends this node's packet in its slot in `round`, which starts at timebase time `start` and
    /// local time `send_at`.
    async fn send(&mut self, round: i64, start: i64, timebase: Timebase, send_at: i64) {
        let Some(group) = &mut self.group else {
            return;
        };
        let base = second_at(start);
        let header = Header {
            sender: group.own(),
            timebase,
            base,
            phase: 0,
            notice: false,
        };
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Builder::new(&mut packet[SIV_LEN..], &header);
        let carried = compose(
            &mut builder,
            round,
            base,
            Sources {
                table: &self.table,
                group,
                messages: &self.messages,
                requests: &self.requests,
                summary: self
                    .summary
                    .as_deref()
                    .map(|(summary, _)| summary.as_slice()),
                on_key: self.removals.on_key_for(round, send_at),
            },
        );
        let plain_len = builder.finish();
        let len = seal::seal(group.key(), &mut packet, plain_len);
        if carried.on_key {
            self.removals.carried_on_key();
        }

        let Some(done) = self.radio.transmit(&packet[..len], Some(send_at)).await else {
            return;
        };
        if let Some(group) = &mut self.group {
            carried.sent(
                &mut self.table,
                &mut self.requests,
                &mut self.messages,
                group,
            );
            let own = group.own();
            for &name in carried.messages() {
                if name.0 == own {
                    self.inbox.sent(name);
                }
            }
            self.show_messages();
        }
        if carried.summary
            && let Some(prepared) = self.summary.take()
        {
            self.summaries.sent(prepared.1);
        }
        info!(
            "[MESH] sent round={} len={} entries={} records={:#010x} asked={:#010x} neighbours={:#010x} messages={} summary={} done={}",
            round,
            len,
            carried.positions().len(),
            carried.records,
            carried.requests,
            carried.neighbours,
            carried.messages().len(),
            carried.summary,
            done
        );
    }

    /// Runs a pairing in `role` on the pairing channel until it ends, then returns to the mesh's
    /// channel. A node that joins, or founds a group, starts its timebase afresh.
    async fn pair(&mut self, role: Role, commands: &impl Commands) {
        if self.founding.take().is_some() {
            info!("[MESH] no longer listening for the device a founding left unconfirmed");
        }
        self.shown.recovery = None;
        self.stop_refresh(self.time.now());
        self.shown.sessions += 1;
        let session = self.shown.sessions;
        let (Some(nonce), Some(founding)) = (self.random.bytes::<16>(), self.random.bytes::<32>())
        else {
            warn!("[PAIR] no true random source; not pairing");
            self.shown.pairing = Some(refused(session, role, Refused::NoRandom));
            return;
        };
        if !self
            .radio
            .tune(PAIR_FREQUENCY_HZ, PAIR_SYNC_WORD, PAIR_POWER_DBM)
            .await
        {
            warn!("[PAIR] tuning to the pairing channel failed");
        }
        let now = self.time.now();
        let had_group = self.group.is_some();
        let mut pairing = match role {
            Role::Join => Pairing::join(&self.me, nonce, now),
            Role::Add => {
                let utc = self.utc_seconds(now);
                let group = self
                    .group
                    .clone()
                    .unwrap_or_else(|| Group::found(Key::new(founding), &self.me, utc));
                Pairing::add(&self.me, group, nonce, now, utc)
            }
        };
        let mut frame = [0u8; MAX_FRAME];
        let mut shown = None;
        let mut listening = false;
        let mut saving: Option<(i64, u32)> = None;
        loop {
            let now = self.time.now();
            if let Some(len) = pairing.poll(now, &mut frame) {
                let sent = self.radio.transmit(&frame[..len], None).await;
                debug!(
                    "[PAIR] sent kind={} len={} done={}",
                    frame[1],
                    len,
                    sent.is_some_and(|done| done)
                );
                listening = false;
                continue;
            }
            let phase = pairing.phase();
            if shown != Some(phase) {
                log_pairing(&pairing, phase, self.time.now());
                shown = Some(phase);
            }
            let view = pairing_view(session, &pairing);
            if self.shown.pairing.as_ref() != Some(&view) {
                self.shown.pairing = Some(view);
                let utc = self.utc(now);
                // A pairing that is done has stored its group, which the screens show at once.
                let group = match phase {
                    Phase::Done(_) => pairing.group(),
                    _ => self.group.as_ref(),
                };
                self.shown.heard = self.heard;
                self.shown.refresh = self.refresh;
                self.shown.fill(&mut self.view, &self.me, group, now, utc);
                self.device.publish(&mut self.view);
            }
            if pairing.is_over(now) {
                break;
            }
            if phase == Phase::Storing && saving.is_none() {
                let mut group = pairing
                    .group()
                    .expect("a pairing storing has a group")
                    .clone();
                // The member added is one more id that declining the last removal forgets, and
                // this write may stand in for a switch's not yet stored.
                if self.removals.rekey.changed(group.take_changed()) {
                    self.unsaved.rekey();
                }
                let rekey = self
                    .unsaved
                    .rekey_due()
                    .then(|| self.removals.rekey.clone());
                let number = self
                    .store
                    .send(GroupWrite::Group(Box::new(group), rekey))
                    .await;
                saving = Some((self.time.now(), number));
                continue;
            }
            if phase == Phase::Storing
                && saving.is_some_and(|(started, _)| now - started > STORE_TIMEOUT_US)
            {
                warn!("[PAIR] storing the group timed out");
                pairing.stored(false, now);
                continue;
            }
            if !listening {
                if !self.radio.start_receiving().await {
                    warn!("[PAIR] receive start failed");
                }
                listening = true;
            }
            let wake = pairing.wake_at().min(now + PAIR_LISTEN_US);
            let store = &self.store;
            let saved = async {
                match saving {
                    Some((_, number)) => store.saved(number).await,
                    None => core::future::pending().await,
                }
            };
            match select3(self.radio.wait_received(wake), commands.receive(), saved).await {
                Either3::First(true) => {
                    if let Some((packet, done)) = self.radio.read_packet().await {
                        debug!(
                            "[PAIR] heard kind={} len={} rssi={} snr={}",
                            packet.payload[1], packet.length, packet.rssi, packet.snr
                        );
                        let mut payload = packet.payload;
                        let started = self.time.now();
                        pairing.receive(&mut payload[..packet.length], done);
                        let took = self.time.now() - started;
                        if took > 5_000 {
                            info!("[PAIR] a frame took {}us to take", took);
                        }
                    }
                }
                Either3::First(false) => {}
                Either3::Second(command) => {
                    let now = self.time.now();
                    info!("[PAIR] command {:?}", command);
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
                    // Its result is in, and the write's future would be ready on every turn
                    // after, so that the loop never waited.
                    saving = None;
                    if phase == Phase::Storing {
                        pairing.stored(ok, self.time.now());
                    }
                }
            }
        }
        match (pairing.phase(), pairing.group()) {
            (Phase::Done(_), Some(group)) => {
                self.group = Some(group.clone());
                self.unsaved.group_replaced();
                if role == Role::Join || !had_group {
                    self.restart(group.own());
                    self.refresh = None;
                }
            }
            // A founder whose write failed has sent its done all the same, so the joining
            // device holds the group: the wait stores it again.
            (Phase::Ended(End::Unconfirmed | End::StoreFailed), Some(group))
                if !had_group && role == Role::Add =>
            {
                info!("[MESH] listening for the joining device under the founded group's key");
                let until = self.time.now() + FOUNDING_WAIT_US;
                self.shown.recovery = Some(RecoveryView {
                    session,
                    phase: RecoveryPhase::Listening { until },
                    peer_name: pairing.peer_name(),
                    peer: pairing.peer().unwrap_or_default(),
                    count: group.count() as u8,
                });
                self.founding = Some(Box::new(Founding {
                    group: group.clone(),
                    until,
                    heard: false,
                }));
            }
            _ => {}
        }
        if !self.radio.tune(FREQUENCY_HZ, SYNC_WORD, POWER_DBM).await {
            warn!("[MESH] tuning back to the mesh's channel failed");
        }
    }

    /// Starts the timebase, the table and what the screens show of them afresh, at id `own`.
    fn restart(&mut self, own: u8) {
        self.clock = Clock::new(own, self.time.now());
        self.table = Table::new(own);
        self.after = i64::MIN;
        self.timebase_shown = None;
        self.notice = None;
        self.requests = Requests::default();
        self.heard = [None; IDS as usize];
        self.shown.positions = [None; IDS as usize];
        self.forget_messages();
    }

    /// Makes the summary the next packet carries, while there is time: with a full store it
    /// takes milliseconds, too long for the moments before a slot.
    fn prepare_summary(&mut self) {
        if !self.summaries.pending() || self.summary.is_some() {
            return;
        }
        let mut bytes = [0; SUMMARY_MAX];
        let (len, next) = self.messages.summary(&mut bytes, self.summaries.first());
        if let Ok(summary) = heapless::Vec::from_slice(&bytes[..len]) {
            self.summary = Some(Box::new((summary, next)));
        }
    }

    /// Forgets the messages and removals of a group this device no longer belongs to.
    fn forget_messages(&mut self) {
        *self.messages = Store::zeroed();
        self.inbox.clear();
        self.summaries = Summaries::default();
        self.outbox.clear();
        self.removals.forget();
        self.shown.removals = RemovalsView::default();
        self.summary = None;
    }

    /// Queues the group's removals to be stored, now or, when the queue is full or they wait to
    /// go with the group, at the next step.
    fn save_rekey(&mut self) {
        self.unsaved.rekey();
        if self.unsaved.due(self.group.is_some()) == Some(Due::Rekey)
            && self.queue_write(GroupWrite::Rekey(self.removals.rekey.clone()))
        {
            self.unsaved.queued(Due::Rekey);
        }
    }

    /// Removes the member `id`: makes a new key and sends it to every other member. The group
    /// switches to the key when the round the key messages name starts, and `id` is told then.
    async fn remove(&mut self, id: u8) -> Result<(), Unremovable> {
        let (Some(group), Some((time, _))) = (&self.group, self.clock.at(self.time.now())) else {
            warn!("[REKEY] no group, or no timebase to time a switch on");
            return Err(Unremovable::NoTime);
        };
        if self.removals.rekey.pending().is_some() {
            warn!("[REKEY] a removal is under way");
            return Err(Unremovable::Underway);
        }
        if id == group.own() || group.member(id).is_none() {
            return Err(Unremovable::Changed);
        }
        let remaining = group.ids().bits() & !(1 << id) & !(1 << group.own());
        // Every key message has its number before the removal starts, so none is left behind.
        if !self.reserve(remaining.count_ones(), time).await {
            return Err(Unremovable::Unsaved);
        }
        let Some(key) = self.random.bytes::<32>() else {
            warn!("[REKEY] no random source for a key");
            return Err(Unremovable::NoRandom);
        };
        let round = stored_round_at(time);
        let Some(group) = &self.group else {
            return Err(Unremovable::NoTime);
        };
        let Some(new) = self.removals.rekey.start(group, id, Key::new(key), round) else {
            warn!("[REKEY] cannot remove {} now", id);
            return Err(Unremovable::Underway);
        };
        info!(
            "[REKEY] removing {}: generation {} from round {} (now {}), {} to tell",
            id,
            new.generation,
            new.switch,
            round,
            remaining.count_ones()
        );
        self.removals.started();
        self.save_rekey();
        self.publish();
        self.post_keys(time).await;
        Ok(())
    }

    /// Posts the key messages of this device's removal under way, one to every member but the
    /// one removed, at timebase time `time`. It runs again after a restart or an interruption
    /// until every one is posted; a member that already holds the key ignores another message
    /// with it.
    async fn post_keys(&mut self, time: i64) {
        let (Some(group), Some(pending)) = (&self.group, self.removals.rekey.pending()) else {
            return;
        };
        if pending.remover != group.own() {
            return;
        }
        let new = pending.new.clone();
        let remaining = group.ids().bits() & !(1 << new.removed) & !(1 << group.own());
        if !self.reserve(remaining.count_ones(), time).await {
            return;
        }
        let body = new.encode();
        for member in (0..IDS).filter(|&member| remaining & 1 << member != 0) {
            self.post(
                &Outgoing::new(To::Member(member), Some(new.generation), &body),
                time,
            )
            .await;
            // Each key takes an X25519 the first time; let the other tasks run between.
            self.time.until(self.time.now() + 1_000).await;
        }
        self.removals.posted();
    }

    /// Stores a block holding at least `count` more sequence numbers, if the one held has fewer,
    /// at timebase time `time`. Returns whether they are there to take.
    async fn reserve(&mut self, count: u32, time: i64) -> bool {
        let Some(block) = self.sequence.to_reserve(count, second_at(time)) else {
            return true;
        };
        if !self.store.save(GroupWrite::Sequence(block.1)).await {
            warn!("[MSG] no sequence numbers: their block was not stored");
            return false;
        }
        self.sequence.reserved(block);
        true
    }

    /// Takes a key message from `remover`. Returns `None` when it cannot be acted on yet, with
    /// no timebase.
    fn learned_key(&mut self, remover: u8, new: NewKey) -> Option<Learned> {
        let now = self.time.now();
        let (Some(group), Some((time, _))) = (&self.group, self.clock.at(now)) else {
            return None;
        };
        let round = stored_round_at(time);
        let (removed, generation) = (new.removed, new.generation);
        let rival = self
            .removals
            .rekey
            .pending()
            .filter(|pending| pending.new.generation == generation && pending.new.key != new.key)
            .map(|_| self.removal(&new, remover, RemovalStage::Lost));
        match self.removals.rekey.learned(group, remover, new, round) {
            Learned::Ignored => {
                if let Some(rival) = rival {
                    self.shown.removals.rival = Some(rival);
                }
                info!(
                    "[REKEY] key message from {} for generation {} ignored",
                    remover, generation
                );
                Some(Learned::Ignored)
            }
            Learned::Pending => {
                let switch = self
                    .removals
                    .rekey
                    .pending()
                    .map_or(0, |pending| pending.switch);
                info!(
                    "[REKEY] {} asks to remove {}: generation {} from round {} (now {}) unless declined",
                    remover, removed, generation, switch, round
                );
                self.save_rekey();
                Some(Learned::Pending)
            }
            Learned::Later => {
                debug!(
                    "[REKEY] key message from {} for generation {} follows a key not held yet",
                    remover, generation
                );
                Some(Learned::Later)
            }
        }
    }

    /// Switches the group to the pending key.
    fn switch_key(&mut self) {
        let Some(group) = &mut self.group else {
            return;
        };
        let (old_key, old_generation) = (group.key().clone(), group.generation());
        let Some(switched) = self.removals.rekey.switch(group) else {
            return;
        };
        info!(
            "[REKEY] switched to generation {}: {} removed {:?}",
            group.generation(),
            switched.remover,
            switched.removed
        );
        if let Some(restored) = switched.restored {
            warn!("[REKEY] a rival key won; {} is a member again", restored);
            self.unsaved.slot(restored);
        }
        if !switched.undone.is_empty() {
            warn!(
                "[REKEY] this device's removal lost; removing {:#010x} again",
                switched.undone.bits()
            );
        }
        if let Some(until) = self.removals.rekey.undo_until() {
            info!("[REKEY] it can be declined until round {}", until);
        }

        self.removals.switched(
            group,
            &switched,
            (old_key, old_generation),
            &self.me,
            self.time.now(),
        );
        self.save_switch();
        self.sync_schedule();
        let decline = match self.removals.rekey.undo_until() {
            Some(until) => self
                .local_at_round(until + 1)
                .map_or(Decline::Expired, Decline::Until),
            None => Decline::Own,
        };
        if let Some(key) = self
            .group
            .as_ref()
            .map(|group| key_fingerprint(group.key()))
        {
            let at = self.time.now();
            self.show_stage(key, RemovalStage::Switched { at, decline });
        }
    }

    /// Keeps a key message new to this node for catch-up, once its origin's signature checks
    /// out. That takes tens of milliseconds, once for each key message.
    fn keep_key(&mut self, message: &Message) {
        let Some(generation) = message.generation() else {
            return;
        };
        let Some(public) = self
            .group
            .as_ref()
            .and_then(|group| group.member(message.origin()))
            .map(|member| member.public)
        else {
            return;
        };
        if !message.verify_key(&public) {
            warn!(
                "[REKEY] a key message from {} for generation {} is not its own",
                message.origin(),
                generation
            );
            return;
        }
        let remover = self.removals.rekey.remover_of(generation).or_else(|| {
            self.removals
                .rekey
                .pending()
                .filter(|pending| pending.new.generation == generation)
                .map(|pending| pending.remover)
        });
        self.removals.keep(message, remover);
    }

    /// Keeps for catch-up the key messages of `generation` from `remover` that the store holds
    /// and nothing kept yet, as a switch that dropped a rival's asks, with a pause between
    /// signatures.
    async fn refill_kept(&mut self, generation: u16, remover: u8) {
        let Some(public) = self
            .group
            .as_ref()
            .and_then(|group| group.member(remover))
            .map(|member| member.public)
        else {
            return;
        };
        let missing: heapless::Vec<messages::Name, { IDS as usize }> = self
            .messages
            .iter()
            .filter(|message| {
                message.generation() == Some(generation)
                    && message.origin() == remover
                    && matches!(message.to(), To::Member(dest)
                        if self.removals.kept(dest, generation).is_none())
            })
            .map(Message::name)
            .take(IDS as usize)
            .collect();
        for name in missing {
            if let Some(message) = self.messages.get(name).copied()
                && message.verify_key(&public)
            {
                self.removals.keep(&message, Some(remover));
            }
            self.time.until(self.time.now() + 1_000).await;
        }
    }

    /// Declines the removal of the member `removed`, under way, or the last one switched to
    /// within a day of its switch. Going back to the key before that one forgets the positions,
    /// messages and records that arrived since, so that the member brought back is not sent
    /// them.
    fn keep(&mut self, removed: u8) {
        let Some(group) = &mut self.group else {
            warn!("[REKEY] no removal to decline");
            return;
        };
        if self
            .removals
            .rekey
            .pending()
            .is_some_and(|pending| pending.new.removed == removed)
            && self.removals.rekey.decline(group)
        {
            info!("[REKEY] declined: the member stays, and this device keeps its key");
            self.save_rekey();
            if let Some(current) = &mut self.shown.removals.current
                && current.removed == removed
            {
                current.stage = RemovalStage::Declined {
                    at: self.time.now(),
                };
            }
            self.publish();
            return;
        }
        let Some((time, _)) = self.clock.at(self.time.now()) else {
            warn!("[REKEY] no timebase to decline by");
            return;
        };
        let key = key_fingerprint(group.key());
        let Some(reverted) = self
            .removals
            .rekey
            .undo(group, stored_round_at(time), removed)
        else {
            warn!("[REKEY] no removal of {} to decline", removed);
            return;
        };

        let positions = self.table.forget_others();
        let messages = self.messages.forget_since(reverted.since);
        self.inbox.prune(&self.messages);
        info!(
            "[REKEY] declined after the switch: back on generation {}, forgot records {:#010x}, {} positions and {} messages",
            group.generation(),
            reverted.forgotten.bits(),
            positions,
            messages
        );
        self.summaries = Summaries::default();
        self.summary = None;
        self.removals.reverted();
        self.save_switch();
        self.sync_schedule();
        let at = self.time.now();
        self.show_stage(key, RemovalStage::Declined { at });
        self.shown.positions(&self.table);
        self.publish();
    }

    /// Makes the message telling the member this device removed that it was, now that the
    /// group has switched, and queues it to go under the old key.
    async fn tell_removed(&mut self, time: i64) {
        let Some(removed) = self.removals.take_to_tell() else {
            return;
        };
        let notice = Outgoing::new(To::Member(removed.0), None, &[kind::REMOVED]);
        match self.make(&notice, time).await {
            Made::Message(message) => self.removals.tell(message, removed),
            Made::Wait => self.removals.tell_later(removed),
            Made::Dropped => {}
        }
    }

    /// When the pending key's switch round starts, on the local clock.
    fn switch_at(&self, now: i64) -> Option<i64> {
        let pending = self.removals.rekey.pending()?;
        let (time, _) = self.clock.at(now)?;
        Some(now + i64::from(pending.switch) * ROUND_US - time)
    }

    /// The member at `id` as the other member of a message stamped `stamp`: none when it joined
    /// after the message was sent, so that a device given the id of one removed is not taken for
    /// it. On a clock started from a boot, stamps are no UTC to compare with.
    fn peer(&self, id: u8, stamp: u32) -> crate::inbox::Peer {
        let member = self.group.as_ref()?.member(id)?;
        let utc = self
            .clock
            .at(self.time.now())
            .is_some_and(|(_, timebase)| timebase.source.is_utc());
        (!utc || member.joined <= stamp).then(|| (fingerprint(&member.public), member.name))
    }

    /// UTC seconds at local time `now`, from GNSS or the RTC, or else from a timebase its root
    /// started from UTC.
    fn utc(&self, now: i64) -> Option<i64> {
        utc_now(&self.device, now).or_else(|| {
            let (time, timebase) = self.clock.at(now)?;
            timebase
                .source
                .is_utc()
                .then_some(time.div_euclid(1_000_000))
        })
    }

    /// As [`utc`](Self::utc), or 0 without UTC.
    fn utc_seconds(&self, now: i64) -> u32 {
        self.utc(now)
            .map_or(0, |utc| utc.clamp(0, i64::from(u32::MAX)) as u32)
    }

    /// Takes a packet from `sender` under the old key `key` of generation `generation`: it
    /// missed a switch, or took a rival key that lost, and the key message of generation
    /// `catches_up_with` goes to it.
    fn heard_on_old(
        &mut self,
        header: &Header,
        (key, generation, catches_up_with): (Key, u16, u16),
        done: i64,
    ) {
        let sender = header.sender;
        let Some((time, ours)) = self.clock.at(done) else {
            return;
        };
        // A packet replayed long after it was sent must not spend what goes to its sender.
        if ours.source.is_utc()
            && header.timebase.source.is_utc()
            && (i64::from(header.base) * 1_000_000 - time).abs() > UTC_BOUND_US
        {
            info!(
                "[REKEY] {} on generation {}, but its packet is far from this clock: nothing goes to it",
                sender, generation
            );
            return;
        }
        self.removals
            .heard_on_old(sender, (key, generation, catches_up_with), round_at(time));
    }

    /// Queues text to `to`, a member, or the whole group with `None`.
    fn queue_text(&mut self, to: Option<u8>, text: Text) {
        let Some(group) = &self.group else {
            warn!("[MSG] in no group to send to");
            return;
        };
        let to = match to {
            None => To::Group,
            Some(id) if id != group.own() && group.member(id).is_some() => To::Member(id),
            Some(id) => {
                warn!("[MSG] no other member has id {}", id);
                return;
            }
        };
        let mut plain = [0; 1 + TEXT_MAX];
        plain[0] = kind::TEXT;
        plain[1..1 + text.as_bytes().len()].copy_from_slice(text.as_bytes());
        let mut outgoing = Outgoing::new(to, None, &plain[..1 + text.as_bytes().len()]);
        if self.outbox.is_full() {
            warn!("[MSG] the outbox is full");
            return;
        }
        let destination = match to {
            To::Group => None,
            To::Member(id) => Some(id),
        };
        let own = group.own();
        let peer = destination
            .and_then(|id| group.member(id))
            .map(|member| (fingerprint(&member.public), member.name));
        outgoing.shown = self
            .inbox
            .queue(own, destination, text.as_bytes(), self.time.now(), peer);
        _ = self.outbox.push_back(outgoing);
        info!("[MSG] queued to {:?}: {}", to, Ascii(text.as_bytes()));
        self.show_messages();
    }

    /// Posts each message waiting in the outbox, in order, until one has to wait.
    async fn post_outbox(&mut self, time: i64) {
        while let Some(outgoing) = self.outbox.front().copied() {
            if !self.post(&outgoing, time).await {
                return;
            }
            self.outbox.pop_front();
        }
    }

    /// Gives a message made here a sequence number and seals it, at timebase time `time`. A
    /// number needs its block stored first.
    async fn make(&mut self, outgoing: &Outgoing, time: i64) -> Made {
        if !self.reserve(1, time).await {
            return Made::Wait;
        }
        let Some(group) = &self.group else {
            return Made::Dropped;
        };
        let own = group.own();
        let plain = &outgoing.plain[..usize::from(outgoing.len)];
        let stamp = second_at(time);
        let made = match outgoing.to {
            To::Group => self
                .sequence
                .take()
                .and_then(|(seq, prev)| Message::to_group(own, seq, prev, stamp, plain)),
            To::Member(dest) => {
                // A member just removed is still told so.
                let Some(public) = group
                    .member(dest)
                    .map(|member| member.public)
                    .or_else(|| group.gone(dest).map(|gone| gone.public))
                else {
                    warn!("[MSG] id {} went before its message did", dest);
                    return Made::Dropped;
                };
                let Some(key) = self.pairwise.key(&self.me, dest, &public) else {
                    warn!("[MSG] no key shared with id {}", dest);
                    return Made::Dropped;
                };
                let key = key.clone();
                self.sequence
                    .take()
                    .and_then(|(seq, prev)| match outgoing.generation {
                        Some(generation) => Message::key(
                            own, dest, seq, prev, stamp, generation, plain, &key, &self.me,
                        ),
                        None => Message::private(own, dest, seq, prev, stamp, plain, &key),
                    })
            }
        };
        match made {
            Some(message) => Made::Message(message),
            None => {
                warn!("[MSG] a message could not be made");
                Made::Dropped
            }
        }
    }

    /// Puts a message made here in the store to be sent, at timebase time `time`. Returns false
    /// when it has to wait, and true once it is posted or cannot be.
    async fn post(&mut self, outgoing: &Outgoing, time: i64) -> bool {
        let message = match self.make(outgoing, time).await {
            Made::Wait => return false,
            Made::Dropped => {
                if outgoing.shown != 0 {
                    self.inbox.dropped(outgoing.shown);
                }
                return true;
            }
            Made::Message(message) => message,
        };
        if outgoing.shown != 0 {
            self.inbox.posted(outgoing.shown, message.seq());
        }
        self.removals.keep(&message, Some(message.origin()));
        let inserted = self.messages.insert(message, round_start_s(time));
        info!("[MSG] posted {} {:?}", message, inserted);
        true
    }

    /// Takes a message new to this node: shows one for it, opens a private one, and
    /// acknowledges what it opens. `own` is this node's id.
    fn arrived(&mut self, message: &Message, own: u8) -> Arrival {
        let origin = message.origin();
        match message.to() {
            To::Group => {
                match message.body().split_first() {
                    Some((&kind::TEXT, text)) => {
                        if origin != own {
                            info!("[MSG] from {} to all: {}", origin, Ascii(text));
                        }
                        let at = self.local_at(message.stamp());
                        let peer = (origin != own)
                            .then(|| self.peer(origin, message.stamp()))
                            .flatten();
                        self.inbox.arrived(
                            message.name(),
                            own,
                            None,
                            message.stamp(),
                            at,
                            text,
                            peer,
                        );
                    }
                    _ if origin != own => info!("[MSG] from {} to all, kind unknown", origin),
                    _ => {}
                }
                Arrival::Done
            }
            To::Member(dest) if dest == own && origin != own => {
                let Some(public) = self
                    .group
                    .as_ref()
                    .and_then(|group| group.member(origin))
                    .map(|member| member.public)
                else {
                    info!(
                        "[MSG] a private message from id {}, whose record is not here yet",
                        origin
                    );
                    return Arrival::Later;
                };
                let Some(key) = self.pairwise.key(&self.me, origin, &public) else {
                    return Arrival::Done;
                };
                let mut out = [0; BODY_MAX];
                let Ok(plain) = message.open(key, &mut out) else {
                    warn!("[MSG] a private message from {} did not open", origin);
                    return Arrival::Done;
                };
                match plain.split_first() {
                    Some((&kind::TEXT, text)) => {
                        info!("[MSG] from {} to this device: {}", origin, Ascii(text));
                        let at = self.local_at(message.stamp());
                        let peer = self.peer(origin, message.stamp());
                        self.inbox.arrived(
                            message.name(),
                            own,
                            Some(own),
                            message.stamp(),
                            at,
                            text,
                            peer,
                        );
                    }
                    Some((&kind::ACK, seq)) => {
                        let seq = seq.try_into().map(u32::from_be_bytes).unwrap_or(0);
                        info!("[MSG] {}/{} delivered to {}", own, seq, origin);
                        self.inbox.delivered(own, origin, seq);
                        return Arrival::Done;
                    }
                    // Only a removal shown is acknowledged: a key ignored, as one declined is,
                    // would otherwise answer every catch-up.
                    Some((&kind::KEY, _)) => match NewKey::decode(plain)
                        .filter(|new| message.generation() == Some(new.generation))
                    {
                        Some(new) => match self.learned_key(origin, new) {
                            None | Some(Learned::Later) => return Arrival::Later,
                            Some(Learned::Ignored) => return Arrival::Done,
                            Some(Learned::Pending) => {}
                        },
                        None => {
                            warn!("[REKEY] a key message from {} is malformed", origin);
                            return Arrival::Done;
                        }
                    },
                    Some((&kind::REMOVED, _)) => {
                        warn!("[REKEY] {} removed this device from the group", origin);
                        let name = self
                            .group
                            .as_ref()
                            .and_then(|group| group.member(origin))
                            .map_or_else(
                                || Name::from_mac(&[0, 0, 0, 0, 0, origin]),
                                |member| member.name,
                            );
                        self.shown.removals.removed_by = Some((origin, name, self.time.now()));
                    }
                    Some((&kind, _)) => {
                        warn!("[MSG] from {}, kind {} unknown", origin, kind);
                    }
                    None => return Arrival::Done,
                }
                let mut ack = [kind::ACK, 0, 0, 0, 0];
                ack[1..].copy_from_slice(&message.seq().to_be_bytes());
                if self
                    .outbox
                    .push_back(Outgoing::new(To::Member(origin), None, &ack))
                    .is_err()
                {
                    warn!(
                        "[MSG] the outbox is full; {}/{} goes unacknowledged",
                        origin,
                        message.seq()
                    );
                }
                Arrival::Done
            }
            // This device's own private message, back from another member after a restart.
            To::Member(dest) if origin == own => {
                let Some(public) = self.group.as_ref().and_then(|group| {
                    group
                        .member(dest)
                        .map(|member| member.public)
                        .or_else(|| group.gone(dest).map(|gone| gone.public))
                }) else {
                    return Arrival::Done;
                };
                let Some(key) = self.pairwise.key(&self.me, dest, &public) else {
                    return Arrival::Done;
                };
                let mut out = [0; BODY_MAX];
                if let Ok(plain) = message.open(key, &mut out)
                    && let Some((&kind::TEXT, text)) = plain.split_first()
                {
                    let at = self.local_at(message.stamp());
                    // The device whose key opened it, while it is still the member there.
                    let peer = self
                        .group
                        .as_ref()
                        .and_then(|group| group.member(dest))
                        .filter(|member| member.public == public)
                        .map(|member| (fingerprint(&public), member.name));
                    self.inbox.arrived(
                        message.name(),
                        own,
                        Some(dest),
                        message.stamp(),
                        at,
                        text,
                        peer,
                    );
                }
                Arrival::Done
            }
            _ => Arrival::Done,
        }
    }

    /// Acts on the private messages to this device that could not be acted on as they came.
    fn retry_unread(&mut self, own: u8) {
        let mut from = 0;
        while let Some((at, message)) = self.messages.unread_from(from) {
            let message = *message;
            from = at + 1;
            if self.arrived(&message, own) == Arrival::Done {
                self.messages.read(message.name());
            }
        }
    }

    /// Listens throughout for a packet under the group a founding left unconfirmed, until its
    /// wait ends. One means the joining device stored the group, which this device then stores
    /// and takes up. A failed write is tried again while the wait lasts.
    async fn await_joiner(&mut self) {
        let Some((end, heard)) = self
            .founding
            .as_ref()
            .map(|founding| (founding.until, founding.heard))
        else {
            return;
        };
        if heard {
            self.time
                .until((self.time.now() + STORE_RETRY_US).min(end))
                .await;
            if self.time.now() >= end {
                warn!("[MESH] the founded group was never stored; this device founded no group");
                self.founding = None;
                self.set_recovery(RecoveryPhase::NotStored);
            } else {
                self.store_founded(None).await;
            }
            return;
        }
        if !self.radio.start_receiving().await {
            warn!("[MESH] receive start failed");
        }
        while self.radio.wait_received(end).await {
            let Some((packet, done)) = self.radio.read_packet().await else {
                continue;
            };
            let Some(founded) = self.founding.as_ref().map(|founding| &founding.group) else {
                return;
            };
            let mut bytes = packet.payload;
            if seal::open(founded.key(), &mut bytes[..packet.length]).is_err() {
                info!("[MESH] not ours len={} rssi={}", packet.length, packet.rssi);
                continue;
            }
            info!("[MESH] heard the joining device; storing the founded group");
            self.radio.standby().await;
            self.store_founded(Some((packet, done))).await;
            return;
        }
        info!("[MESH] the joining device was not heard; this device founded no group");
        self.founding = None;
        self.set_recovery(RecoveryPhase::Expired);
        self.radio.standby().await;
    }

    /// Stores the group a founding left unconfirmed, its joining device heard, and takes it up
    /// once it is stored. `heard` is the packet that proved the join, while it is fresh.
    async fn store_founded(&mut self, heard: Option<(Received, i64)>) {
        let Some(founding) = &mut self.founding else {
            return;
        };
        founding.heard = true;
        let (group, until) = (founding.group.clone(), founding.until);
        self.set_recovery(RecoveryPhase::Storing);
        self.publish();
        if !self
            .store
            .save(GroupWrite::Group(Box::new(group), None))
            .await
        {
            warn!("[MESH] storing the founded group failed; trying again");
            self.set_recovery(RecoveryPhase::SaveFailed { until });
            return;
        }
        let Some(founding) = self.founding.take() else {
            return;
        };
        info!("[MESH] the founded group is this device's");
        let own = founding.group.own();
        self.group = Some(founding.group);
        self.sync_schedule();
        self.unsaved.group_replaced();
        self.restart(own);
        if let Some((packet, done)) = heard {
            self.take(&packet, done);
        }
        self.set_recovery(RecoveryPhase::Stored);
    }

    fn set_recovery(&mut self, phase: RecoveryPhase) {
        if let Some(recovery) = &mut self.shown.recovery {
            recovery.phase = phase;
        }
    }

    /// Starts a refresh, which listens throughout for three rounds and goes on sending in this
    /// node's slot. One under way keeps its time.
    fn start_refresh(&mut self) {
        let Some(group) = &self.group else {
            warn!("[MESH] no group to refresh");
            return;
        };
        if self.refresh.is_some_and(|refresh| refresh.is_listening()) {
            return;
        }
        let now = self.time.now();
        self.clock.sweep_to(now + SWEEP_US);
        self.refresh_known = Some(Box::new(core::array::from_fn(|id| {
            group.member(id as u8).map(|member| member.mac)
        })));
        self.refreshes += 1;
        self.refresh = Some(RefreshView {
            session: self.refreshes,
            phase: RefreshPhase::Listening {
                until: now + SWEEP_US,
            },
            heard: 0,
            learned: 0,
        });
        info!("[MESH] refreshing for {}s", SWEEP_US / 1_000_000);
    }

    /// When the refresh under way ends.
    fn refresh_until(&self) -> Option<i64> {
        match self.refresh?.phase {
            RefreshPhase::Listening { until } => Some(until),
            _ => None,
        }
    }

    /// Notes the members a refresh under way has learned, and ends it at its time. Returns
    /// whether that changed what the screens show.
    fn update_refresh(&mut self, now: i64) -> bool {
        let (Some(refresh), Some(group), Some(known)) = (
            &mut self.refresh,
            &self.group,
            self.refresh_known.as_deref(),
        ) else {
            return false;
        };
        let RefreshPhase::Listening { until } = refresh.phase else {
            return false;
        };
        // A timebase taken up or a fix ends a sweep; a refresh listens on to its end.
        if now < until {
            self.clock.sweep_to(until);
        }
        let before = *refresh;
        refresh.learned = group
            .members()
            .filter(|(_, member)| !known.contains(&Some(member.mac)))
            .fold(0, |learned, (id, _)| learned | 1 << id);
        if now >= until {
            refresh.phase = RefreshPhase::Ended { at: until };
            info!(
                "[MESH] refresh ended heard={:#010x} learned={:#010x}",
                refresh.heard, refresh.learned
            );
            self.refresh_known = None;
        }
        *refresh != before
    }

    /// Ends a refresh under way early, as a pairing takes the radio.
    fn stop_refresh(&mut self, now: i64) {
        self.update_refresh(now);
        if let Some(refresh) = &mut self.refresh
            && refresh.is_listening()
        {
            refresh.phase = RefreshPhase::Interrupted { at: now };
            self.refresh_known = None;
            info!("[MESH] refresh stopped for a pairing");
        }
    }
}

fn log_pairing(pairing: &Pairing, phase: Phase, now: i64) {
    let left = pairing
        .deadline()
        .map_or(0, |deadline| (deadline - now).max(0) / 1_000_000);
    match phase {
        Phase::Found => {
            for (i, mac) in pairing.candidates().enumerate() {
                info!("[PAIR] found {}: {}", i, Mac(&mac));
            }
        }
        Phase::Compare { code } => info!(
            "[PAIR] code {:06} with {}; {}s to confirm",
            code,
            Mac(&pairing.peer().unwrap_or_default()),
            left
        ),
        _ => {}
    }
    info!("[PAIR] {:?} {:?} ({}s left)", pairing.role(), phase, left);
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
        Some(Timebase {
            source: Source::Boot(root),
            hops,
        }) => info!("[MESH] timebase root={} from its boot hops={}", root, hops),
    }
}

/// Stands in for the node on a device whose radio did not answer: it keeps this device's name and
/// can leave the group, and tells the screens there is no radio to pair with.
pub async fn offline(
    start: Start,
    time: &impl Time,
    device: &impl Device,
    store: &impl GroupStore,
    commands: &impl Commands,
) -> ! {
    let mut me = start.me;
    let mut group = start.group.map(|group| *group);
    let mut shown = Shown::new(false);
    let mut view = blank_view();
    loop {
        let now = time.now();
        shown.fill(&mut view, &me, group.as_ref(), now, utc_now(device, now));
        device.publish(&mut view);
        match commands.receive().await {
            Command::Leave => {
                // With no radio there is nobody to tell.
                let (left, _) = leave(&mut group, store).await;
                shown.answer(Answer::Left(left));
            }
            Command::Rename(name) => {
                let saved = rename(
                    &mut me,
                    group.as_mut(),
                    name,
                    utc_seconds(device, time.now()),
                    store,
                )
                .await;
                if saved && let Some(group) = &group {
                    let id = group.own();
                    store.queue(GroupWrite::Slot {
                        id,
                        slot: group.slot(id).copied(),
                    });
                }
                shown.answer(Answer::Renamed(saved));
            }
            command => warn!("[MESH] no radio for {:?}", command),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEND_AT: i64 = 10_000_000;

    /// The latest local time a packet of `len` bytes can go out and still leave the own slot
    /// its preparation.
    fn last_start(len: usize) -> i64 {
        SEND_AT - PREPARE_US - airtime_us(len) - 1
    }

    #[test]
    fn a_notice_that_fits_goes_ahead_of_an_old_key_packet() {
        let at = last_start(NOTICE_LEN);
        assert_eq!(
            Next::choose(SEND_AT, Some(at), Some((0, 7))),
            Next::Notice { at }
        );
    }

    #[test]
    fn an_old_key_packet_goes_when_the_notice_does_not_fit() {
        let at = last_start(MAX_PACKET);
        assert_eq!(
            Next::choose(SEND_AT, Some(last_start(NOTICE_LEN) + 1), Some((at, 7))),
            Next::Old { at, round: 7 }
        );
    }

    #[test]
    fn the_own_packet_goes_when_nothing_ends_in_time() {
        assert_eq!(Next::choose(SEND_AT, None, None), Next::Own);
        assert_eq!(
            Next::choose(
                SEND_AT,
                Some(last_start(NOTICE_LEN) + 1),
                Some((last_start(MAX_PACKET) + 1, 7))
            ),
            Next::Own
        );
    }
}
