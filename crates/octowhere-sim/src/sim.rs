//! The executor that runs the nodes on virtual time, and what a scenario does with them.

use std::{
    alloc::Global,
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Wake, Waker},
};

use octowhere_mesh::{
    IDS,
    identity::SIGNATURE_LEN,
    members::{Group, Member, Name, Slot},
    pair::Identity,
    seal::Key,
};
use octowhere_node::{Command, Fix, Mesh, Start, view::MeshView};

use crate::{
    logs::{self, Line},
    rng::SplitMix,
    seams::{SimCommands, SimDevice, SimRadio, SimRandom, SimStore, SimTime},
    store::Stored,
    world::{Link, Micros, Mode, Node, Radio, World},
};

/// UTC at the start of every simulation: 2026-09-21.
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

pub struct Sim {
    world: Rc<World>,
    tasks: Vec<Task>,
    seed: u64,
    starts: u64,
}

impl Sim {
    pub fn new(seed: u64) -> Self {
        Self {
            world: Rc::new(World::new(seed, UTC0_S * 1_000_000)),
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
            stored: Stored::new(&start),
            fail_writes: false,
            commands: VecDeque::new(),
            commands_waker: None,
        });
        let task = self.task(node, start);
        self.tasks.push(task);
        node
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

    /// Restarts `node` from what it stored, as a reset does: it loses everything else.
    pub fn restart(&mut self, node: usize) {
        let start = {
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
            state.stored.start()
        };
        self.tasks[node] = self.task(node, start);
    }

    /// Sets how a packet from `from` reaches `to`, or that it does not.
    pub fn link(&self, from: usize, to: usize, link: Option<Link>) {
        self.world.link(from, to, link);
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

    /// How many of `node`'s lines hold `needle`.
    pub fn count(&self, node: usize, needle: &str) -> usize {
        self.world
            .lines
            .borrow()
            .iter()
            .filter(|line| line.node == node && line.text.contains(needle))
            .count()
    }

    /// What `node` shows its screens.
    pub fn view(&self, node: usize) -> Option<MeshView> {
        self.world.nodes.borrow()[node].view.as_deref().cloned()
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
