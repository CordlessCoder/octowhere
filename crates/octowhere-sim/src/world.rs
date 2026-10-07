//! What the simulated nodes share: virtual time and its timers, each node's clock, radio and
//! surroundings, and the air between their radios.

use std::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap, VecDeque},
    task::Waker,
};

use octowhere_mesh::schedule::airtime_us;
use octowhere_node::{
    Command, Fix, RECEIVED_MAX, Received,
    view::{MeshView, MessagesView},
};

use crate::{logs::Lines, rng::SplitMix, store::Stored};

/// Microseconds of virtual time.
pub type Micros = u64;

/// How much stronger a packet must arrive than one overlapping it on its channel to be heard
/// through it.
pub const CAPTURE_DB: i16 = 6;
/// How long a receiver takes to detect a packet's preamble: about five of its symbols.
pub const DETECT_US: Micros = 5_000;

/// How a packet from one node reaches another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Link {
    /// The chance it is lost, 0 to 1.
    pub loss: f64,
    pub rssi: i16,
    pub snr: i16,
    /// How long after the packet ends its receiver sees it end, as a board sees `DIO0` rise.
    pub delay_us: Micros,
}

impl Default for Link {
    fn default() -> Self {
        Self {
            loss: 0.0,
            rssi: -60,
            snr: 8,
            delay_us: 1_050,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Sleep,
    Standby,
    /// Receiving since this time, without a break.
    Receiving(Micros),
    Sending,
}

pub struct Radio {
    /// The frequency and sync word: only a packet sent with both is heard.
    pub channel: (u32, u8),
    pub mode: Mode,
    /// The packet that arrived last and is not read yet, and when its end was seen.
    pub received: Option<(Received, Micros)>,
    pub waker: Option<Waker>,
}

/// One node's clock, radio and surroundings.
pub struct Node {
    /// When it started, and how fast its clock runs, in parts per billion off true time.
    pub boot: Micros,
    pub drift_ppb: i64,
    pub radio: Radio,
    pub fix: Option<Fix>,
    pub gps: bool,
    /// How far its RTC is off UTC, or `None` without RTC time.
    pub rtc_error_us: Option<i64>,
    pub view: Option<Box<MeshView>>,
    pub views: u32,
    /// The messages it shows its screens, and how many times it has shown them.
    pub messages: Box<MessagesView>,
    pub message_views: u32,
    pub stored: Stored,
    pub fail_writes: bool,
    pub commands: VecDeque<Command>,
    pub commands_waker: Option<Waker>,
    /// What a bare radio is to send next, and on which channel. It wakes `commands_waker`, as a
    /// bare radio takes no commands.
    pub to_send: VecDeque<(Vec<u8>, (u32, u8))>,
}

impl Node {
    /// Its clock's reading at virtual time `at`.
    pub fn local(&self, at: Micros) -> i64 {
        let elapsed = i128::from(at.saturating_sub(self.boot));
        (elapsed * (1_000_000_000 + i128::from(self.drift_ppb)) / 1_000_000_000) as i64
    }

    /// The virtual time its clock reads `local` at, rounded up.
    pub fn global(&self, local: i64) -> Micros {
        let local = i128::from(local.max(0));
        let rate = 1_000_000_000 + i128::from(self.drift_ppb);
        self.boot + ((local * 1_000_000_000 + rate - 1) / rate) as Micros
    }
}

/// A packet on the air.
#[derive(Clone, Debug)]
pub struct Transmission {
    pub sender: usize,
    /// The frequency and sync word it was sent with.
    pub channel: (u32, u8),
    pub start: Micros,
    pub end: Micros,
    pub bytes: Vec<u8>,
}

pub struct World {
    pub now: Cell<Micros>,
    /// UTC at virtual time 0, in microseconds.
    pub utc0: i64,
    timers: RefCell<BinaryHeap<Reverse<(Micros, u64)>>>,
    wakers: RefCell<BTreeMap<u64, Waker>>,
    next_timer: Cell<u64>,
    pub nodes: RefCell<Vec<Node>>,
    links: RefCell<BTreeMap<(usize, usize), Link>>,
    air: RefCell<Vec<Transmission>>,
    /// Every packet sent while recording, oldest first.
    pub recorded: RefCell<Option<Vec<Transmission>>>,
    rng: RefCell<SplitMix>,
    pub lines: Lines,
    /// Bench: what each packet came to at each node linked to its sender.
    pub receptions: Cell<Receptions>,
}

/// Bench: receptions by what became of them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Receptions {
    pub delivered: u64,
    /// Overlapped by another packet at the receiver, not 6 dB weaker.
    pub drowned: u64,
    /// Lost to the link's loss.
    pub lost: u64,
    /// The receiver was sending, or not receiving, when the packet started.
    pub deaf: u64,
}

impl World {
    pub fn new(seed: u64, utc0: i64) -> Self {
        Self {
            now: Cell::new(0),
            utc0,
            timers: RefCell::default(),
            wakers: RefCell::default(),
            next_timer: Cell::new(0),
            nodes: RefCell::default(),
            links: RefCell::default(),
            air: RefCell::default(),
            recorded: RefCell::default(),
            rng: RefCell::new(SplitMix::new(seed)),
            lines: Lines::default(),
            receptions: Cell::default(),
        }
    }

