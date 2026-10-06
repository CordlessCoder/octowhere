//! Runs the location mesh on a radio: listens throughout, sends this node's packets when the
//! channel is clear (`access`), and keeps the timebase that names their times. Pairing takes the
//! radio over, on a channel of its own, until it ends. `octowhere_mesh` holds the protocol and
//! `context/LORA-PROTOCOL.md` the design.

use alloc::boxed::Box;
use core::alloc::Allocator;

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn};
use embassy_futures::select::{Either, Either3, select, select3};
use octowhere_mesh::{
    IDS, Ids, Zeroable,
    absorb::{Event, State, When, absorb},
    clock::{Clock, UTC_BOUND_US},
    compose::{Sources, compose},
    members::{Gone, Group, Member, Name, PUBLIC_LEN, Requests, fingerprint},
    messages::{
        self, BODY_MAX, Insert, Message, Pairwise, SETTLE_US, Sequence, Store, Summaries, TEXT_MAX,
        To, kind,
    },
    packet::{Entry, Hdop, Header, MAX_PACKET, Plain, Quality, Record, Sealing, Source, Timebase},
    pair::{Done, End, Identity, MAX_FRAME, Pairing, Phase, Role},
    rekey::{Learned, NewKey, OLD_KEYS, Rekey, key_fingerprint},
    relays::Relays,
    schedule::{
        ROUND_US, airtime_us, is_sweep_round, round_at, round_start_s, second_at, stored_round_at,
    },
    seal::{self, Key, SIV_LEN},
    table::Table,
};

