//! The executor that runs the nodes on virtual time, and what a scenario does with them.

use std::{
    alloc::Global,
    cell::{RefCell, RefMut},
    collections::VecDeque,
    future::{Future, poll_fn},
    pin::Pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

use octowhere_mesh::{
    IDS,
    identity::SIGNATURE_LEN,
    members::{Group, Member, Name, Slot},
    pair::Identity,
    seal::Key,
};
use octowhere_node::{
    Command, Fix, Mesh, Start,
    view::{MeshView, MessagesView},
};

use crate::{
    logs::{self, Line},
    rng::SplitMix,
    seams::{SimCommands, SimDevice, SimRadio, SimRandom, SimStore, SimTime, Sleep},
    store::Stored,
    world::{Link, Micros, Mode, Node, Radio, Transmission, World},
};

/// UTC at the start of a simulation unless it is started at another: 2026-09-21.
pub const UTC0_S: i64 = 1_790_000_000;

/// How a node starts.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// How fast its clock runs off true time, in parts per million.
    pub drift_ppm: f64,
    /// Whether its RTC holds UTC, and how far off.
    pub rtc_error_us: Option<i64>,
    /// Whether it has GPS time, which comes with a fix.
    pub gps: bool,
    pub fix: Option<Fix>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            drift_ppm: 0.0,
            rtc_error_us: Some(0),
            gps: false,
            fix: None,
        }
    }
}

/// Wakes a node: the executor polls it again.
struct Flag(AtomicBool);

impl Wake for Flag {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
    flag: Arc<Flag>,
    waker: Waker,
}

impl Task {
    /// A node powered off: nothing to poll, ever.
    fn off() -> Self {
        let flag = Arc::new(Flag(AtomicBool::new(false)));
        Self {
            future: Box::pin(std::future::pending()),
            waker: Waker::from(flag.clone()),
            flag,
        }
    }
}

pub struct Sim {
    world: Rc<World>,
    tasks: Vec<Task>,
    seed: u64,
    starts: u64,
}

impl Sim {
    pub fn new(seed: u64) -> Self {
        Self::starting_at(seed, UTC0_S)
    }

    /// A simulation whose virtual time starts at UTC second `utc_s`.
    pub fn starting_at(seed: u64, utc_s: i64) -> Self {
        Self {
            world: Rc::new(World::new(seed, utc_s * 1_000_000)),
            tasks: Vec::new(),
            seed,
            starts: 0,
        }
    }

    /// Starts a node from `start`, and returns its number.
    pub fn add(&mut self, start: Start, config: Config) -> usize {
        let node = self.world.nodes.borrow().len();
        self.world.nodes.borrow_mut().push(Node {
            boot: self.world.now(),
            drift_ppb: (config.drift_ppm * 1_000.0) as i64,
            radio: Radio {
                channel: (0, 0),
                mode: Mode::Sleep,
                received: None,
                waker: None,
            },
            fix: config.fix,
            gps: config.gps,
            rtc_error_us: config.rtc_error_us,
            view: None,
            views: 0,
            messages: MessagesView::boxed(),
            message_views: 0,
            stored: Stored::new(&start),
            fail_writes: false,
            commands: VecDeque::new(),
            commands_waker: None,
            to_send: VecDeque::new(),
        });
        let task = self.task(node, start);
        self.tasks.push(task);
        node
    }