    pub fn now(&self) -> Micros {
        self.now.get()
    }

    /// Wakes `waker` at virtual time `at`.
    pub fn wake_at(&self, at: Micros, waker: &Waker) {
        let id = self.next_timer.get();
        self.next_timer.set(id + 1);
        self.timers.borrow_mut().push(Reverse((at, id)));
        self.wakers.borrow_mut().insert(id, waker.clone());
    }

    /// The time of the next timer, if any.
    pub fn next_timer(&self) -> Option<Micros> {
        self.timers.borrow().peek().map(|Reverse((at, _))| *at)
    }

    /// Wakes every timer due by now.
    pub fn fire_timers(&self) {
        let now = self.now();
        loop {
            let id = {
                let mut timers = self.timers.borrow_mut();
                match timers.peek() {
                    Some(Reverse((at, _))) if *at <= now => timers.pop().map(|Reverse((_, id))| id),
                    _ => None,
                }
            };
            let Some(id) = id else {
                return;
            };
            if let Some(waker) = self.wakers.borrow_mut().remove(&id) {
                waker.wake();
            }
        }
    }

    pub fn link(&self, from: usize, to: usize, link: Option<Link>) {
        let mut links = self.links.borrow_mut();
        match link {
            Some(link) => links.insert((from, to), link),
            None => links.remove(&(from, to)),
        };
    }

    pub fn link_of(&self, from: usize, to: usize) -> Option<Link> {
        self.links.borrow().get(&(from, to)).copied()
    }

    /// Whether `node`'s radio, receiving, finds the channel clear: no packet on its channel by
    /// a link to it that started long enough ago to detect, and none arrived that it has not
    /// read.
    pub fn is_clear(&self, node: usize) -> bool {
        let now = self.now();
        let nodes = self.nodes.borrow();
        let radio = &nodes[node].radio;
        if radio.received.is_some() {
            return false;
        }
        let links = self.links.borrow();
        !self.air.borrow().iter().any(|sent| {
            sent.sender != node
                && sent.channel == radio.channel
                && sent.start + DETECT_US <= now
                && now < sent.end
                && links.contains_key(&(sent.sender, node))
        })
    }

    /// Ends `node`'s transmission in flight, unheard, as a node powered off mid-packet would.
    pub fn silence(&self, node: usize) {
        let now = self.now();
        self.air
            .borrow_mut()
            .retain(|sent| sent.sender != node || sent.end <= now);
    }

    /// Starts `node`'s transmission of `bytes` now, and returns when it ends.
    pub fn send(&self, node: usize, bytes: &[u8]) -> Micros {
        let now = self.now();
        let end = now + airtime_us(bytes.len()) as Micros;
        let mut nodes = self.nodes.borrow_mut();
        let radio = &mut nodes[node].radio;
        radio.mode = Mode::Sending;
        let mut air = self.air.borrow_mut();
        // Long past any transmission that could still overlap one starting now.
        air.retain(|old| old.end + 1_000_000 > now);
        let sent = Transmission {
            sender: node,
            channel: radio.channel,
            start: now,
            end,
            bytes: bytes.to_vec(),
        };
        if let Some(recorded) = self.recorded.borrow_mut().as_mut() {
            recorded.push(sent.clone());
        }
        air.push(sent);
        end
    }

    /// Ends `node`'s transmission, delivering it to every node that heard it whole: receiving on
    /// its channel throughout, by a link, and not drowned by a packet overlapping it.
    pub fn sent(&self, node: usize) {
        let air = self.air.borrow();
        let Some(sent) = air.iter().rev().find(|sent| sent.sender == node) else {
            return;
        };
        let links = self.links.borrow();
        let mut nodes = self.nodes.borrow_mut();
        nodes[node].radio.mode = Mode::Standby;
        for (to, receiver) in nodes.iter_mut().enumerate() {
            let Some(link) = links.get(&(node, to)) else {
                continue;
            };
            let radio = &mut receiver.radio;
            let mut counts = self.receptions.get();
            let Mode::Receiving(since) = radio.mode else {
                counts.deaf += 1;
                self.receptions.set(counts);
                continue;
            };
            if since > sent.start || radio.channel != sent.channel {
                counts.deaf += 1;
                self.receptions.set(counts);
                continue;
            }
            let drowned = air.iter().any(|other| {
                other.sender != node
                    && other.sender != to
                    && other.channel == sent.channel
                    && other.start < sent.end
                    && sent.start < other.end
                    && links
                        .get(&(other.sender, to))
                        .is_some_and(|other| link.rssi < other.rssi + CAPTURE_DB)
            });
            if drowned {
                counts.drowned += 1;
                self.receptions.set(counts);
                continue;
            }
            if self.rng.borrow_mut().unit() < link.loss {
                counts.lost += 1;
                self.receptions.set(counts);
                continue;
            }
            counts.delivered += 1;
            self.receptions.set(counts);
            let mut payload = [0; RECEIVED_MAX];
            payload[..sent.bytes.len()].copy_from_slice(&sent.bytes);
            radio.received = Some((
                Received {
                    payload,
                    length: sent.bytes.len(),
                    rssi: link.rssi,
                    snr: link.snr,
                },
                sent.end + link.delay_us,
            ));
            if let Some(waker) = radio.waker.take() {
                waker.wake();
            }
        }
    }
}