use crate::{
    GroupWrite, KeptRow,
    access::{Access, BUSY_STEPS, Holding, REPAIR_SPREAD_US, SPREAD_US, STEP_US, STEPS},
    fmt::{Ascii, Mac},
    inbox::Inbox,
    removals::Removals,
    unsaved::{Due, Unsaved},
    view::{
        Answer, Decline, GroupView, MemberView, MeshView, MessagesView, PairingView, Position,
        RecoveryPhase, RecoveryView, Refusal, Refused, RemovalStage, RemovalView, RemovalsView,
        Request, Text, Thread, Unremovable,
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
/// How long after a packet ends `DIO0`'s RxDone is seen, plus how long after the time its header
/// names a sender's transmission begins: half how late a root heard the nodes timing from it, on
/// two boards whose `DIO0` follows the radio, when packets went in slots (2026-10-01). A sender
/// now also checks the channel and loads its packet after naming the time. Polling the flags
/// adds half a poll.
const ARRIVAL_LATENCY_US: i64 = 1_050;
/// How long a pairing waits for the group to be stored before it counts as a failure.
const STORE_TIMEOUT_US: i64 = 10 * 1_000_000;
/// The longest a pairing listens before it runs its timers again.
const PAIR_LISTEN_US: i64 = 250_000;
/// How long a device that founded a group, without hearing the last acknowledgement, listens
/// for the joining device under the group's key. That device waits 30 s for done, sweeps for
/// three rounds for a timebase, then sends at once, since it starts its own: about 2¾ minutes in
/// all.
const FOUNDING_WAIT_US: i64 = 10 * 60 * 1_000_000;
/// How long after a failed write a founding's wait tries to store its group again.
const STORE_RETRY_US: i64 = 10 * 1_000_000;
/// The messages made here that can wait for a sequence number and a timebase.
const OUTBOX: usize = 8;
/// The packets a device that leaves sends its gone record in.
const LEAVE_REPEATS: u8 = 2;
/// How far apart they go, so that whatever lost the first does not lose the second.
const LEAVE_GAP_US: i64 = 10 * 1_000_000;
/// How long a device that left tries to tell the others.
const LEAVE_WAIT_US: i64 = 3 * ROUND_US;
/// A sweep round's packets go at a time each node draws in its first half, so that they do not
/// all meet at its start, and one that finds the channel busy still goes within the round.
const SWEEP_SPREAD_US: i64 = ROUND_US / 2;

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
    /// Sends text to one member, privately, or with `None` to the whole group.
    Send {
        to: Option<u8>,
        text: Text,
    },
    /// Sends text privately to the member at `id`, if it is still the device with this
    /// fingerprint.
    SendDevice {
        id: u8,
        device: [u8; 8],
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
            Request::Send {
                to: Thread::Group,
                text,
            } => Self::Send { to: None, text },
            Request::Send {
                to: Thread::Member(id, device),
                text,
            } => Self::SendDevice { id, device, text },
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
    /// The key messages stored for each member that missed a switch.
    pub kept: [Option<Box<KeptRow>>; IDS as usize],
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
        &mut Fingerprints::default(),
    );
    device.publish(&mut view);
}

/// What the screens are shown beyond the group itself.
struct Shown {
    radio: bool,
    sessions: u32,
    pairing: Option<PairingView>,
    refusal: Option<Refusal>,
    answered: u32,
    answer: Option<Answer>,
    /// The node's `heard`, copied in as it renders. Protocol decisions read the node's own,
    /// never this.
    heard: [Option<i64>; IDS as usize],
    /// The newest position held for each id: its UTC second and its coordinates. Kept past the
    /// table's expiry, so an old position shows as old rather than never received.
    positions: [Option<(u32, (i32, i32))>; IDS as usize],
    /// A founding's wait, under way or ended, until another pairing starts.
    recovery: Option<RecoveryView>,
    removals: RemovalsView,
}

/// Each id's public key and its fingerprint, so that a publish hashes only a key that changed.
struct Fingerprints([Option<([u8; PUBLIC_LEN], [u8; 8])>; IDS as usize]);

impl Default for Fingerprints {
    fn default() -> Self {
        Self([None; IDS as usize])
    }
}

impl Fingerprints {
    fn of(&mut self, index: usize, public: &[u8; PUBLIC_LEN]) -> [u8; 8] {
        match &mut self.0[index] {
            Some((key, made)) if key == public => *made,
            slot => {
                let made = fingerprint(public);
                *slot = Some((*public, made));
                made
            }
        }
    }
}

impl Shown {
    fn new(radio: bool) -> Self {
        Self {
            radio,
            sessions: 0,
            pairing: None,
            refusal: None,
            answered: 0,
            answer: None,
            heard: [None; IDS as usize],
            positions: [None; IDS as usize],
            recovery: None,
            removals: RemovalsView::default(),
        }
    }

    fn answer(&mut self, answer: Answer) {
        self.answered += 1;
        self.answer = Some(answer);
    }

    /// Shows the pairing asked for in `session` as refused.
    fn refuse(&mut self, session: u32, role: Role, refused: Refused) {
        self.pairing = None;
        self.refusal = Some(Refusal {
            session,
            role,
            refused,
        });
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
        fingerprints: &mut Fingerprints,
    ) {
        // Named in full, so that a field added to the view cannot be left unfilled.
        let MeshView {
            radio,
            mac,
            name,
            group: shown_group,
            sessions,
            pairing,
            refusal,
            answered,
            answer,
            recovery,
            removals,
        } = view;
        *radio = self.radio;
        *mac = me.mac;
        *name = me.name;
        *sessions = self.sessions;
        pairing.clone_from(&self.pairing);
        *refusal = self.refusal;
        *answered = self.answered;
        *answer = self.answer;
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
                device: fingerprints.of(index, &member.public),
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
    utc: Option<u32>,
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

/// UTC seconds at local time `now`, from GNSS or the RTC.
fn utc_seconds(device: &impl Device, now: i64) -> Option<u32> {
    utc_now(device, now).map(|utc| utc.clamp(0, i64::from(u32::MAX)) as u32)
}

/// UTC seconds at local time `now`, from GNSS or the RTC.
fn utc_now(device: &impl Device, now: i64) -> Option<i64> {
    device
        .gps_time()
        .map(|gps| now - gps.offset)
        .or_else(|| device.rtc_utc(now))
        .map(|utc| utc / 1_000_000)
}

/// A group this device left, kept until it has told the others.
struct Leaving {
    group: Group,
    gone: Gone,
    /// The packets it is still to be sent in.
    left: u8,
    /// When the next may go, on the local clock.
    next: i64,
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

/// What became of a packet offered on a channel that had to be clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Sent {
    /// A packet was under way, or one arrived that is not yet read: nothing went.
    Busy,
    /// It could not be loaded, or the antenna switched to transmit: nothing went.
    Failed,
    /// It went, starting at local time `started`, and was seen to finish if `finished`.
    Done { started: i64, finished: bool },
}

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
    /// Starts receiving until the radio is put in standby, or goes on without a break if it is
    /// receiving already. Returns whether it is receiving.
    async fn start_receiving(&mut self) -> bool;
    /// Waits until local time `deadline` for a packet to arrive. Returns whether one did.
    async fn wait_received(&mut self, deadline: i64) -> bool;
    /// Reads the packet that arrived, and when its end was seen.
    async fn read_packet(&mut self) -> Option<(Received, i64)>;
    /// How late, on average, a packet's end is seen after it ends.
    fn seen_late_us(&self) -> i64;
    /// Sends `packet` at once, and leaves the radio as [`Radio::idle_receive`] does. Returns
    /// whether it was seen to finish, or `None` when nothing went, as [`Sent::Failed`].
    async fn transmit(&mut self, packet: &[u8]) -> Option<bool>;
    /// Sends `packet` as [`Radio::transmit`] does if, while receiving, the channel is clear: no
    /// packet under way that the radio can detect, and none arrived that is not yet read.
    /// Nothing the radio waits on may come between the check and the transmission's start.
    /// Leaves the radio receiving when the channel is busy.
    async fn transmit_if_clear(&mut self, packet: &[u8]) -> Sent;
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
    /// The member records the next packet asks for.
    requests: Requests,
    /// What changed in the group and its removals and is not yet queued to be stored.
    unsaved: Unsaved,
    clock: Clock,
    table: Table,
    access: Access,
    /// The sweep round this node last drew a time to send in, and that time on the local timer.
    sweep_at: Option<(i64, i64)>,
    /// When a catch-up under an old key goes, on the local timer, drawn once within
    /// [`REPAIR_SPREAD_US`] of its being due.
    catch_up_at: Option<i64>,
    timebase_shown: Option<Timebase>,
    /// When each id was last heard sending, on the local clock.
    heard: [Option<i64>; IDS as usize],
    /// What the screens are shown.
    shown: Shown,
    fingerprints: Box<Fingerprints>,
    /// The view [`publish`] fills.
    view: Box<MeshView>,
    /// Every message the node holds.
    messages: Box<Store, A>,
    /// The messages the screens are shown.
    inbox: Inbox<A>,
    summaries: Summaries,
    /// When no message new to this node has arrived for [`SETTLE_US`], on the local timer.
    settle: Option<i64>,
    relays: Box<Relays>,
    pairwise: Box<Pairwise>,
    sequence: Sequence,
    outbox: Box<heapless::Deque<Outgoing, OUTBOX>>,
    removals: Removals<A>,
    /// The numbers of the writes queued without waiting, whose results are yet to be checked.
    writes: heapless::Vec<u32, 8>,
    /// A summary made before the backoff, with the origin the next one starts from.
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
            requests: Requests::default(),
            unsaved: Unsaved::default(),
            clock: Clock::new(own, now),
            table: Table::new(own),
            access: Access::default(),
            sweep_at: None,
            catch_up_at: None,
            timebase_shown: None,
            heard: [None; IDS as usize],
            shown: Shown::new(true),
            fingerprints: Box::default(),
            view: blank_view(),
            messages: zeroed_in(alloc.clone()),
            inbox: Inbox::new(zeroed_in(alloc.clone())),
            summaries: Summaries::default(),
            settle: None,
            relays: Box::default(),
            pairwise: Box::default(),
            sequence: Sequence::new(start.sequence),
            outbox: Box::default(),
            removals: Removals::new(rekey, zeroed_in(alloc)),
            writes: heapless::Vec::new(),
            summary: None,
        };
        for (id, row) in (0..IDS).zip(&start.kept) {
            if let Some(row) = row {
                mesh.removals.restore_kept(id, row);
            }
        }
        // A switch just before a restart may not have told the others yet, who wait for it.
        if let Some(group) = mesh.group.as_ref().filter(|group| group.generation() > 0) {
            mesh.removals.carry_on_key(group, &mesh.me, now);
        }
        let tuned = mesh.radio.tune(FREQUENCY_HZ, SYNC_WORD, POWER_DBM).await;
        info!("[MESH] tuned={}", tuned);
        mesh.publish();
        mesh
    }

    #[inline(never)]
    fn publish(&mut self) {
        self.show_pending();
        let now = self.time.now();
        let utc = self.utc(now);
        self.shown.heard = self.heard;
        self.shown.fill(
            &mut self.view,
            &self.me,
            self.group.as_ref(),
            now,
            utc,
            &mut self.fingerprints,
        );
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
        let switch = self.local_at_round(pending.own_switch);
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
        let (Some(group), Some(declinable)) = (&self.group, self.removals.rekey.declinable())
        else {
            return;
        };
        let (Some(remover), Some(at), Some(until)) = (
            self.removals.rekey.remover_of(group.generation()),
            self.local_at_round(declinable.switched),
            self.local_at_round(declinable.until + 1),
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
            removed: declinable.removed,
            removed_name: name(declinable.removed, declinable.record.as_ref()),
            device: declinable.fingerprint,
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
                        self.radio.idle_receive().await;
                        // A pairing takes the radio, so the gone record goes out once before it,
                        // as long as the wait to tell the others allows.
                        if matches!(command, Command::Add | Command::Join) {
                            while self
                                .leaving
                                .as_ref()
                                .is_some_and(|leaving| leaving.left == LEAVE_REPEATS)
                            {
                                self.tell_leaving().await;
                            }
                            self.leaving = None;
                            self.radio.idle_receive().await;
                        }
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
            Command::Add if let Some(pending) = self.removals.rekey.pending() => {
                warn!("[MESH] a removal is under way; adding waits for its switch");
                let key = key_fingerprint(&pending.new.key);
                self.shown.sessions += 1;
                self.shown
                    .refuse(self.shown.sessions, Role::Add, Refused::Removing { key });
            }
            Command::Add => self.pair(Role::Add, commands).await,
            Command::Join if self.group.is_some() => {
                warn!("[MESH] in a group; it must leave before it can join another");
                self.shown.sessions += 1;
                self.shown
                    .refuse(self.shown.sessions, Role::Join, Refused::InGroup);
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
                        next: now,
                        until: now + LEAVE_WAIT_US,
                    }));
                }
                if left {
                    self.forget_messages();
                    self.unsaved.group_replaced();
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
            Command::Send { to, text } => self.queue_text(to, text),
            Command::SendDevice { id, device, text } => {
                if self.holds(id, device) {
                    self.queue_text(Some(id), text);
                } else {
                    warn!("[MSG] {} is another device now; not sent", id);
                }
            }
            Command::Read(id) => {
                self.inbox.read(id);
                self.show_messages();
            }
            Command::Remove(id) => {
                let started = self.remove(id).await;
                self.shown.answer(Answer::Removing(started));
            }
            Command::RemoveDevice { id, device } => {
                let started = if self.holds(id, device) {
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
                        joined: octowhere_mesh::members::stamp(now),
                        changed: octowhere_mesh::members::stamp(now),
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
            self.removals.kept_lost();
        }
        if let Some(group) = &mut self.group {
            for id in group.take_moved().without(group.own()).iter() {
                self.table.forget(id);
                self.heard[usize::from(id)] = None;
                self.shown.positions[usize::from(id)] = None;
            }
            let changed = group.take_changed();
            for id in changed.iter() {
                self.unsaved.slot(id);
            }
            if self.removals.rekey.changed(changed) {
                self.unsaved.rekey();
            }
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
        // After the removals, which say who is still waited for.
        while let Some(id) = self.removals.kept_due() {
            let row = self.removals.kept_row(id);
            let stores = row.is_some();
            if !self.queue_write(GroupWrite::Kept { id, row }) {
                return;
            }
            self.removals.kept_queued(id, stores);
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

    /// Does what is due, then listens until a packet is due and sends it after the backoff, or
    /// returns early once what is due may have changed.
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
        self.removals.at_round(stored_round_at(time));
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
        if self
            .group
            .as_ref()
            .is_some_and(|group| !self.removals.keys_due(group).is_empty())
        {
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
        if self.settle.is_some_and(|at| at <= now) {
            self.settle = None;
            if self.summaries.settled() {
                info!("[MESH] a neighbour held more than it gave; a summary goes");
            }
        }
        if let Some(own) = self.group.as_ref().map(|group| group.own()) {
            self.relays.bench_answered(own);
        }
        while let Some(name) = self.relays.due(now) {
            if self.messages.get(name).is_some() {
                info!(
                    "[MSG] {}/{} not heard passed on; it goes again",
                    name.0, name.1
                );
                if let Some(own) = self.group.as_ref().map(|group| group.own()) {
                    self.relays.bench_named(own, name);
                }
                self.messages.mark(name);
            } else {
                self.relays.forget(name);
            }
        }
        self.prepare_summary();
        let round = round_at(time);
        let round_ends = now + (round + 1) * ROUND_US - time;
        let old = self.old_due(round, now, time, own);
        let holding = self.holding(round, now, time, own);
        let spread = self.draw(REPAIR_SPREAD_US, own);
        let own_at = self.access.own_due(holding, now, || spread);
        let (at, old) = match old {
            Some(at) if at <= own_at => (at, true),
            _ => (own_at, false),
        };
        if at > now {
            // A packet heard, a new round, a flood settling or a relay unheard can change what
            // is due.
            let wake = [self.settle, self.relays.next()]
                .into_iter()
                .flatten()
                .fold(at.min(round_ends), i64::min);
            self.listen(wake).await;
            return;
        }
        let steps = match holding.records {
            true => crate::access::BENCH_RECORD_STEPS.load(core::sync::atomic::Ordering::Relaxed),
            false => STEPS,
        };
        let wait = self.draw(i64::from(steps), own) * STEP_US;
        if self.listen(now + wait).await {
            return;
        }
        if old {
            self.send_old(own, timebase).await;
        } else {
            self.send(timebase).await;
        }
    }

    /// What this node holds to send in `round`, at local time `now` and timebase time `time`.
    fn holding(&mut self, round: i64, now: i64, time: i64, own: u8) -> Holding {
        let records =
            self.group.as_ref().is_some_and(Group::has_unsent) || self.messages.has_unsent();
        let repair = !self.requests.pending().is_empty() || self.summary.is_some();
        let news = self.table.has_news() || self.removals.on_key_first();
        let start = now - (time - round * ROUND_US);
        let sweep = (is_sweep_round(round)
            && self.removals.on_key_in_sweeps(now)
            && !self.access.sent_since(start))
        .then(|| self.sweep_at(round, start, own));
        Holding {
            records,
            repair,
            news,
            sweep,
        }
    }

    /// When this node sends in sweep round `round`, which starts at local time `start`: a time
    /// it draws once a round, in its first half.
    fn sweep_at(&mut self, round: i64, start: i64, own: u8) -> i64 {
        match self.sweep_at {
            Some((drawn, at)) if drawn == round => at,
            _ => {
                let at = start + self.draw(SWEEP_SPREAD_US, own);
                self.sweep_at = Some((round, at));
                at
            }
        }
    }

    /// Notes a transmission that did not go, `sent` saying why, and backs off before the next.
    fn back_off(&mut self, sent: Sent, own: u8) {
        let wait = self.draw(i64::from(BUSY_STEPS), own) * STEP_US;
        if sent == Sent::Busy {
            debug!("[MESH] the channel is busy; backing off {}ms", wait / 1_000);
        }
        self.access.busy(self.time.now(), wait);
    }

    /// A random number below `bound`, or without a random source one that differs for each id.
    fn draw(&mut self, bound: i64, own: u8) -> i64 {
        match self.random.bytes::<4>() {
            Some(bytes) => i64::from(u32::from_le_bytes(bytes)) % bound,
            None => bound * i64::from(own) / i64::from(IDS),
        }
    }

    /// Sends the gone record of the group this device left, [`LEAVE_REPEATS`] times, and
    /// forgets the group once it has gone out in all of them or the wait is over.
    async fn tell_leaving(&mut self) {
        let now = self.time.now();
        let (Some(leaving), Some((_, timebase))) = (&self.leaving, self.clock.at(now)) else {
            info!("[MESH] left with no timebase to tell the others on");
            self.leaving = None;
            return;
        };
        if now >= leaving.until || leaving.left == 0 {
            self.leaving = None;
            return;
        }
        let (own, next, until) = (leaving.group.own(), leaving.next, leaving.until);
        let wait = self.draw(i64::from(STEPS), own) * STEP_US;
        let at = (self.access.after_quiet(next) + wait).min(until);
        // Nothing heard is taken now, but the channel is listened to for the check.
        if !self.radio.start_receiving().await {
            warn!("[MESH] receive start failed");
        }
        if self.radio.wait_received(at).await {
            let _ = self.radio.read_packet().await;
            return;
        }
        let now = self.time.now();
        let (Some(leaving), Some((time, _))) = (&self.leaving, self.clock.at(now)) else {
            return;
        };
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Sealing::new(&mut packet, &Header::new(own, timebase, time));
        let _ = builder.neighbours(self.table.neighbours(round_at(time)).bits());
        let _ = builder.gone(own, &leaving.gone);
        let len = builder.seal(leaving.group.key());
        let sent = self.radio.transmit_if_clear(&packet[..len]).await;
        let Sent::Done { started, finished } = sent else {
            self.back_off(sent, own);
            return;
        };
        self.access.sent(started, len, None);
        info!("[MESH] told the group it left done={}", finished);
        if let Some(leaving) = &mut self.leaving {
            leaving.left -= 1;
            leaving.next = started + LEAVE_GAP_US;
        }
    }

    /// When a packet under an old key is due, on the local timer: at a time drawn to catch a
    /// member up, since every neighbour that heard it on the old key answers it and those hidden
    /// from each other collide at it; and otherwise at this node's time in a sweep round while a
    /// member is waited for, so that parts of the group on rival keys still hear each other.
    fn old_due(&mut self, round: i64, now: i64, time: i64, own: u8) -> Option<i64> {
        let due = self.removals.sends_old() && self.removals.old_packet(round).is_some();
        if !(due && self.removals.catching_up()) {
            self.catch_up_at = None;
        }
        if !due {
            return None;
        }
        let at = match self.catch_up_at {
            Some(at) => at,
            None if self.removals.catching_up() => {
                let at = now + self.draw(REPAIR_SPREAD_US, own);
                self.catch_up_at = Some(at);
                at
            }
            None => self.sweep_at(round, now - (time - round * ROUND_US), own),
        };
        Some(self.access.after_quiet(at))
    }

    /// Sends a packet under an old key: a header, and the key message of each member waiting
    /// for one on that key.
    async fn send_old(&mut self, own: u8, timebase: Timebase) {
        let now = self.time.now();
        let Some((time, _)) = self.clock.at(now) else {
            return;
        };
        let round = round_at(time);
        let Some((key, generation, ids)) = self.removals.old_packet(round) else {
            return;
        };
        let key = key.clone();
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Sealing::new(&mut packet, &Header::new(own, timebase, time));
        let (mut caught, mut lost) = (Ids::EMPTY, Ids::EMPTY);
        for &(id, catches_up_with) in &ids {
            match self.removals.message_for(id, catches_up_with) {
                Some(message) if builder.message(message).is_ok() => caught.insert(id),
                Some(_) => {}
                // Gone since it was queued, as a rival's switch drops the losing key's.
                None => lost.insert(id),
            }
        }
        let len = builder.seal(&key);
        let sent = self.radio.transmit_if_clear(&packet[..len]).await;
        let Sent::Done { started, finished } = sent else {
            self.back_off(sent, own);
            return;
        };
        self.access.sent(started, len, None);
        self.removals.sent_old(caught, lost, round);
        self.catch_up_at = None;
        info!(
            "[REKEY] sent under generation {} round={} caught={:#010x} len={} done={}",
            generation,
            round,
            caught.bits(),
            len,
            finished
        );
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

    /// Receives until local time `end`, making a switch as it comes.
    /// Returns whether it took a packet or made a switch, after which the step looks afresh at
    /// what is due.
    async fn listen(&mut self, end: i64) -> bool {
        if !self.radio.start_receiving().await {
            warn!("[MESH] receive start failed");
        }
        loop {
            let now = self.time.now();
            if self.switch_at(now).is_some_and(|at| at <= now) {
                self.switch_key();
                return true;
            }
            if now >= end {
                return false;
            }
            let close = self
                .switch_at(now)
                .filter(|&at| at > now)
                .map_or(end, |at| at.min(end));
            if self.radio.wait_received(close).await {
                self.receive().await;
                return true;
            }
        }
    }

    /// Takes the packet `DIO0` reported.
    async fn receive(&mut self) {
        if let Some((packet, done)) = self.radio.read_packet().await {
            self.take(&packet, done);
        }
    }

    /// When a packet of `len` bytes whose RxDone was seen at local time `done` started.
    fn started(&self, len: usize, done: i64) -> i64 {
        done - airtime_us(len) - ARRIVAL_LATENCY_US - self.radio.seen_late_us()
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
                    old: (old.key.clone(), old.generation, old.catches_up_with),
                };
            }
        }
        Opened::Not
    }

    /// Takes a packet whose RxDone was seen at local time `done`.
    fn take(&mut self, packet: &Received, done: i64) {
        let len = packet.length;
        let mut bytes = packet.payload;
        match self.open(&mut bytes[..len]) {
            Opened::Current => {}
            // Nothing else in it is taken before this node's own switch: it still sends under the
            // old key, which the member being removed reads.
            Opened::Pending { header } => {
                info!("[REKEY] heard {} on the key to switch to", header.sender);
                if let Some(heard) = self.heard.get_mut(usize::from(header.sender)) {
                    *heard = Some(done);
                }
                self.adopt_pending(&header, len, done);
                return;
            }
            Opened::Old { header, old } => {
                self.heard_on_old(&header, old, done);
                self.take_rivals(&bytes[SIV_LEN..len]);
                return;
            }
            Opened::Not => {
                info!("[MESH] not ours len={} rssi={}", len, packet.rssi);
                return;
            }
        }
        let utc = self.utc_seconds(done);
        let start = self.started(len, done);
        let Some(group) = &mut self.group else {
            return;
        };
        let Ok(plain) = Plain::parse(&bytes[SIV_LEN..len]) else {
            warn!("[MESH] malformed len={}", len);
            return;
        };
        let header = plain.header;
        let arrival = self.clock.arrival(&header, start, done);
        if let Some(heard) = self.heard.get_mut(usize::from(header.sender)) {
            *heard = Some(done);
        }
        let round = round_at(header.start());
        self.table.heard(header.sender, round);
        // A clock started from a boot is no UTC to judge a record's time by.
        let now = self
            .clock
            .at(done)
            .filter(|(_, timebase)| timebase.source.is_utc())
            .map(|(time, _)| second_at(time));
        // Messages are judged by this node's clock: the header's own time is the sender's word.
        let round_s = self.clock.at(done).map_or_else(
            || round_start_s(header.start()),
            |(time, _)| round_start_s(time),
        );
        let own = group.own();
        let absorbed = absorb(
            plain.records(),
            header.sender,
            When {
                at: header.start(),
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
        octowhere_mesh::relays::BENCH_NAMED_SETS.answer(
            own,
            header.sender,
            absorbed
                .carried_names()
                .iter()
                .copied()
                .filter(|name| !absorbed.arrivals().contains(name)),
        );
        self.relays.heard(
            own,
            header.sender,
            absorbed.neighbours,
            absorbed.carried_names(),
        );
        if !absorbed.arrivals().is_empty() {
            self.settle = Some(done + SETTLE_US);
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
            self.rekey_changed();
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
            absorbed.neighbours.bits(),
            absorbed.arrivals().len(),
            absorbed.carried,
            absorbed.summary,
        );
    }

    /// Moves to the clock of a packet under the key to switch to, if it outranks this node's. A
    /// node that restarted with no UTC finds the group's clock in no other packet once the
    /// others have switched.
    fn adopt_pending(&mut self, header: &Header, len: usize, done: i64) {
        let ours = self.clock.at(done).map(|(_, timebase)| timebase.source);
        if ours.is_some_and(|ours| !header.timebase.source.outranks(ours))
            || self.removals.rekey.pending().is_none()
        {
            return;
        }
        let start = self.started(len, done);
        let arrival = self.clock.arrival(header, start, done);
        info!(
            "[REKEY] clock from the key to switch to: {:?} late_us={:?}",
            arrival.taken, arrival.late_us
        );
    }

    /// Takes from a packet under an old key, opened, the key messages of this node's
    /// generation and the few before it: rivals of the keys it switched to, which reach it no
    /// other way once both parts of the group have switched. Nothing else in it is taken.
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
            // Older than a key this node can still rank, it is no rival it could take.
            if !message.is_key()
                || !message
                    .generation()
                    .is_some_and(|of| generation.wrapping_sub(of) < OLD_KEYS as u16)
            {
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

    /// Sends this node's packet, at once.
    async fn send(&mut self, timebase: Timebase) {
        let now = self.time.now();
        let (Some(group), Some((time, _))) = (&mut self.group, self.clock.at(now)) else {
            return;
        };
        let (round, own) = (round_at(time), group.own());
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Sealing::new(&mut packet, &Header::new(own, timebase, time));
        let carried = compose(
            &mut builder,
            round,
            second_at(time),
            Sources {
                table: &self.table,
                group,
                messages: &self.messages,
                requests: &self.requests,
                summary: self
                    .summary
                    .as_deref()
                    .map(|(summary, _)| summary.as_slice()),
                on_key: self.removals.on_key_for(round, now),
            },
        );
        let len = builder.seal(group.key());
        let sent = self.radio.transmit_if_clear(&packet[..len]).await;
        let Sent::Done { started, finished } = sent else {
            self.back_off(sent, own);
            return;
        };
        let spread = self.draw(SPREAD_US, own);
        let own_keys = carried
            .messages()
            .iter()
            .any(|&name| name.0 == own && self.messages.get(name).is_some_and(Message::is_key));
        match own_keys {
            true => self.access.sent_keys(started, len, spread),
            false => self.access.sent(started, len, Some(spread)),
        }
        self.relays
            .sent(own, carried.neighbours, carried.messages(), started);
        if carried.on_key {
            self.removals.carried_on_key();
        }
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
            carried.records.bits(),
            carried.requests.bits(),
            carried.neighbours.bits(),
            carried.messages().len(),
            carried.summary,
            finished
        );
    }

    /// Runs a pairing in `role` on the pairing channel until it ends, then returns to the mesh's
    /// channel. A node that joins, or founds a group, starts its timebase afresh.
    async fn pair(&mut self, role: Role, commands: &impl Commands) {
        if self.founding.take().is_some() {
            info!("[MESH] no longer listening for the device a founding left unconfirmed");
        }
        self.shown.recovery = None;
        self.shown.sessions += 1;
        self.shown.refusal = None;
        let session = self.shown.sessions;
        let (Some(nonce), Some(founding)) = (self.random.bytes::<16>(), self.random.bytes::<32>())
        else {
            warn!("[PAIR] no true random source; not pairing");
            self.shown.refuse(session, role, Refused::NoRandom);
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
                let sent = self.radio.transmit(&frame[..len]).await;
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
                self.shown.fill(
                    &mut self.view,
                    &self.me,
                    group,
                    now,
                    utc,
                    &mut self.fingerprints,
                );
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
        let phase = pairing.phase();
        match (phase, pairing.take_group()) {
            (Phase::Done(_), Some(group)) => {
                let own = group.own();
                self.group = Some(group);
                self.unsaved.group_replaced();
                if role == Role::Join || !had_group {
                    self.restart(own);
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
                    group,
                    until,
                    heard: false,
                }));
            }
            _ => {}
        }
        // The member has the group's key from this device, so needs no older one.
        if let Phase::Done(Done::Added { id, .. }) = phase
            && let Some(generation) = self.group.as_ref().map(Group::generation)
            && self.removals.rekey.on_key(id, generation)
        {
            self.rekey_changed();
        }
        if !self.radio.tune(FREQUENCY_HZ, SYNC_WORD, POWER_DBM).await {
            warn!("[MESH] tuning back to the mesh's channel failed");
        }
    }

    /// Starts the timebase, the table and what the screens show of them afresh, at id `own`.
    fn restart(&mut self, own: u8) {
        self.clock = Clock::new(own, self.time.now());
        self.table = Table::new(own);
        self.access = Access::default();
        self.timebase_shown = None;
        self.requests = Requests::default();
        self.heard = [None; IDS as usize];
        self.shown.positions = [None; IDS as usize];
        self.forget_messages();
    }

    /// Makes the summary the next packet carries, while there is time: with a full store it
    /// takes milliseconds, too long between the channel check and the transmission.
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
        self.settle = None;
        *self.relays = Relays::default();
        self.outbox.clear();
        self.removals.forget();
        self.shown.removals = RemovalsView::default();
        self.summary = None;
    }

    /// Stores the removals after a member stopped being waited for, dropping the old key and
    /// what was kept for it once nobody is.
    fn rekey_changed(&mut self) {
        if !self.removals.rekey.is_waiting() {
            info!("[REKEY] every member is on the new key; the old one is dropped");
            self.removals.all_on_new_key();
        }
        self.save_rekey();
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
        if let Some(pending) = self.removals.rekey.pending() {
            warn!("[REKEY] a removal is under way");
            let key = key_fingerprint(&pending.new.key);
            return Err(Unremovable::Underway { key });
        }
        if id == group.own() || group.member(id).is_none() {
            return Err(Unremovable::Changed);
        }
        let remaining = group.ids().without(id).without(group.own());
        // Every key message has its number before the removal starts, so none is left behind.
        if !self.reserve(remaining.count(), time).await {
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
            // Another removal may have come while the numbers were stored.
            return Err(match self.removals.rekey.pending() {
                Some(pending) => Unremovable::Underway {
                    key: key_fingerprint(&pending.new.key),
                },
                None => Unremovable::Changed,
            });
        };
        info!(
            "[REKEY] removing {}: generation {} from round {} (now {}), {} to tell",
            id,
            new.generation,
            new.switch,
            round,
            remaining.count()
        );
        self.removals.started();
        self.save_rekey();
        self.publish();
        self.post_keys(time).await;
        Ok(())
    }

    /// Posts the key messages of this device's removal under way, one to every member but the
    /// one removed, at timebase time `time`. An interruption leaves the rest to post later; a
    /// restart posts them all again, and a member that already holds the key ignores another
    /// message with it.
    async fn post_keys(&mut self, time: i64) {
        let (Some(group), Some(pending)) = (&self.group, self.removals.rekey.pending()) else {
            return;
        };
        let due = self.removals.keys_due(group);
        let new = pending.new.clone();
        if due.is_empty() || !self.reserve(due.count(), time).await {
            return;
        }
        let body = new.encode();
        for member in due.iter() {
            let outgoing = Outgoing::new(To::Member(member), Some(new.generation), &body);
            if !self.post(&outgoing, time).await {
                return;
            }
            self.removals.posted(member);
            // Each key takes an X25519 the first time; let the other tasks run between.
            self.time.until(self.time.now() + 1_000).await;
        }
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
                    .map_or(0, |pending| pending.own_switch);
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
        for restored in switched.restored.iter() {
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
        let missing: heapless::Vec<messages::MessageId, { IDS as usize }> = self
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
        Some(now + i64::from(pending.own_switch) * ROUND_US - time)
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

    /// As [`utc`](Self::utc), in seconds that a record or a pairing stamps.
    fn utc_seconds(&self, now: i64) -> Option<u32> {
        self.utc(now)
            .map(|utc| utc.clamp(0, i64::from(u32::MAX)) as u32)
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
    /// Whether the member at `id` is the device with fingerprint `device`. A removed member's id
    /// can go to another device, which a request made before must not reach.
    fn holds(&self, id: u8, device: [u8; 8]) -> bool {
        self.group
            .as_ref()
            .and_then(|group| group.member(id))
            .is_some_and(|member| fingerprint(&member.public) == device)
    }

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
                    // Not acknowledged: a key message stays out of the inbox, so nothing would
                    // read it, and a member's signed word that it is on the new key is what
                    // ends the wait for it.
                    Some((&kind::KEY, _)) => {
                        return match NewKey::decode(plain)
                            .filter(|new| message.generation() == Some(new.generation))
                        {
                            Some(new) => match self.learned_key(origin, new) {
                                None | Some(Learned::Later) => Arrival::Later,
                                Some(Learned::Ignored | Learned::Pending) => Arrival::Done,
                            },
                            None => {
                                warn!("[REKEY] a key message from {} is malformed", origin);
                                Arrival::Done
                            }
                        };
                    }
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
        let utc = utc_now(device, now);
        shown.fill(
            &mut view,
            &me,
            group.as_ref(),
            now,
            utc,
            &mut Fingerprints::default(),
        );
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