    /// Adds a radio with no node behind it, which sends what [`transmit`](Self::transmit) gives
    /// it and hears nothing: a device that replays or forges packets. Returns its number, which
    /// [`link`](Self::link) places it by. Restarting it or powering it off makes it a node.
    pub fn add_radio(&mut self) -> usize {
        let node = self.add(alone(0xEE), Config::default());
        let world = self.world.clone();
        let future = Box::pin(async move {
            loop {
                let (bytes, channel) = poll_fn(|cx| {
                    let mut nodes = world.nodes.borrow_mut();
                    let state = &mut nodes[node];
                    match state.to_send.pop_front() {
                        Some(next) => Poll::Ready(next),
                        None => {
                            state.commands_waker = Some(cx.waker().clone());
                            Poll::Pending
                        }
                    }
                })
                .await;
                world.nodes.borrow_mut()[node].radio.channel = channel;
                let end = world.send(node, &bytes);
                Sleep::new(&world, end).await;
                world.sent(node);
            }
        });
        let flag = Arc::new(Flag(AtomicBool::new(true)));
        self.tasks[node] = Task {
            future,
            waker: Waker::from(flag.clone()),
            flag,
        };
        node
    }

    /// Has `radio`, from [`add_radio`](Self::add_radio), send `bytes` on `channel` as soon as
    /// the nodes next run.
    pub fn transmit(&self, radio: usize, bytes: &[u8], channel: (u32, u8)) {
        let mut nodes = self.world.nodes.borrow_mut();
        let state = &mut nodes[radio];
        state.to_send.push_back((bytes.to_vec(), channel));
        if let Some(waker) = state.commands_waker.take() {
            waker.wake();
        }
    }

    /// Starts keeping every packet sent, or stops and forgets them.
    pub fn record(&self, on: bool) {
        *self.world.recorded.borrow_mut() = on.then(Vec::new);
    }

    /// The packets sent since recording started, oldest first.
    pub fn recorded(&self) -> Vec<Transmission> {
        self.world.recorded.borrow().clone().unwrap_or_default()
    }

    fn random(&mut self) -> SplitMix {
        self.starts += 1;
        SplitMix::new(self.seed ^ self.starts.wrapping_mul(0x5851_F42D_4C95_7F2D))
    }

    fn task(&mut self, node: usize, start: Start) -> Task {
        let random = SimRandom(self.random());
        let world = &self.world;
        let radio = SimRadio {
            world: world.clone(),
            node,
        };
        let time = SimTime {
            world: world.clone(),
            node,
        };
        let device = SimDevice {
            world: world.clone(),
            node,
        };
        let store = SimStore {
            world: world.clone(),
            node,
            results: RefCell::default(),
        };
        let commands = SimCommands {
            world: world.clone(),
            node,
        };
        let future = Box::pin(async move {
            Mesh::new(radio, time, random, device, store, Global, start)
                .await
                .run(&commands)
                .await;
        });
        let flag = Arc::new(Flag(AtomicBool::new(true)));
        let waker = Waker::from(flag.clone());
        Task {
            future,
            flag,
            waker,
        }
    }

    /// Restarts `node` from what it stored, as a reset does: it loses everything else. A node
    /// powered off starts again.
    pub fn restart(&mut self, node: usize) {
        let start = self.stop(node).stored.start();
        self.tasks[node] = self.task(node, start);
    }

    /// Powers `node` off until [`restart`](Self::restart): it neither sends nor hears, and
    /// keeps only what it stored.
    pub fn power_off(&mut self, node: usize) {
        self.stop(node);
        self.tasks[node] = Task::off();
    }

