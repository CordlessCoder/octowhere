//! What the simulated nodes share: virtual time and its timers, each node's clock, radio and
//! surroundings, and the air between their radios.

use std::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap, VecDeque},
    task::Waker,
};

use octowhere_mesh::schedule::airtime_us;
use octowhere_node::{Command, Fix, RECEIVED_MAX, Received, view::MeshView};

use crate::{logs::Lines, rng::SplitMix, store::Stored};

/// Microseconds of virtual time.
pub type Micros = u64;

/// How much stronger a packet must arrive than one overlapping it on its channel to be heard
/// through it.
pub const CAPTURE_DB: i16 = 6;

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
    pub stored: Stored,
    pub fail_writes: bool,
    pub commands: VecDeque<Command>,
    pub commands_waker: Option<Waker>,
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

struct Transmission {
    sender: usize,
    channel: (u32, u8),
    start: Micros,
    end: Micros,
    bytes: Vec<u8>,
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
    rng: RefCell<SplitMix>,
    pub lines: Lines,
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
            rng: RefCell::new(SplitMix::new(seed)),
            lines: Lines::default(),
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
        air.push(Transmission {
            sender: node,
            channel: radio.channel,
            start: now,
            end,
            bytes: bytes.to_vec(),
        });
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
            let Mode::Receiving(since) = radio.mode else {
                continue;
            };
            if since > sent.start || radio.channel != sent.channel {
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
            if drowned || self.rng.borrow_mut().unit() < link.loss {
                continue;
            }
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