    /// Clears what `node` holds outside its store and takes its packet off the air, restarts its
    /// clock from now, and returns it.
    fn stop(&self, node: usize) -> RefMut<'_, Node> {
        self.world.silence(node);
        let mut nodes = self.world.nodes.borrow_mut();
        let state = &mut nodes[node];
        state.boot = self.world.now();
        state.radio = Radio {
            channel: (0, 0),
            mode: Mode::Sleep,
            received: None,
            waker: None,
        };
        state.view = None;
        state.commands.clear();
        RefMut::map(nodes, |nodes| &mut nodes[node])
    }

    /// Sets how a packet from `from` reaches `to`, or that it does not.
    pub fn link(&self, from: usize, to: usize, link: Option<Link>) {
        self.world.link(from, to, link);
    }

    /// How a packet from `from` reaches `to`, if it does.
    pub fn link_of(&self, from: usize, to: usize) -> Option<Link> {
        self.world.link_of(from, to)
    }

    /// Links every pair of nodes both ways by `link`.
    pub fn link_all(&self, link: Link) {
        let count = self.world.nodes.borrow().len();
        for from in 0..count {
            for to in (0..count).filter(|&to| to != from) {
                self.link(from, to, Some(link));
            }
        }
    }

    /// Makes `node`'s writes fail, or succeed again.
    pub fn fail_writes(&self, node: usize, fail: bool) {
        self.world.nodes.borrow_mut()[node].fail_writes = fail;
    }

    pub fn set_fix(&self, node: usize, fix: Option<Fix>, gps: bool) {
        let mut nodes = self.world.nodes.borrow_mut();
        nodes[node].fix = fix;
        nodes[node].gps = gps;
    }

    /// Sets how far `node`'s RTC is off UTC, or that it has no time.
    pub fn set_rtc(&self, node: usize, error_us: Option<i64>) {
        self.world.nodes.borrow_mut()[node].rtc_error_us = error_us;
    }

    /// Gives `node` a command, as its screens would.
    pub fn command(&self, node: usize, command: Command) {
        let mut nodes = self.world.nodes.borrow_mut();
        let state = &mut nodes[node];
        state.commands.push_back(command);
        if let Some(waker) = state.commands_waker.take() {
            waker.wake();
        }
    }

    /// Virtual time, in seconds.
    pub fn now_s(&self) -> f64 {
        self.world.now() as f64 / 1e6
    }

    /// Virtual time, in microseconds.
    pub fn now_us(&self) -> u64 {
        self.world.now()
    }

    /// UTC now, in microseconds.
    pub fn utc_us(&self) -> i64 {
        self.world.utc0 + self.world.now() as i64
    }

    /// What `node`'s clock reads now, in microseconds since it started.
    pub fn clock(&self, node: usize) -> i64 {
        self.world.nodes.borrow()[node].local(self.world.now())
    }

    /// Runs until virtual time `at`, in microseconds.
    pub fn run_to(&mut self, at: u64) {
        self.run_until(at.max(self.world.now()), |_| false);
    }

    /// Runs for `seconds` of virtual time.
    pub fn run_for(&mut self, seconds: u64) {
        let end = self.world.now() + seconds * 1_000_000;
        self.run_until(end, |_| false);
    }

    /// Runs until `done` holds, checked whenever the nodes have nothing left to do at a time, or
    /// for at most `seconds`. Returns whether `done` held.
    pub fn run_while_not(&mut self, seconds: u64, mut done: impl FnMut(&Self) -> bool) -> bool {
        let end = self.world.now() + seconds * 1_000_000;
        self.run_until(end, |sim| done(sim))
    }

    fn run_until(&mut self, end: Micros, mut done: impl FnMut(&Self) -> bool) -> bool {
        loop {
            self.poll_woken();
            if done(self) {
                return true;
            }
            match self.world.next_timer() {
                Some(at) if at <= end => {
                    self.world.now.set(at.max(self.world.now()));
                    self.world.fire_timers();
                }
                _ => {
                    self.world.now.set(end);
                    return false;
                }
            }
        }
    }

    fn poll_woken(&mut self) {
        loop {
            let mut polled = false;
            for (node, task) in self.tasks.iter_mut().enumerate() {
                if !task.flag.0.swap(false, Ordering::Relaxed) {
                    continue;
                }
                polled = true;
                let mut cx = Context::from_waker(&task.waker);
                let future = task.future.as_mut();
                // A node runs for ever, so a finished one has panicked.
                logs::polling(node, self.world.now(), &self.world.lines, || {
                    let _ = future.poll(&mut cx);
                });
            }
            if !polled {
                return;
            }
        }
    }

    /// The lines `node` logged.
    pub fn lines(&self, node: usize) -> Vec<Line> {
        self.world
            .lines
            .borrow()
            .iter()
            .filter(|line| line.node == node)
            .cloned()
            .collect()
    }

    /// Keeps only the latest `most` lines, so that a long run holds a bounded log.
    pub fn keep_lines(&self, most: usize) {
        self.world.lines.borrow_mut().keep(most);
    }

    /// Every node's lines from line number `from` on, of those kept, and the number the next
    /// line will take.
    pub fn lines_since(&self, from: usize) -> (Vec<Line>, usize) {
        let lines = self.world.lines.borrow();
        (lines.since(from).cloned().collect(), lines.total())
    }

    /// How many of `node`'s lines hold `needle`.
    pub fn count(&self, node: usize, needle: &str) -> usize {
        self.world
            .lines
            .borrow()
            .iter()
            .filter(|line| line.node == node && line.text.contains(needle))
            .count()
    }

    /// The key and generation of the group `node` stored.
    pub fn key(&self, node: usize) -> Option<(Key, u16)> {
        self.world.nodes.borrow()[node].stored.key()
    }

    /// The ids `node` stored members at, as a set.
    pub fn members(&self, node: usize) -> u32 {
        self.world.nodes.borrow()[node].stored.members()
    }

    /// What `node` shows its screens.
    pub fn view(&self, node: usize) -> Option<MeshView> {
        self.world.nodes.borrow()[node].view.as_deref().cloned()
    }

    /// The messages `node` last showed its screens.
    pub fn messages(&self, node: usize) -> Box<MessagesView> {
        let mut messages = MessagesView::boxed();
        messages.copy_from(&self.world.nodes.borrow()[node].messages);
        messages
    }

    /// Copies the messages `node` shows its screens into `into`, if it published them after the
    /// time it counted `seen`, and returns that count.
    pub fn messages_since(&self, node: usize, seen: u32, into: &mut MessagesView) -> Option<u32> {
        let nodes = self.world.nodes.borrow();
        let state = &nodes[node];
        if state.message_views == seen {
            return None;
        }
        into.copy_from(&state.messages);
        Some(state.message_views)
    }

    /// What `node` shows its screens, if it published a view after the one it counted `seen`,
    /// and that view's count.
    pub fn view_since(&self, node: usize, seen: u32) -> Option<(MeshView, u32)> {
        let nodes = self.world.nodes.borrow();
        let state = &nodes[node];
        if state.views == seen {
            return None;
        }
        Some((state.view.as_deref()?.clone(), state.views))
    }
}

/// What device `n` starts with in no group.
pub fn alone(n: u8) -> Start {
    let mac = [0x02, 0, 0, 0, 0, n];
    Start {
        me: Identity::new([n + 1; 32], mac, Name::from_mac(&mac)),
        group: None,
        sequence: None,
        rekey: None,
    }
}

/// What `count` devices start with in one group, device `n` at id `n`, joined at UTC `utc`.
pub fn grouped(count: u8, utc: u32) -> Vec<Start> {
    let key = Key::new([7; 32]);
    let identities: Vec<Identity> = (0..count)
        .map(|n| {
            let mac = [0x02, 0, 0, 0, 0, n];
            Identity::new([n + 1; 32], mac, Name::from_mac(&mac))
        })
        .collect();
    let mut slots = [None; IDS as usize];
    for (id, me) in identities.iter().enumerate() {
        let mut record = Member {
            public: me.public(),
            joined: utc,
            changed: utc,
            mac: me.mac,
            name: me.name,
            signature: [0; SIGNATURE_LEN],
        };
        record.sign(id as u8, me);
        slots[id] = Some(Slot::Member(record));
    }
    identities
        .into_iter()
        .enumerate()
        .map(|(id, me)| Start {
            me,
            group: Group::restore(key.clone(), 0, id as u8, slots).map(Box::new),
            sequence: None,
            rekey: None,
        })
        .collect()
}
